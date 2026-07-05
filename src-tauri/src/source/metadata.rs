//! Keyless metadata for the local source. Resolution order: persistent cache →
//! `.nfo`/local-artwork sidecar (offline, instant) → keyless online lookup
//! (iTunes Search for movies/shows, TVmaze for episodes), with the filename
//! parse left as the floor. Online lookups run in the background, rate-limited,
//! and land in the cache for the next browse — `items()` never blocks on the
//! network.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;

use super::ItemDto;

/// Minimum gap between online lookups (~20/min, the iTunes soft limit).
const ONLINE_MIN_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CachedMeta {
    pub title: Option<String>,
    pub year: Option<u32>,
    pub summary: Option<String>,
    /// Poster: an `https` URL (online) or a local file path (sidecar art).
    pub poster: Option<String>,
}

impl CachedMeta {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.year.is_none()
            && self.summary.is_none()
            && self.poster.is_none()
    }
}

/// Cap on cached entries, so churn (renamed/deleted files) can't grow the cache
/// and its on-disk JSON without bound.
const MAX_ENTRIES: usize = 20_000;

pub struct MetaCache {
    file: Option<PathBuf>,
    map: Mutex<HashMap<String, CachedMeta>>,
    /// Serializes disk writes without holding `map` across the (slow) write.
    write_lock: Mutex<()>,
    /// Keys with an online lookup already in flight, so we don't refetch.
    pending: Mutex<HashSet<String>>,
    http: reqwest::Client,
    /// Last online call time, to space requests out.
    gate: AsyncMutex<Instant>,
}

/// Process-wide shared cache. The local source is rebuilt on every folder
/// add/remove; sharing one cache means in-flight online tasks from an old
/// instance and the new instance use the same map + write lock, so they can't
/// race the cache file or drop each other's entries.
pub fn shared() -> std::sync::Arc<MetaCache> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<std::sync::Arc<MetaCache>> = OnceLock::new();
    CACHE.get_or_init(MetaCache::load).clone()
}

impl MetaCache {
    pub fn load() -> std::sync::Arc<Self> {
        let file = crate::config::config_dir_file("metadata_cache.json").ok();
        let map = file
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        std::sync::Arc::new(Self {
            file,
            map: Mutex::new(map),
            write_lock: Mutex::new(()),
            pending: Mutex::new(HashSet::new()),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(12))
                .build()
                .unwrap_or_default(),
            gate: AsyncMutex::new(Instant::now() - ONLINE_MIN_INTERVAL),
        })
    }

    fn get(&self, key: &str) -> Option<CachedMeta> {
        self.map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .cloned()
    }

    fn store(&self, key: &str, meta: CachedMeta) {
        {
            let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(key.to_string(), meta);
            if map.len() > MAX_ENTRIES {
                // Evict a chunk to bound memory + disk. Order is arbitrary (it's a
                // cache; a dropped entry is just re-resolved later).
                let excess = map.len() - MAX_ENTRIES + MAX_ENTRIES / 10;
                let drop: Vec<String> = map.keys().take(excess).cloned().collect();
                for k in drop {
                    map.remove(&k);
                }
            }
        }
        self.persist();
    }

    /// Persist the cache. The `write_lock` serializes writers so a slower one
    /// can't write an older snapshot after a newer one, while the (brief)
    /// snapshot is the only thing taken under the map lock — the disk write
    /// happens without holding it. The write itself is defensive like the
    /// config's: owner-only temp file, fsync, atomic rename — a crash mid-
    /// write can never truncate the existing cache file.
    fn persist(&self) {
        let Some(path) = &self.file else { return };
        let _w = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let snapshot = self.map.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let Ok(json) = serde_json::to_string(&snapshot) else {
            return;
        };
        let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
        let write = || -> std::io::Result<()> {
            {
                let mut opts = std::fs::OpenOptions::new();
                opts.write(true).create(true).truncate(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    opts.mode(0o600);
                }
                use std::io::Write;
                let mut f = opts.open(&tmp)?;
                f.write_all(json.as_bytes())?;
                f.sync_all()?;
            }
            std::fs::rename(&tmp, path)
        };
        if let Err(e) = write() {
            let _ = std::fs::remove_file(&tmp);
            eprintln!("vela: metadata cache write failed (kept in memory): {e}");
        }
    }
}

