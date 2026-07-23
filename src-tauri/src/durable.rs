use crate::config::{self, AppConfig, ConnectionsSplitBackup};
use crate::connections::{self, ConnectionsConfig};
use crate::source::{self, SourceRegistry};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static COMMANDS_READY: AtomicBool = AtomicBool::new(cfg!(test));
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();
static DURABLE_LOAD_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn register_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

pub(crate) fn set_commands_ready(ready: bool) {
    COMMANDS_READY.store(ready, Ordering::Release);
}

pub(crate) fn ensure_commands_ready() -> Result<(), String> {
    if COMMANDS_READY.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err("Vela's settings or connections require attention".to_string())
    }
}

fn publish_runtime_fault(file: DurableFile, error: &io::Error) {
    set_commands_ready(false);
    let Some(app) = APP_HANDLE.get().cloned() else {
        return;
    };
    let status_kind = if error.kind() == io::ErrorKind::InvalidData {
        DurableStatusKind::RecoverableInvalid
    } else {
        DurableStatusKind::Unavailable
    };
    tauri::async_runtime::spawn(async move {
        use tauri::{Emitter, Manager};
        let state = app.state::<crate::AppState>();
        *state.registry.lock().await = SourceRegistry::default();
        let next = {
            let mut status = state.durable_status.lock().await;
            match file {
                DurableFile::Settings => {
                    let layout = connections_path()
                        .map(|path| classify_layout(&path))
                        .unwrap_or(DurableLayout::PostSplit);
                    status.settings = DurableFileStatus {
                        status: status_kind,
                        layout,
                    };
                }
                DurableFile::Connections => {
                    status.connections = DurableFileStatus {
                        status: status_kind,
                        layout: DurableLayout::PostSplit,
                    };
                }
            }
            status.clone()
        };
        let _ = app.emit("durable-state-fault", next);
    });
}

#[derive(Clone, Copy)]
enum DurableFile {
    Settings,
    Connections,
}

pub(crate) fn report_settings_fault(error: &io::Error) {
    publish_runtime_fault(DurableFile::Settings, error);
}

