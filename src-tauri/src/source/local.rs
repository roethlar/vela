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

use super::metadata::{self, Hint, MetaCache};
use super::{namespace_key, HubDto, ItemDto, MediaSource, SectionDto, StreamResolution};
use crate::config::LocalFolder;
use crate::playback::ProgressTarget;

pub const LOCAL_SOURCE_ID: &str = "local";

const VIDEO_EXTS: &[&str] = &[
    "mkv", "mp4", "m4v", "mov", "avi", "ts", "m2ts", "webm", "wmv", "flv", "mpg", "mpeg",
];

pub struct LocalSource {
    folders: Vec<LocalFolder>,
    meta: Arc<MetaCache>,
}

impl LocalSource {
    pub fn new(folders: Vec<LocalFolder>) -> Self {
        Self {
            folders,
            meta: metadata::shared(),
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
        LOCAL_SOURCE_ID.to_string()
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
            folders: self.folders.clone(),
            meta: self.meta.clone(),
        }
    }
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
        view_offset_ms: None,
        index: None,
        parent_index: None,
        grandparent_title: None,
        parent_title: None,
        source_id: source_id.to_string(),
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
        "Local".to_string()
    }
    fn kind(&self) -> &'static str {
        "local"
    }

    async fn sections(&self) -> Result<Vec<SectionDto>, String> {
        let this = self.worker();
        run_blocking(move || {
            Ok(this
                .folders
                .iter()
                .map(|f| SectionDto {
                    key: namespace_key(LOCAL_SOURCE_ID, &f.path),
                    title: if f.name.is_empty() {
                        dir_name(Path::new(&f.path))
                    } else {
                        f.name.clone()
                    },
                    section_type: if f.kind.is_empty() {
                        detect_kind(Path::new(&f.path)).to_string()
                    } else {
                        f.kind.clone()
                    },
                    source_id: this.id_str(),
                    source_name: "Local".to_string(),
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
            Ok(sort_and_page(items, sort.as_deref(), start, size))
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
                    let mut item =
                        base_item(&this.id_str(), &entry, clean_title(&stem), None, "episode");
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
            Ok(items.into_iter().skip(start).take(size).collect())
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
            let prefix = format!("{LOCAL_SOURCE_ID}:");
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
}
