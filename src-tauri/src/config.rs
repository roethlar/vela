use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const LEGACY_PLEX_SOURCE_ID: &str = "plex";

/// Serializes load-modify-save cycles so concurrent commands (and Plex
/// rediscovery) can't lose each other's updates via interleaved read/writes.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)] // missing fields fall back to Default rather than failing the parse
pub struct AppConfig {
    /// Pre-multi-Plex singleton credentials. Read only by the one-shot
    /// migration; new code persists Plex credentials on `sources` entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_server_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_server_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// What to do after a clean EOF once a single item or named playlist has
    /// genuinely ended: `"off"`, `"on"`, or `"only-tv"`. Missing and unknown
    /// values fail closed to the product default (`"only-tv"`) in the command
    /// layer, keeping older configs compatible without baking policy into serde.
    pub continue_playing: Option<String>,
    /// Persisted media-server connections (Plex, Jellyfin, and Emby). Kept
    /// deliberately provider-neutral so backends can diverge without a schema
    /// change.
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
    /// Present only while the cross-file legacy Plex re-key is incomplete.
    /// Persisting the minted id before touching playlists makes the migration
    /// retry-safe across a crash between the two atomic JSON writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) plex_source_migration: Option<PlexSourceMigration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PlexSourceMigration {
    from_id: String,
    to_id: String,
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
    /// Plex's stable physical-server identity. Other providers leave this
    /// empty. A restored Plex endpoint carrying this value is pinned before
    /// any rediscovery can select among the account's other servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_identifier: Option<String>,
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
    update_at(&path, &lock_path, f)
}

fn update_at<T, F>(path: &Path, lock_path: &Path, f: F) -> Result<T, String>
where
    F: FnOnce(&mut AppConfig) -> Result<T, String>,
{
    crate::storage::update_json("config", &CONFIG_LOCK, path, lock_path, f)
}

fn rekey_namespaced(value: &mut String, old_source_id: &str, new_source_id: &str) {
    let Some((source_id, raw)) = value.split_once(':') else {
        return;
    };
    if source_id == old_source_id {
        *value = crate::source::namespace_key(new_source_id, raw);
    }
}

fn rekey_config_references(cfg: &mut AppConfig, old_source_id: &str, new_source_id: &str) {
    if let Some(section) = &mut cfg.last_section_key {
        rekey_namespaced(section, old_source_id, new_source_id);
    }
    for source_id in cfg.merged_overrides.values_mut() {
        if source_id == old_source_id {
            *source_id = new_source_id.to_string();
        }
    }
    for recent in &mut cfg.recents {
        crate::source::rekey_item_source(&mut recent.item, old_source_id, new_source_id);
    }
    for key in &mut cfg.hidden_from_continue {
        rekey_namespaced(key, old_source_id, new_source_id);
    }

    let old_sort_keys = cfg
        .section_sorts
        .keys()
        .filter(|key| key.split_once(':').is_some_and(|(source_id, _)| source_id == old_source_id))
        .cloned()
        .collect::<Vec<_>>();
    for old_key in old_sort_keys {
        if let Some(value) = cfg.section_sorts.remove(&old_key) {
            let mut new_key = old_key;
            rekey_namespaced(&mut new_key, old_source_id, new_source_id);
            cfg.section_sorts.entry(new_key).or_insert(value);
        }
    }
}

fn legacy_server_base(cfg: &AppConfig) -> Option<String> {
    let host = cfg.last_server_host.as_deref()?;
    let port = cfg.last_server_port?;
    let scheme = cfg.last_server_scheme.as_deref()?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    Some(format!("{scheme}://{host}:{port}"))
}

fn migration_needed(cfg: &AppConfig) -> bool {
    cfg.plex_source_migration.is_some()
        || (cfg.auth_token.is_some() && cfg.client_identifier.is_some())
        || cfg
            .sources
            .iter()
            .any(|source| source.kind == "plex" && source.id == LEGACY_PLEX_SOURCE_ID)
}

