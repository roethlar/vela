use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

/// Serializes load-modify-save cycles so concurrent commands (and Plex
/// rediscovery) can't lose each other's updates via interleaved read/writes.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)] // missing fields fall back to Default rather than failing the parse
pub struct AppConfig {
    pub auth_token: Option<String>,
    pub client_identifier: Option<String>,
    pub last_server_host: Option<String>,
    pub last_server_port: Option<u16>,
    pub last_server_scheme: Option<String>,
    pub last_section_key: Option<String>,
    pub last_page_start: Option<usize>,
    pub watched_threshold_percent: Option<u8>,
    pub auto_play_via_mpv: Option<bool>,
    /// Explicit path to the mpv executable, set by the user when auto-discovery
    /// can't find it (e.g. mpv installed somewhere unusual, or not on PATH).
    /// Takes precedence over every other discovery step.
    pub mpv_path: Option<String>,
    /// User-supplied advanced mpv options, one per line (blank lines and `#`
    /// comments ignored). Appended at launch so they override Vela's render
    /// defaults; the IPC socket / resume seek / media URL are re-asserted after and
    /// can't be overridden. Empowers power users (and weak-hardware users who want a
    /// lighter profile) without Vela having to guess.
    pub mpv_extra_args: Option<String>,
    /// When true, Vela drops `--no-config` so mpv loads the user's own
    /// `~/.config/mpv/mpv.conf`. Off by default for a reproducible launch.
    pub mpv_use_own_config: Option<bool>,
    /// Black-bar cropping via mpv's bundled `autocrop.lua`, three-state:
    /// `"off"` (default; nothing injected), `"manual"` (script loaded with
    /// `autocrop-auto=no` — crops only on an explicit in-player `Shift+C`), or
    /// `"auto"` (script's own crop-on-playback-start). Missing/unknown = off.
    /// `"auto"` auto-fires mpv's live `video-crop`, which can hang mpv on some
    /// HDR/Wayland stacks — the Settings UI carries that warning.
    pub mpv_autocrop: Option<String>,
    /// Non-Plex sources (Jellyfin/Emby today; more later). Kept deliberately
    /// provider-neutral so backends can diverge without a schema change.
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
    /// INERT since 2026-07-08 (local/SMB/SSH sources removed — see
    /// `.agents/decisions.md` "Vela is a multi-server client"): these three
    /// fields are parsed, ignored, and preserved on save so an older build
    /// can still read its config after a rollback. Never strip them here.
    #[serde(default)]
    pub local_folders: Vec<LocalFolder>,
    #[serde(default)]
    pub smb_mounts: Vec<SmbMount>,
    #[serde(default)]
    pub ssh_mounts: Vec<SshMount>,
    /// Per-title playback-source overrides for the merged All view: canonical
    /// title identity → preferred source id. Set from the card's context
    /// menu; titles without an entry follow the default kind ranking.
    #[serde(default)]
    pub merged_overrides: std::collections::HashMap<String, String>,
    /// Vela's own "recently played" history feeding the Continue Watching
    /// hero (see `recents.rs`): item snapshots at play time, final position
    /// stamped at mpv exit, finished entries dropped.
    #[serde(default)]
    pub recents: Vec<crate::recents::RecentEntry>,
    /// Continue Watching tombstones: rating keys the user explicitly removed
    /// from the flow. The hero merge suppresses these even when a server hub
    /// still carries the item; replaying an item clears its tombstone.
    #[serde(default)]
    pub hidden_from_continue: Vec<String>,
    /// Per-library sort preference (owner ask 2026-07-10, "sort should stick
    /// per library"): source-namespaced section key → Vela sort key. Written
    /// on every sort change in a section view; stamped onto SectionDto when
    /// sections are listed. Values are re-validated against the sort
    /// whitelist on read (fail-closed to the default), so a stale or
    /// hand-edited entry degrades instead of erroring. Entries for removed
    /// sections linger harmlessly (bounded by how many libraries existed).
    #[serde(default)]
    pub section_sorts: std::collections::BTreeMap<String, String>,
}

/// A configured SMB share. INERT: kept only so old configs round-trip
/// (rollback-safe); no code mounts or browses these anymore. Every field —
/// including the legacy `kind`/`local_folder_id` pair — must survive
/// load→save (they were `skip_serializing` while a migrator handled them;
/// with the migrator gone, dropping them on save would lose rollback data).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)] // missing fields fall back to Default rather than failing the parse
pub struct SmbMount {
    pub id: String,
    pub name: String,
    pub server: String,
    pub share: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub domain: String,
    pub mountpoint: String,
    #[serde(default)]
    pub folders: Vec<SmbFolder>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub local_folder_id: String,
}

/// One selected folder inside an inert `SmbMount` record (see above).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SmbFolder {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub kind: String,
}

/// A configured SSH/SFTP folder. INERT: kept only so old configs round-trip
/// (rollback-safe); no code mounts or browses these anymore. Vela stores no
/// SSH passwords.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)] // missing fields fall back to Default rather than failing the parse
pub struct SshMount {
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub username: String,
    pub remote_path: String,
    #[serde(default)]
    pub identity_file: String,
    #[serde(default)]
    pub kind: String,
    /// Where sshfs mounts it (and the path the local folder points at).
    pub mountpoint: String,
    /// The `local_folders` entry this mount feeds, removed on unmount.
    pub local_folder_id: String,
}

fn default_ssh_port() -> u16 {
    22
}

