//! Media-source abstraction. Each backend (Plex, Jellyfin/Emby, and a local
//! source) implements [`MediaSource`] and is registered in the [`SourceRegistry`].
//! Commands talk to the registry, not to any one backend, so the UI can present
//! a unified library while still being able to scope to a single source.

use async_trait::async_trait;
use serde::Serialize;

pub mod jellyfin;
pub mod local;
pub mod metadata;
pub mod plex;

/// A browsable library/section, tagged with the source it came from.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SectionDto {
    /// Source-namespaced key (`"<source_id>:<raw>"`); opaque to the frontend.
    pub key: String,
    pub title: String,
    pub section_type: String,
    pub source_id: String,
    pub source_name: String,
}

/// A playable/browsable item (movie, show, season, episode), source-tagged.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    /// Source-namespaced key (`"<source_id>:<raw>"`); opaque to the frontend.
    pub rating_key: String,
    pub title: String,
    pub year: Option<u32>,
    pub summary: Option<String>,
    pub duration_ms: Option<u64>,
    pub media_type: Option<String>,
    pub poster: Option<String>,
    pub view_offset_ms: Option<u64>,
    /// Whether the item is marked watched. `None` when the source doesn't report
    /// it (e.g. local files), so the UI can distinguish "unwatched" from "unknown".
    pub played: Option<bool>,
    pub index: Option<u32>,
    pub parent_index: Option<u32>,
    pub grandparent_title: Option<String>,
    pub parent_title: Option<String>,
    pub source_id: String,
}

/// A home-screen rail of items, source-tagged.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HubDto {
    pub title: String,
    pub hub_identifier: String,
    pub hub_type: String,
    pub items: Vec<ItemDto>,
    pub source_id: String,
    pub source_name: String,
}

/// What `resolve_stream` hands back to the playback layer: the media URL, where
/// to resume from, and how (if at all) to report progress.
pub struct StreamResolution {
    pub url: String,
    pub resume_ms: u64,
    pub progress: crate::playback::ProgressTarget,
    /// HTTP headers mpv must send when fetching `url` — e.g. `X-Plex-Token`,
    /// which travels as a header so the URL stays clean of credentials
    /// (mpv renders `${path}` in its title, stats overlay, and playlist).
    /// Empty when the stream needs none (local files) or the backend still
    /// carries auth in the URL (Jellyfin/Emby, pending parity).
    pub http_headers: Vec<(String, String)>,
}

/// A configured media backend. Methods that *receive* a key get the raw
/// (un-namespaced) key — the registry strips the `"<source_id>:"` prefix before
/// dispatching. Methods that *emit* keys must namespace them via [`namespace_key`].
#[async_trait]
pub trait MediaSource: Send + Sync {
    /// Stable, unique id used to namespace keys and route requests.
    fn id(&self) -> String;
    /// Human-friendly name for the UI (e.g. the server or folder name).
    fn name(&self) -> String;
    /// Backend kind: `"plex"`, `"jellyfin"`, `"emby"`, or a local-family kind
    /// (`"local"` for plain folders, `"smb"`/`"ssh"` for per-mount sources).
    fn kind(&self) -> &'static str;

