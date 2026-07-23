use crate::config::{self, AppConfig, ConnectionsSplitBackup};
use crate::connections::{self, ConnectionsConfig};
use crate::source::{self, SourceRegistry};
use serde::{Deserialize, Serialize};
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
    let error_kind = error.kind();
    tauri::async_runtime::spawn(async move {
        use tauri::{Emitter, Manager};
        let error = io::Error::from(error_kind);
        let state = app.state::<crate::AppState>();
        *state.registry.lock().await = SourceRegistry::default();
        let next = {
            let mut gate = state.durable_gate.lock().await;
            match file {
                DurableFile::Settings => {
                    let path = config::config_path();
                    let layout = connections_path()
                        .map(|connections| classify_layout(&connections))
                        .unwrap_or(DurableLayout::PostSplit);
                    let (status, snapshot) = path
                        .map(|path| {
                            status_and_snapshot_for_error(
                                DurableFile::Settings,
                                &error,
                                &path,
                                layout,
                            )
                        })
                        .unwrap_or_else(|_| (unavailable_file(layout), None));
                    gate.status.settings = status;
                    gate.settings_snapshot = snapshot.map(Box::new);
                }
                DurableFile::Connections => {
                    let path = connections_path();
                    let (status, snapshot) = path
                        .map(|path| {
                            status_and_snapshot_for_error(
                                DurableFile::Connections,
                                &error,
                                &path,
                                DurableLayout::PostSplit,
                            )
                        })
                        .unwrap_or_else(|_| {
                            (unavailable_file(DurableLayout::PostSplit), None)
                        });
                    gate.status.connections = status;
                    gate.connections_snapshot = snapshot.map(Box::new);
                }
            }
            if gate.recovery_incomplete {
                gate.status.settings.can_recover = false;
                gate.status.connections.can_recover = false;
                gate.status.settings.rollback_versions.clear();
                gate.status.connections.rollback_versions.clear();
                gate.settings_snapshot = None;
                gate.connections_snapshot = None;
            }
            gate.status.clone()
        };
        let _ = app.emit("durable-state-fault", next);
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DurableFile {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    pub(crate) can_recover: bool,
    pub(crate) rollback_versions: Vec<DurableRollbackVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DurableRollbackVersion {
    pub(crate) id: String,
    pub(crate) created_at_unix_ms: u64,
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
                can_recover: false,
                rollback_versions: Vec::new(),
            },
            connections: DurableFileStatus {
                status: DurableStatusKind::Ready,
                layout: DurableLayout::PostSplit,
                can_recover: false,
                rollback_versions: Vec::new(),
            },
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.settings.status == DurableStatusKind::Ready
            && self.connections.status == DurableStatusKind::Ready
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InvalidSnapshot {
    byte_length: u64,
    sha256: String,
    rollback_versions: Vec<ValidHistorySnapshot>,
}

impl InvalidSnapshot {
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            byte_length: bytes.len() as u64,
            sha256: sha256_hex(bytes),
            rollback_versions: Vec::new(),
        }
    }

    fn matches(&self, bytes: &[u8]) -> bool {
        self.byte_length == bytes.len() as u64 && self.sha256 == sha256_hex(bytes)
    }

    fn rollback(&self, id: &str) -> Option<&ValidHistorySnapshot> {
        self.rollback_versions
            .iter()
            .find(|version| version.public.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidHistorySnapshot {
    public: DurableRollbackVersion,
    file_name: String,
    byte_length: u64,
    sha256: String,
}

impl ValidHistorySnapshot {
    fn matches(&self, bytes: &[u8]) -> bool {
        self.byte_length == bytes.len() as u64 && self.sha256 == sha256_hex(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryMarker {
    file: DurableFile,
    layout: DurableLayout,
    backup_file_name: String,
    byte_length: u64,
    sha256: String,
    #[serde(default)]
    replacement: RecoveryReplacement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RecoveryReplacement {
    #[default]
    Default,
    History {
        #[serde(rename = "versionId")]
        version_id: String,
        #[serde(rename = "fileName")]
        file_name: String,
        #[serde(rename = "createdAtUnixMs")]
        created_at_unix_ms: u64,
        #[serde(rename = "byteLength")]
        byte_length: u64,
        sha256: String,
    },
}

impl RecoveryReplacement {
    fn is_default(&self) -> bool {
        matches!(self, Self::Default)
    }

    fn public_version(&self) -> Option<DurableRollbackVersion> {
        match self {
            Self::Default => None,
            Self::History {
                version_id,
                created_at_unix_ms,
                ..
            } => Some(DurableRollbackVersion {
                id: version_id.clone(),
                created_at_unix_ms: *created_at_unix_ms,
            }),
        }
    }
}

impl RecoveryMarker {
    fn new(
        file: DurableFile,
        layout: DurableLayout,
        backup_file_name: String,
        snapshot: &InvalidSnapshot,
    ) -> io::Result<Self> {
        let marker = Self {
            file,
            layout,
            backup_file_name,
            byte_length: snapshot.byte_length,
            sha256: snapshot.sha256.clone(),
            replacement: RecoveryReplacement::Default,
        };
        marker.validate()?;
        Ok(marker)
    }

    fn new_history(
        file: DurableFile,
        layout: DurableLayout,
        backup_file_name: String,
        snapshot: &InvalidSnapshot,
        version: &ValidHistorySnapshot,
    ) -> io::Result<Self> {
        let marker = Self {
            file,
            layout,
            backup_file_name,
            byte_length: snapshot.byte_length,
            sha256: snapshot.sha256.clone(),
            replacement: RecoveryReplacement::History {
                version_id: version.public.id.clone(),
                file_name: version.file_name.clone(),
                created_at_unix_ms: version.public.created_at_unix_ms,
                byte_length: version.byte_length,
                sha256: version.sha256.clone(),
            },
        };
        marker.validate()?;
        Ok(marker)
    }

    fn snapshot(&self) -> InvalidSnapshot {
        InvalidSnapshot {
            byte_length: self.byte_length,
            sha256: self.sha256.clone(),
            rollback_versions: Vec::new(),
        }
    }

    fn validate(&self) -> io::Result<()> {
        if self.file == DurableFile::Connections && self.layout != DurableLayout::PostSplit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid recovery marker",
            ));
        }
        let expected_prefix = match self.file {
            DurableFile::Settings => "config.invalid-",
            DurableFile::Connections => "connections.invalid-",
        };
        let suffixless = self
            .backup_file_name
            .strip_prefix(expected_prefix)
            .and_then(|value| value.strip_suffix(".json"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid recovery marker"))?;
        let (timestamp, uuid) = suffixless
            .split_once('-')
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid recovery marker"))?;
        timestamp
            .parse::<u64>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid recovery marker"))?;
        uuid::Uuid::parse_str(uuid)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid recovery marker"))?;
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid recovery marker",
            ));
        }
        if let RecoveryReplacement::History {
            version_id,
            file_name,
            created_at_unix_ms,
            byte_length: _,
            sha256,
        } = &self.replacement
        {
            let parsed = parse_history_file_name(self.file, file_name).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid recovery marker")
            })?;
            if parsed.public.id != *version_id
                || parsed.public.created_at_unix_ms != *created_at_unix_ms
                || parsed.sha256 != *sha256
                || sha256.len() != 64
                || !sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid recovery marker",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DurableGate {
    pub(crate) status: DurableStateStatus,
    settings_snapshot: Option<Box<InvalidSnapshot>>,
    connections_snapshot: Option<Box<InvalidSnapshot>>,
    recovery_incomplete: bool,
}

impl DurableGate {
    pub(crate) fn ready() -> Self {
        Self {
            status: DurableStateStatus::ready(),
            settings_snapshot: None,
            connections_snapshot: None,
            recovery_incomplete: false,
        }
    }

    fn snapshot(&self, file: DurableFile) -> Option<&InvalidSnapshot> {
        match file {
            DurableFile::Settings => self.settings_snapshot.as_deref(),
            DurableFile::Connections => self.connections_snapshot.as_deref(),
        }
    }

    pub(crate) fn can_recover(&self, file: DurableFile) -> bool {
        let status = match file {
            DurableFile::Settings => &self.status.settings,
            DurableFile::Connections => &self.status.connections,
        };
        status.status == DurableStatusKind::RecoverableInvalid
            && status.can_recover
            && self.snapshot(file).is_some()
            && !self.recovery_incomplete
    }

    pub(crate) fn recovery_incomplete(&self) -> bool {
        self.recovery_incomplete
    }

    pub(crate) fn can_rollback(&self, file: DurableFile, version_id: &str) -> bool {
        self.can_recover(file)
            && self
                .snapshot(file)
                .and_then(|snapshot| snapshot.rollback(version_id))
                .is_some()
    }
}

pub(crate) struct ReadyDurableState {
    pub(crate) registry: SourceRegistry,
}

pub(crate) struct DurableLoadFailure {
    pub(crate) gate: DurableGate,
}

fn status_for_error(error: &io::Error, layout: DurableLayout) -> DurableFileStatus {
    DurableFileStatus {
        status: if error.kind() == io::ErrorKind::InvalidData {
            DurableStatusKind::RecoverableInvalid
        } else {
            DurableStatusKind::Unavailable
        },
        layout,
        can_recover: false,
        rollback_versions: Vec::new(),
    }
}

fn ready_file(layout: DurableLayout) -> DurableFileStatus {
    DurableFileStatus {
        status: DurableStatusKind::Ready,
        layout,
        can_recover: false,
        rollback_versions: Vec::new(),
    }
}

fn unavailable_file(layout: DurableLayout) -> DurableFileStatus {
    DurableFileStatus {
        status: DurableStatusKind::Unavailable,
        layout,
        can_recover: false,
        rollback_versions: Vec::new(),
    }
}

fn status_and_snapshot_for_error(
    file: DurableFile,
    error: &io::Error,
    path: &Path,
    layout: DurableLayout,
) -> (DurableFileStatus, Option<InvalidSnapshot>) {
    if error.kind() != io::ErrorKind::InvalidData {
        return (status_for_error(error, layout), None);
    }
    match crate::storage::read_regular_bytes(path) {
        Ok(Some(bytes)) => {
            let rollback_versions = valid_history_at(file, path).unwrap_or_default();
            let public_versions = rollback_versions
                .iter()
                .map(|version| version.public.clone())
                .collect();
            (
                DurableFileStatus {
                status: DurableStatusKind::RecoverableInvalid,
                layout,
                can_recover: true,
                    rollback_versions: public_versions,
                },
                Some(InvalidSnapshot {
                    rollback_versions,
                    ..InvalidSnapshot::from_bytes(&bytes)
                }),
            )
        }
        _ => (unavailable_file(layout), None),
    }
}

fn connection_result_status(
    result: &io::Result<ConnectionsConfig>,
    path: &Path,
) -> (DurableFileStatus, Option<InvalidSnapshot>) {
    match result {
        Ok(_) => (ready_file(DurableLayout::PostSplit), None),
        Err(error) => status_and_snapshot_for_error(
            DurableFile::Connections,
            error,
            path,
            DurableLayout::PostSplit,
        ),
    }
}

fn failure_gate(
    settings: DurableFileStatus,
    connections: DurableFileStatus,
    settings_snapshot: Option<InvalidSnapshot>,
    connections_snapshot: Option<InvalidSnapshot>,
) -> DurableGate {
    DurableGate {
        status: DurableStateStatus {
            settings,
            connections,
        },
        settings_snapshot: settings_snapshot.map(Box::new),
        connections_snapshot: connections_snapshot.map(Box::new),
        recovery_incomplete: false,
    }
}

fn recovery_incomplete_gate(
    marker: Option<&RecoveryMarker>,
    previous: Option<&DurableGate>,
) -> DurableGate {
    let mut gate = previous.cloned().unwrap_or_else(DurableGate::ready);
    match marker.map(|value| value.file) {
        Some(DurableFile::Settings) => {
            gate.status.settings = DurableFileStatus {
                status: DurableStatusKind::MigrationBlocked,
                layout: marker.expect("present marker").layout,
                can_recover: false,
                rollback_versions: Vec::new(),
            };
            gate.settings_snapshot = None;
        }
        Some(DurableFile::Connections) => {
            gate.status.connections = DurableFileStatus {
                status: DurableStatusKind::MigrationBlocked,
                layout: DurableLayout::PostSplit,
                can_recover: false,
                rollback_versions: Vec::new(),
            };
            gate.connections_snapshot = None;
        }
        None => {
            gate.status.settings = DurableFileStatus {
                status: DurableStatusKind::MigrationBlocked,
                layout: DurableLayout::PostSplit,
                can_recover: false,
                rollback_versions: Vec::new(),
            };
            gate.status.connections = DurableFileStatus {
                status: DurableStatusKind::MigrationBlocked,
                layout: DurableLayout::PostSplit,
                can_recover: false,
                rollback_versions: Vec::new(),
            };
            gate.settings_snapshot = None;
            gate.connections_snapshot = None;
        }
    }
    gate.status.settings.can_recover = false;
    gate.status.connections.can_recover = false;
    gate.status.settings.rollback_versions.clear();
    gate.status.connections.rollback_versions.clear();
    gate.settings_snapshot = None;
    gate.connections_snapshot = None;
    gate.recovery_incomplete = true;
    gate
}

fn connections_path() -> io::Result<PathBuf> {
    config::config_dir_file("connections.json")
}

fn recovery_marker_path() -> io::Result<PathBuf> {
    config::config_dir_file("durable-recovery.json")
}

fn load_recovery_marker(path: &Path) -> io::Result<Option<RecoveryMarker>> {
    let Some(bytes) = crate::storage::read_regular_bytes(path)? else {
        return Ok(None);
    };
    let marker: RecoveryMarker = serde_json::from_slice(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid recovery marker"))?;
    marker.validate()?;
    Ok(Some(marker))
}

fn install_recovery_marker(path: &Path, marker: &RecoveryMarker) -> io::Result<()> {
    marker.validate()?;
    crate::storage::install_json_new(path, marker)?;
    if load_recovery_marker(path)?.as_ref() != Some(marker) {
        return Err(io::Error::other("recovery marker verification failed"));
    }
    Ok(())
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

const HISTORY_LIMIT: usize = 3;

fn unix_timestamp_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn history_stem(file: DurableFile) -> &'static str {
    match file {
        DurableFile::Settings => "config",
        DurableFile::Connections => "connections",
    }
}

struct ParsedHistoryName {
    public: DurableRollbackVersion,
    sha256: String,
}

fn parse_history_file_name(file: DurableFile, file_name: &str) -> Option<ParsedHistoryName> {
    let body = file_name
        .strip_prefix(&format!("{}.valid-", history_stem(file)))?
        .strip_suffix(".json")?;
    let (timestamp, sha256) = body.split_once('-')?;
    let created_at_unix_ms = timestamp.parse::<u64>().ok()?;
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return None;
    }
    Some(ParsedHistoryName {
        public: DurableRollbackVersion {
            id: sha256.to_string(),
            created_at_unix_ms,
        },
        sha256: sha256.to_string(),
    })
}

fn validate_selected_bytes(file: DurableFile, bytes: &[u8]) -> io::Result<()> {
    match file {
        DurableFile::Settings => {
            let value: AppConfig = serde_json::from_slice(bytes)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings"))?;
            value
                .validate()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings"))
        }
        DurableFile::Connections => {
            let value: ConnectionsConfig = serde_json::from_slice(bytes)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid connections"))?;
            value.validate().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid connections")
            })
        }
    }
}

fn all_valid_history_at(
    file: DurableFile,
    canonical_path: &Path,
) -> io::Result<Vec<ValidHistorySnapshot>> {
    let parent = canonical_path
        .parent()
        .ok_or_else(|| io::Error::other("durable file has no parent"))?;
    let mut versions = Vec::new();
    for entry in fs::read_dir(parent)? {
        let Ok(entry) = entry else {
            continue;
        };
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Some(parsed) = parse_history_file_name(file, file_name) else {
            continue;
        };
        let path = entry.path();
        if !crate::storage::is_private_regular(&path) {
            continue;
        }
        let Some(bytes) = crate::storage::read_regular_bytes(&path)? else {
            continue;
        };
        if sha256_hex(&bytes) != parsed.sha256 || validate_selected_bytes(file, &bytes).is_err() {
            continue;
        }
        versions.push(ValidHistorySnapshot {
            public: parsed.public,
            file_name: file_name.to_string(),
            byte_length: bytes.len() as u64,
            sha256: parsed.sha256,
        });
    }
    versions.sort_by(|left, right| {
        right
            .public
            .created_at_unix_ms
            .cmp(&left.public.created_at_unix_ms)
            .then_with(|| right.public.id.cmp(&left.public.id))
    });
    Ok(versions)
}

fn valid_history_at(
    file: DurableFile,
    canonical_path: &Path,
) -> io::Result<Vec<ValidHistorySnapshot>> {
    let mut versions = all_valid_history_at(file, canonical_path)?;
    versions.truncate(HISTORY_LIMIT);
    Ok(versions)
}

pub(crate) fn preserve_valid_history(
    file: DurableFile,
    canonical_path: &Path,
    bytes: &[u8],
) -> Result<(), String> {
    validate_selected_bytes(file, bytes)
        .map_err(|_| "could not validate the prior durable version".to_string())?;
    let parent = canonical_path
        .parent()
        .ok_or_else(|| "durable file has no parent".to_string())?;
    let mut versions = all_valid_history_at(file, canonical_path)
        .map_err(|_| "could not inspect durable version history".to_string())?;
    let duplicate = versions.iter().any(|version| {
        version.matches(bytes)
            && crate::storage::read_regular_bytes(&parent.join(&version.file_name))
                .is_ok_and(|candidate| candidate.as_deref() == Some(bytes))
    });
    if !duplicate {
        let next_timestamp = versions
            .first()
            .map(|version| version.public.created_at_unix_ms.saturating_add(1))
            .unwrap_or_default()
            .max(unix_timestamp_millis());
        let file_name = format!(
            "{}.valid-{}-{}.json",
            history_stem(file),
            next_timestamp,
            sha256_hex(bytes)
        );
        let path = parent.join(&file_name);
        crate::storage::write_private_new(&path, bytes)
            .map_err(|_| "could not preserve durable version history".to_string())?;
        let verify = crate::storage::read_regular_bytes(&path)
            .map_err(|_| "could not verify durable version history".to_string())?
            .ok_or_else(|| "durable version history disappeared".to_string())?;
        if verify != bytes
            || !crate::storage::is_private_regular(&path)
            || validate_selected_bytes(file, &verify).is_err()
        {
            return Err("could not verify durable version history".to_string());
        }
        versions = all_valid_history_at(file, canonical_path)
            .map_err(|_| "could not verify durable version history".to_string())?;
    }
    for version in versions.iter().skip(HISTORY_LIMIT) {
        crate::storage::remove_private_regular(&parent.join(&version.file_name))
            .map_err(|_| "could not prune durable version history".to_string())?;
    }
    Ok(())
}

fn invalid_backup_name(file: DurableFile) -> String {
    let stem = history_stem(file);
    format!(
        "{stem}.invalid-{}-{}.json",
        unix_timestamp_seconds(),
        uuid::Uuid::new_v4()
    )
}

fn validate_selected_file(file: DurableFile, path: &Path) -> io::Result<()> {
    match file {
        DurableFile::Settings => config::load_unmigrated_at(path).map(|_| ()),
        DurableFile::Connections => connections::load_at(path).map(|_| ()),
    }
}

fn install_selected_default(file: DurableFile, path: &Path) -> io::Result<()> {
    match file {
        DurableFile::Settings => {
            let value = AppConfig::default();
            value
                .validate()
                .map_err(|_| io::Error::other("fresh settings failed validation"))?;
            crate::storage::install_json_new(path, &value)
        }
        DurableFile::Connections => {
            let value = ConnectionsConfig::default();
            value
                .validate()
                .map_err(|_| io::Error::other("fresh connections failed validation"))?;
            crate::storage::install_json_new(path, &value)
        }
    }
}

#[derive(Debug)]
enum RecoveryFileError {
    Stale,
    BeforeMarker,
    BeforeRename,
    AfterRename { backup_file_name: String },
    Incomplete {
        backup_file_name: Option<String>,
    },
}

fn finish_selected_recovery(
    file: DurableFile,
    path: &Path,
    backup_path: &Path,
    current: &[u8],
    expected: &InvalidSnapshot,
) -> io::Result<()> {
    crate::storage::harden_existing_regular(backup_path)?;
    if !crate::storage::is_private_regular(backup_path) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "preserved file is not private",
        ));
    }
    let preserved = crate::storage::read_regular_bytes(backup_path)?
        .ok_or_else(|| io::Error::other("preserved file disappeared"))?;
    if !expected.matches(&preserved) || preserved != current {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preserved file verification failed",
        ));
    }
    install_selected_replacement(file, path, &RecoveryReplacement::Default)?;
    validate_selected_file(file, path)
}

