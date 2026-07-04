//! Local (and OS-mounted SMB) folder backend. Indexes files by name/structure
//! and direct-plays them in mpv. No metadata lookup (that's P2d) and no resume
//! tracking — local playback uses `ProgressTarget::None` by design.
//!
//! Item keys are the absolute filesystem path. The registry splits a namespaced
//! key on the *first* `:` only, so a Windows `C:\…` path survives intact as the
//! raw key — no encoding or id↔path map needed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use super::listing_cache::{self, ListingCache};
use super::metadata::{self, Hint, MetaCache};
use super::{namespace_key, HubDto, ItemDto, MediaSource, SectionDto, StreamResolution};
use crate::config::{AppConfig, LocalFolder, SmbMount, SshMount};
use crate::playback::ProgressTarget;

pub const LOCAL_SOURCE_ID: &str = "local";

/// The source kinds that make up the "local family": plain configured
/// folders plus SMB/SSH mounts, each registered as its own source so the UI
/// can tell a share from a plain folder. Rebuilds replace the whole family.
pub const LOCAL_FAMILY_KINDS: &[&str] = &["local", "smb", "ssh"];

pub fn smb_source_id(mount_id: &str) -> String {
    format!("smb-{mount_id}")
}

pub fn ssh_source_id(mount_id: &str) -> String {
    format!("ssh-{mount_id}")
}

/// True for ids owned by the local family (the plain local source or a
/// per-mount SMB/SSH source). These are managed via folder/mount entries,
/// never removed as free-standing sources.
pub fn is_local_family_id(id: &str) -> bool {
    id == LOCAL_SOURCE_ID || id.starts_with("smb-") || id.starts_with("ssh-")
}

/// One member of the local family, ready to register.
pub struct LocalFamilyMember {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub folders: Vec<LocalFolder>,
}

impl LocalFamilyMember {
    pub fn build(&self) -> std::sync::Arc<dyn MediaSource> {
        std::sync::Arc::new(LocalSource::new(
            self.id.clone(),
            self.name.clone(),
            self.kind,
            self.folders.clone(),
        ))
    }
}

/// Group the config into local-family members: plain folders (minus the ones
/// owned by SSH mounts) under the "local"/"Local" source, then one source per
/// SMB mount and per SSH mount carrying the mount's human name. Folder
/// resolution is injected because boot and live rebuild derive mount folders
/// differently; `safe_root` is applied uniformly to every member's folders.
/// Members that end up with no folders are omitted entirely.
pub fn local_family<FS, FH, SR>(
    cfg: &AppConfig,
    smb_folders: FS,
    ssh_folder: FH,
    safe_root: SR,
) -> Vec<LocalFamilyMember>
where
    FS: Fn(&SmbMount) -> Vec<LocalFolder>,
    FH: Fn(&SshMount) -> Option<LocalFolder>,
    SR: Fn(&str) -> bool,
{
    let ssh_folder_ids: std::collections::HashSet<_> = cfg
        .ssh_mounts
        .iter()
        .map(|m| m.local_folder_id.as_str())
        .collect();
    let mut members = Vec::new();
    let plain: Vec<_> = cfg
        .local_folders
        .iter()
        .filter(|f| !ssh_folder_ids.contains(f.id.as_str()))
        .filter(|f| safe_root(&f.path))
        .cloned()
        .collect();
    if !plain.is_empty() {
        members.push(LocalFamilyMember {
            id: LOCAL_SOURCE_ID.to_string(),
            name: "Local".to_string(),
            kind: "local",
            folders: plain,
        });
    }
    for m in &cfg.smb_mounts {
        let folders: Vec<_> = smb_folders(m)
            .into_iter()
            .filter(|f| safe_root(&f.path))
            .collect();
        if !folders.is_empty() {
            members.push(LocalFamilyMember {
                id: smb_source_id(&m.id),
                name: m.name.clone(),
                kind: "smb",
                folders,
            });
        }
    }
    for m in &cfg.ssh_mounts {
        if let Some(f) = ssh_folder(m).filter(|f| safe_root(&f.path)) {
            members.push(LocalFamilyMember {
                id: ssh_source_id(&m.id),
                name: m.name.clone(),
                kind: "ssh",
                folders: vec![f],
            });
        }
    }
    members
}