pub(crate) fn report_connections_fault(error: &io::Error) {
    publish_runtime_fault(DurableFile::Connections, error);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DurableStatusKind {
    Ready,
    RecoverableInvalid,
    Unavailable,
    MigrationBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DurableLayout {
    PostSplit,
    LegacyCombined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DurableFileStatus {
    pub(crate) status: DurableStatusKind,
    pub(crate) layout: DurableLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DurableStateStatus {
    pub(crate) settings: DurableFileStatus,
    pub(crate) connections: DurableFileStatus,
}

impl DurableStateStatus {
    pub(crate) fn ready() -> Self {
        Self {
            settings: DurableFileStatus {
                status: DurableStatusKind::Ready,
                layout: DurableLayout::PostSplit,
            },
            connections: DurableFileStatus {
                status: DurableStatusKind::Ready,
                layout: DurableLayout::PostSplit,
            },
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.settings.status == DurableStatusKind::Ready
            && self.connections.status == DurableStatusKind::Ready
    }
}

pub(crate) struct ReadyDurableState {
    pub(crate) registry: SourceRegistry,
}

pub(crate) struct DurableLoadFailure {
    pub(crate) status: DurableStateStatus,
}

fn status_for_error(error: &io::Error, layout: DurableLayout) -> DurableFileStatus {
    DurableFileStatus {
        status: if error.kind() == io::ErrorKind::InvalidData {
            DurableStatusKind::RecoverableInvalid
        } else {
            DurableStatusKind::Unavailable
        },
        layout,
    }
}

fn ready_file(layout: DurableLayout) -> DurableFileStatus {
    DurableFileStatus {
        status: DurableStatusKind::Ready,
        layout,
    }
}

fn connection_result_status(result: &io::Result<ConnectionsConfig>) -> DurableFileStatus {
    match result {
        Ok(_) => ready_file(DurableLayout::PostSplit),
        Err(error) => status_for_error(error, DurableLayout::PostSplit),
    }
}

fn connections_path() -> io::Result<PathBuf> {
    config::config_dir_file("connections.json")
}

fn classify_layout(connections_path: &Path) -> DurableLayout {
    match fs::symlink_metadata(connections_path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => DurableLayout::LegacyCombined,
        _ => DurableLayout::PostSplit,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn private_split_backup(config_path: &Path, original: &[u8]) -> io::Result<ConnectionsSplitBackup> {
    let parent = config_path
        .parent()
        .ok_or_else(|| io::Error::other("settings path has no parent"))?;
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("config.pre-connections-split-")
                && name.ends_with(".json")
                && fs::symlink_metadata(entry.path())
                    .is_ok_and(|metadata| metadata.file_type().is_file())
                && crate::storage::is_private_regular(&entry.path())
                && fs::read(entry.path()).is_ok_and(|bytes| bytes == original)
            {
                return Ok(ConnectionsSplitBackup {
                    file_name: name.into_owned(),
                    byte_length: original.len() as u64,
                    sha256: sha256_hex(original),
                });
            }
        }
    }

    let name = format!(
        "config.pre-connections-split-{}-{}.json",
        unix_timestamp_seconds(),
        uuid::Uuid::new_v4()
    );
    crate::storage::write_private_new(&parent.join(&name), original)?;
    Ok(ConnectionsSplitBackup {
        file_name: name,
        byte_length: original.len() as u64,
        sha256: sha256_hex(original),
    })
}

fn verify_split_backup(config_path: &Path, backup: &ConnectionsSplitBackup) -> io::Result<()> {
    let parent = config_path
        .parent()
        .ok_or_else(|| io::Error::other("settings path has no parent"))?;
    let path = parent.join(&backup.file_name);
    if !crate::storage::is_private_regular(&path) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "connection migration backup is not private",
        ));
    }
    let bytes = crate::storage::read_regular_bytes(&path)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "connection backup is absent"))?;
    if bytes.len() as u64 != backup.byte_length || sha256_hex(&bytes) != backup.sha256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "connection migration backup verification failed",
        ));
    }
    Ok(())
}