fn install_selected_replacement(
    file: DurableFile,
    path: &Path,
    replacement: &RecoveryReplacement,
) -> io::Result<()> {
    match replacement {
        RecoveryReplacement::Default => install_selected_default(file, path),
        RecoveryReplacement::History {
            version_id,
            file_name,
            created_at_unix_ms,
            byte_length,
            sha256,
        } => {
            let version = ValidHistorySnapshot {
                public: DurableRollbackVersion {
                    id: version_id.clone(),
                    created_at_unix_ms: *created_at_unix_ms,
                },
                file_name: file_name.clone(),
                byte_length: *byte_length,
                sha256: sha256.clone(),
            };
            let bytes = exact_history_bytes(file, path, &version)?;
            crate::storage::write_private_new(path, &bytes)
        }
    }
}

fn exact_history_bytes(
    file: DurableFile,
    canonical_path: &Path,
    version: &ValidHistorySnapshot,
) -> io::Result<Vec<u8>> {
    let parsed = parse_history_file_name(file, &version.file_name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid history"))?;
    if parsed.public != version.public || parsed.sha256 != version.sha256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history selection changed",
        ));
    }
    let parent = canonical_path
        .parent()
        .ok_or_else(|| io::Error::other("durable file has no parent"))?;
    let history_path = parent.join(&version.file_name);
    if !crate::storage::is_private_regular(&history_path) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "history version is not private",
        ));
    }
    let bytes = crate::storage::read_regular_bytes(&history_path)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "history is absent"))?;
    if !version.matches(&bytes) || validate_selected_bytes(file, &bytes).is_err() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history version changed",
        ));
    }
    Ok(bytes)
}