/// A configured local folder. INERT: kept only so old configs round-trip
/// (rollback-safe); no code browses these anymore.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)] // missing fields fall back to Default rather than failing the parse
pub struct LocalFolder {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub kind: String,
}

/// Persisted connection for one configured source. Provider-neutral: `kind`
/// selects the backend, and the optional fields cover the union of what the
/// backends need (e.g. a user token vs. a pre-issued API key) without forcing
/// every backend into one exact shape.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)] // missing fields fall back to Default rather than failing the parse
pub struct SourceConfig {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
}

impl AppConfig {
    /// Add or replace a source by id, then return self for chaining.
    pub fn upsert_source(&mut self, src: SourceConfig) {
        self.sources.retain(|s| s.id != src.id);
        self.sources.push(src);
    }
}

/// A file inside the app's config dir, creating the dir if needed. Surfaces a
/// real IO error rather than collapsing it into "no path".
pub fn config_dir_file(name: &str) -> io::Result<PathBuf> {
    crate::storage::config_dir_file(name)
}

fn config_path() -> io::Result<PathBuf> {
    config_dir_file("config.json")
}

/// Atomically load → modify → save the config under a process-wide lock. `f`
/// can validate and return an error (no save happens then) and may return a
/// value (e.g. a clone of the updated config). This is fully synchronous, so
/// callers must not hold its result-producing closure across an await.
pub fn update<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&mut AppConfig) -> Result<T, String>,
{
    let path = config_path().map_err(|error| format!("config unavailable: {error}"))?;
    let lock_path = config_dir_file("config.lock")
        .map_err(|error| format!("config lock unavailable: {error}"))?;
    crate::storage::update_json("config", &CONFIG_LOCK, &path, &lock_path, f)
}

pub fn load_config() -> io::Result<AppConfig> {
    crate::storage::load_json(&config_path()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Per-library sort preferences: absent in old configs (defaults empty),
    // round-trips entries, unknown keys are the reader's problem (get_sections
    // fail-closes against the sort whitelist — this layer just persists).
    #[test]
    fn section_sorts_default_empty_and_round_trip() {
        let old: AppConfig = serde_json::from_str(r#"{"auth_token":"tok"}"#).expect("parses");
        assert!(old.section_sorts.is_empty(), "missing field defaults empty");

        let mut cfg = AppConfig::default();
        cfg.section_sorts
            .insert("plex-1:6".into(), "episodeAddedAt:desc".into());
        let saved = serde_json::to_string(&cfg).expect("serializes");
        let back: AppConfig = serde_json::from_str(&saved).expect("round-trips");
        assert_eq!(
            back.section_sorts.get("plex-1:6").map(String::as_str),
            Some("episodeAddedAt:desc")
        );
    }

    // Rollback rail for the 2026-07-08 local-source removal: every inert
    // local-family field — including the legacy pre-migration SmbMount shape
    // (`kind`/`local_folder_id` set, no `folders`) — must survive a
    // serde round trip unchanged. This fails if the old `skip_serializing`
    // attrs return. (It exercises the serde layer directly — the same layer
    // load_config/save_config use — not the file I/O around it.)
    #[test]
    fn inert_local_family_config_round_trips_unchanged() {
        let legacy = r#"{
            "auth_token": "tok",
            "local_folders": [
                {"id": "legacy-folder", "name": "Movies", "path": "/Volumes/media", "kind": "movie"}
            ],
            "smb_mounts": [
                {"id": "mount", "name": "Media", "server": "nas", "share": "media",
                 "username": "user", "password": "pass", "mountpoint": "/Volumes/media",
                 "kind": "movie", "local_folder_id": "legacy-folder"}
            ],
            "ssh_mounts": [
                {"id": "sshm", "name": "NAS ssh", "host": "nas", "port": 22,
                 "username": "user", "remote_path": "/srv/media",
                 "mountpoint": "/mnt/vela-ssh", "local_folder_id": "lf-ssh"}
            ]
        }"#;
        let cfg: AppConfig = serde_json::from_str(legacy).expect("legacy config parses");

        // Loaded untouched: no migrator moves/strips legacy fields anymore.
        assert_eq!(
            cfg.local_folders.len(),
            1,
            "local_folders preserved on load"
        );
        assert_eq!(cfg.smb_mounts[0].kind, "movie");
        assert_eq!(cfg.smb_mounts[0].local_folder_id, "legacy-folder");
        assert!(
            cfg.smb_mounts[0].folders.is_empty(),
            "no synthesized folders"
        );

        // Saved with everything intact: a rollback build sees its data.
        let saved = serde_json::to_string(&cfg).expect("serializes");
        let back: AppConfig = serde_json::from_str(&saved).expect("round-trips");
        assert_eq!(back.local_folders.len(), 1);
        assert_eq!(back.local_folders[0].path, "/Volumes/media");
        assert_eq!(
            back.smb_mounts[0].kind, "movie",
            "legacy kind survives save"
        );
        assert_eq!(
            back.smb_mounts[0].local_folder_id, "legacy-folder",
            "legacy local_folder_id survives save"
        );
        assert_eq!(
            back.smb_mounts[0].username, "user",
            "legacy SMB username survives save (rollback credentials)"
        );
        assert_eq!(
            back.smb_mounts[0].password, "pass",
            "legacy SMB password survives save (rollback credentials)"
        );
        assert_eq!(back.ssh_mounts[0].local_folder_id, "lf-ssh");
        assert_eq!(back.ssh_mounts[0].mountpoint, "/mnt/vela-ssh");
    }
}