/// Context the resolver needs about an item to query the right source.
pub struct Hint<'a> {
    pub file: &'a Path,
    pub media_type: &'a str, // "movie" | "show" | "season" | "episode"
    pub title: &'a str,
    pub year: Option<u32>,
    pub show_title: Option<&'a str>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

/// Fill an item's metadata from cache or sidecar synchronously; if neither has
/// it, leave the filename data in place and kick off a background online lookup.
pub fn enrich(
    cache: &std::sync::Arc<MetaCache>,
    vfs: &dyn crate::source::vfs::Vfs,
    item: &mut ItemDto,
    hint: Hint,
) {
    let key = hint.file.to_string_lossy().to_string();

    // Sidecar is re-read every browse (cheap) so a newly added or edited
    // .nfo / artwork is picked up immediately rather than being shadowed by a
    // stale cache entry (e.g. an earlier online miss).
    let mut local_poster = None;
    if let Some(meta) = read_sidecar(vfs, hint.file) {
        apply(item, &meta);
        // A real .nfo (title/year/summary) is authoritative — done. But an
        // artwork-only sidecar (poster.jpg with no .nfo) must NOT block title/
        // year/summary enrichment: keep its poster and fall through.
        if meta.title.is_some() || meta.year.is_some() || meta.summary.is_some() {
            return;
        }
        local_poster = meta.poster;
    }
    if let Some(meta) = cache.get(&key) {
        apply(item, &meta);
        if local_poster.is_some() {
            item.poster = local_poster; // prefer local artwork over online
        }
        return;
    }
    // Seasons have nothing useful to look up online; skip them.
    if hint.media_type == "season" {
        return;
    }
    spawn_online(cache, key, &hint);
}

fn apply(item: &mut ItemDto, meta: &CachedMeta) {
    if let Some(t) = &meta.title {
        if !t.is_empty() {
            item.title = t.clone();
        }
    }
    if meta.year.is_some() {
        item.year = meta.year;
    }
    if meta.summary.is_some() {
        item.summary = meta.summary.clone();
    }
    if meta.poster.is_some() {
        item.poster = meta.poster.clone();
    }
}

// ---- sidecar (.nfo + local artwork) -------------------------------------

fn read_sidecar(vfs: &dyn crate::source::vfs::Vfs, file: &Path) -> Option<CachedMeta> {
    let mut meta = CachedMeta::default();
    if let Some(nfo) = read_nfo(vfs, file) {
        meta.title = xml_text(&nfo, "title");
        meta.year = xml_text(&nfo, "year").and_then(|y| y.trim().parse().ok());
        meta.summary = xml_text(&nfo, "plot");
    }
    meta.poster = local_artwork(vfs, file).and_then(|p| match vfs.resolve_stream_url(&p) {
        // Native provider: only a fetchable URL is usable as artwork.
        Some(Ok(url)) => Some(url),
        Some(Err(_)) => None,
        // Local file: the path itself is served via the asset protocol.
        None => Some(p.to_string_lossy().to_string()),
    });
    if meta.is_empty() {
        None
    } else {
        Some(meta)
    }
}

/// Where to look for sidecars: inside a directory item (show/season), or beside
/// a file item (movie/episode). Returns the base dir and the file stem (empty
/// for directory items).
fn meta_base(vfs: &dyn crate::source::vfs::Vfs, path: &Path) -> (PathBuf, String) {
    if vfs.is_dir(path) {
        (path.to_path_buf(), String::new())
    } else {
        (
            path.parent().map(Path::to_path_buf).unwrap_or_default(),
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
        )
    }
}