const VIDEO_EXTS: &[&str] = &[
    "mkv", "mp4", "m4v", "mov", "avi", "ts", "m2ts", "webm", "wmv", "flv", "mpg", "mpeg",
];

pub struct LocalSource {
    id: String,
    name: String,
    kind: &'static str,
    folders: Vec<LocalFolder>,
    meta: Arc<MetaCache>,
    listings: Arc<ListingCache>,
}

impl LocalSource {
    pub fn new(id: String, name: String, kind: &'static str, folders: Vec<LocalFolder>) -> Self {
        Self {
            id,
            name,
            kind,
            folders,
            meta: metadata::shared(),
            listings: listing_cache::shared(),
        }
    }

    fn folder(&self, path: &str) -> Option<&LocalFolder> {
        self.folders.iter().find(|f| f.path == path)
    }

    /// Guard against playing/listing outside the configured roots.
    fn within_roots(&self, p: &Path) -> bool {
        let Ok(canon) = std::fs::canonicalize(p) else {
            return false;
        };
        self.folders.iter().any(|f| {
            std::fs::canonicalize(&f.path)
                .map(|root| canon.starts_with(root))
                .unwrap_or(false)
        })
    }

    fn item_movie(&self, file: &Path, title: String, year: Option<u32>) -> ItemDto {
        base_item(&self.id_str(), file, title, year, "movie")
    }

    fn id_str(&self) -> String {
        self.id.clone()
    }

    /// Overlay cached/sidecar/online metadata onto an item (online runs async).
    #[allow(clippy::too_many_arguments)]
    fn enrich(
        &self,
        item: &mut ItemDto,
        file: &Path,
        media_type: &str,
        title: &str,
        year: Option<u32>,
        show_title: Option<&str>,
        season: Option<u32>,
        episode: Option<u32>,
    ) {
        metadata::enrich(
            &self.meta,
            item,
            Hint {
                file,
                media_type,
                title,
                year,
                show_title,
                season,
                episode,
            },
        );
    }

    /// A cheap owned clone (folders Vec + cache Arc) to move into a blocking task,
    /// so the synchronous filesystem walks below don't run on an async worker.
    fn worker(&self) -> LocalSource {
        LocalSource {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind,
            folders: self.folders.clone(),
            meta: self.meta.clone(),
            listings: self.listings.clone(),
        }
    }
}

/// What a cached level contains, and how to (re-)walk it. Part of the cache
/// key so a directory that is simultaneously a section root and a container
/// (nested roots) can't cross-serve the wrong shape.
enum LevelKind {
    Items { section_type: String },
    Children,
}

impl LevelKind {
    fn cache_key(&self, source_id: &str, path: &str) -> String {
        match self {
            LevelKind::Items { section_type } => {
                format!("{source_id}|items:{section_type}:{path}")
            }
            LevelKind::Children => format!("{source_id}|children:{path}"),
        }
    }
}

fn walk_level(this: &LocalSource, path: &str, kind: &LevelKind) -> Result<Vec<ItemDto>, String> {
    match kind {
        LevelKind::Items { section_type } => walk_items_level(this, path, section_type),
        LevelKind::Children => walk_children_level(this, path),
    }
}

/// Full (unsorted, unpaged) listing of a section root. Both the interactive
/// path and the background revalidation walk through here, so the guards
/// re-apply even when the config changed since the level was cached.
fn walk_items_level(
    this: &LocalSource,
    path: &str,
    section_type: &str,
) -> Result<Vec<ItemDto>, String> {
    let root = Path::new(path);
    if this.folder(path).is_none() || !root.is_dir() {
        return Err("unknown local folder".into());
    }
    let mut items = Vec::new();
    if section_type == "show" {
        // Each immediate subdirectory is a show.
        for entry in read_dir_sorted(root)
            .into_iter()
            .filter(|p| p.is_dir() && this.within_roots(p))
        {
            let (title, year) = parse_movie(&dir_name(&entry));
            let mut item = base_item(&this.id_str(), &entry, title.clone(), year, "show");
            this.enrich(&mut item, &entry, "show", &title, year, None, None, None);
            items.push(item);
        }
    } else {
        // Movies: loose video files, plus subfolders that wrap one movie.
        for entry in read_dir_sorted(root) {
            if !this.within_roots(&entry) {
                continue;
            }
            let (file, title, year) = if entry.is_file() && is_video(&entry) {
                let (t, y) = parse_movie(&file_stem(&entry));
                (entry.clone(), t, y)
            } else if entry.is_dir() {
                match largest_video_in(&entry) {
                    Some(file) => {
                        let (t, y) = parse_movie(&dir_name(&entry));
                        (file, t, y)
                    }
                    None => continue,
                }
            } else {
                continue;
            };
            let mut item = this.item_movie(&file, title.clone(), year);
            this.enrich(&mut item, &file, "movie", &title, year, None, None, None);
            items.push(item);
        }
    }
    Ok(items)
}