fn prepare_legacy_plex_migration(
    cfg: &mut AppConfig,
    make_id: impl FnOnce() -> String,
) -> Result<Option<PlexSourceMigration>, String> {
    if let Some(migration) = cfg.plex_source_migration.clone() {
        if migration.from_id != LEGACY_PLEX_SOURCE_ID
            || !cfg.sources.iter().any(|source| {
                source.kind == "plex" && source.id == migration.to_id
            })
        {
            return Err("invalid partial Plex source migration in config".to_string());
        }
        rekey_config_references(cfg, &migration.from_id, &migration.to_id);
        return Ok(Some(migration));
    }

    let legacy_positions = cfg
        .sources
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            (source.kind == "plex" && source.id == LEGACY_PLEX_SOURCE_ID).then_some(index)
        })
        .collect::<Vec<_>>();
    if legacy_positions.len() > 1 {
        return Err("multiple persisted Plex sources use the retired `plex` id".to_string());
    }
    if legacy_positions.is_empty()
        && !(cfg.auth_token.is_some() && cfg.client_identifier.is_some())
    {
        return Ok(None);
    }

    let new_id = make_id();
    if new_id.is_empty()
        || new_id.contains(':')
        || new_id == LEGACY_PLEX_SOURCE_ID
        || cfg.sources.iter().any(|source| source.id == new_id)
    {
        return Err("could not mint a unique Plex source id".to_string());
    }

    let legacy_base = legacy_server_base(cfg);
    let legacy_token = cfg.auth_token.clone();
    let legacy_client = cfg.client_identifier.clone();
    if let Some(index) = legacy_positions.into_iter().next() {
        let source = &mut cfg.sources[index];
        source.id = new_id.clone();
        if source.name.trim().is_empty() {
            source.name = "Plex".to_string();
        }
        if source.base_url.trim().is_empty() {
            source.base_url = legacy_base.unwrap_or_default();
        }
        if source.access_token.is_none() {
            source.access_token = legacy_token;
        }
        if source.device_id.is_none() {
            source.device_id = legacy_client;
        }
    } else {
        cfg.sources.push(SourceConfig {
            id: new_id.clone(),
            kind: "plex".to_string(),
            name: "Plex".to_string(),
            base_url: legacy_base.unwrap_or_default(),
            access_token: legacy_token,
            api_key: None,
            user_id: None,
            device_id: legacy_client,
            machine_identifier: None,
        });
    }

    rekey_config_references(cfg, LEGACY_PLEX_SOURCE_ID, &new_id);
    let migration = PlexSourceMigration {
        from_id: LEGACY_PLEX_SOURCE_ID.to_string(),
        to_id: new_id,
    };
    cfg.plex_source_migration = Some(migration.clone());
    Ok(Some(migration))
}

fn finish_legacy_plex_migration(
    cfg: &mut AppConfig,
    migration: &PlexSourceMigration,
) -> Result<(), String> {
    match cfg.plex_source_migration.as_ref() {
        Some(current) if current == migration => {}
        None if cfg
            .sources
            .iter()
            .any(|source| source.kind == "plex" && source.id == migration.to_id) =>
        {
            return Ok(());
        }
        _ => return Err("Plex source migration changed while it was running".to_string()),
    }
    cfg.auth_token = None;
    cfg.client_identifier = None;
    cfg.last_server_host = None;
    cfg.last_server_port = None;
    cfg.last_server_scheme = None;
    cfg.plex_source_migration = None;
    Ok(())
}

fn migrate_legacy_plex_with(
    path: &Path,
    lock_path: &Path,
    make_id: impl FnOnce() -> String,
    migrate_playlists: impl FnOnce(&str, &str) -> Result<(), String>,
) -> Result<(), String> {
    let migration = update_at(path, lock_path, |cfg| {
        prepare_legacy_plex_migration(cfg, make_id)
    })?;
    let Some(migration) = migration else {
        return Ok(());
    };
    migrate_playlists(&migration.from_id, &migration.to_id)?;
    update_at(path, lock_path, |cfg| {
        finish_legacy_plex_migration(cfg, &migration)
    })
}

fn load_config_with(
    path: &Path,
    lock_path: &Path,
    make_id: impl FnOnce() -> String,
    migrate_playlists: impl FnOnce(&str, &str) -> Result<(), String>,
) -> io::Result<AppConfig> {
    let cfg: AppConfig = crate::storage::load_json(path)?;
    if !migration_needed(&cfg) {
        return Ok(cfg);
    }
    migrate_legacy_plex_with(path, lock_path, make_id, migrate_playlists)
        .map_err(io::Error::other)?;
    crate::storage::load_json(path)
}