fn read_nfo(vfs: &dyn crate::source::vfs::Vfs, file: &Path) -> Option<String> {
    let (dir, stem) = meta_base(vfs, file);
    let mut candidates = Vec::new();
    if stem.is_empty() {
        // Directory item (show/season): the series / movie-folder nfo.
        candidates.push(dir.join("tvshow.nfo"));
        candidates.push(dir.join("movie.nfo"));
    } else {
        // File item (movie/episode): its own sidecar, or a movie-folder nfo.
        // Never tvshow.nfo — an episode beside one must not inherit the series.
        candidates.push(dir.join(format!("{stem}.nfo")));
        candidates.push(dir.join("movie.nfo"));
    }
    candidates.iter().find_map(|p| vfs.read_to_string(p))
}

fn local_artwork(vfs: &dyn crate::source::vfs::Vfs, file: &Path) -> Option<PathBuf> {
    let (dir, stem) = meta_base(vfs, file);
    let mut names = Vec::new();
    if !stem.is_empty() {
        names.push(format!("{stem}-poster.jpg"));
        names.push(format!("{stem}.jpg"));
        names.push(format!("{stem}-poster.png"));
    }
    names.extend(["poster.jpg", "folder.jpg", "cover.jpg", "poster.png"].map(String::from));
    names.iter().map(|n| dir.join(n)).find(|p| vfs.is_file(p))
}

/// Extract the text of the first `<tag>…</tag>` (handles a CDATA wrapper).
fn xml_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let raw = xml[start..end].trim();
    let raw = raw
        .strip_prefix("<![CDATA[")
        .and_then(|r| r.strip_suffix("]]>"))
        .unwrap_or(raw);
    let cleaned = raw.trim();
    (!cleaned.is_empty()).then(|| cleaned.to_string())
}

// ---- online (iTunes Search + TVmaze), keyless ---------------------------

fn spawn_online(cache: &std::sync::Arc<MetaCache>, key: String, hint: &Hint) {
    {
        let mut pending = cache.pending.lock().unwrap_or_else(|e| e.into_inner());
        if cache.get(&key).is_some() || !pending.insert(key.clone()) {
            return; // already cached or already in flight
        }
    }
    // Owned copy of the query context for the background task.
    let cache = cache.clone();
    let media_type = hint.media_type.to_string();
    let title = hint.title.to_string();
    let year = hint.year;
    let show_title = hint.show_title.map(|s| s.to_string());
    let season = hint.season;
    let episode = hint.episode;

    tokio::spawn(async move {
        // Space requests out so we stay under the keyless rate limits.
        {
            let mut last = cache.gate.lock().await;
            let wait = ONLINE_MIN_INTERVAL.saturating_sub(last.elapsed());
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
            *last = Instant::now();
        }

        let result = match media_type.as_str() {
            "episode" => resolve_episode(&cache.http, show_title.as_deref(), season, episode).await,
            "show" => resolve_itunes(&cache.http, &title, year, true).await,
            _ => resolve_itunes(&cache.http, &title, year, false).await,
        };

        // Cache a definitive result (a hit, or a confirmed "no match", incl. an
        // empty miss so we don't refetch). A transient failure (Err: network /
        // 5xx / parse) is left uncached so it retries on the next browse.
        if let Ok(meta) = result {
            cache.store(&key, meta);
        }
        cache
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key);
    });
}

#[derive(Deserialize)]
struct ITunesResp {
    results: Vec<ITunesItem>,
}

#[derive(Deserialize)]
struct ITunesItem {
    #[serde(rename = "artworkUrl100")]
    artwork: Option<String>,
    #[serde(rename = "longDescription")]
    long_description: Option<String>,
    #[serde(rename = "shortDescription")]
    short_description: Option<String>,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
}