/// Full (sorted, unpaged) listing of a container (show or season folder).
fn walk_children_level(this: &LocalSource, path: &str) -> Result<Vec<ItemDto>, String> {
    let dir = Path::new(path);
    if !this.within_roots(dir) || !dir.is_dir() {
        return Err("not a browsable local folder".into());
    }
    // Figure out the show this folder belongs to (for episode lookups):
    // if `dir` is itself a season folder, the show is its parent.
    let dir_is_season = parse_season_dir(&dir_name(dir)).is_some();
    let show_dir = if dir_is_season {
        dir.parent().unwrap_or(dir)
    } else {
        dir
    };
    let show_title = parse_movie(&dir_name(show_dir)).0;

    let mut items = Vec::new();
    for entry in read_dir_sorted(dir) {
        if !this.within_roots(&entry) {
            continue;
        }
        if entry.is_dir() {
            // A season folder.
            let name = dir_name(&entry);
            let index = parse_season_dir(&name);
            let mut item = base_item(&this.id_str(), &entry, name, None, "season");
            item.index = index;
            this.enrich(
                &mut item, &entry, "season", &show_title, None, None, None, None,
            );
            items.push(item);
        } else if entry.is_file() && is_video(&entry) {
            // An episode file.
            let stem = file_stem(&entry);
            let mut item = base_item(&this.id_str(), &entry, clean_title(&stem), None, "episode");
            let parsed = parse_episode(&stem);
            if let Some((s, e)) = parsed {
                item.parent_index = Some(s);
                item.index = Some(e);
            }
            item.grandparent_title = Some(show_title.clone());
            let season = parsed
                .map(|(s, _)| s)
                .or_else(|| parse_season_dir(&dir_name(dir)));
            let episode = parsed.map(|(_, e)| e);
            this.enrich(
                &mut item,
                &entry,
                "episode",
                &stem,
                None,
                Some(&show_title),
                season,
                episode,
            );
            items.push(item);
        }
    }
    // Natural episode/season order: by (season, episode), else name.
    items.sort_by(|a, b| {
        (a.parent_index, a.index, a.title.to_lowercase()).cmp(&(
            b.parent_index,
            b.index,
            b.title.to_lowercase(),
        ))
    });
    Ok(items)
}

/// Re-walk a cached level in the background; on change, store and ping the
/// UI (`listings-updated`). At most one revalidation per level in flight.
/// Requires an ambient Tokio runtime (present under `run_blocking`) so the
/// enrichment's online lookups can spawn.
fn spawn_revalidate(this: LocalSource, path: String, kind: LevelKind) {
    let key = kind.cache_key(&this.id, &path);
    if !this.listings.begin_revalidate(&key) {
        return;
    }
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        this.listings.finish_revalidate(&key);
        return;
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _enter = handle.enter();
        let walked = walk_level(&this, &path, &kind);
        let changed = match walked {
            Ok(items) => this.listings.store_level(&key, items),
            Err(_) => false, // root vanished mid-walk; serve nothing new
        };
        this.listings.finish_revalidate(&key);
        if changed {
            crate::ui_events::listings_updated();
        }
    });
}

