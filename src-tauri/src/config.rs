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
    /// Non-Plex sources (Jellyfin/Emby today; more later). Kept deliberately
    /// provider-neutral so backends can diverge without a schema change.
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
    /// Local (and mounted remote) folders browsed by the built-in local source.
    #[serde(default)]
    pub local_folders: Vec<LocalFolder>,
    /// SMB shares Vela mounts itself; selected folders inside each share feed
    /// the local source.
    #[serde(default)]
    pub smb_mounts: Vec<SmbMount>,
    /// SSH/SFTP folders mounted through sshfs (each feeds a `local_folders` entry).
    #[serde(default)]
    pub ssh_mounts: Vec<SshMount>,
    /// Per-title playback-source overrides for the merged All view: canonical
    /// title identity → preferred source id. Set from the card's context
    /// menu; titles without an entry follow the default kind ranking.
    #[serde(default)]
    pub merged_overrides: std::collections::HashMap<String, String>,
}

/// An SMB/CIFS share Vela exposes through the local source. On macOS/Windows
/// Vela stores credentials for OS mounting; on Linux it resolves user-space
/// desktop FUSE mounts instead.
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
    /// Where the OS mounts it.
    pub mountpoint: String,
    /// Selected folders inside this share, each with its own media type.
    #[serde(default)]
    pub folders: Vec<SmbFolder>,
    #[serde(default, skip_serializing)]
    pub kind: String,
    #[serde(default, skip_serializing)]
    pub local_folder_id: String,
}

/// One selected folder inside an SMB share. `path` is relative to the mounted
/// share root, using `/` as the separator; empty means the share root itself.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SmbFolder {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub kind: String,
}

/// An SSH/SFTP folder mounted with sshfs, then browsed through the local source.
/// Authentication is delegated to OpenSSH (`~/.ssh/config`, keys, and agent);
/// Vela stores no SSH passwords.
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

/// One folder the local source browses. `kind` declares whether it holds movies
/// or shows; empty means "auto-detect from contents".
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

    fn normalize_legacy_smb_mounts(&mut self) {
        let mut legacy_folder_ids = std::collections::HashSet::new();
        {
            let local_folders = &self.local_folders;
            for mount in &mut self.smb_mounts {
                if !mount.local_folder_id.is_empty() {
                    legacy_folder_ids.insert(mount.local_folder_id.clone());
                }
                if !mount.folders.is_empty() || mount.local_folder_id.is_empty() {
                    continue;
                }

                let legacy_folder = local_folders
                    .iter()
                    .find(|folder| folder.id == mount.local_folder_id);
                let fallback_name = if mount.name.trim().is_empty() {
                    mount.share.as_str()
                } else {
                    mount.name.as_str()
                };
                let name = legacy_folder
                    .map(|folder| folder.name.trim())
                    .filter(|name| !name.is_empty())
                    .unwrap_or(fallback_name)
                    .to_string();
                let kind = if mount.kind.trim().is_empty() {
                    legacy_folder
                        .map(|folder| folder.kind.clone())
                        .unwrap_or_default()
                } else {
                    mount.kind.clone()
                };

                mount.folders.push(SmbFolder {
                    id: mount.local_folder_id.clone(),
                    name,
                    path: String::new(),
                    kind,
                });
            }
        }

        if !legacy_folder_ids.is_empty() {
            self.local_folders
                .retain(|folder| !legacy_folder_ids.contains(&folder.id));
        }
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
    let mut cfg: AppConfig =
        serde_json::from_str(&s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    cfg.normalize_legacy_smb_mounts();
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

    #[test]
    fn legacy_smb_mount_becomes_selected_folder() {
        let mut cfg = AppConfig {
            local_folders: vec![LocalFolder {
                id: "legacy-folder".to_string(),
                name: "Movies".to_string(),
                path: "/Volumes/media".to_string(),
                kind: "movie".to_string(),
            }],
            smb_mounts: vec![SmbMount {
                id: "mount".to_string(),
                name: "Media".to_string(),
                server: "nas".to_string(),
                share: "media".to_string(),
                username: "user".to_string(),
                password: "pass".to_string(),
                domain: String::new(),
                mountpoint: "/Volumes/media".to_string(),
                folders: Vec::new(),
                kind: String::new(),
                local_folder_id: "legacy-folder".to_string(),
            }],
            ..Default::default()
        };

        cfg.normalize_legacy_smb_mounts();

        assert!(cfg.local_folders.is_empty());
        assert_eq!(cfg.smb_mounts[0].folders.len(), 1);
        let folder = &cfg.smb_mounts[0].folders[0];
        assert_eq!(folder.id, "legacy-folder");
        assert_eq!(folder.name, "Movies");
        assert_eq!(folder.path, "");
        assert_eq!(folder.kind, "movie");
    }
}
