//! Persistent listing cache for the local family (plain folders + SMB/SSH
//! mounts). Directory walks are slow on network FUSE mounts, so browsed
//! levels (a section root or a show/season folder) are cached lazily: a hit
//! serves instantly while a background re-walk refreshes the entry; a change
//! rewrites the cache and pings the UI (`listings-updated`). The cache never
//! *adds* reachability — `LocalSource` validates roots before consulting it.
//!
//! Persistence follows `config::save_config`'s defensive pattern: write to a
//! process-unique temp file (owner-only on Unix), fsync, atomic rename.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use super::ItemDto;

/// Bound the cache: enough for hundreds of browsed folders, small enough
/// that the whole-file JSON rewrite stays cheap. Oldest walks evict first.
const MAX_LEVELS: usize = 512;
const SCHEMA: u32 = 1;

#[derive(Serialize, Deserialize, Clone)]
struct LevelSnapshot {
    items: Vec<ItemDto>,
    walked_at_ms: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct CacheFile {
    #[serde(default)]
    schema: u32,
    /// Browsed level (absolute directory path) → its full, unpaged listing.
    #[serde(default)]
    levels: HashMap<String, LevelSnapshot>,
    /// Detected section kind ("movie"/"show") per root with no declared kind.
    #[serde(default)]
    kinds: HashMap<String, String>,
}

pub struct ListingCache {
    path: PathBuf,
    levels: Mutex<HashMap<String, LevelSnapshot>>,
    kinds: Mutex<HashMap<String, String>>,
    /// Levels with a background revalidation already in flight.
    pending: Mutex<HashSet<String>>,
    /// Serializes disk writes; data locks are never held across file I/O.
    write_lock: Mutex<()>,
}

static SHARED: OnceLock<std::sync::Arc<ListingCache>> = OnceLock::new();

pub fn shared() -> std::sync::Arc<ListingCache> {
    SHARED
        .get_or_init(|| {
            let path = crate::config::config_dir_file("listing_cache.json")
                .unwrap_or_else(|_| PathBuf::from("listing_cache.json"));
            std::sync::Arc::new(ListingCache::load(path))
        })
        .clone()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl ListingCache {
    /// Load from `path`; any read/parse error starts empty and leaves the
    /// file intact (it is replaced wholesale on the next persist).
    pub fn load(path: PathBuf) -> Self {
        let parsed: CacheFile = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .filter(|c: &CacheFile| c.schema == SCHEMA)
            .unwrap_or_default();
        Self {
            path,
            levels: Mutex::new(parsed.levels),
            kinds: Mutex::new(parsed.kinds),
            pending: Mutex::new(HashSet::new()),
            write_lock: Mutex::new(()),
        }
    }

    pub fn level(&self, dir: &str) -> Option<Vec<ItemDto>> {
        self.levels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(dir)
            .map(|s| s.items.clone())
    }

    /// Store a freshly walked level. Returns whether the stored data differs
    /// from what was cached before (drives the UI change signal).
    pub fn store_level(&self, dir: &str, items: Vec<ItemDto>) -> bool {
        let changed = {
            let mut map = self.levels.lock().unwrap_or_else(|e| e.into_inner());
            let changed = match map.get(dir) {
                Some(prev) => !same_items(&prev.items, &items),
                None => true,
            };
            map.insert(
                dir.to_string(),
                LevelSnapshot {
                    items,
                    walked_at_ms: now_ms(),
                },
            );
            if map.len() > MAX_LEVELS {
                let excess = map.len() - MAX_LEVELS;
                evict_oldest(&mut map, excess);
            }
            changed
        };
        self.persist();
        changed
    }

    pub fn kind(&self, root: &str) -> Option<String> {
        self.kinds
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(root)
            .cloned()
    }

    /// Store a detected section kind. Returns whether it changed.
    pub fn store_kind(&self, root: &str, kind: &str) -> bool {
        let changed = {
            let mut map = self.kinds.lock().unwrap_or_else(|e| e.into_inner());
            let changed = map.get(root).map(String::as_str) != Some(kind);
            map.insert(root.to_string(), kind.to_string());
            changed
        };
        self.persist();
        changed
    }

    /// Claim a background revalidation slot for `key`; the caller must call
    /// `finish_revalidate` when done. False when one is already in flight.
    pub fn begin_revalidate(&self, key: &str) -> bool {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.to_string())
    }

    pub fn finish_revalidate(&self, key: &str) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(key);
    }