fn unix_timestamp_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn split_connections(
    config_path: &Path,
    config_lock_path: &Path,
    connections_path: &Path,
    connections_lock_path: &Path,
    make_plex_id: impl FnOnce() -> String,
    migrate_playlists: impl FnOnce(&str, &str) -> Result<(), String>,
) -> Result<(AppConfig, ConnectionsConfig), String> {
    let _config_process_guard = config::lock_process();
    let config_file_lock = crate::storage::open_private_lock(config_lock_path)
        .map_err(|_| "settings lock is unavailable".to_string())?;
    config_file_lock
        .lock()
        .map_err(|_| "settings lock could not be acquired".to_string())?;

    let original = crate::storage::read_regular_bytes(config_path)
        .map_err(|_| "legacy settings are unavailable".to_string())?
        .ok_or_else(|| "legacy settings disappeared".to_string())?;
    let mut raw = config::load_unmigrated_at(config_path)
        .map_err(|_| "could not validate legacy settings".to_string())?;
    let backup = match raw.connections_split_backup.clone() {
        Some(backup) => {
            verify_split_backup(config_path, &backup)
                .map_err(|_| "could not verify preserved legacy settings".to_string())?;
            backup
        }
        None => {
            let backup = private_split_backup(config_path, &original)
                .map_err(|_| "could not preserve legacy settings".to_string())?;
            raw.connections_split_backup = Some(backup.clone());
            raw.validate()?;
            crate::storage::save_json(config_path, &raw)
                .map_err(|_| "could not record connection migration".to_string())?;
            backup
        }
    };
    verify_split_backup(config_path, &backup)
        .map_err(|_| "connection migration backup is unavailable".to_string())?;

    // The existing retry-safe Plex re-key owns its playlist lock and stable
    // identity marker. It runs only after the exact pre-split backup exists.
    let migration = config::prepare_legacy_plex_migration(&mut raw, make_plex_id)
        .map_err(|_| "could not prepare legacy Plex migration".to_string())?;
    if let Some(migration) = migration {
        raw.validate()?;
        crate::storage::save_json(config_path, &raw)
            .map_err(|_| "could not record legacy Plex migration".to_string())?;
        migrate_playlists(&migration.from_id, &migration.to_id)
            .map_err(|_| "could not migrate Plex playlist identities".to_string())?;
        config::finish_legacy_plex_migration(&mut raw, &migration)
            .map_err(|_| "could not finish legacy Plex migration".to_string())?;
        raw.validate()?;
        crate::storage::save_json(config_path, &raw)
            .map_err(|_| "could not finish legacy Plex migration".to_string())?;
    }
    let proposed = ConnectionsConfig {
        sources: raw.sources.clone(),
    };
    proposed.validate()?;

    let _connections_process_guard = connections::lock_process();
    let connections_file_lock = crate::storage::open_private_lock(connections_lock_path)
        .map_err(|_| "connections lock is unavailable".to_string())?;
    connections_file_lock
        .lock()
        .map_err(|_| "connections lock could not be acquired".to_string())?;
    match crate::storage::read_regular_bytes(connections_path)
        .map_err(|_| "connections path is unavailable".to_string())?
    {
        None => crate::storage::install_json_new(connections_path, &proposed)
            .map_err(|_| "could not install connections".to_string())?,
        Some(_) => {
            let existing = connections::load_at(connections_path)
                .map_err(|_| "existing connections are invalid".to_string())?;
            if existing != proposed {
                return Err("legacy and split connections differ".to_string());
            }
        }
    }

    raw.sources.clear();
    raw.auth_token = None;
    raw.client_identifier = None;
    raw.last_server_host = None;
    raw.last_server_port = None;
    raw.last_server_scheme = None;
    raw.plex_source_migration = None;
    raw.connections_split_backup = None;
    raw.validate()?;
    crate::storage::save_json(config_path, &raw)
        .map_err(|_| "could not finish connection migration".to_string())?;

    let settings = config::load_unmigrated_at(config_path)
        .map_err(|_| "could not verify migrated settings".to_string())?;
    let connections = connections::load_at(connections_path)
        .map_err(|_| "could not verify migrated connections".to_string())?;
    Ok((settings, connections))
}

fn build_registry(connections: &ConnectionsConfig) -> Result<SourceRegistry, String> {
    let mut registry = SourceRegistry::default();
    for source_config in &connections.sources {
        let source = match source_config.kind.as_str() {
            "plex" => source::plex::build_source(source_config),
            "jellyfin" | "emby" => source::jellyfin::build_source(source_config),
            _ => Err("unknown connection kind".to_string()),
        }?;
        registry.upsert(source);
    }
    Ok(registry)
}