/// `Err(())` = transient failure (don't cache); `Ok(meta)` = definitive, where
/// an all-`None` `CachedMeta` is a confirmed "no match" worth caching.
async fn resolve_itunes(
    http: &reqwest::Client,
    title: &str,
    year: Option<u32>,
    is_show: bool,
) -> Result<CachedMeta, ()> {
    let media = if is_show { "tvShow" } else { "movie" };
    let resp = http
        .get("https://itunes.apple.com/search")
        .timeout(Duration::from_secs(12))
        .query(&[("term", title), ("media", media), ("limit", "1")])
        .send()
        .await
        .map_err(|_| ())?
        .error_for_status()
        .map_err(|_| ())?;
    let resp: ITunesResp = resp.json().await.map_err(|_| ())?;
    let Some(item) = resp.results.into_iter().next() else {
        return Ok(CachedMeta::default()); // request succeeded, no match
    };
    Ok(CachedMeta {
        title: None, // keep our parsed title; iTunes can be noisy
        year: item
            .release_date
            .as_deref()
            .and_then(|d| d.get(0..4))
            .and_then(|y| y.parse().ok())
            .or(year),
        summary: item.long_description.or(item.short_description),
        // Bump the thumbnail to a usable poster size.
        poster: item.artwork.map(|a| a.replace("100x100bb", "600x600bb")),
    })
}

#[derive(Deserialize)]
struct TvmazeShow {
    #[serde(rename = "_embedded")]
    embedded: Option<TvmazeEmbedded>,
}

#[derive(Deserialize)]
struct TvmazeEmbedded {
    episodes: Vec<TvmazeEpisode>,
}

#[derive(Deserialize)]
struct TvmazeEpisode {
    season: Option<u32>,
    number: Option<u32>,
    name: Option<String>,
    summary: Option<String>,
    image: Option<TvmazeImage>,
}

#[derive(Deserialize)]
struct TvmazeImage {
    original: Option<String>,
    medium: Option<String>,
}

async fn resolve_episode(
    http: &reqwest::Client,
    show: Option<&str>,
    season: Option<u32>,
    episode: Option<u32>,
) -> Result<CachedMeta, ()> {
    let Some(show) = show else {
        return Ok(CachedMeta::default());
    };
    let resp = http
        .get("https://api.tvmaze.com/singlesearch/shows")
        .timeout(Duration::from_secs(12))
        .query(&[("q", show), ("embed", "episodes")])
        .send()
        .await
        .map_err(|_| ())?;
    // 404 = no such show: a confirmed miss. Other errors (5xx/429) = transient.
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(CachedMeta::default());
    }
    let resp = resp.error_for_status().map_err(|_| ())?;
    let data: TvmazeShow = resp.json().await.map_err(|_| ())?;
    let Some(found) = data.embedded.and_then(|e| {
        e.episodes
            .into_iter()
            .find(|ep| ep.season == season && ep.number == episode)
    }) else {
        return Ok(CachedMeta::default()); // show found, episode not listed
    };
    Ok(CachedMeta {
        title: found.name,
        year: None,
        summary: found.summary.map(|s| strip_html(&s)),
        poster: found.image.and_then(|i| i.original.or(i.medium)),
    })
}

/// TVmaze summaries are HTML; strip tags for plain display.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfo_text() {
        let xml =
            "<movie><title>Heat</title><year>1995</year><plot><![CDATA[A crew.]]></plot></movie>";
        assert_eq!(xml_text(xml, "title"), Some("Heat".into()));
        assert_eq!(xml_text(xml, "year"), Some("1995".into()));
        assert_eq!(xml_text(xml, "plot"), Some("A crew.".into()));
        assert_eq!(xml_text(xml, "missing"), None);
    }

    #[test]
    fn html_stripping() {
        assert_eq!(strip_html("<p>Hello <b>world</b></p>"), "Hello world");
    }

    #[test]
    fn sidecar_beside_file() {
        // Temp dir with an .nfo and a poster next to a (nonexistent) video file.
        let dir = std::env::temp_dir().join(format!("vela_meta_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("Heat.nfo"),
            "<movie><title>Heat</title><year>1995</year></movie>",
        )
        .unwrap();
        std::fs::write(dir.join("poster.jpg"), b"x").unwrap();

        let meta = read_sidecar(&crate::source::vfs::StdFs, &dir.join("Heat.mkv")).expect("sidecar found");
        assert_eq!(meta.title.as_deref(), Some("Heat"));
        assert_eq!(meta.year, Some(1995));
        assert!(meta.poster.as_deref().unwrap().ends_with("poster.jpg"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