    /// Snapshot the maps (briefly holding their locks), then write the JSON
    /// outside them under `write_lock` only: temp file with owner-only perms,
    /// fsync, atomic rename — a crash never truncates the existing cache.
    fn persist(&self) {
        let file = CacheFile {
            schema: SCHEMA,
            levels: self
                .levels
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            kinds: self.kinds.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        };
        let Ok(json) = serde_json::to_string(&file) else {
            return;
        };
        let _w = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = self
            .path
            .with_extension(format!("json.tmp.{}", std::process::id()));
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
            std::fs::rename(&tmp, &self.path)
        };
        if let Err(e) = write() {
            let _ = std::fs::remove_file(&tmp);
            eprintln!("vela: listing cache write failed (kept in memory): {e}");
        }
    }
}

/// Listings compare by identity + display fields; transient metadata like a
/// freshly resolved poster also counts as a change so the UI picks it up.
fn same_items(a: &[ItemDto], b: &[ItemDto]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.rating_key == y.rating_key
                && x.title == y.title
                && x.year == y.year
                && x.poster == y.poster
                && x.media_type == y.media_type
                && x.index == y.index
                && x.parent_index == y.parent_index
        })
}

fn evict_oldest(map: &mut HashMap<String, LevelSnapshot>, n: usize) {
    let mut by_age: Vec<(String, u64)> = map
        .iter()
        .map(|(k, v)| (k.clone(), v.walked_at_ms))
        .collect();
    by_age.sort_by_key(|(_, t)| *t);
    for (k, _) in by_age.into_iter().take(n) {
        map.remove(&k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, title: &str) -> ItemDto {
        ItemDto {
            rating_key: key.into(),
            title: title.into(),
            year: None,
            summary: None,
            duration_ms: None,
            media_type: Some("movie".into()),
            poster: None,
            series_poster: None,
            backdrop: None,
            view_offset_ms: None,
            played: None,
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            provider_ids: vec![],
            backing: None,
            source_id: "local".into(),
        }
    }

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("vela-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn store_get_and_change_detection() {
        let cache = ListingCache::load(tmp("lc-change.json"));
        let level = "/media/movies";

        assert!(cache.level(level).is_none());
        assert!(cache.store_level(level, vec![item("local:/a", "A")]));
        assert_eq!(cache.level(level).unwrap().len(), 1);
        // Same content again: not a change.
        assert!(!cache.store_level(level, vec![item("local:/a", "A")]));
        // Different content: a change.
        assert!(cache.store_level(level, vec![item("local:/a", "A2")]));
        let _ = std::fs::remove_file(tmp("lc-change.json"));
    }

    #[test]
    fn persists_and_reloads_atomically_written_file() {
        let path = tmp("lc-roundtrip.json");
        let _ = std::fs::remove_file(&path);
        {
            let cache = ListingCache::load(path.clone());
            cache.store_level("/media/tv", vec![item("local:/t", "T")]);
            cache.store_kind("/media/tv", "show");
        }
        let reloaded = ListingCache::load(path.clone());
        assert_eq!(reloaded.level("/media/tv").unwrap()[0].title, "T");
        assert_eq!(reloaded.kind("/media/tv").as_deref(), Some("show"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "cache file must be owner-only");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn eviction_caps_levels_by_oldest_walk() {
        let path = tmp("lc-evict.json");
        let _ = std::fs::remove_file(&path);
        let cache = ListingCache::load(path.clone());
        {
            // Seed oldest-first timestamps directly so the test doesn't sleep.
            let mut map = cache.levels.lock().unwrap();
            for i in 0..MAX_LEVELS {
                map.insert(
                    format!("/root/{i}"),
                    LevelSnapshot {
                        items: vec![],
                        walked_at_ms: i as u64,
                    },
                );
            }
        }
        cache.store_level("/root/new", vec![]);
        let map = cache.levels.lock().unwrap();
        assert_eq!(map.len(), MAX_LEVELS);
        assert!(!map.contains_key("/root/0"), "oldest walk must evict first");
        assert!(map.contains_key("/root/new"));
        drop(map);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn revalidation_slots_are_exclusive() {
        let cache = ListingCache::load(tmp("lc-pending.json"));
        assert!(cache.begin_revalidate("/x"));
        assert!(!cache.begin_revalidate("/x"));
        cache.finish_revalidate("/x");
        assert!(cache.begin_revalidate("/x"));
        let _ = std::fs::remove_file(tmp("lc-pending.json"));
    }
}