fn installed_replacement_matches(
    file: DurableFile,
    path: &Path,
    replacement: &RecoveryReplacement,
) -> bool {
    let Ok(Some(bytes)) = crate::storage::read_regular_bytes(path) else {
        return false;
    };
    if validate_selected_bytes(file, &bytes).is_err() {
        return false;
    }
    match replacement {
        RecoveryReplacement::Default => true,
        RecoveryReplacement::History {
            byte_length,
            sha256,
            ..
        } => bytes.len() as u64 == *byte_length && sha256_hex(&bytes) == *sha256,
    }
}

fn finish_selected_recovery_with_replacement(
    file: DurableFile,
    path: &Path,
    backup_path: &Path,
    current: &[u8],
    expected: &InvalidSnapshot,
    replacement: &RecoveryReplacement,
) -> io::Result<()> {
    crate::storage::harden_existing_regular(backup_path)?;
    if !crate::storage::is_private_regular(backup_path) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "preserved file is not private",
        ));
    }
    let preserved = crate::storage::read_regular_bytes(backup_path)?
        .ok_or_else(|| io::Error::other("preserved file disappeared"))?;
    if !expected.matches(&preserved) || preserved != current {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preserved file verification failed",
        ));
    }
    install_selected_replacement(file, path, replacement)?;
    validate_selected_file(file, path)
}

#[cfg(test)]
fn recover_selected_at_with(
    file: DurableFile,
    path: &Path,
    expected: &InvalidSnapshot,
    backup_file_name: String,
    finish: impl FnOnce(
        DurableFile,
        &Path,
        &Path,
        &[u8],
        &InvalidSnapshot,
    ) -> io::Result<()>,
) -> Result<String, RecoveryFileError> {
    recover_selected_at_with_hooks(
        file,
        path,
        expected,
        backup_file_name,
        || Ok(()),
        finish,
    )
}

fn recover_selected_at_with_hooks(
    file: DurableFile,
    path: &Path,
    expected: &InvalidSnapshot,
    backup_file_name: String,
    before_rename: impl FnOnce() -> io::Result<()>,
    finish: impl FnOnce(
        DurableFile,
        &Path,
        &Path,
        &[u8],
        &InvalidSnapshot,
    ) -> io::Result<()>,
) -> Result<String, RecoveryFileError> {
    let current = crate::storage::read_regular_bytes(path)
        .map_err(|_| RecoveryFileError::Stale)?
        .ok_or(RecoveryFileError::Stale)?;
    if !expected.matches(&current) {
        return Err(RecoveryFileError::Stale);
    }
    match validate_selected_file(file, path) {
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {}
        _ => return Err(RecoveryFileError::Stale),
    }

    let parent = path.parent().ok_or(RecoveryFileError::BeforeRename)?;
    let backup_path = parent.join(&backup_file_name);
    before_rename().map_err(|_| RecoveryFileError::BeforeMarker)?;
    crate::storage::rename_noreplace(path, &backup_path)
        .map_err(|_| RecoveryFileError::BeforeRename)?;

    if finish(file, path, &backup_path, &current, expected).is_err() {
        return Err(RecoveryFileError::AfterRename { backup_file_name });
    }
    Ok(backup_file_name)
}

fn recover_selected_at_with_marker(
    file: DurableFile,
    path: &Path,
    expected: &InvalidSnapshot,
    layout: DurableLayout,
    marker_path: &Path,
) -> Result<String, RecoveryFileError> {
    recover_selected_at_with_marker_and_finish(
        file,
        path,
        expected,
        layout,
        marker_path,
        finish_selected_recovery,
    )
}

fn recover_selected_history_at_with_marker(
    file: DurableFile,
    path: &Path,
    expected: &InvalidSnapshot,
    layout: DurableLayout,
    marker_path: &Path,
    version: &ValidHistorySnapshot,
) -> Result<String, RecoveryFileError> {
    exact_history_bytes(file, path, version).map_err(|_| RecoveryFileError::Stale)?;
    let backup_file_name = invalid_backup_name(file);
    let marker = RecoveryMarker::new_history(
        file,
        layout,
        backup_file_name.clone(),
        expected,
        version,
    )
    .map_err(|_| RecoveryFileError::BeforeMarker)?;
    let replacement = marker.replacement.clone();
    let result = recover_selected_at_with_hooks(
        file,
        path,
        expected,
        backup_file_name.clone(),
        || install_recovery_marker(marker_path, &marker),
        |file, path, backup_path, current, expected| {
            finish_selected_recovery_with_replacement(
                file,
                path,
                backup_path,
                current,
                expected,
                &replacement,
            )
        },
    );
    match result {
        Ok(backup_file_name) => {
            crate::storage::remove_private_regular(marker_path).map_err(|_| {
                RecoveryFileError::Incomplete {
                    backup_file_name: Some(backup_file_name.clone()),
                }
            })?;
            Ok(backup_file_name)
        }
        Err(RecoveryFileError::BeforeRename) => {
            crate::storage::remove_private_regular(marker_path)
                .map_err(|_| RecoveryFileError::Incomplete {
                    backup_file_name: None,
                })?;
            Err(RecoveryFileError::BeforeRename)
        }
        other => other,
    }
}

fn recover_selected_at_with_marker_and_finish(
    file: DurableFile,
    path: &Path,
    expected: &InvalidSnapshot,
    layout: DurableLayout,
    marker_path: &Path,
    finish: impl FnOnce(
        DurableFile,
        &Path,
        &Path,
        &[u8],
        &InvalidSnapshot,
    ) -> io::Result<()>,
) -> Result<String, RecoveryFileError> {
    let backup_file_name = invalid_backup_name(file);
    let marker = RecoveryMarker::new(file, layout, backup_file_name.clone(), expected)
        .map_err(|_| RecoveryFileError::BeforeMarker)?;
    let result = recover_selected_at_with_hooks(
        file,
        path,
        expected,
        backup_file_name.clone(),
        || install_recovery_marker(marker_path, &marker),
        finish,
    );
    match result {
        Ok(backup_file_name) => {
            crate::storage::remove_private_regular(marker_path).map_err(|_| {
                RecoveryFileError::Incomplete {
                    backup_file_name: Some(backup_file_name.clone()),
                }
            })?;
            Ok(backup_file_name)
        }
        Err(RecoveryFileError::BeforeRename) => {
            crate::storage::remove_private_regular(marker_path)
                .map_err(|_| RecoveryFileError::Incomplete {
                    backup_file_name: None,
                })?;
            Err(RecoveryFileError::BeforeRename)
        }
        other => other,
    }
}