pub(crate) fn load() -> Result<ReadyDurableState, DurableLoadFailure> {
    let _process_guard = DURABLE_LOAD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let durable_lock_path = match config::config_dir_file("durable-state.lock") {
        Ok(path) => path,
        Err(error) => {
            return Err(DurableLoadFailure {
                status: DurableStateStatus {
                    settings: status_for_error(&error, DurableLayout::PostSplit),
                    connections: status_for_error(&error, DurableLayout::PostSplit),
                },
            });
        }
    };
    let durable_lock = match crate::storage::open_private_lock(&durable_lock_path) {
        Ok(file) => file,
        Err(error) => {
            return Err(DurableLoadFailure {
                status: DurableStateStatus {
                    settings: status_for_error(&error, DurableLayout::PostSplit),
                    connections: status_for_error(&error, DurableLayout::PostSplit),
                },
            });
        }
    };
    if let Err(error) = durable_lock.lock() {
        return Err(DurableLoadFailure {
            status: DurableStateStatus {
                settings: status_for_error(&error, DurableLayout::PostSplit),
                connections: status_for_error(&error, DurableLayout::PostSplit),
            },
        });
    }

    let config_path = match config::config_path() {
        Ok(path) => path,
        Err(error) => {
            return Err(DurableLoadFailure {
                status: DurableStateStatus {
                    settings: status_for_error(&error, DurableLayout::PostSplit),
                    connections: ready_file(DurableLayout::PostSplit),
                },
            });
        }
    };
    let connections_path = match connections_path() {
        Ok(path) => path,
        Err(error) => {
            return Err(DurableLoadFailure {
                status: DurableStateStatus {
                    settings: ready_file(DurableLayout::PostSplit),
                    connections: status_for_error(&error, DurableLayout::PostSplit),
                },
            });
        }
    };
    let config_lock_path = match config::config_dir_file("config.lock") {
        Ok(path) => path,
        Err(error) => {
            return Err(DurableLoadFailure {
                status: DurableStateStatus {
                    settings: status_for_error(&error, DurableLayout::PostSplit),
                    connections: ready_file(DurableLayout::PostSplit),
                },
            });
        }
    };
    let connections_lock_path = match config::config_dir_file("connections.lock") {
        Ok(path) => path,
        Err(error) => {
            return Err(DurableLoadFailure {
                status: DurableStateStatus {
                    settings: ready_file(DurableLayout::PostSplit),
                    connections: status_for_error(&error, DurableLayout::PostSplit),
                },
            });
        }
    };
    let layout = classify_layout(&connections_path);
    let connections_result = connections::load_at(&connections_path);

    match crate::storage::read_regular_bytes(&config_path) {
        Ok(_) => {}
        Err(error) => {
            return Err(DurableLoadFailure {
                status: DurableStateStatus {
                    settings: status_for_error(&error, layout),
                    connections: connection_result_status(&connections_result),
                },
            });
        }
    }
    let raw_settings = match config::load_unmigrated_at(&config_path) {
        Ok(settings) => settings,
        Err(error) => {
            return Err(DurableLoadFailure {
                status: DurableStateStatus {
                    settings: status_for_error(&error, layout),
                    connections: connection_result_status(&connections_result),
                },
            });
        }
    };
    let settings_layout = if config::has_legacy_connections(&raw_settings) {
        DurableLayout::LegacyCombined
    } else {
        DurableLayout::PostSplit
    };
    let existing_connections = match connections_result {
        Ok(connections) => connections,
        Err(error) => {
            return Err(DurableLoadFailure {
                status: DurableStateStatus {
                    settings: ready_file(settings_layout),
                    connections: status_for_error(&error, DurableLayout::PostSplit),
                },
            });
        }
    };

    let loaded = if config::has_legacy_connections(&raw_settings) {
        split_connections(
            &config_path,
            &config_lock_path,
            &connections_path,
            &connections_lock_path,
            || format!("plex-{}", uuid::Uuid::new_v4()),
            crate::playlists::migrate_source_id,
        )
        .map_err(|_| DurableLoadFailure {
            status: DurableStateStatus {
                settings: DurableFileStatus {
                    status: DurableStatusKind::MigrationBlocked,
                    layout: DurableLayout::LegacyCombined,
                },
                connections: ready_file(DurableLayout::PostSplit),
            },
        })?
    } else {
        (raw_settings, existing_connections)
    };

    let (_settings, connections) = loaded;
    let registry = build_registry(&connections).map_err(|_| DurableLoadFailure {
        status: DurableStateStatus {
            settings: ready_file(DurableLayout::PostSplit),
            connections: DurableFileStatus {
                status: DurableStatusKind::RecoverableInvalid,
                layout: DurableLayout::PostSplit,
            },
        },
    })?;
    Ok(ReadyDurableState { registry })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SourceConfig;

    fn temp_root(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("vela-durable-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn jellyfin_source(id: &str) -> SourceConfig {
        SourceConfig {
            id: id.to_string(),
            kind: "jellyfin".to_string(),
            name: "Test server".to_string(),
            base_url: "http://127.0.0.1:8096".to_string(),
            access_token: Some("synthetic-split-token".to_string()),
            api_key: None,
            user_id: Some("synthetic-user".to_string()),
            device_id: Some("synthetic-device".to_string()),
            machine_identifier: None,
        }
    }

    fn legacy_plex_settings() -> AppConfig {
        AppConfig {
            auth_token: Some("synthetic-legacy-token".to_string()),
            client_identifier: Some("synthetic-legacy-device".to_string()),
            last_server_host: Some("plex.example".to_string()),
            last_server_port: Some(443),
            last_server_scheme: Some("https".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn status_is_ready_only_when_both_files_are_ready() {
        let mut status = DurableStateStatus::ready();
        assert!(status.is_ready());
        status.connections.status = DurableStatusKind::RecoverableInvalid;
        assert!(!status.is_ready());
    }

    #[test]
    fn valid_combined_settings_split_after_an_exact_private_backup() {
        let root = temp_root("split");
        let config_path = root.join("config.json");
        let config_lock = root.join("config.lock");
        let connections_path = root.join("connections.json");
        let connections_lock = root.join("connections.lock");
        let settings = AppConfig {
            sources: vec![jellyfin_source("jf-one")],
            continue_playing: Some("on".to_string()),
            ..Default::default()
        };
        crate::storage::save_json(&config_path, &settings).unwrap();
        let original = fs::read(&config_path).unwrap();

        let (migrated, connections) = split_connections(
            &config_path,
            &config_lock,
            &connections_path,
            &connections_lock,
            || "plex-unused".to_string(),
            |_, _| Ok(()),
        )
        .unwrap();

        assert!(migrated.sources.is_empty());
        assert_eq!(migrated.continue_playing.as_deref(), Some("on"));
        assert_eq!(connections.sources, vec![jellyfin_source("jf-one")]);
        assert_eq!(
            connections::load_at(&connections_path).unwrap(),
            connections
        );

        let live_settings = fs::read_to_string(&config_path).unwrap();
        assert!(!live_settings.contains("synthetic-split-token"));
        assert!(!live_settings.contains("\"sources\""));
        let backups = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.pre-connections-split-")
            })
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(backups[0].path()).unwrap(), original);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(backups[0].path())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&connections_path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_combined_settings_are_not_mined_for_connections() {
        let root = temp_root("invalid-combined");
        let config_path = root.join("config.json");
        let config_lock = root.join("config.lock");
        let connections_path = root.join("connections.json");
        let connections_lock = root.join("connections.lock");
        let original = br#"{"sources":[{"id":"jf","kind":"jellyfin","name":"Test","base_url":"http://127.0.0.1:8096","access_token":"synthetic","user_id":"u","device_id":"d"}],"future":true}"#.to_vec();
        fs::write(&config_path, &original).unwrap();

        assert!(split_connections(
            &config_path,
            &config_lock,
            &connections_path,
            &connections_lock,
            || "plex-unused".to_string(),
            |_, _| Ok(()),
        )
        .is_err());
        assert!(!connections_path.exists());
        assert_eq!(fs::read(&config_path).unwrap(), original);
        assert_eq!(
            fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.pre-connections-split-"))
                .count(),
            0
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_plex_split_reuses_the_exact_backup_and_minted_identity() {
        let root = temp_root("split-retry");
        let config_path = root.join("config.json");
        let config_lock = root.join("config.lock");
        let connections_path = root.join("connections.json");
        let connections_lock = root.join("connections.lock");
        crate::storage::save_json(&config_path, &legacy_plex_settings()).unwrap();
        let original = fs::read(&config_path).unwrap();

        let first = split_connections(
            &config_path,
            &config_lock,
            &connections_path,
            &connections_lock,
            || "plex-stable".to_string(),
            |from, to| {
                assert_eq!((from, to), ("plex", "plex-stable"));
                Err("injected playlist failure".to_string())
            },
        );
        assert!(first.is_err());
        assert!(!connections_path.exists());

        let (settings, connections) = split_connections(
            &config_path,
            &config_lock,
            &connections_path,
            &connections_lock,
            || "plex-must-not-be-minted".to_string(),
            |from, to| {
                assert_eq!((from, to), ("plex", "plex-stable"));
                Ok(())
            },
        )
        .unwrap();
        assert!(settings.sources.is_empty());
        assert_eq!(connections.sources.len(), 1);
        assert_eq!(connections.sources[0].id, "plex-stable");
        let backups = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.pre-connections-split-")
            })
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(backups[0].path()).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_changed_split_backup_blocks_retry_before_connection_install() {
        let root = temp_root("split-backup-changed");
        let config_path = root.join("config.json");
        let config_lock = root.join("config.lock");
        let connections_path = root.join("connections.json");
        let connections_lock = root.join("connections.lock");
        crate::storage::save_json(&config_path, &legacy_plex_settings()).unwrap();

        assert!(split_connections(
            &config_path,
            &config_lock,
            &connections_path,
            &connections_lock,
            || "plex-stable".to_string(),
            |_, _| Err("injected playlist failure".to_string()),
        )
        .is_err());
        let backup = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.pre-connections-split-")
            })
            .unwrap();
        let mut changed = fs::read(backup.path()).unwrap();
        changed[0] ^= 1;
        fs::write(backup.path(), changed).unwrap();

        assert!(split_connections(
            &config_path,
            &config_lock,
            &connections_path,
            &connections_lock,
            || panic!("a retry marker must keep the original source identity"),
            |_, _| panic!("a changed backup must block before playlist migration"),
        )
        .is_err());
        assert!(!connections_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn differing_legacy_and_split_connections_block_without_overwrite() {
        let root = temp_root("split-different");
        let config_path = root.join("config.json");
        let config_lock = root.join("config.lock");
        let connections_path = root.join("connections.json");
        let connections_lock = root.join("connections.lock");
        let settings = AppConfig {
            sources: vec![jellyfin_source("legacy")],
            ..Default::default()
        };
        crate::storage::save_json(&config_path, &settings).unwrap();
        let existing = ConnectionsConfig {
            sources: vec![jellyfin_source("different")],
        };
        crate::storage::save_json(&connections_path, &existing).unwrap();
        let connection_bytes = fs::read(&connections_path).unwrap();

        assert!(split_connections(
            &config_path,
            &config_lock,
            &connections_path,
            &connections_lock,
            || "plex-unused".to_string(),
            |_, _| Ok(()),
        )
        .is_err());
        assert_eq!(fs::read(&connections_path).unwrap(), connection_bytes);
        let held = config::load_unmigrated_at(&config_path).unwrap();
        assert_eq!(held.sources, vec![jellyfin_source("legacy")]);
        assert!(held.connections_split_backup.is_some());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn equal_preexisting_connections_finish_split_cleanup() {
        let root = temp_root("split-equal");
        let config_path = root.join("config.json");
        let config_lock = root.join("config.lock");
        let connections_path = root.join("connections.json");
        let connections_lock = root.join("connections.lock");
        let source = jellyfin_source("same");
        let settings = AppConfig {
            sources: vec![source.clone()],
            ..Default::default()
        };
        crate::storage::save_json(&config_path, &settings).unwrap();
        crate::storage::save_json(
            &connections_path,
            &ConnectionsConfig {
                sources: vec![source.clone()],
            },
        )
        .unwrap();
        let connection_bytes = fs::read(&connections_path).unwrap();

        let (settings, connections) = split_connections(
            &config_path,
            &config_lock,
            &connections_path,
            &connections_lock,
            || "plex-unused".to_string(),
            |_, _| Ok(()),
        )
        .unwrap();
        assert!(settings.sources.is_empty());
        assert_eq!(connections.sources, vec![source]);
        assert_eq!(fs::read(&connections_path).unwrap(), connection_bytes);
        fs::remove_dir_all(root).unwrap();
    }
}
