use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
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
    let proj = ProjectDirs::from("com", "vela", "vela")
        .ok_or_else(|| io::Error::other("could not determine a config directory"))?;
    let dir = proj.config_dir();
    fs::create_dir_all(dir)?;
    Ok(dir.join(name))
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
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Mandatory cross-process advisory lock so a second Vela instance can't
    // interleave its own load-modify-save and clobber ours (the in-process lock
    // above only serializes this process). We fail rather than proceed unlocked.
    // Held (and released) when `lock_file` drops at the end of update().
    let lock_path =
        config_dir_file("config.lock").map_err(|e| format!("config lock unavailable: {e}"))?;
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false) // lock-only sentinel; never written/truncated
        .open(&lock_path)
        .map_err(|e| format!("could not open config lock: {e}"))?;
    lock_file
        .lock()
        .map_err(|e| format!("could not acquire config lock: {e}"))?;
    // Propagate a read/parse error rather than defaulting — otherwise we could
    // save an empty config over a good-but-unparseable file and wipe everything.
    let mut cfg = load_config().map_err(|e| format!("could not read config: {e}"))?;
    let out = f(&mut cfg)?;
    save_config(&cfg).map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn load_config() -> io::Result<AppConfig> {
    let path = config_path()?;
    // Open directly: only a genuine "not found" is treated as absent (→ default).
    // A metadata error / broken symlink / permission issue surfaces as an error
    // (Path::exists() would hide those as "absent", risking an overwrite); a
    // parse failure surfaces too, so a later save can't overwrite a good file.
    let mut s = String::new();
    match fs::File::open(&path) {
        Ok(mut f) => f.read_to_string(&mut s)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // open() also reports NotFound for a *dangling symlink* (its target is
            // gone). Use symlink_metadata (lstat, doesn't follow) to tell that
            // apart from a genuinely missing file: only the latter is "absent".
            match fs::symlink_metadata(&path) {
                Err(me) if me.kind() == io::ErrorKind::NotFound => return Ok(AppConfig::default()),
                _ => return Err(e), // path exists (e.g. broken symlink) but can't be read
            }
        }
        Err(e) => return Err(e),
    };
    let cfg: AppConfig =
        serde_json::from_str(&s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(cfg)
}

pub fn save_config(cfg: &AppConfig) -> io::Result<()> {
    // A failure to resolve/create the config dir is a real error, not a
    // successful no-op (callers must not report success while nothing persists).
    let path = config_path()?;
    let s = serde_json::to_string_pretty(cfg).unwrap_or_else(|_| "{}".into());

    // Write to a temp file then atomically rename over the target, so a reader
    // never sees a half-written config. The config holds auth tokens and SMB
    // passwords, so on Unix the temp file is created owner-only (0600) from the
    // start — never briefly group/world-readable — and a chmod failure is fatal.
    // The temp name is process-unique so separate processes can't clobber it.
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    // Remove any stale temp from a prior run. A failure other than "not found"
    // means we can't guarantee a clean file, so abort rather than risk writing
    // secrets into an existing one with looser permissions.
    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    {
        // create_new (O_CREAT|O_EXCL) requires a brand-new file, so on Unix it
        // gets 0600 from creation — never a write-before-chmod window.
        #[cfg(unix)]
        let mut f = {
            use std::os::unix::fs::OpenOptionsExt;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)?
        };
        #[cfg(not(unix))]
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(s.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(cfg.local_folders.len(), 1, "local_folders preserved on load");
        assert_eq!(cfg.smb_mounts[0].kind, "movie");
        assert_eq!(cfg.smb_mounts[0].local_folder_id, "legacy-folder");
        assert!(cfg.smb_mounts[0].folders.is_empty(), "no synthesized folders");

        // Saved with everything intact: a rollback build sees its data.
        let saved = serde_json::to_string(&cfg).expect("serializes");
        let back: AppConfig = serde_json::from_str(&saved).expect("round-trips");
        assert_eq!(back.local_folders.len(), 1);
        assert_eq!(back.local_folders[0].path, "/Volumes/media");
        assert_eq!(back.smb_mounts[0].kind, "movie", "legacy kind survives save");
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