fn selected_path(parent: &Path, file: DurableFile) -> PathBuf {
    match file {
        DurableFile::Settings => parent.join("config.json"),
        DurableFile::Connections => parent.join("connections.json"),
    }
}

fn exact_private_backup(
    backup_path: &Path,
    expected: &InvalidSnapshot,
) -> io::Result<Option<Vec<u8>>> {
    let Some(bytes) = crate::storage::read_regular_bytes(backup_path)? else {
        return Ok(None);
    };
    if !crate::storage::is_private_regular(backup_path) || !expected.matches(&bytes) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preserved recovery file does not match its marker",
        ));
    }
    Ok(Some(bytes))
}

fn resume_recovery_at(marker_path: &Path, marker: &RecoveryMarker) -> io::Result<()> {
    marker.validate()?;
    let parent = marker_path
        .parent()
        .ok_or_else(|| io::Error::other("recovery marker has no parent"))?;
    let path = selected_path(parent, marker.file);
    let backup_path = parent.join(&marker.backup_file_name);
    let expected = marker.snapshot();
    let current = crate::storage::read_regular_bytes(&path)?;
    let backup = exact_private_backup(&backup_path, &expected)?;

    match (current, backup) {
        (Some(current), None) if expected.matches(&current) => {
            match validate_selected_file(marker.file, &path) {
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {}
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "recovery state is ambiguous",
                    ));
                }
            }
            crate::storage::rename_noreplace(&path, &backup_path)?;
            finish_selected_recovery_with_replacement(
                marker.file,
                &path,
                &backup_path,
                &current,
                &expected,
                &marker.replacement,
            )?;
        }
        (None, Some(backup)) => {
            finish_selected_recovery_with_replacement(
                marker.file,
                &path,
                &backup_path,
                &backup,
                &expected,
                &marker.replacement,
            )?;
        }
        (Some(_), Some(_))
            if installed_replacement_matches(marker.file, &path, &marker.replacement) => {}
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "recovery state is ambiguous",
            ));
        }
    }

    crate::storage::remove_private_regular(marker_path)
}

fn unavailable_after_recovery(
    mut gate: DurableGate,
    file: DurableFile,
    recovery_incomplete: bool,
) -> DurableGate {
    match file {
        DurableFile::Settings => {
            let layout = gate.status.settings.layout;
            gate.status.settings = unavailable_file(layout);
            gate.settings_snapshot = None;
        }
        DurableFile::Connections => {
            gate.status.connections = unavailable_file(DurableLayout::PostSplit);
            gate.connections_snapshot = None;
        }
    }
    gate.recovery_incomplete = recovery_incomplete;
    gate
}

pub(crate) enum RecoveryTransaction {
    Changed {
        backup_file_name: String,
        reconnect_required: bool,
        restored_version: Option<DurableRollbackVersion>,
    },
    Stale,
    Failed {
        gate: DurableGate,
        backup_file_name: Option<String>,
        message: &'static str,
    },
}

pub(crate) fn recover_invalid_file(
    file: DurableFile,
    expected_gate: DurableGate,
) -> RecoveryTransaction {
    recover_invalid_file_with_version(file, expected_gate, None)
}

pub(crate) fn rollback_invalid_file(
    file: DurableFile,
    version_id: &str,
    expected_gate: DurableGate,
) -> RecoveryTransaction {
    if !expected_gate.can_rollback(file, version_id) {
        return RecoveryTransaction::Stale;
    }
    let version = expected_gate
        .snapshot(file)
        .and_then(|snapshot| snapshot.rollback(version_id))
        .expect("eligible rollback must retain its exact history version")
        .clone();
    recover_invalid_file_with_version(file, expected_gate, Some(version))
}

fn recover_invalid_file_with_version(
    file: DurableFile,
    expected_gate: DurableGate,
    selected_version: Option<ValidHistorySnapshot>,
) -> RecoveryTransaction {
    if !expected_gate.can_recover(file) {
        return RecoveryTransaction::Stale;
    }
    let expected = expected_gate
        .snapshot(file)
        .expect("recoverable gate must retain its exact snapshot")
        .clone();
    let reconnect_required = expected_gate.status.settings.layout == DurableLayout::LegacyCombined
        || (file == DurableFile::Connections && selected_version.is_none());

    let _process_guard = DURABLE_LOAD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let durable_lock_path = match config::config_dir_file("durable-state.lock") {
        Ok(path) => path,
        Err(_) => {
            return RecoveryTransaction::Failed {
                gate: unavailable_after_recovery(expected_gate, file, false),
                backup_file_name: None,
                message: "Vela could not safely open its recovery lock.",
            };
        }
    };
    let durable_lock = match crate::storage::open_private_lock(&durable_lock_path) {
        Ok(lock) => lock,
        Err(_) => {
            return RecoveryTransaction::Failed {
                gate: unavailable_after_recovery(expected_gate, file, false),
                backup_file_name: None,
                message: "Vela could not safely open its recovery lock.",
            };
        }
    };
    if durable_lock.lock().is_err() {
        return RecoveryTransaction::Failed {
            gate: unavailable_after_recovery(expected_gate, file, false),
            backup_file_name: None,
            message: "Vela could not safely acquire its recovery lock.",
        };
    }

    let (path, lock_path) = match file {
        DurableFile::Settings => (
            config::config_path(),
            config::config_dir_file("config.lock"),
        ),
        DurableFile::Connections => (
            connections_path(),
            config::config_dir_file("connections.lock"),
        ),
    };
    let (path, lock_path) = match (path, lock_path) {
        (Ok(path), Ok(lock_path)) => (path, lock_path),
        _ => {
            return RecoveryTransaction::Failed {
                gate: unavailable_after_recovery(expected_gate, file, false),
                backup_file_name: None,
                message: "Vela could not safely resolve the damaged file.",
            };
        }
    };
    let marker_path = match recovery_marker_path() {
        Ok(path) => path,
        Err(_) => {
            return RecoveryTransaction::Failed {
                gate: unavailable_after_recovery(expected_gate, file, false),
                backup_file_name: None,
                message: "Vela could not safely resolve its recovery record.",
            };
        }
    };
    let layout = match file {
        DurableFile::Settings => expected_gate.status.settings.layout,
        DurableFile::Connections => DurableLayout::PostSplit,
    };

    let result = match file {
        DurableFile::Settings => {
            let _selected_process_guard = config::lock_process();
            let lock = match crate::storage::open_private_lock(&lock_path) {
                Ok(lock) => lock,
                Err(_) => {
                    return RecoveryTransaction::Failed {
                        gate: unavailable_after_recovery(expected_gate, file, false),
                        backup_file_name: None,
                        message: "Vela could not safely open the settings lock.",
                    };
                }
            };
            if lock.lock().is_err() {
                return RecoveryTransaction::Failed {
                    gate: unavailable_after_recovery(expected_gate, file, false),
                    backup_file_name: None,
                    message: "Vela could not safely acquire the settings lock.",
                };
            }
            match selected_version.as_ref() {
                Some(version) => recover_selected_history_at_with_marker(
                    file,
                    &path,
                    &expected,
                    layout,
                    &marker_path,
                    version,
                ),
                None => {
                    recover_selected_at_with_marker(file, &path, &expected, layout, &marker_path)
                }
            }
        }
        DurableFile::Connections => {
            let _selected_process_guard = connections::lock_process();
            let lock = match crate::storage::open_private_lock(&lock_path) {
                Ok(lock) => lock,
                Err(_) => {
                    return RecoveryTransaction::Failed {
                        gate: unavailable_after_recovery(expected_gate, file, false),
                        backup_file_name: None,
                        message: "Vela could not safely open the connections lock.",
                    };
                }
            };
            if lock.lock().is_err() {
                return RecoveryTransaction::Failed {
                    gate: unavailable_after_recovery(expected_gate, file, false),
                    backup_file_name: None,
                    message: "Vela could not safely acquire the connections lock.",
                };
            }
            match selected_version.as_ref() {
                Some(version) => recover_selected_history_at_with_marker(
                    file,
                    &path,
                    &expected,
                    layout,
                    &marker_path,
                    version,
                ),
                None => {
                    recover_selected_at_with_marker(file, &path, &expected, layout, &marker_path)
                }
            }
        }
    };

    match result {
        Ok(backup_file_name) => RecoveryTransaction::Changed {
            backup_file_name,
            reconnect_required,
            restored_version: selected_version.map(|version| version.public),
        },
        Err(RecoveryFileError::Stale) => RecoveryTransaction::Stale,
        Err(RecoveryFileError::BeforeMarker) => match load_recovery_marker(&marker_path) {
            Ok(None) => RecoveryTransaction::Failed {
                gate: expected_gate,
                backup_file_name: None,
                message: "Vela could not safely record the recovery attempt.",
            },
            Ok(Some(marker)) => RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(Some(&marker), Some(&expected_gate)),
                backup_file_name: None,
                message: "Vela could not verify its recovery record. Recovery remains blocked.",
            },
            Err(_) => RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(None, Some(&expected_gate)),
                backup_file_name: None,
                message: "Vela could not verify its recovery record. Recovery remains blocked.",
            },
        },
        Err(RecoveryFileError::BeforeRename) => RecoveryTransaction::Failed {
            gate: expected_gate,
            backup_file_name: None,
            message: "Vela could not safely rename the damaged file.",
        },
        Err(RecoveryFileError::AfterRename { backup_file_name }) => {
            let marker = match selected_version.as_ref() {
                Some(version) => RecoveryMarker::new_history(
                    file,
                    layout,
                    backup_file_name.clone(),
                    &expected,
                    version,
                )
                .ok(),
                None => {
                    RecoveryMarker::new(file, layout, backup_file_name.clone(), &expected).ok()
                }
            };
            RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(marker.as_ref(), Some(&expected_gate)),
                backup_file_name: Some(backup_file_name),
                message: "Vela preserved the damaged file but could not safely create the fresh file.",
            }
        }
        Err(RecoveryFileError::Incomplete { backup_file_name }) => {
            let marker = backup_file_name.as_ref().and_then(|backup_file_name| {
                match selected_version.as_ref() {
                    Some(version) => RecoveryMarker::new_history(
                        file,
                        layout,
                        backup_file_name.clone(),
                        &expected,
                        version,
                    )
                    .ok(),
                    None => {
                        RecoveryMarker::new(file, layout, backup_file_name.clone(), &expected).ok()
                    }
                }
            });
            RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(marker.as_ref(), Some(&expected_gate)),
                backup_file_name,
                message: "Vela preserved the damaged file, but recovery is not yet complete.",
            }
        }
    }
}