    async fn sections(&self) -> Result<Vec<SectionDto>, String>;
    async fn hubs(&self) -> Result<Vec<HubDto>, String>;
    async fn items(
        &self,
        section_key: &str,
        section_type: &str,
        sort: Option<&str>,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String>;
    async fn search(&self, query: &str) -> Result<Vec<ItemDto>, String>;
    async fn children(
        &self,
        item_key: &str,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String>;
    async fn resolve_stream(
        &self,
        item_key: &str,
        duration_ms: Option<u64>,
    ) -> Result<StreamResolution, String>;

    /// Mark an item watched (`played = true`) or unwatched on its source.
    /// Defaults to a no-op error; sources that support it override this.
    async fn mark_played(&self, _item_key: &str, _played: bool) -> Result<(), String> {
        Err("this source doesn't support marking watched state".to_string())
    }
}

/// Build a source-namespaced key. Raw Plex/Jellyfin keys never contain `:`,
/// so splitting on the first `:` recovers `(source_id, raw)`.
pub fn namespace_key(source_id: &str, raw: &str) -> String {
    format!("{source_id}:{raw}")
}

/// Split a namespaced key into `(source_id, raw_key)`.
fn split_key(key: &str) -> Option<(&str, &str)> {
    key.split_once(':')
}

/// Holds the configured sources and routes requests to them.
#[derive(Default)]
pub struct SourceRegistry {
    sources: Vec<std::sync::Arc<dyn MediaSource>>,
}

impl SourceRegistry {
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn all(&self) -> &[std::sync::Arc<dyn MediaSource>] {
        &self.sources
    }

    /// Add a source, replacing any existing one with the same id.
    pub fn upsert(&mut self, source: std::sync::Arc<dyn MediaSource>) {
        let id = source.id();
        self.sources.retain(|s| s.id() != id);
        self.sources.push(source);
    }

    pub fn get(&self, id: &str) -> Option<std::sync::Arc<dyn MediaSource>> {
        self.sources.iter().find(|s| s.id() == id).cloned()
    }

    pub fn remove(&mut self, id: &str) {
        self.sources.retain(|s| s.id() != id);
    }

    /// Remove every source whose kind is one of `kinds`. Used to replace the
    /// whole local family (plain folders + SMB/SSH mounts) on rebuild, since
    /// `upsert` alone can't drop a source whose mount went away.
    pub fn remove_kinds(&mut self, kinds: &[&str]) {
        self.sources.retain(|s| !kinds.contains(&s.kind()));
    }

    /// Resolve a namespaced key to its source and the raw (un-prefixed) key.
    pub fn route(
        &self,
        namespaced_key: &str,
    ) -> Result<(std::sync::Arc<dyn MediaSource>, String), String> {
        let (id, raw) = split_key(namespaced_key).ok_or("malformed item key")?;
        let src = self.get(id).ok_or("unknown source for item")?;
        Ok((src, raw.to_string()))
    }

    /// Sources to use for a request: a specific one if `source_id` is given,
    /// else all of them (for the unified/aggregate view).
    pub fn selected(&self, source_id: Option<&str>) -> Vec<std::sync::Arc<dyn MediaSource>> {
        match source_id {
            Some(id) => self.get(id).into_iter().collect(),
            None => self.sources.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        id: &'static str,
        kind: &'static str,
    }

    #[async_trait]
    impl MediaSource for Fake {
        fn id(&self) -> String {
            self.id.to_string()
        }
        fn name(&self) -> String {
            self.id.to_string()
        }
        fn kind(&self) -> &'static str {
            self.kind
        }
        async fn sections(&self) -> Result<Vec<SectionDto>, String> {
            Ok(vec![])
        }
        async fn hubs(&self) -> Result<Vec<HubDto>, String> {
            Ok(vec![])
        }
        async fn items(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
            _: usize,
            _: usize,
        ) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn search(&self, _: &str) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn children(&self, _: &str, _: usize, _: usize) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn resolve_stream(
            &self,
            _: &str,
            _: Option<u64>,
        ) -> Result<StreamResolution, String> {
            Err("fake source".into())
        }
    }

    // Rebuilds replace the whole local family: stale mount sources must drop
    // while non-family sources survive untouched.
    #[test]
    fn remove_kinds_drops_only_the_local_family() {
        let mut reg = SourceRegistry::default();
        reg.upsert(std::sync::Arc::new(Fake { id: "plex", kind: "plex" }));
        reg.upsert(std::sync::Arc::new(Fake { id: "local", kind: "local" }));
        reg.upsert(std::sync::Arc::new(Fake { id: "smb-old", kind: "smb" }));
        reg.upsert(std::sync::Arc::new(Fake { id: "ssh-old", kind: "ssh" }));

        reg.remove_kinds(&["local", "smb", "ssh"]);

        let ids: Vec<_> = reg.all().iter().map(|s| s.id()).collect();
        assert_eq!(ids, vec!["plex".to_string()]);
    }
}