/// Background re-detection of a root's section kind (cheap directory probe).
fn spawn_redetect_kind(this: LocalSource, root: String) {
    let key = format!("kind:{root}");
    if !this.listings.begin_revalidate(&key) {
        return;
    }
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        this.listings.finish_revalidate(&key);
        return;
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _enter = handle.enter();
        let kind = detect_kind(Path::new(&root)).to_string();
        let changed = this.listings.store_kind(&root, &kind);
        this.listings.finish_revalidate(&key);
        if changed {
            crate::ui_events::listings_updated();
        }
    });
}

/// Run a synchronous local-filesystem operation on the blocking pool instead of
/// an async worker thread (slow disks/mounts mustn't occupy the async runtime).
/// Enters the current Tokio runtime inside the blocking thread so the background
/// online-metadata lookups that `enrich` kicks off via `tokio::spawn` still work.
async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let handle = tokio::runtime::Handle::current();
    tauri::async_runtime::spawn_blocking(move || {
        let _enter = handle.enter();
        f()
    })
    .await
    .map_err(|e| format!("local filesystem task failed: {e}"))?
}

// ---- filename / structure parsing ---------------------------------------

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .map(|e| VIDEO_EXTS.contains(&e.as_str()))
        .unwrap_or(false)
}

/// Tidy a raw name: dots/underscores → spaces, collapse runs.
fn clean_title(s: &str) -> String {
    s.replace(['.', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `Title (2021)` or `Title.2021.1080p…` → (title, year).
fn parse_movie(stem: &str) -> (String, Option<u32>) {
    if let Some(open) = stem.find('(') {
        if let Some(close_rel) = stem[open..].find(')') {
            let inner = &stem[open + 1..open + close_rel];
            if let Ok(y) = inner.trim().parse::<u32>() {
                if (1900..=2100).contains(&y) {
                    return (clean_title(&stem[..open]), Some(y));
                }
            }
        }
    }
    let cleaned = clean_title(stem);
    let tokens: Vec<&str> = cleaned.split(' ').collect();
    for (i, t) in tokens.iter().enumerate() {
        if i > 0 && t.len() == 4 {
            if let Ok(y) = t.parse::<u32>() {
                if (1900..=2100).contains(&y) {
                    return (tokens[..i].join(" "), Some(y));
                }
            }
        }
    }
    (cleaned, None)
}

/// Find an `SxxEyy` marker (case-insensitive) → (season, episode).
fn parse_episode(name: &str) -> Option<(u32, u32)> {
    let bytes = name.to_ascii_lowercase().into_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b's' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() && bytes[j] == b'e' {
                let mut k = j + 1;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                if k > j + 1 {
                    let s: u32 = std::str::from_utf8(&bytes[i + 1..j]).ok()?.parse().ok()?;
                    let e: u32 = std::str::from_utf8(&bytes[j + 1..k]).ok()?.parse().ok()?;
                    return Some((s, e));
                }
            }
        }
        i += 1;
    }
    None
}

/// `Season 01` → 1, `Specials` → 0.
fn parse_season_dir(name: &str) -> Option<u32> {
    let lower = name.trim().to_ascii_lowercase();
    if lower == "specials" {
        return Some(0);
    }
    lower
        .strip_prefix("season")
        .and_then(|r| r.trim().parse::<u32>().ok())
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

/// One level of a directory, sorted by name.
fn read_dir_sorted(path: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    entries
}

/// Largest video file directly inside `dir` (a movie-in-its-own-folder layout).
fn largest_video_in(dir: &Path) -> Option<PathBuf> {
    read_dir_sorted(dir)
        .into_iter()
        .filter(|p| p.is_file() && is_video(p))
        .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
}

// ---- ItemDto builders ----------------------------------------------------

fn base_item(
    source_id: &str,
    path: &Path,
    title: String,
    year: Option<u32>,
    media_type: &str,
) -> ItemDto {
    ItemDto {
        rating_key: namespace_key(source_id, &path.to_string_lossy()),
        title,
        year,
        summary: None,
        duration_ms: None,
        media_type: Some(media_type.to_string()),
        poster: None, // local artwork/metadata is P2d
        series_poster: None, // local series art deferred (see artwork plan)
        backdrop: None,      // local items never reach resume rows
        view_offset_ms: None,
        played: None, // local files have no server-tracked watched state
        index: None,
        parent_index: None,
        grandparent_title: None,
        parent_title: None,
        source_id: source_id.to_string(),
        provider_ids: vec![], // local identity is parsed title+year only
        backing: None,
    }
}

/// Sort items by the UI's Plex-style token (title or year), then page.
fn sort_and_page(
    mut items: Vec<ItemDto>,
    sort: Option<&str>,
    start: usize,
    size: usize,
) -> Vec<ItemDto> {
    let (field, desc) = match sort.unwrap_or("titleSort:asc").split_once(':') {
        Some((f, d)) => (f, d.eq_ignore_ascii_case("desc")),
        None => ("titleSort", false),
    };
    match field {
        "year" => items.sort_by_key(|a| a.year),
        _ => items.sort_by_key(|a| a.title.to_lowercase()),
    }
    if desc {
        items.reverse();
    }
    items.into_iter().skip(start).take(size).collect()
}

// ---- MediaSource impl ----------------------------------------------------

#[async_trait]
impl MediaSource for LocalSource {
    fn id(&self) -> String {
        self.id_str()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn sections(&self) -> Result<Vec<SectionDto>, String> {
        let this = self.worker();
        run_blocking(move || {
            Ok(this
                .folders
                .iter()
                .map(|f| SectionDto {
                    key: namespace_key(&this.id, &f.path),
                    title: if f.name.is_empty() {
                        dir_name(Path::new(&f.path))
                    } else {
                        f.name.clone()
                    },
                    // Detection probes the filesystem (slow over SMB/SSH), so
                    // serve the cached kind and re-detect in the background.
                    section_type: if f.kind.is_empty() {
                        match this.listings.kind(&f.path) {
                            Some(k) => {
                                spawn_redetect_kind(this.worker(), f.path.clone());
                                k
                            }
                            None => {
                                let k = detect_kind(Path::new(&f.path)).to_string();
                                this.listings.store_kind(&f.path, &k);
                                k
                            }
                        }
                    } else {
                        f.kind.clone()
                    },
                    source_id: this.id_str(),
                    source_name: this.name.clone(),
                })
                .collect())
        })
        .await
    }

    /// Local contributes no home rails (no resume / recently-added tracking here).
    async fn hubs(&self) -> Result<Vec<HubDto>, String> {
        Ok(vec![])
    }

    async fn items(
        &self,
        section_key: &str,
        section_type: &str,
        sort: Option<&str>,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String> {
        let this = self.worker();
        let (section_key, section_type, sort) = (
            section_key.to_string(),
            section_type.to_string(),
            sort.map(str::to_string),
        );
        run_blocking(move || {
            let root = Path::new(&section_key);
            if this.folder(&section_key).is_none() || !root.is_dir() {
                return Err("unknown local folder".into());
            }
            let kind = LevelKind::Items {
                section_type: section_type.clone(),
            };
            // Cache hit serves instantly; the level re-walks in the background
            // and pings the UI if anything changed. A miss walks inline (as
            // every browse did before the cache) and seeds the cache.
            let full = match this.listings.level(&kind.cache_key(&this.id, &section_key)) {
                Some(hit) => {
                    spawn_revalidate(this.worker(), section_key.clone(), kind);
                    hit
                }
                None => {
                    let walked = walk_level(&this, &section_key, &kind)?;
                    this.listings
                        .store_level(&kind.cache_key(&this.id, &section_key), walked.clone());
                    walked
                }
            };
            Ok(sort_and_page(full, sort.as_deref(), start, size))
        })
        .await
    }

    async fn children(
        &self,
        item_key: &str,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String> {
        let this = self.worker();
        let item_key = item_key.to_string();
        run_blocking(move || {
            let dir = Path::new(&item_key);
            if !this.within_roots(dir) || !dir.is_dir() {
                return Err("not a browsable local folder".into());
            }
            let kind = LevelKind::Children;
            let full = match this.listings.level(&kind.cache_key(&this.id, &item_key)) {
                Some(hit) => {
                    spawn_revalidate(this.worker(), item_key.clone(), kind);
                    hit
                }
                None => {
                    let walked = walk_level(&this, &item_key, &kind)?;
                    this.listings
                        .store_level(&kind.cache_key(&this.id, &item_key), walked.clone());
                    walked
                }
            };
            Ok(full.into_iter().skip(start).take(size).collect())
        })
        .await
    }

    async fn search(&self, query: &str) -> Result<Vec<ItemDto>, String> {
        let this = self.worker();
        let query = query.to_string();
        run_blocking(move || {
            let needle = query.to_lowercase();
            let mut items = Vec::new();
            for folder in &this.folders {
                let root = Path::new(&folder.path);
                walk_search(root, root, &needle, &this.id_str(), &mut items, 0);
                if items.len() >= 100 {
                    break;
                }
            }
            items.truncate(100);
            let prefix = format!("{}:", this.id);
            for item in &mut items {
                let path =
                    PathBuf::from(item.rating_key.strip_prefix(&prefix).unwrap_or_default());
                if item.media_type.as_deref() == Some("episode") {
                    let show = item.grandparent_title.clone();
                    let (season, episode) = (item.parent_index, item.index);
                    this.enrich(
                        item,
                        &path,
                        "episode",
                        "",
                        None,
                        show.as_deref(),
                        season,
                        episode,
                    );
                } else {
                    let (title, year) = (item.title.clone(), item.year);
                    this.enrich(item, &path, "movie", &title, year, None, None, None);
                }
            }
            Ok(items)
        })
        .await
    }

    async fn resolve_stream(
        &self,
        item_key: &str,
        _duration_ms: Option<u64>,
    ) -> Result<StreamResolution, String> {
        let path = Path::new(item_key);
        if !self.within_roots(path) || !path.is_file() {
            return Err("file is not inside a configured local folder".into());
        }
        // mpv plays the path directly; no token, no network, no resume tracking.
        Ok(StreamResolution {
            url: item_key.to_string(),
            resume_ms: 0,
            progress: ProgressTarget::None,
            http_headers: Vec::new(),
        })
    }
}

/// Bounded recursive filename search.
fn walk_search(
    root: &Path,
    dir: &Path,
    needle: &str,
    source_id: &str,
    out: &mut Vec<ItemDto>,
    depth: usize,
) {
    if depth > 6 || out.len() >= 100 {
        return;
    }
    for entry in read_dir_sorted(dir) {
        if out.len() >= 100 {
            return;
        }
        if !within_root(root, &entry) {
            continue;
        }
        if entry.is_dir() {
            walk_search(root, &entry, needle, source_id, out, depth + 1);
        } else if entry.is_file() && is_video(&entry) {
            let stem = file_stem(&entry);
            if !stem.to_lowercase().contains(needle) {
                continue;
            }
            if let Some((season, episode)) = parse_episode(&stem) {
                let mut item = base_item(source_id, &entry, clean_title(&stem), None, "episode");
                item.parent_index = Some(season);
                item.index = Some(episode);
                item.grandparent_title = Some(show_title_for(&entry));
                out.push(item);
            } else {
                let (title, year) = parse_movie(&stem);
                out.push(base_item(source_id, &entry, title, year, "movie"));
            }
        }
    }
}

fn within_root(root: &Path, p: &Path) -> bool {
    let (Ok(root), Ok(canon)) = (std::fs::canonicalize(root), std::fs::canonicalize(p)) else {
        return false;
    };
    canon.starts_with(root)
}

/// Derive the show title for an episode file: its parent folder, or the
/// grandparent if the parent is a `Season NN` folder.
fn show_title_for(file: &Path) -> String {
    let parent = file.parent();
    let parent_is_season = parent
        .map(|p| parse_season_dir(&dir_name(p)).is_some())
        .unwrap_or(false);
    let show_dir = if parent_is_season {
        parent.and_then(|p| p.parent())
    } else {
        parent
    };
    show_dir
        .map(|d| parse_movie(&dir_name(d)).0)
        .unwrap_or_default()
}

/// Heuristic for folders added without a declared kind: a library is "show" if
/// an immediate subdirectory looks like a show (has a season folder or an
/// `SxxEyy` episode); otherwise "movie".
fn detect_kind(root: &Path) -> &'static str {
    for entry in read_dir_sorted(root)
        .into_iter()
        .filter(|p| p.is_dir())
        .take(8)
    {
        if looks_like_show(&entry) {
            return "show";
        }
    }
    "movie"
}

fn looks_like_show(dir: &Path) -> bool {
    read_dir_sorted(dir).into_iter().take(30).any(|child| {
        (child.is_dir() && parse_season_dir(&dir_name(&child)).is_some())
            || (child.is_file() && is_video(&child) && parse_episode(&file_stem(&child)).is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movie_names() {
        assert_eq!(
            parse_movie("The Matrix (1999)"),
            ("The Matrix".into(), Some(1999))
        );
        assert_eq!(
            parse_movie("Inception.2010.1080p.BluRay"),
            ("Inception".into(), Some(2010))
        );
        assert_eq!(parse_movie("Some_Movie"), ("Some Movie".into(), None));
    }

    #[test]
    fn episode_markers() {
        assert_eq!(parse_episode("Show.S01E05.Title"), Some((1, 5)));
        assert_eq!(parse_episode("show s1e2"), Some((1, 2)));
        assert_eq!(parse_episode("Show.S12E113"), Some((12, 113)));
        assert_eq!(parse_episode("no markers here"), None);
    }

    #[test]
    fn season_dirs() {
        assert_eq!(parse_season_dir("Season 01"), Some(1));
        assert_eq!(parse_season_dir("season 3"), Some(3));
        assert_eq!(parse_season_dir("Specials"), Some(0));
        assert_eq!(parse_season_dir("Extras"), None);
    }

    #[test]
    fn video_detection() {
        assert!(is_video(Path::new("/x/a.mkv")));
        assert!(is_video(Path::new("/x/a.MP4")));
        assert!(!is_video(Path::new("/x/a.nfo")));
        assert!(!is_video(Path::new("/x/a")));
    }

    fn folder(id: &str, name: &str, path: &str) -> LocalFolder {
        LocalFolder {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            kind: String::new(),
        }
    }

    #[test]
    fn local_family_groups_plain_smb_ssh_with_identity() {
        let mut cfg = AppConfig::default();
        cfg.local_folders.push(folder("f1", "Movies", "/media/movies"));
        // Owned by the SSH mount below: must not also appear under "Local".
        cfg.local_folders.push(folder("sshfld", "remote", "/mnt/ssh"));
        cfg.smb_mounts.push(SmbMount {
            id: "m1".into(),
            name: "nagatha/media".into(),
            ..SmbMount::default()
        });
        cfg.ssh_mounts.push(SshMount {
            id: "s1".into(),
            name: "skippy:/video".into(),
            local_folder_id: "sshfld".into(),
            ..SshMount::default()
        });

        let fam = local_family(
            &cfg,
            |_m| vec![folder("smbf", "movies", "/mnt/smb/movies")],
            |_m| Some(folder("sshfld", "remote", "/mnt/ssh")),
            |_p| true,
        );

        let ids: Vec<_> = fam.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["local", "smb-m1", "ssh-s1"]);
        assert_eq!(
            fam[0].folders.len(),
            1,
            "SSH-owned folder must be excluded from the plain local member"
        );
        assert_eq!(fam[0].name, "Local");
        assert_eq!(fam[1].name, "nagatha/media");
        assert_eq!(fam[1].kind, "smb");
        assert_eq!(fam[2].name, "skippy:/video");
        assert_eq!(fam[2].kind, "ssh");
    }

    #[test]
    fn local_family_omits_empty_members_and_unsafe_roots() {
        let mut cfg = AppConfig::default();
        cfg.local_folders.push(folder("f1", "Movies", "/"));
        cfg.smb_mounts.push(SmbMount {
            id: "m1".into(),
            name: "empty share".into(),
            ..SmbMount::default()
        });

        // The SMB mount resolves no folders and the plain folder fails the
        // safe-root check, so no member survives at all.
        let fam = local_family(&cfg, |_m| Vec::new(), |_m| None, |p| p != "/");
        assert!(fam.is_empty());
    }

    #[test]
    fn local_family_id_prefixes_route_and_classify() {
        assert!(is_local_family_id(LOCAL_SOURCE_ID));
        assert!(is_local_family_id(&smb_source_id("abc")));
        assert!(is_local_family_id(&ssh_source_id("abc")));
        assert!(!is_local_family_id("plex"));
    }
}