pub(crate) fn resume_incomplete_recovery(
    expected_gate: DurableGate,
) -> RecoveryTransaction {
    let _process_guard = DURABLE_LOAD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let durable_lock_path = match config::config_dir_file("durable-state.lock") {
        Ok(path) => path,
        Err(_) => {
            return RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(None, Some(&expected_gate)),
                backup_file_name: None,
                message: "Vela could not safely open its recovery lock.",
            };
        }
    };
    let durable_lock = match crate::storage::open_private_lock(&durable_lock_path) {
        Ok(lock) => lock,
        Err(_) => {
            return RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(None, Some(&expected_gate)),
                backup_file_name: None,
                message: "Vela could not safely open its recovery lock.",
            };
        }
    };
    if durable_lock.lock().is_err() {
        return RecoveryTransaction::Failed {
            gate: recovery_incomplete_gate(None, Some(&expected_gate)),
            backup_file_name: None,
            message: "Vela could not safely acquire its recovery lock.",
        };
    }

    let marker_path = match recovery_marker_path() {
        Ok(path) => path,
        Err(_) => {
            return RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(None, Some(&expected_gate)),
                backup_file_name: None,
                message: "Vela could not safely resolve its recovery record.",
            };
        }
    };
    let marker = match load_recovery_marker(&marker_path) {
        Ok(Some(marker)) => marker,
        Ok(None) => {
            return RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(None, Some(&expected_gate)),
                backup_file_name: None,
                message: "Vela's recovery record is missing. Recovery remains blocked.",
            };
        }
        Err(_) => {
            return RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(None, Some(&expected_gate)),
                backup_file_name: None,
                message: "Vela could not verify its recovery record. Recovery remains blocked.",
            };
        }
    };

    let (path, lock_path) = match marker.file {
        DurableFile::Settings => (
            config::config_path(),
            config::config_dir_file("config.lock"),
        ),
        DurableFile::Connections => (
            connections_path(),
            config::config_dir_file("connections.lock"),
        ),
    };
    if path.is_err() || lock_path.is_err() {
        return RecoveryTransaction::Failed {
            gate: recovery_incomplete_gate(Some(&marker), Some(&expected_gate)),
            backup_file_name: None,
            message: "Vela could not safely resolve the recovering file.",
        };
    }
    let lock_path = lock_path.expect("checked recovery lock path");

    let result = match marker.file {
        DurableFile::Settings => {
            let _selected_process_guard = config::lock_process();
            let lock = match crate::storage::open_private_lock(&lock_path) {
                Ok(lock) => lock,
                Err(_) => {
                    return RecoveryTransaction::Failed {
                        gate: recovery_incomplete_gate(Some(&marker), Some(&expected_gate)),
                        backup_file_name: None,
                        message: "Vela could not safely open the settings lock.",
                    };
                }
            };
            if lock.lock().is_err() {
                return RecoveryTransaction::Failed {
                    gate: recovery_incomplete_gate(Some(&marker), Some(&expected_gate)),
                    backup_file_name: None,
                    message: "Vela could not safely acquire the settings lock.",
                };
            }
            resume_recovery_at(&marker_path, &marker)
        }
        DurableFile::Connections => {
            let _selected_process_guard = connections::lock_process();
            let lock = match crate::storage::open_private_lock(&lock_path) {
                Ok(lock) => lock,
                Err(_) => {
                    return RecoveryTransaction::Failed {
                        gate: recovery_incomplete_gate(Some(&marker), Some(&expected_gate)),
                        backup_file_name: None,
                        message: "Vela could not safely open the connections lock.",
                    };
                }
            };
            if lock.lock().is_err() {
                return RecoveryTransaction::Failed {
                    gate: recovery_incomplete_gate(Some(&marker), Some(&expected_gate)),
                    backup_file_name: None,
                    message: "Vela could not safely acquire the connections lock.",
                };
            }
            resume_recovery_at(&marker_path, &marker)
        }
    };

    match result {
        Ok(()) => RecoveryTransaction::Changed {
            backup_file_name: marker.backup_file_name,
            reconnect_required: marker.layout == DurableLayout::LegacyCombined
                || (marker.file == DurableFile::Connections && marker.replacement.is_default()),
            restored_version: marker.replacement.public_version(),
        },
        Err(_) => {
            let backup_file_name = marker_path.parent().and_then(|parent| {
                exact_private_backup(&parent.join(&marker.backup_file_name), &marker.snapshot())
                    .ok()
                    .flatten()
                    .map(|_| marker.backup_file_name.clone())
            });
            RecoveryTransaction::Failed {
                gate: recovery_incomplete_gate(Some(&marker), Some(&expected_gate)),
                backup_file_name,
                message: "Vela could not safely finish the recorded recovery. Recovery remains blocked.",
            }
        }
    }
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
                gate: failure_gate(
                    status_for_error(&error, DurableLayout::PostSplit),
                    status_for_error(&error, DurableLayout::PostSplit),
                    None,
                    None,
                ),
            });
        }
    };
    let durable_lock = match crate::storage::open_private_lock(&durable_lock_path) {
        Ok(file) => file,
        Err(error) => {
            return Err(DurableLoadFailure {
                gate: failure_gate(
                    status_for_error(&error, DurableLayout::PostSplit),
                    status_for_error(&error, DurableLayout::PostSplit),
                    None,
                    None,
                ),
            });
        }
    };
    if let Err(error) = durable_lock.lock() {
        return Err(DurableLoadFailure {
            gate: failure_gate(
                status_for_error(&error, DurableLayout::PostSplit),
                status_for_error(&error, DurableLayout::PostSplit),
                None,
                None,
            ),
        });
    }
    let marker_path = match recovery_marker_path() {
        Ok(path) => path,
        Err(_) => {
            return Err(DurableLoadFailure {
                gate: recovery_incomplete_gate(None, None),
            });
        }
    };
    match load_recovery_marker(&marker_path) {
        Ok(None) => {}
        Ok(Some(marker)) => {
            return Err(DurableLoadFailure {
                gate: recovery_incomplete_gate(Some(&marker), None),
            });
        }
        Err(_) => {
            return Err(DurableLoadFailure {
                gate: recovery_incomplete_gate(None, None),
            });
        }
    }

    let config_path = match config::config_path() {
        Ok(path) => path,
        Err(error) => {
            return Err(DurableLoadFailure {
                gate: failure_gate(
                    status_for_error(&error, DurableLayout::PostSplit),
                    ready_file(DurableLayout::PostSplit),
                    None,
                    None,
                ),
            });
        }
    };
    let connections_path = match connections_path() {
        Ok(path) => path,
        Err(error) => {
            return Err(DurableLoadFailure {
                gate: failure_gate(
                    ready_file(DurableLayout::PostSplit),
                    status_for_error(&error, DurableLayout::PostSplit),
                    None,
                    None,
                ),
            });
        }
    };
    let config_lock_path = match config::config_dir_file("config.lock") {
        Ok(path) => path,
        Err(error) => {
            return Err(DurableLoadFailure {
                gate: failure_gate(
                    status_for_error(&error, DurableLayout::PostSplit),
                    ready_file(DurableLayout::PostSplit),
                    None,
                    None,
                ),
            });
        }
    };
    let connections_lock_path = match config::config_dir_file("connections.lock") {
        Ok(path) => path,
        Err(error) => {
            return Err(DurableLoadFailure {
                gate: failure_gate(
                    ready_file(DurableLayout::PostSplit),
                    status_for_error(&error, DurableLayout::PostSplit),
                    None,
                    None,
                ),
            });
        }
    };
    let layout = classify_layout(&connections_path);
    let connections_result = connections::load_at(&connections_path);

    match crate::storage::read_regular_bytes(&config_path) {
        Ok(_) => {}
        Err(error) => {
            let (settings, settings_snapshot) =
                status_and_snapshot_for_error(
                    DurableFile::Settings,
                    &error,
                    &config_path,
                    layout,
                );
            let (connections, connections_snapshot) =
                connection_result_status(&connections_result, &connections_path);
            return Err(DurableLoadFailure {
                gate: failure_gate(
                    settings,
                    connections,
                    settings_snapshot,
                    connections_snapshot,
                ),
            });
        }
    }
    let raw_settings = match config::load_unmigrated_at(&config_path) {
        Ok(settings) => settings,
        Err(error) => {
            let (settings, settings_snapshot) =
                status_and_snapshot_for_error(
                    DurableFile::Settings,
                    &error,
                    &config_path,
                    layout,
                );
            let (connections, connections_snapshot) =
                connection_result_status(&connections_result, &connections_path);
            return Err(DurableLoadFailure {
                gate: failure_gate(
                    settings,
                    connections,
                    settings_snapshot,
                    connections_snapshot,
                ),
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
            let (connections, connections_snapshot) =
                status_and_snapshot_for_error(
                    DurableFile::Connections,
                    &error,
                    &connections_path,
                    DurableLayout::PostSplit,
                );
            return Err(DurableLoadFailure {
                gate: failure_gate(
                    ready_file(settings_layout),
                    connections,
                    None,
                    connections_snapshot,
                ),
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
            gate: failure_gate(
                DurableFileStatus {
                    status: DurableStatusKind::MigrationBlocked,
                    layout: DurableLayout::LegacyCombined,
                    can_recover: false,
                    rollback_versions: Vec::new(),
                },
                ready_file(DurableLayout::PostSplit),
                None,
                None,
            ),
        })?
    } else {
        (raw_settings, existing_connections)
    };

    let (_settings, connections) = loaded;
    let registry = build_registry(&connections).map_err(|_| DurableLoadFailure {
        gate: {
            let error = io::Error::new(
                io::ErrorKind::InvalidData,
                "persisted connection could not be restored",
            );
            let (status, snapshot) = status_and_snapshot_for_error(
                DurableFile::Connections,
                &error,
                &connections_path,
                DurableLayout::PostSplit,
            );
            failure_gate(
                ready_file(DurableLayout::PostSplit),
                status,
                None,
                snapshot,
            )
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
    fn recoverable_status_retains_an_exact_private_snapshot() {
        let root = temp_root("status-snapshot");
        let path = root.join("config.json");
        let original = br#"{"continue_playing":"future"}"#;
        fs::write(&path, original).unwrap();
        let error = io::Error::new(io::ErrorKind::InvalidData, "synthetic invalid settings");

        let (status, snapshot) = status_and_snapshot_for_error(
            DurableFile::Settings,
            &error,
            &path,
            DurableLayout::PostSplit,
        );
        assert_eq!(status.status, DurableStatusKind::RecoverableInvalid);
        assert!(status.can_recover);
        let snapshot = snapshot.unwrap();
        assert!(snapshot.matches(original));
        let gate = failure_gate(
            status,
            ready_file(DurableLayout::PostSplit),
            Some(snapshot.clone()),
            None,
        );
        assert!(gate.can_recover(DurableFile::Settings));
        let mut unavailable = gate.clone();
        unavailable.status.settings = unavailable_file(DurableLayout::PostSplit);
        assert!(!unavailable.can_recover(DurableFile::Settings));
        let mut incomplete = gate;
        incomplete.recovery_incomplete = true;
        assert!(!incomplete.can_recover(DurableFile::Settings));

        let mut changed = original.to_vec();
        changed[1] ^= 1;
        assert_eq!(changed.len(), original.len());
        assert!(!snapshot.matches(&changed));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn settings_and_connections_keep_only_three_distinct_private_valid_versions() {
        let root = temp_root("valid-history-ring");
        let config_path = root.join("config.json");
        let connections_path = root.join("connections.json");

        let settings_versions = (10..14)
            .map(|threshold| {
                serde_json::to_vec_pretty(&serde_json::json!({
                    "watched_threshold_percent": threshold
                }))
                .unwrap()
            })
            .collect::<Vec<_>>();
        for bytes in &settings_versions {
            preserve_valid_history(DurableFile::Settings, &config_path, bytes).unwrap();
        }
        let before_duplicate = valid_history_at(DurableFile::Settings, &config_path).unwrap();
        preserve_valid_history(
            DurableFile::Settings,
            &config_path,
            settings_versions.last().unwrap(),
        )
        .unwrap();
        let settings_history = valid_history_at(DurableFile::Settings, &config_path).unwrap();
        assert_eq!(settings_history.len(), 3);
        assert_eq!(
            settings_history
                .iter()
                .map(|version| &version.public.id)
                .collect::<Vec<_>>(),
            before_duplicate
                .iter()
                .map(|version| &version.public.id)
                .collect::<Vec<_>>()
        );
        for (version, expected) in settings_history
            .iter()
            .zip(settings_versions.iter().rev().take(3))
        {
            let path = root.join(&version.file_name);
            assert_eq!(fs::read(&path).unwrap(), *expected);
            assert!(crate::storage::is_private_regular(&path));
        }

        for id in ["one", "two", "three", "four"] {
            let value = ConnectionsConfig {
                sources: vec![jellyfin_source(id)],
            };
            let bytes = serde_json::to_vec_pretty(&value).unwrap();
            preserve_valid_history(DurableFile::Connections, &connections_path, &bytes).unwrap();
        }
        let connection_history =
            valid_history_at(DurableFile::Connections, &connections_path).unwrap();
        assert_eq!(connection_history.len(), 3);
        assert!(connection_history
            .windows(2)
            .all(|pair| pair[0].public.created_at_unix_ms > pair[1].public.created_at_unix_ms));
        assert_eq!(
            fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("connections.valid-"))
                .count(),
            3
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_status_offers_newest_valid_versions_and_omits_tampering() {
        let root = temp_root("valid-history-status");
        let config_path = root.join("config.json");
        for threshold in 20..24 {
            let bytes = serde_json::to_vec_pretty(&serde_json::json!({
                "watched_threshold_percent": threshold
            }))
            .unwrap();
            preserve_valid_history(DurableFile::Settings, &config_path, &bytes).unwrap();
        }
        let newest = valid_history_at(DurableFile::Settings, &config_path)
            .unwrap()
            .remove(0);
        let newest_path = root.join(&newest.file_name);
        let mut changed = fs::read(&newest_path).unwrap();
        let digit = changed.iter().position(|byte| *byte == b'3').unwrap();
        changed[digit] = b'9';
        fs::write(&newest_path, &changed).unwrap();
        assert_eq!(changed.len() as u64, newest.byte_length);

        let invalid = br#"{"continue_playing":"future"}"#;
        fs::write(&config_path, invalid).unwrap();
        let error = io::Error::new(io::ErrorKind::InvalidData, "invalid settings");
        let (status, snapshot) = status_and_snapshot_for_error(
            DurableFile::Settings,
            &error,
            &config_path,
            DurableLayout::PostSplit,
        );

        assert!(status.can_recover);
        assert_eq!(status.rollback_versions.len(), HISTORY_LIMIT - 1);
        assert!(!status
            .rollback_versions
            .iter()
            .any(|version| version.id == newest.public.id));
        assert_eq!(
            snapshot.unwrap().rollback_versions.len(),
            HISTORY_LIMIT - 1
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn selected_history_rollback_preserves_damage_and_installs_only_that_version() {
        let root = temp_root("selected-history-recovery");
        let config_path = root.join("config.json");
        let marker_path = root.join("durable-recovery.json");
        let connections_path = root.join("connections.json");
        let playlists_path = root.join("playlists.json");
        let selected = serde_json::to_vec_pretty(&serde_json::json!({
            "continue_playing": "on"
        }))
        .unwrap();
        preserve_valid_history(DurableFile::Settings, &config_path, &selected).unwrap();
        let version = valid_history_at(DurableFile::Settings, &config_path)
            .unwrap()
            .remove(0);
        let damaged = br#"{"continue_playing":"future","secret":"preserve"}"#;
        let connections = br#"{"sources":[]}"#;
        let playlists = br#"{"schemaVersion":1,"playlists":[]}"#;
        fs::write(&config_path, damaged).unwrap();
        fs::write(&connections_path, connections).unwrap();
        fs::write(&playlists_path, playlists).unwrap();
        let expected = InvalidSnapshot::from_bytes(damaged);

        let backup = recover_selected_history_at_with_marker(
            DurableFile::Settings,
            &config_path,
            &expected,
            DurableLayout::PostSplit,
            &marker_path,
            &version,
        )
        .unwrap();

        assert_eq!(fs::read(&config_path).unwrap(), selected);
        assert_eq!(fs::read(root.join(backup)).unwrap(), damaged);
        assert_eq!(fs::read(&connections_path).unwrap(), connections);
        assert_eq!(fs::read(&playlists_path).unwrap(), playlists);
        assert!(!marker_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retry_resumes_the_exact_recorded_history_choice_after_rename() {
        let root = temp_root("history-recovery-resume");
        let config_path = root.join("config.json");
        let marker_path = root.join("durable-recovery.json");
        let selected = serde_json::to_vec_pretty(&serde_json::json!({
            "continue_playing": "only-tv"
        }))
        .unwrap();
        preserve_valid_history(DurableFile::Settings, &config_path, &selected).unwrap();
        let version = valid_history_at(DurableFile::Settings, &config_path)
            .unwrap()
            .remove(0);
        let damaged = br#"{"continue_playing":"future"}"#;
        fs::write(&config_path, damaged).unwrap();
        let expected = InvalidSnapshot::from_bytes(damaged);
        let backup_name =
            "config.invalid-1-00000000-0000-0000-0000-000000000000.json".to_string();
        let marker = RecoveryMarker::new_history(
            DurableFile::Settings,
            DurableLayout::PostSplit,
            backup_name.clone(),
            &expected,
            &version,
        )
        .unwrap();
        install_recovery_marker(&marker_path, &marker).unwrap();
        crate::storage::rename_noreplace(&config_path, &root.join(&backup_name)).unwrap();

        resume_recovery_at(&marker_path, &marker).unwrap();

        assert_eq!(fs::read(&config_path).unwrap(), selected);
        assert_eq!(fs::read(root.join(backup_name)).unwrap(), damaged);
        assert!(!marker_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retry_refuses_a_different_valid_file_for_recorded_history() {
        let root = temp_root("history-recovery-wrong-install");
        let config_path = root.join("config.json");
        let marker_path = root.join("durable-recovery.json");
        let selected = serde_json::to_vec_pretty(&serde_json::json!({
            "continue_playing": "only-tv"
        }))
        .unwrap();
        preserve_valid_history(DurableFile::Settings, &config_path, &selected).unwrap();
        let version = valid_history_at(DurableFile::Settings, &config_path)
            .unwrap()
            .remove(0);
        let damaged = br#"{"continue_playing":"future"}"#;
        let expected = InvalidSnapshot::from_bytes(damaged);
        let backup_name =
            "config.invalid-1-00000000-0000-0000-0000-000000000000.json".to_string();
        let marker = RecoveryMarker::new_history(
            DurableFile::Settings,
            DurableLayout::PostSplit,
            backup_name.clone(),
            &expected,
            &version,
        )
        .unwrap();
        crate::storage::write_private_new(&root.join(&backup_name), damaged).unwrap();
        let different_valid = br#"{"continue_playing":"off"}"#;
        crate::storage::write_private_new(&config_path, different_valid).unwrap();
        install_recovery_marker(&marker_path, &marker).unwrap();

        assert!(resume_recovery_at(&marker_path, &marker).is_err());
        assert!(marker_path.exists());
        assert_eq!(fs::read(&config_path).unwrap(), different_valid);
        assert_eq!(fs::read(root.join(backup_name)).unwrap(), damaged);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_selected_history_is_refused_before_the_damaged_file_moves() {
        let root = temp_root("changed-history-recovery");
        let config_path = root.join("config.json");
        let marker_path = root.join("durable-recovery.json");
        let valid = serde_json::to_vec_pretty(&serde_json::json!({
            "continue_playing": "on"
        }))
        .unwrap();
        preserve_valid_history(DurableFile::Settings, &config_path, &valid).unwrap();
        let version = valid_history_at(DurableFile::Settings, &config_path)
            .unwrap()
            .remove(0);
        let version_path = root.join(&version.file_name);
        let mut changed = fs::read(&version_path).unwrap();
        let value = changed.iter().position(|byte| *byte == b'o').unwrap();
        changed[value] = b'x';
        fs::write(&version_path, &changed).unwrap();
        assert_eq!(changed.len() as u64, version.byte_length);

        let damaged = br#"{"continue_playing":"future"}"#;
        fs::write(&config_path, damaged).unwrap();
        let expected = InvalidSnapshot::from_bytes(damaged);
        assert!(matches!(
            recover_selected_history_at_with_marker(
                DurableFile::Settings,
                &config_path,
                &expected,
                DurableLayout::PostSplit,
                &marker_path,
                &version,
            ),
            Err(RecoveryFileError::Stale)
        ));
        assert_eq!(fs::read(&config_path).unwrap(), damaged);
        assert!(!marker_path.exists());
        assert_eq!(
            fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.invalid-"))
                .count(),
            0
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_refuses_a_non_regular_path() {
        let root = temp_root("recover-directory");
        let config_path = root.join("config.json");
        fs::create_dir(&config_path).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(b"synthetic");

        assert!(matches!(
            recover_selected_at_with(
                DurableFile::Settings,
                &config_path,
                &snapshot,
                "config.invalid-directory.json".to_string(),
                finish_selected_recovery,
            ),
            Err(RecoveryFileError::Stale)
        ));
        assert!(fs::metadata(&config_path).unwrap().is_dir());
        assert!(!root.join("config.invalid-directory.json").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_marker_is_strict_and_cannot_escape_its_directory() {
        let root = temp_root("recovery-marker-invalid");
        let marker_path = root.join("durable-recovery.json");
        let snapshot = InvalidSnapshot::from_bytes(b"damaged");
        let invalid = serde_json::json!({
            "file": "settings",
            "layout": "post_split",
            "backupFileName": "../config.invalid-1-00000000-0000-0000-0000-000000000000.json",
            "byteLength": snapshot.byte_length,
            "sha256": snapshot.sha256,
            "unexpected": true,
        });
        crate::storage::install_json_new(&marker_path, &invalid).unwrap();

        assert!(load_recovery_marker(&marker_path).is_err());
        assert!(!root
            .parent()
            .unwrap()
            .join("config.invalid-1-00000000-0000-0000-0000-000000000000.json")
            .exists());
        crate::storage::remove_private_regular(&marker_path).unwrap();

        let history_sha256 = "a".repeat(64);
        let history = ValidHistorySnapshot {
            public: DurableRollbackVersion {
                id: history_sha256.clone(),
                created_at_unix_ms: 1,
            },
            file_name: format!("config.valid-1-{history_sha256}.json"),
            byte_length: 2,
            sha256: history_sha256,
        };
        let marker = RecoveryMarker::new_history(
            DurableFile::Settings,
            DurableLayout::PostSplit,
            "config.invalid-1-00000000-0000-0000-0000-000000000000.json".to_string(),
            &snapshot,
            &history,
        )
        .unwrap();
        let mut nested_unknown = serde_json::to_value(marker).unwrap();
        nested_unknown["replacement"]["unexpected"] = serde_json::Value::Bool(true);
        crate::storage::install_json_new(&marker_path, &nested_unknown).unwrap();

        assert!(load_recovery_marker(&marker_path).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_marker_blocks_only_its_selected_file() {
        let snapshot = InvalidSnapshot::from_bytes(b"damaged");
        let marker = RecoveryMarker::new(
            DurableFile::Settings,
            DurableLayout::LegacyCombined,
            "config.invalid-1-00000000-0000-0000-0000-000000000000.json".to_string(),
            &snapshot,
        )
        .unwrap();
        let gate = recovery_incomplete_gate(Some(&marker), None);

        assert!(gate.recovery_incomplete());
        assert_eq!(
            gate.status.settings.status,
            DurableStatusKind::MigrationBlocked
        );
        assert_eq!(
            gate.status.settings.layout,
            DurableLayout::LegacyCombined
        );
        assert_eq!(gate.status.connections.status, DurableStatusKind::Ready);
        assert!(!gate.can_recover(DurableFile::Settings));

        let previous = failure_gate(
            DurableFileStatus {
                status: DurableStatusKind::RecoverableInvalid,
                layout: DurableLayout::PostSplit,
                can_recover: true,
                rollback_versions: Vec::new(),
            },
            DurableFileStatus {
                status: DurableStatusKind::RecoverableInvalid,
                layout: DurableLayout::PostSplit,
                can_recover: true,
                rollback_versions: Vec::new(),
            },
            Some(snapshot.clone()),
            Some(snapshot),
        );
        let blocked = recovery_incomplete_gate(Some(&marker), Some(&previous));
        assert!(!blocked.status.settings.can_recover);
        assert!(!blocked.status.connections.can_recover);
        assert!(!blocked.can_recover(DurableFile::Settings));
        assert!(!blocked.can_recover(DurableFile::Connections));
    }

    #[test]
    fn retry_resumes_before_the_recorded_rename() {
        let root = temp_root("recovery-resume-before-rename");
        let config_path = root.join("config.json");
        let marker_path = root.join("durable-recovery.json");
        let original = br#"{"continue_playing":"future"}"#;
        fs::write(&config_path, original).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(original);
        let marker = RecoveryMarker::new(
            DurableFile::Settings,
            DurableLayout::PostSplit,
            "config.invalid-1-00000000-0000-0000-0000-000000000000.json".to_string(),
            &snapshot,
        )
        .unwrap();
        install_recovery_marker(&marker_path, &marker).unwrap();

        resume_recovery_at(&marker_path, &marker).unwrap();

        assert!(!marker_path.exists());
        assert_eq!(
            fs::read(root.join(&marker.backup_file_name)).unwrap(),
            original
        );
        let fresh: serde_json::Value =
            serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
        assert_eq!(fresh, serde_json::to_value(AppConfig::default()).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retry_resumes_after_the_recorded_rename() {
        let root = temp_root("recovery-resume-after-rename");
        let connections_path = root.join("connections.json");
        let marker_path = root.join("durable-recovery.json");
        let original = br#"{"sources":[{"kind":"future"}]}"#;
        let snapshot = InvalidSnapshot::from_bytes(original);
        let marker = RecoveryMarker::new(
            DurableFile::Connections,
            DurableLayout::PostSplit,
            "connections.invalid-1-00000000-0000-0000-0000-000000000000.json".to_string(),
            &snapshot,
        )
        .unwrap();
        crate::storage::write_private_new(&root.join(&marker.backup_file_name), original).unwrap();
        install_recovery_marker(&marker_path, &marker).unwrap();

        resume_recovery_at(&marker_path, &marker).unwrap();

        assert!(!marker_path.exists());
        assert_eq!(
            connections::load_at(&connections_path).unwrap(),
            ConnectionsConfig::default()
        );
        assert_eq!(
            fs::read(root.join(&marker.backup_file_name)).unwrap(),
            original
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retry_finishes_after_fresh_file_installation() {
        let root = temp_root("recovery-resume-after-install");
        let config_path = root.join("config.json");
        let marker_path = root.join("durable-recovery.json");
        let original = br#"{"continue_playing":"future"}"#;
        let valid = br#"{"continue_playing":"off"}"#;
        let snapshot = InvalidSnapshot::from_bytes(original);
        let marker = RecoveryMarker::new(
            DurableFile::Settings,
            DurableLayout::PostSplit,
            "config.invalid-1-00000000-0000-0000-0000-000000000000.json".to_string(),
            &snapshot,
        )
        .unwrap();
        crate::storage::write_private_new(&root.join(&marker.backup_file_name), original).unwrap();
        fs::write(&config_path, valid).unwrap();
        install_recovery_marker(&marker_path, &marker).unwrap();

        resume_recovery_at(&marker_path, &marker).unwrap();

        assert!(!marker_path.exists());
        assert_eq!(fs::read(&config_path).unwrap(), valid);
        assert_eq!(
            fs::read(root.join(&marker.backup_file_name)).unwrap(),
            original
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retry_refuses_an_ambiguous_recorded_state() {
        let root = temp_root("recovery-resume-ambiguous");
        let config_path = root.join("config.json");
        let marker_path = root.join("durable-recovery.json");
        let original = br#"{"continue_playing":"future"}"#;
        let changed = br#"{"continue_playing":"different"}"#;
        let snapshot = InvalidSnapshot::from_bytes(original);
        let marker = RecoveryMarker::new(
            DurableFile::Settings,
            DurableLayout::PostSplit,
            "config.invalid-1-00000000-0000-0000-0000-000000000000.json".to_string(),
            &snapshot,
        )
        .unwrap();
        fs::write(&config_path, changed).unwrap();
        install_recovery_marker(&marker_path, &marker).unwrap();

        assert!(resume_recovery_at(&marker_path, &marker).is_err());
        assert!(marker_path.exists());
        assert_eq!(fs::read(&config_path).unwrap(), changed);
        assert!(!root.join(&marker.backup_file_name).exists());
        fs::remove_dir_all(root).unwrap();
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

    #[test]
    fn settings_recovery_preserves_exact_invalid_bytes_and_every_other_file() {
        let root = temp_root("recover-settings");
        let config_path = root.join("config.json");
        let connections_path = root.join("connections.json");
        let playlist_path = root.join("playlists.json");
        let original =
            br#"{"continue_playing":"future","synthetic_secret":"must-stay-only-in-backup"}"#;
        let connections_bytes = br#"{"sources":[]}"#;
        let playlist_bytes = br#"{"schema_version":1,"playlists":[]}"#;
        fs::write(&config_path, original).unwrap();
        fs::write(&connections_path, connections_bytes).unwrap();
        fs::write(&playlist_path, playlist_bytes).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(original);

        let backup_name = recover_selected_at_with(
            DurableFile::Settings,
            &config_path,
            &snapshot,
            "config.invalid-test.json".to_string(),
            finish_selected_recovery,
        )
        .unwrap();

        assert_eq!(backup_name, "config.invalid-test.json");
        assert_eq!(fs::read(root.join(&backup_name)).unwrap(), original);
        let fresh: serde_json::Value =
            serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
        assert_eq!(fresh, serde_json::to_value(AppConfig::default()).unwrap());
        assert_eq!(fs::read(&connections_path).unwrap(), connections_bytes);
        assert_eq!(fs::read(&playlist_path).unwrap(), playlist_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_combined_recovery_extracts_nothing_from_the_invalid_file() {
        let root = temp_root("recover-combined");
        let config_path = root.join("config.json");
        let connections_path = root.join("connections.json");
        let original = br#"{"sources":[{"id":"jf","kind":"jellyfin","name":"Test","base_url":"http://127.0.0.1:8096","access_token":"synthetic-token","user_id":"u","device_id":"d"}],"future":true}"#;
        fs::write(&config_path, original).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(original);

        recover_selected_at_with(
            DurableFile::Settings,
            &config_path,
            &snapshot,
            "config.invalid-combined.json".to_string(),
            finish_selected_recovery,
        )
        .unwrap();

        assert_eq!(
            fs::read(root.join("config.invalid-combined.json")).unwrap(),
            original
        );
        let fresh: serde_json::Value =
            serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
        assert_eq!(fresh, serde_json::to_value(AppConfig::default()).unwrap());
        assert!(!connections_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn connections_recovery_installs_empty_connections_and_preserves_other_files() {
        let root = temp_root("recover-connections");
        let config_path = root.join("config.json");
        let connections_path = root.join("connections.json");
        let playlist_path = root.join("playlists.json");
        let settings_bytes = br#"{"continue_playing":"on"}"#;
        let playlist_bytes = br#"{"schema_version":1,"playlists":[]}"#;
        let original = br#"{"sources":[{"id":"broken","kind":"future"}]}"#;
        fs::write(&config_path, settings_bytes).unwrap();
        fs::write(&connections_path, original).unwrap();
        fs::write(&playlist_path, playlist_bytes).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(original);

        recover_selected_at_with(
            DurableFile::Connections,
            &connections_path,
            &snapshot,
            "connections.invalid-test.json".to_string(),
            finish_selected_recovery,
        )
        .unwrap();

        assert_eq!(
            fs::read(root.join("connections.invalid-test.json")).unwrap(),
            original
        );
        assert_eq!(
            connections::load_at(&connections_path).unwrap(),
            ConnectionsConfig::default()
        );
        assert_eq!(fs::read(&config_path).unwrap(), settings_bytes);
        assert_eq!(fs::read(&playlist_path).unwrap(), playlist_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_or_now_valid_snapshots_cannot_recover() {
        let root = temp_root("recover-stale");
        let config_path = root.join("config.json");
        let original = br#"{"continue_playing":"future"}"#;
        fs::write(&config_path, original).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(original);
        let changed = br#"{"continue_playing":"future","changed":true}"#;
        fs::write(&config_path, changed).unwrap();

        assert!(matches!(
            recover_selected_at_with(
                DurableFile::Settings,
                &config_path,
                &snapshot,
                "config.invalid-stale.json".to_string(),
                finish_selected_recovery,
            ),
            Err(RecoveryFileError::Stale)
        ));
        assert_eq!(fs::read(&config_path).unwrap(), changed);
        assert!(!root.join("config.invalid-stale.json").exists());

        let valid = br#"{"continue_playing":"on"}"#;
        fs::write(&config_path, valid).unwrap();
        let valid_snapshot = InvalidSnapshot::from_bytes(valid);
        assert!(matches!(
            recover_selected_at_with(
                DurableFile::Settings,
                &config_path,
                &valid_snapshot,
                "config.invalid-valid.json".to_string(),
                finish_selected_recovery,
            ),
            Err(RecoveryFileError::Stale)
        ));
        assert_eq!(fs::read(&config_path).unwrap(), valid);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_collision_leaves_the_canonical_and_backup_byte_identical() {
        let root = temp_root("recover-collision");
        let config_path = root.join("config.json");
        let backup_path = root.join("config.invalid-collision.json");
        let original = br#"{"continue_playing":"future"}"#;
        let existing_backup = b"existing backup";
        fs::write(&config_path, original).unwrap();
        fs::write(&backup_path, existing_backup).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(original);

        assert!(matches!(
            recover_selected_at_with(
                DurableFile::Settings,
                &config_path,
                &snapshot,
                "config.invalid-collision.json".to_string(),
                finish_selected_recovery,
            ),
            Err(RecoveryFileError::BeforeRename)
        ));
        assert_eq!(fs::read(&config_path).unwrap(), original);
        assert_eq!(fs::read(&backup_path).unwrap(), existing_backup);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn post_rename_failure_keeps_the_exact_private_backup_and_no_partial_fresh_file() {
        let root = temp_root("recover-after-rename");
        let config_path = root.join("config.json");
        let original = br#"{"continue_playing":"future"}"#;
        fs::write(&config_path, original).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(original);

        let result = recover_selected_at_with(
            DurableFile::Settings,
            &config_path,
            &snapshot,
            "config.invalid-preserved.json".to_string(),
            |_, _, backup, current, expected| {
                crate::storage::harden_existing_regular(backup)?;
                let preserved = fs::read(backup)?;
                assert!(expected.matches(&preserved));
                assert_eq!(preserved, current);
                Err(io::Error::other("injected fresh-install failure"))
            },
        );

        assert!(matches!(
            result,
            Err(RecoveryFileError::AfterRename { backup_file_name })
                if backup_file_name == "config.invalid-preserved.json"
        ));
        assert!(!config_path.exists());
        let backup_path = root.join("config.invalid-preserved.json");
        assert_eq!(fs::read(&backup_path).unwrap(), original);
        assert!(crate::storage::is_private_regular(&backup_path));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn post_rename_failure_keeps_a_private_restart_marker() {
        let root = temp_root("recover-marker-after-rename");
        let config_path = root.join("config.json");
        let marker_path = root.join("durable-recovery.json");
        let original = br#"{"continue_playing":"future"}"#;
        fs::write(&config_path, original).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(original);

        let result = recover_selected_at_with_marker_and_finish(
            DurableFile::Settings,
            &config_path,
            &snapshot,
            DurableLayout::PostSplit,
            &marker_path,
            |_, _, _, _, _| Err(io::Error::other("injected fresh-install failure")),
        );

        let backup_file_name = match result {
            Err(RecoveryFileError::AfterRename { backup_file_name }) => backup_file_name,
            _ => panic!("the injected post-rename failure must remain incomplete"),
        };
        assert!(!config_path.exists());
        assert!(crate::storage::is_private_regular(&marker_path));
        assert_eq!(
            load_recovery_marker(&marker_path)
                .unwrap()
                .unwrap()
                .backup_file_name,
            backup_file_name
        );
        assert_eq!(fs::read(root.join(backup_file_name)).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn recovery_refuses_a_symlink_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let root = temp_root("recover-symlink");
        let target = root.join("target.json");
        let config_path = root.join("config.json");
        let original = br#"{"continue_playing":"future"}"#;
        fs::write(&target, original).unwrap();
        symlink(&target, &config_path).unwrap();
        let snapshot = InvalidSnapshot::from_bytes(original);

        assert!(matches!(
            recover_selected_at_with(
                DurableFile::Settings,
                &config_path,
                &snapshot,
                "config.invalid-symlink.json".to_string(),
                finish_selected_recovery,
            ),
            Err(RecoveryFileError::Stale)
        ));
        assert!(fs::symlink_metadata(&config_path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&target).unwrap(), original);
        assert!(!root.join("config.invalid-symlink.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