pub fn load_config() -> io::Result<AppConfig> {
    let path = config_path()?;
    let lock_path = config_dir_file("config.lock")?;
    load_config_with(
        &path,
        &lock_path,
        || format!("plex-{}", uuid::Uuid::new_v4()),
        crate::playlists::migrate_source_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playlists::{Playlist, PlaylistEntry, PlaylistFile};
    use crate::recents::RecentEntry;
    use crate::source::{BackingRef, ItemDto};
    use std::fs;

    fn item(source_id: &str) -> ItemDto {
        ItemDto {
            rating_key: format!("{source_id}:1"),
            title: "Migrated movie".to_string(),
            year: Some(2026),
            summary: None,
            duration_ms: Some(60_000),
            media_type: Some("movie".to_string()),
            poster: None,
            series_poster: None,
            backdrop: None,
            view_offset_ms: Some(1_000),
            played: Some(false),
            last_watched_at_ms: None,
            added_at_ms: None,
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            parent_rating_key: Some(format!("{source_id}:2")),
            grandparent_rating_key: Some(format!("{source_id}:3")),
            source_id: source_id.to_string(),
            provider_ids: vec!["imdb:tt1".to_string()],
            backing: Some(vec![
                BackingRef {
                    source_id: source_id.to_string(),
                    rating_key: format!("{source_id}:1"),
                },
                BackingRef {
                    source_id: "jf".to_string(),
                    rating_key: "jf:other".to_string(),
                },
            ]),
            canonical_id: Some("imdb:tt1".to_string()),
            watch_key: Some(format!("{source_id}:4")),
            detail_key: Some(format!("{source_id}:5")),
        }
    }

    fn temp_paths(label: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "vela-config-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        (
            root.join("config.json"),
            root.join("config.lock"),
            root.join("playlists.json"),
            root.join("playlists.lock"),
            root,
        )
    }

    fn legacy_config() -> AppConfig {
        let mut cfg = AppConfig {
            auth_token: Some("legacy-token".to_string()),
            client_identifier: Some("legacy-client".to_string()),
            last_server_host: Some("plex.example".to_string()),
            last_server_port: Some(443),
            last_server_scheme: Some("https".to_string()),
            last_section_key: Some("plex:7".to_string()),
            recents: vec![RecentEntry {
                item: item("plex"),
                session_id: Some("session".to_string()),
                started_at_ms: 1,
                ended_at_ms: 2,
            }],
            hidden_from_continue: vec!["plex:6".to_string(), "jf:keep".to_string()],
            ..Default::default()
        };
        cfg.merged_overrides
            .insert("imdb:tt1".to_string(), "plex".to_string());
        cfg.section_sorts
            .insert("plex:7".to_string(), "titleSort:asc".to_string());
        cfg
    }

    fn playlist_file() -> PlaylistFile {
        PlaylistFile {
            schema_version: 1,
            playlists: vec![Playlist {
                id: "playlist".to_string(),
                name: "Migration".to_string(),
                items: vec![PlaylistEntry {
                    id: "entry".to_string(),
                    item: item("plex"),
                    source_name: Some("Plex".to_string()),
                }],
                created_ms: 1,
                updated_ms: 1,
            }],
        }
    }

    #[test]
    fn legacy_plex_migration_rekeys_every_persisted_route_and_is_idempotent() {
        let (config, config_lock, playlists, playlists_lock, root) =
            temp_paths("plex-migration");
        crate::storage::save_json(&config, &legacy_config()).unwrap();
        crate::storage::save_json(&playlists, &playlist_file()).unwrap();

        let migrated = load_config_with(
            &config,
            &config_lock,
            || "plex-new".to_string(),
            |old, new| {
                crate::playlists::migrate_source_id_at(
                    &playlists,
                    &playlists_lock,
                    old,
                    new,
                )
            },
        )
        .unwrap();

        assert_eq!(migrated.sources.len(), 1);
        let source = &migrated.sources[0];
        assert_eq!(source.id, "plex-new");
        assert_eq!(source.kind, "plex");
        assert_eq!(source.base_url, "https://plex.example:443");
        assert_eq!(source.access_token.as_deref(), Some("legacy-token"));
        assert_eq!(source.device_id.as_deref(), Some("legacy-client"));
        assert_eq!(source.machine_identifier, None);
        assert_eq!(migrated.auth_token, None);
        assert_eq!(migrated.client_identifier, None);
        assert_eq!(migrated.last_server_host, None);
        assert_eq!(migrated.plex_source_migration, None);
        assert_eq!(migrated.last_section_key.as_deref(), Some("plex-new:7"));
        assert_eq!(
            migrated.merged_overrides.get("imdb:tt1").map(String::as_str),
            Some("plex-new")
        );
        assert_eq!(
            migrated.section_sorts.get("plex-new:7").map(String::as_str),
            Some("titleSort:asc")
        );
        assert_eq!(
            migrated.hidden_from_continue,
            vec!["plex-new:6".to_string(), "jf:keep".to_string()]
        );
        let recent = &migrated.recents[0].item;
        assert_eq!(recent.rating_key, "plex-new:1");
        assert_eq!(recent.source_id, "plex-new");
        assert_eq!(recent.parent_rating_key.as_deref(), Some("plex-new:2"));
        assert_eq!(recent.grandparent_rating_key.as_deref(), Some("plex-new:3"));
        assert_eq!(recent.watch_key.as_deref(), Some("plex-new:4"));
        assert_eq!(recent.detail_key.as_deref(), Some("plex-new:5"));
        assert_eq!(recent.backing.as_ref().unwrap()[0].source_id, "plex-new");
        assert_eq!(
            recent.backing.as_ref().unwrap()[0].rating_key,
            "plex-new:1"
        );
        assert_eq!(recent.backing.as_ref().unwrap()[1].rating_key, "jf:other");

        let migrated_playlists: PlaylistFile = crate::storage::load_json(&playlists).unwrap();
        let playlist_item = &migrated_playlists.playlists[0].items[0].item;
        assert_eq!(playlist_item.source_id, "plex-new");
        assert_eq!(playlist_item.rating_key, "plex-new:1");
        assert_eq!(playlist_item.watch_key.as_deref(), Some("plex-new:4"));

        let config_bytes = fs::read(&config).unwrap();
        let playlist_bytes = fs::read(&playlists).unwrap();
        let loaded_again = load_config_with(
            &config,
            &config_lock,
            || panic!("a completed migration must never mint another id"),
            |old, new| {
                crate::playlists::migrate_source_id_at(
                    &playlists,
                    &playlists_lock,
                    old,
                    new,
                )
            },
        )
        .unwrap();
        assert_eq!(loaded_again.sources[0].id, "plex-new");
        assert_eq!(fs::read(&config).unwrap(), config_bytes);
        assert_eq!(fs::read(&playlists).unwrap(), playlist_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_playlist_rekey_keeps_a_retryable_config_marker_and_id() {
        let (config, config_lock, playlists, playlists_lock, root) =
            temp_paths("plex-migration-retry");
        crate::storage::save_json(&config, &legacy_config()).unwrap();
        fs::write(&playlists, b"{not json").unwrap();
        let bad_playlist = fs::read(&playlists).unwrap();

        let first = load_config_with(
            &config,
            &config_lock,
            || "plex-first".to_string(),
            |old, new| {
                crate::playlists::migrate_source_id_at(
                    &playlists,
                    &playlists_lock,
                    old,
                    new,
                )
            },
        );
        assert!(first.is_err());
        assert_eq!(fs::read(&playlists).unwrap(), bad_playlist);
        let partial: AppConfig = crate::storage::load_json(&config).unwrap();
        assert_eq!(partial.sources[0].id, "plex-first");
        assert_eq!(partial.auth_token.as_deref(), Some("legacy-token"));
        assert_eq!(
            partial.plex_source_migration,
            Some(PlexSourceMigration {
                from_id: "plex".to_string(),
                to_id: "plex-first".to_string(),
            })
        );

        crate::storage::save_json(&playlists, &playlist_file()).unwrap();
        let complete = load_config_with(
            &config,
            &config_lock,
            || panic!("a retry must reuse the persisted target id"),
            |old, new| {
                crate::playlists::migrate_source_id_at(
                    &playlists,
                    &playlists_lock,
                    old,
                    new,
                )
            },
        )
        .unwrap();
        assert_eq!(complete.sources[0].id, "plex-first");
        assert_eq!(complete.auth_token, None);
        assert_eq!(complete.plex_source_migration, None);
        let migrated_playlists: PlaylistFile = crate::storage::load_json(&playlists).unwrap();
        assert_eq!(
            migrated_playlists.playlists[0].items[0].item.rating_key,
            "plex-first:1"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migration_does_not_create_an_absent_playlist_store() {
        let (config, config_lock, playlists, playlists_lock, root) =
            temp_paths("plex-migration-no-playlists");
        crate::storage::save_json(&config, &legacy_config()).unwrap();
        load_config_with(
            &config,
            &config_lock,
            || "plex-new".to_string(),
            |old, new| {
                crate::playlists::migrate_source_id_at(
                    &playlists,
                    &playlists_lock,
                    old,
                    new,
                )
            },
        )
        .unwrap();
        assert!(!playlists.exists());
        fs::remove_dir_all(root).unwrap();
    }

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
