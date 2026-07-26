use serde::Serialize;
use std::collections::{HashMap, VecDeque};
#[cfg(not(target_os = "windows"))]
use std::path::Path;
#[cfg(not(target_os = "windows"))]
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::State;

use crate::config::{self, SourceConfig};
use crate::connections::{self, ConnectionsConfig};
use crate::display::{self, DisplayOverrides, HdrOverride, ResolutionOverride};
use crate::playback;
use crate::plex_library::{PlexLibrary, PlexServer};
use crate::source::jellyfin::{self, Flavor, JellyfinClient};
use crate::source::{
    plex, BackingRef, DetailDto, EpisodeContext, HubDto, ItemDto, MediaSource, PlaybackVersion,
    SectionDto,
};
use crate::AppState;

const PRODUCT: &str = "Vela";
/// Derived from Cargo.toml's `version` so it can't drift from package metadata.
/// Bumped on EVERY build (see scripts/bump.sh) so each build is uniquely
/// identifiable — in the window footer and in the bundle filename.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// UTC date the build was cut; updated alongside the version by scripts/bump.sh.
const BUILD_DATE: &str = "2026-07-26";

/// Project home, shown (and opened) from the build-info footer.
const REPO_URL: &str = "https://github.com/roethlar/vela";
const MAX_PAGE_SIZE: usize = 100;
const MAX_SEARCH_LEN: usize = 200;
const ALLOWED_SORTS: &[&str] = config::ALLOWED_SECTION_SORTS;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDto {
    authenticated: bool,
    server: Option<String>,
}

/// Version / build identity for the UI footer.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    version: String,
    /// UTC date the build was cut (YYYY-MM-DD), from the BUILD_DATE constant.
    build_date: String,
    repo_url: String,
}

/// A configured source, for the UI's source switcher / per-source filtering.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDto {
    id: String,
    name: String,
    kind: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinDto {
    id: String,
    code: String,
    client_identifier: String,
    /// plex.tv link URL for the 4-character PIN; opening it pre-fills the code.
    auth_url: String,
    /// Inline SVG QR encoding `auth_url`, for scanning from a phone.
    qr_svg: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexServerChoiceDto {
    machine_identifier: String,
    name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum LinkPollDto {
    Pending,
    ChooseServer { servers: Vec<PlexServerChoiceDto> },
    Connected { source: SourceDto },
}

pub(crate) enum PlexLinkSession {
    ChooseServer {
        created_at: Instant,
        client_identifier: String,
        token: String,
        servers: Vec<PlexServer>,
    },
    Connected {
        created_at: Instant,
        client_identifier: String,
        source: SourceDto,
    },
}

impl PlexLinkSession {
    fn created_at(&self) -> Instant {
        match self {
            Self::ChooseServer { created_at, .. } | Self::Connected { created_at, .. } => {
                *created_at
            }
        }
    }

    fn client_identifier(&self) -> &str {
        match self {
            Self::ChooseServer {
                client_identifier, ..
            }
            | Self::Connected {
                client_identifier, ..
            } => client_identifier,
        }
    }

    fn response(&self) -> LinkPollDto {
        match self {
            Self::ChooseServer { servers, .. } => LinkPollDto::ChooseServer {
                servers: server_choices(servers),
            },
            Self::Connected { source, .. } => LinkPollDto::Connected {
                source: source.clone(),
            },
        }
    }
}

pub(crate) type PlexLinkSessions = HashMap<String, PlexLinkSession>;

const PLEX_LINK_SESSION_TTL: Duration = Duration::from_secs(15 * 60);
const MAX_PLEX_LINK_SESSIONS: usize = 8;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MpvInfo {
    available: bool,
    /// The resolved mpv command/path Vela will actually launch (bare `mpv` if
    /// found on PATH, otherwise an absolute path), or null if none was found.
    path: Option<String>,
    /// The user's explicit override from config, if any (echoed back so the
    /// settings UI can show/edit it).
    configured_path: Option<String>,
    /// Whether an automated installer is available to install mpv from inside
    /// the app on this machine.
    can_auto_install: bool,
    /// Detected/manual install command the user can copy (e.g. `brew install mpv`),
    /// when the install method has a useful shell equivalent.
    install_command: Option<String>,
    /// What the automatic installer will do on this machine, or a manual hint if
    /// no supported automatic installer was found.
    install_description: String,
    /// Where to get mpv if the command doesn't apply.
    install_url: String,
}

#[tauri::command]
pub async fn get_durable_state_status(
    state: State<'_, AppState>,
) -> Result<crate::durable::DurableStateStatus, String> {
    Ok(state.durable_gate.lock().await.status.clone())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableRecoveryResult {
    status: crate::durable::DurableStateStatus,
    recovered: bool,
    backup_file_name: Option<String>,
    reconnect_required: bool,
    restored_version: Option<crate::durable::DurableRollbackVersion>,
    error: Option<String>,
}

#[tauri::command]
pub async fn retry_durable_state(
    state: State<'_, AppState>,
) -> Result<crate::durable::DurableStateStatus, String> {
    let incomplete_gate = {
        let gate = state.durable_gate.lock().await;
        if gate.recovery_incomplete() {
            Some(gate.clone())
        } else {
            None
        }
    };
    if let Some(gate) = incomplete_gate {
        let transaction = tauri::async_runtime::spawn_blocking(move || {
            crate::durable::resume_incomplete_recovery(gate)
        })
        .await
        .map_err(|_| "could not retry Vela's recorded recovery".to_string())?;
        if let crate::durable::RecoveryTransaction::Failed { gate, .. } = transaction {
            let status = gate.status.clone();
            *state.registry.lock().await = crate::source::SourceRegistry::default();
            *state.durable_gate.lock().await = gate;
            crate::durable::set_commands_ready(false);
            return Ok(status);
        }
    }
    let loaded = tauri::async_runtime::spawn_blocking(crate::durable::load)
        .await
        .map_err(|_| "could not retry Vela's durable state".to_string())?;
    match loaded {
        Ok(ready) => {
            let gate = crate::durable::DurableGate::ready();
            let status = gate.status.clone();
            *state.registry.lock().await = ready.registry;
            *state.durable_gate.lock().await = gate;
            crate::durable::set_commands_ready(true);
            Ok(status)
        }
        Err(failure) => {
            *state.registry.lock().await = crate::source::SourceRegistry::default();
            let status = failure.gate.status.clone();
            *state.durable_gate.lock().await = failure.gate;
            crate::durable::set_commands_ready(false);
            Ok(status)
        }
    }
}

#[tauri::command]
pub async fn recover_invalid_file(
    file: crate::durable::DurableFile,
    state: State<'_, AppState>,
) -> Result<DurableRecoveryResult, String> {
    run_file_recovery(file, None, state).await
}

#[tauri::command]
pub async fn rollback_invalid_file(
    file: crate::durable::DurableFile,
    version_id: String,
    state: State<'_, AppState>,
) -> Result<DurableRecoveryResult, String> {
    run_file_recovery(file, Some(version_id), state).await
}

async fn run_file_recovery(
    file: crate::durable::DurableFile,
    version_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<DurableRecoveryResult, String> {
    let expected_gate = state.durable_gate.lock().await.clone();
    let eligible = match version_id.as_deref() {
        Some(version_id) => expected_gate.can_rollback(file, version_id),
        None => expected_gate.can_recover(file),
    };
    if !eligible {
        return Ok(DurableRecoveryResult {
            status: expected_gate.status.clone(),
            recovered: false,
            backup_file_name: None,
            reconnect_required: false,
            restored_version: None,
            error: Some("Recovery is not available for the file's current status.".to_string()),
        });
    }
    let transaction = tauri::async_runtime::spawn_blocking(move || match version_id {
        Some(version_id) => {
            crate::durable::rollback_invalid_file(file, &version_id, expected_gate)
        }
        None => crate::durable::recover_invalid_file(file, expected_gate),
    })
    .await
    .map_err(|_| "Vela could not safely run file recovery.".to_string())?;

    match transaction {
        crate::durable::RecoveryTransaction::Changed {
            backup_file_name,
            reconnect_required,
            restored_version,
        } => {
            let loaded = tauri::async_runtime::spawn_blocking(crate::durable::load)
                .await
                .map_err(|_| "Vela could not safely verify file recovery.".to_string())?;
            match loaded {
                Ok(ready) => {
                    let gate = crate::durable::DurableGate::ready();
                    let status = gate.status.clone();
                    *state.registry.lock().await = ready.registry;
                    *state.durable_gate.lock().await = gate;
                    crate::durable::set_commands_ready(true);
                    Ok(DurableRecoveryResult {
                        status,
                        recovered: true,
                        backup_file_name: Some(backup_file_name),
                        reconnect_required,
                        restored_version,
                        error: None,
                    })
                }
                Err(failure) => {
                    let status = failure.gate.status.clone();
                    *state.registry.lock().await = crate::source::SourceRegistry::default();
                    *state.durable_gate.lock().await = failure.gate;
                    crate::durable::set_commands_ready(false);
                    Ok(DurableRecoveryResult {
                        status,
                        recovered: true,
                        backup_file_name: Some(backup_file_name),
                        reconnect_required,
                        restored_version,
                        error: Some(
                            "The damaged file was preserved, but another file still requires attention."
                                .to_string(),
                        ),
                    })
                }
            }
        }
        crate::durable::RecoveryTransaction::Stale => {
            let loaded = tauri::async_runtime::spawn_blocking(crate::durable::load)
                .await
                .map_err(|_| "Vela could not safely recheck the changed file.".to_string())?;
            let status = match loaded {
                Ok(ready) => {
                    let gate = crate::durable::DurableGate::ready();
                    let status = gate.status.clone();
                    *state.registry.lock().await = ready.registry;
                    *state.durable_gate.lock().await = gate;
                    crate::durable::set_commands_ready(true);
                    status
                }
                Err(failure) => {
                    let status = failure.gate.status.clone();
                    *state.registry.lock().await = crate::source::SourceRegistry::default();
                    *state.durable_gate.lock().await = failure.gate;
                    crate::durable::set_commands_ready(false);
                    status
                }
            };
            Ok(DurableRecoveryResult {
                status,
                recovered: false,
                backup_file_name: None,
                reconnect_required: false,
                restored_version: None,
                error: Some(
                    "The file changed after Vela detected the problem. Review the new status and try again."
                        .to_string(),
                ),
            })
        }
        crate::durable::RecoveryTransaction::Failed {
            gate,
            backup_file_name,
            message,
        } => {
            let status = gate.status.clone();
            *state.registry.lock().await = crate::source::SourceRegistry::default();
            *state.durable_gate.lock().await = gate;
            crate::durable::set_commands_ready(false);
            Ok(DurableRecoveryResult {
                status,
                recovered: false,
                backup_file_name,
                reconnect_required: false,
                restored_version: None,
                error: Some(message.to_string()),
            })
        }
    }
}

#[tauri::command]
pub fn exit_vela(app: tauri::AppHandle) {
    app.exit(0);
}

// ---- status & auth -------------------------------------------------------

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<StatusDto, String> {
    crate::durable::ensure_commands_ready()?;
    let reg = state.registry.lock().await;
    let first = reg.all().first();
    Ok(StatusDto {
        authenticated: !reg.is_empty(),
        server: first.map(|s| s.name()),
    })
}

/// Version / build identity for the UI footer (synchronous — all constants).
#[tauri::command]
pub fn get_app_info() -> AppInfoDto {
    AppInfoDto {
        version: VERSION.to_string(),
        build_date: BUILD_DATE.to_string(),
        repo_url: REPO_URL.to_string(),
    }
}

/// List configured sources, for the UI's switcher and per-source filtering.
#[tauri::command]
pub async fn get_sources(state: State<'_, AppState>) -> Result<Vec<SourceDto>, String> {
    crate::durable::ensure_commands_ready()?;
    let reg = state.registry.lock().await;
    Ok(reg
        .all()
        .iter()
        .map(|s| SourceDto {
            id: s.id(),
            name: s.name(),
            kind: s.kind().to_string(),
        })
        .collect())
}

// ---- adding sources (Jellyfin/Emby) -------------------------------------
//
// These connect to the user's *server* (to obtain an access token) — they do
// not gate Vela itself. Username/password is the normal path; a pre-issued API
// key is the headless fallback.

/// Connect a Jellyfin/Emby server with username + password (password may be
/// empty for open accounts). Persists the issued token and registers the source.
#[tauri::command]
pub async fn connect_jellyfin(
    kind: String,
    base_url: String,
    username: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<SourceDto, String> {
    crate::durable::ensure_commands_ready()?;
    let flavor = Flavor::from_kind(&kind).ok_or("unknown server kind")?;
    let base = normalize_base_url(&base_url)?;
    let authed = JellyfinClient::authenticate(flavor, &base, &username, &password).await?;
    let cfg = SourceConfig {
        id: format!("{}-{}", kind, uuid::Uuid::new_v4()),
        kind,
        name: authed.server_name,
        base_url: base,
        access_token: Some(authed.token),
        api_key: None,
        user_id: Some(authed.user_id),
        device_id: Some(authed.device_id),
        machine_identifier: None,
    };
    register_source(&state, cfg).await
}

/// Connect with a pre-issued API key / access token (headless/preconfigured
/// servers). `user_id` is optional — the server's first user is used if omitted.
#[tauri::command]
pub async fn connect_jellyfin_token(
    kind: String,
    base_url: String,
    api_key: String,
    user_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<SourceDto, String> {
    crate::durable::ensure_commands_ready()?;
    let flavor = Flavor::from_kind(&kind).ok_or("unknown server kind")?;
    let base = normalize_base_url(&base_url)?;
    let authed = JellyfinClient::from_api_key(flavor, &base, &api_key, user_id.as_deref()).await?;
    let cfg = SourceConfig {
        id: format!("{}-{}", kind, uuid::Uuid::new_v4()),
        kind,
        name: authed.server_name,
        base_url: base,
        access_token: None,
        api_key: Some(api_key),
        user_id: Some(authed.user_id),
        device_id: Some(authed.device_id),
        machine_identifier: None,
    };
    register_source(&state, cfg).await
}

/// Remove one configured source by id, regardless of provider. Credentials are
/// stored per row, so removing one Plex server cannot affect another server or
/// account.
#[tauri::command]
pub async fn remove_source(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let _source_guard = state.source_lock.lock().await;
    let id2 = id.clone();
    config_store(move || connections::update(move |cfg| remove_source_config(cfg, &id2))).await?;
    state.registry.lock().await.remove(&id);
    Ok(())
}

fn remove_source_config(cfg: &mut ConnectionsConfig, source_id: &str) -> Result<(), String> {
    if !cfg.sources.iter().any(|source| source.id == source_id) {
        return Err("no such source".to_string());
    }
    cfg.sources.retain(|source| source.id != source_id);
    Ok(())
}

/// Persist a source config and add it to the live registry.
async fn register_source(
    state: &State<'_, AppState>,
    cfg: SourceConfig,
) -> Result<SourceDto, String> {
    let _source_guard = state.source_lock.lock().await;
    let source = jellyfin::build_source(&cfg)?;
    let dto = SourceDto {
        id: source.id(),
        name: source.name(),
        kind: source.kind().to_string(),
    };
    config_store(move || {
        connections::update(move |stored| stored.upsert(cfg))
    })
    .await
    .map_err(|e| format!("connected but failed to save the connection: {e}"))?;
    state.registry.lock().await.upsert(source);
    Ok(dto)
}

async fn config_store<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| format!("config task failed: {error}"))?
}

/// Normalize a user-entered server URL: default to http:// if no scheme, trim `/`.
fn normalize_base_url(input: &str) -> Result<String, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("server URL is required".into());
    }
    let s = if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else {
        format!("http://{s}")
    };
    Ok(s.trim_end_matches('/').to_string())
}

/// Request a plex.tv device PIN; the user enters the code at plex.tv/link.
#[tauri::command]
pub async fn link_begin() -> Result<PinDto, String> {
    crate::durable::ensure_commands_ready()?;
    let client_identifier = uuid::Uuid::new_v4().to_string();
    let client = plextv_client()?;
    let resp = client
        .post("https://plex.tv/api/v2/pins")
        .query(&[("strong", "false")])
        .header("X-Plex-Product", PRODUCT)
        .header("X-Plex-Version", VERSION)
        .header("X-Plex-Client-Identifier", &client_identifier)
        .header("Accept", "application/xml")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("plex.tv error while starting link: {}", status));
    }
    let id = attr(&body, "id").ok_or("no pin id in response")?;
    let code = attr(&body, "code").ok_or("no pin code in response")?;

    // `strong=false` yields the short code intended for the Plex link page.
    // The hosted Auth App URL is a different flow and rejects these weak PINs.
    let auth_url = plex_link_url(&code);
    // Render the QR as an <img>-able data URI rather than raw SVG injected with
    // {@html}, so the frontend never inline-injects backend markup.
    let qr_svg = qrcode::QrCode::new(auth_url.as_bytes())
        .ok()
        .map(|c| {
            use qrcode::render::svg;
            let svg = c
                .render()
                .min_dimensions(220, 220)
                .quiet_zone(true)
                .dark_color(svg::Color("#101014"))
                .light_color(svg::Color("#ffffff"))
                .build();
            use base64::Engine;
            format!(
                "data:image/svg+xml;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(svg)
            )
        })
        .unwrap_or_default();

    Ok(PinDto {
        id,
        code,
        client_identifier,
        auth_url,
        qr_svg,
    })
}

const MPV_INSTALL_URL: &str = "https://mpv.io/installation/";

#[derive(Clone)]
struct MpvInstallInfo {
    can_auto_install: bool,
    install_command: Option<String>,
    install_description: String,
    install_url: String,
}

#[cfg(not(target_os = "windows"))]
#[derive(Clone)]
struct CommandInstaller {
    program: String,
    args: Vec<String>,
    display_command: String,
    description: String,
}

#[cfg(not(target_os = "windows"))]
impl CommandInstaller {
    fn info(self) -> MpvInstallInfo {
        MpvInstallInfo {
            can_auto_install: true,
            install_command: Some(self.display_command),
            install_description: self.description,
            install_url: MPV_INSTALL_URL.to_string(),
        }
    }
}

/// Whether mpv is available, where it resolved, the user's override (if any),
/// plus the install method/hint for this machine.
#[tauri::command]
pub fn check_mpv() -> Result<MpvInfo, String> {
    let install = mpv_install_info();
    let resolved = playback::resolve_mpv()?;
    let configured_path = config::load_config()
        .map_err(|_| "could not read Vela settings".to_string())?
        .mpv_path
        .filter(|s| !s.trim().is_empty());
    Ok(MpvInfo {
        available: resolved.is_some(),
        path: resolved,
        configured_path,
        can_auto_install: install.can_auto_install,
        install_command: install.install_command,
        install_description: install.install_description,
        install_url: install.install_url,
    })
}

#[cfg(target_os = "windows")]
fn mpv_install_info() -> MpvInstallInfo {
    MpvInstallInfo {
        can_auto_install: true,
        install_command: None,
        install_description: "Downloads a CPU-compatible mpv build into your user profile"
            .to_string(),
        install_url: MPV_INSTALL_URL.to_string(),
    }
}

#[cfg(target_os = "macos")]
fn mpv_install_info() -> MpvInstallInfo {
    if let Some(installer) = mpv_command_installer() {
        return installer.info();
    }

    MpvInstallInfo {
        can_auto_install: false,
        install_command: Some(macos_manual_install_command()),
        install_description: "Install mpv with Homebrew or MacPorts, then restart Vela".to_string(),
        install_url: MPV_INSTALL_URL.to_string(),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn mpv_install_info() -> MpvInstallInfo {
    if let Some(installer) = mpv_command_installer() {
        return installer.info();
    }

    MpvInstallInfo {
        can_auto_install: false,
        install_command: Some(linux_manual_install_command()),
        install_description: "Install mpv with your distro's package manager, then restart Vela"
            .to_string(),
        install_url: MPV_INSTALL_URL.to_string(),
    }
}

#[cfg(target_os = "macos")]
fn mpv_command_installer() -> Option<CommandInstaller> {
    let brew = find_executable(&["/opt/homebrew/bin/brew", "/usr/local/bin/brew", "brew"])?;
    Some(CommandInstaller {
        program: brew,
        args: strings(&["install", "mpv"]),
        display_command: "brew install mpv".to_string(),
        description: "Uses Homebrew to install mpv".to_string(),
    })
}

#[cfg(target_os = "macos")]
fn macos_manual_install_command() -> String {
    if find_executable(&["/opt/local/bin/port", "port"]).is_some() {
        "sudo port install mpv".to_string()
    } else {
        "brew install mpv".to_string()
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[derive(Clone, Copy)]
struct LinuxPackageManager {
    candidates: &'static [&'static str],
    args: &'static [&'static str],
    display_command: &'static str,
    description: &'static str,
}

#[cfg(all(unix, not(target_os = "macos")))]
const LINUX_PACKAGE_MANAGERS: &[LinuxPackageManager] = &[
    LinuxPackageManager {
        candidates: &["/usr/bin/apt-get", "apt-get"],
        args: &["install", "-y", "mpv"],
        display_command: "pkexec apt-get install -y mpv",
        description: "Uses apt through PolicyKit to install mpv",
    },
    LinuxPackageManager {
        candidates: &["/usr/bin/dnf", "dnf"],
        args: &["install", "-y", "mpv"],
        display_command: "pkexec dnf install -y mpv",
        description: "Uses dnf through PolicyKit to install mpv",
    },
    LinuxPackageManager {
        candidates: &["/usr/bin/yum", "yum"],
        args: &["install", "-y", "mpv"],
        display_command: "pkexec yum install -y mpv",
        description: "Uses yum through PolicyKit to install mpv",
    },
    LinuxPackageManager {
        candidates: &["/usr/bin/pacman", "pacman"],
        args: &["-S", "--noconfirm", "mpv"],
        display_command: "pkexec pacman -S --noconfirm mpv",
        description: "Uses pacman through PolicyKit to install mpv",
    },
    LinuxPackageManager {
        candidates: &["/usr/bin/zypper", "zypper"],
        args: &["--non-interactive", "install", "mpv"],
        display_command: "pkexec zypper --non-interactive install mpv",
        description: "Uses zypper through PolicyKit to install mpv",
    },
    LinuxPackageManager {
        candidates: &["/sbin/apk", "/usr/sbin/apk", "apk"],
        args: &["add", "mpv"],
        display_command: "pkexec apk add mpv",
        description: "Uses apk through PolicyKit to install mpv",
    },
    LinuxPackageManager {
        candidates: &["/usr/bin/xbps-install", "xbps-install"],
        args: &["-Sy", "mpv"],
        display_command: "pkexec xbps-install -Sy mpv",
        description: "Uses xbps through PolicyKit to install mpv",
    },
    LinuxPackageManager {
        candidates: &["/usr/bin/eopkg", "eopkg"],
        args: &["install", "-y", "mpv"],
        display_command: "pkexec eopkg install -y mpv",
        description: "Uses eopkg through PolicyKit to install mpv",
    },
    LinuxPackageManager {
        candidates: &["/usr/bin/snap", "snap"],
        args: &["install", "mpv"],
        display_command: "pkexec snap install mpv",
        description: "Uses snap through PolicyKit to install mpv",
    },
];

#[cfg(all(unix, not(target_os = "macos")))]
fn mpv_command_installer() -> Option<CommandInstaller> {
    if let Some(brew) = find_executable(&["/home/linuxbrew/.linuxbrew/bin/brew", "brew"]) {
        return Some(CommandInstaller {
            program: brew,
            args: strings(&["install", "mpv"]),
            display_command: "brew install mpv".to_string(),
            description: "Uses Homebrew on Linux to install mpv".to_string(),
        });
    }

    if let Some(nix) = find_executable(&["/usr/bin/nix", "nix"]) {
        return Some(CommandInstaller {
            program: nix,
            args: strings(&["profile", "install", "nixpkgs#mpv"]),
            display_command: "nix profile install nixpkgs#mpv".to_string(),
            description: "Uses a per-user Nix profile to install mpv".to_string(),
        });
    }

    let pkexec = find_executable(&["/usr/bin/pkexec", "pkexec"])?;
    for manager in LINUX_PACKAGE_MANAGERS {
        if let Some(program) = find_executable(manager.candidates) {
            let mut args = Vec::with_capacity(manager.args.len() + 1);
            args.push(program);
            args.extend(strings(manager.args));
            return Some(CommandInstaller {
                program: pkexec,
                args,
                display_command: manager.display_command.to_string(),
                description: manager.description.to_string(),
            });
        }
    }

    None
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_manual_install_command() -> String {
    if find_executable(&["/home/linuxbrew/.linuxbrew/bin/brew", "brew"]).is_some() {
        return "brew install mpv".to_string();
    }
    if find_executable(&["/usr/bin/nix", "nix"]).is_some() {
        return "nix profile install nixpkgs#mpv".to_string();
    }

    for manager in LINUX_PACKAGE_MANAGERS {
        if find_executable(manager.candidates).is_some() {
            return manager.display_command.replacen("pkexec", "sudo", 1);
        }
    }

    "Install mpv with your distro's package manager".to_string()
}

#[cfg(not(target_os = "windows"))]
fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[cfg(not(target_os = "windows"))]
fn find_executable(candidates: &[&str]) -> Option<String> {
    for candidate in candidates {
        if candidate.contains(std::path::MAIN_SEPARATOR) {
            if Path::new(candidate).is_file() {
                return Some((*candidate).to_string());
            }
        } else if Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
        {
            return Some((*candidate).to_string());
        }
    }
    None
}

/// Set (or clear) the explicit mpv path override. Passing an empty/None value
/// clears it and falls back to auto-discovery. A non-empty path is validated —
/// it must point at a real file that runs as mpv — so a typo can't silently
/// disable playback. Returns the refreshed mpv status.
#[tauri::command]
pub fn set_mpv_path(path: Option<String>) -> Result<MpvInfo, String> {
    let trimmed = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    if let Some(p) = &trimmed {
        if !std::path::Path::new(p).is_file() {
            return Err(format!(
                "No file exists at that path: {p}\nPick the mpv executable (mpv.exe on Windows)."
            ));
        }
        if !playback::mpv_usable(p) {
            // The Windows community builds ship as CPU-specific variants, so a
            // crash-on-launch there is usually a too-new build; elsewhere it's a
            // wrong file or a non-executable bit. Keep the hint platform-apt.
            let hint = if cfg!(target_os = "windows") {
                "\n\nThis often means it's a build for a newer CPU (an AVX2 / \"v3\" build) than this machine supports. Use Install mpv to fetch a matching build, or get the plain \"x86_64\" (not \"v3\") build from mpv.io."
            } else {
                "\n\nMake sure it's the mpv binary and that it's marked executable."
            };
            return Err(format!("That mpv exists but won't run: {p}{hint}"));
        }
    }
    let to_store = trimmed.clone();
    config::update(move |cfg| {
        cfg.mpv_path = to_store;
        Ok(())
    })?;
    check_mpv()
}

/// Advanced mpv configuration the user controls from Settings: free-form extra
/// options and whether to load their own `mpv.conf`. Echoed back so the UI can
/// populate the fields.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MpvAdvanced {
    pub extra_args: String,
    pub use_own_config: bool,
    /// Black-bar cropping mode: `"off" | "manual" | "auto"` (see config
    /// `mpv_autocrop`). Always one of the three for the UI to bind to.
    pub autocrop: String,
    /// Marker skip policy per kind, already resolved through the documented
    /// missing-field default, so the UI always has a concrete value to bind.
    /// No Settings control reads these yet — the controls land in the same
    /// commit that makes them affect playback.
    pub skip_intros: config::SkipPolicy,
    pub skip_credits: config::SkipPolicy,
    pub skip_commercials: config::SkipPolicy,
    /// Current playback quality, already resolved through its default so the UI
    /// always has a concrete value to bind: `"original"`, `"automatic"`, or a
    /// tier id.
    pub playback_quality: String,
    /// The full ladder, so Settings can render labels and bitrates without
    /// duplicating the table in TypeScript. Per-file filtering happens at play
    /// time, not here — this is the global ceiling the user is choosing.
    pub quality_tiers: Vec<crate::source::QualityTier>,
}

#[tauri::command]
pub fn get_mpv_advanced() -> Result<MpvAdvanced, String> {
    let cfg =
        config::load_config().map_err(|_| "could not read Vela settings".to_string())?;
    Ok(MpvAdvanced {
        extra_args: cfg.mpv_extra_args.unwrap_or_default(),
        use_own_config: cfg.mpv_use_own_config.unwrap_or(false),
        autocrop: autocrop_from_config(cfg.mpv_autocrop.as_deref())?,
        skip_intros: config::SkipPolicy::resolve(cfg.skip_intros),
        skip_credits: config::SkipPolicy::resolve(cfg.skip_credits),
        skip_commercials: config::SkipPolicy::resolve(cfg.skip_commercials),
        playback_quality: config::playback_quality(cfg.playback_quality.as_deref()),
        quality_tiers: crate::source::QUALITY_TIERS.to_vec(),
    })
}

/// What a title's quality submenu may offer for one exact copy.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityOptionsDto {
    /// Whether an "Original" entry belongs in the menu at all.
    pub can_direct_play: bool,
    pub source_bitrate_kbps: u32,
    pub source_height: u32,
    /// Only the tiers this server will actually deliver for this copy. Empty
    /// when it will not convert, which the menu renders as no submenu.
    pub tiers: Vec<crate::source::QualityTier>,
}

/// Options for one copy, resolved on demand.
///
/// Deliberately NOT called when the context menu opens: for Plex this costs a
/// decision round trip per version, and paying that on every right-click would
/// make the menu feel slow for the many users who never transcode. The frontend
/// calls this when the quality submenu is opened.
#[tauri::command]
pub async fn quality_options(
    state: tauri::State<'_, AppState>,
    item_key: String,
    version_id: Option<String>,
) -> Result<QualityOptionsDto, String> {
    let (source, raw) = {
        let registry = state.registry.lock().await;
        registry.route(&item_key)?
    };
    let options = source
        .playback_options(&raw, version_id.as_deref())
        .await?;
    Ok(QualityOptionsDto {
        can_direct_play: options.can_direct_play,
        source_bitrate_kbps: options.source_bitrate_kbps,
        source_height: options.source_height,
        tiers: options.tiers,
    })
}

/// The three resolved marker policies for one play. Resolved once, before
/// stream resolution, so `playback::play` never reads config to decide what
/// skipping should do.
#[derive(Debug, Clone, Copy)]
pub struct SkipPolicies {
    pub intro: config::SkipPolicy,
    pub credits: config::SkipPolicy,
    pub commercial: config::SkipPolicy,
}

impl SkipPolicies {
    fn load() -> Result<Self, PlayFailure> {
        let cfg = config::load_config()
            .map_err(|_| PlayFailure::unavailable("could not read Vela settings".to_string()))?;
        Ok(Self {
            intro: config::SkipPolicy::resolve(cfg.skip_intros),
            credits: config::SkipPolicy::resolve(cfg.skip_credits),
            commercial: config::SkipPolicy::resolve(cfg.skip_commercials),
        })
    }

    fn any_enabled(&self) -> bool {
        !(self.intro.is_off() && self.credits.is_off() && self.commercial.is_off())
    }
}

fn skip_policy_for(
    kind: crate::source::MarkerKind,
    policies: &SkipPolicies,
) -> config::SkipPolicy {
    match kind {
        crate::source::MarkerKind::Intro => policies.intro,
        crate::source::MarkerKind::Credits => policies.credits,
        crate::source::MarkerKind::Commercial => policies.commercial,
    }
}

/// Decode the persisted autocrop value. A missing value means `"off"`;
/// an unrecognised stored value is invalid instead of being normalised.
fn autocrop_from_config(value: Option<&str>) -> Result<String, String> {
    match value {
        None | Some("off") => Ok("off".to_string()),
        Some("manual") => Ok("manual".to_string()),
        Some("auto") => Ok("auto".to_string()),
        Some(_) => Err("invalid saved autocrop mode".to_string()),
    }
}

/// Persist the advanced mpv settings. No validation of `extra_args` — these are the
/// user's own machine and their own call; a bad option just makes mpv refuse to
/// launch, which surfaces as a normal playback error. An empty `extra_args` clears
/// the override. `autocrop` is optional so older frontends that don't send it leave
/// the mode unchanged; when present it must be one of the known three states.
///
/// The three marker policies are optional for the same reason. Each is stored
/// explicitly rather than collapsed to `None` for the default value: an
/// explicit choice stays explicit, so changing the product default later cannot
/// silently move a user who deliberately picked today's default.
#[tauri::command]
pub fn set_mpv_advanced(
    extra_args: String,
    use_own_config: bool,
    autocrop: Option<String>,
    skip_intros: Option<config::SkipPolicy>,
    skip_credits: Option<config::SkipPolicy>,
    skip_commercials: Option<config::SkipPolicy>,
    playback_quality: Option<String>,
) -> Result<(), String> {
    let trimmed = extra_args.trim().to_string();
    config::update(move |cfg| {
        cfg.mpv_extra_args = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        cfg.mpv_use_own_config = Some(use_own_config);
        if let Some(mode) = autocrop.as_deref() {
            // Store `None` for "off" so the config stays sparse; missing = off.
            cfg.mpv_autocrop = match mode {
                "off" => None,
                "manual" | "auto" => Some(mode.to_string()),
                _ => return Err("unknown autocrop mode".to_string()),
            };
        }
        if let Some(policy) = skip_intros {
            cfg.skip_intros = Some(policy);
        }
        if let Some(policy) = skip_credits {
            cfg.skip_credits = Some(policy);
        }
        if let Some(policy) = skip_commercials {
            cfg.skip_commercials = Some(policy);
        }
        if let Some(quality) = &playback_quality {
            // Reject here rather than writing a value the loader would later
            // refuse: an invalid setting must never reach the file.
            let known = quality == config::PLAYBACK_QUALITY_ORIGINAL
                || quality == config::PLAYBACK_QUALITY_AUTOMATIC
                || crate::source::QUALITY_TIERS
                    .iter()
                    .any(|tier| tier.id == quality);
            if !known {
                return Err("unknown playback quality".to_string());
            }
            cfg.playback_quality = Some(quality.clone());
        }
        Ok(())
    })
}

/// Decode the persisted Continue Playing mode. A missing value keeps the
/// documented `"only-tv"` default; any other unknown stored value is invalid.
fn continue_playing_from_config(value: Option<&str>) -> Result<String, String> {
    match value {
        Some("off") => Ok("off".to_string()),
        Some("on") => Ok("on".to_string()),
        None | Some("only-tv") => Ok("only-tv".to_string()),
        Some(_) => Err("invalid saved Continue Playing mode".to_string()),
    }
}

#[tauri::command]
pub fn get_continue_playing() -> Result<String, String> {
    let cfg =
        config::load_config().map_err(|_| "could not read Vela settings".to_string())?;
    continue_playing_from_config(cfg.continue_playing.as_deref())
}

#[tauri::command]
pub fn set_continue_playing(mode: String) -> Result<String, String> {
    if !matches!(mode.as_str(), "off" | "on" | "only-tv") {
        return Err("unknown Continue Playing mode".to_string());
    }
    let stored = (mode != "only-tv").then(|| mode.clone());
    config::update(move |cfg| {
        cfg.continue_playing = stored;
        Ok(())
    })?;
    Ok(mode)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackPreferencesDto {
    policy: crate::selection::PlaybackSourcePolicy,
    resolution_override: Option<String>,
    hdr_override: Option<String>,
    detected_display: display::DisplayProfile,
    effective_display: display::DisplayProfile,
}

fn playback_display_overrides(cfg: &config::AppConfig) -> Result<DisplayOverrides, String> {
    Ok(DisplayOverrides {
        resolution: ResolutionOverride::from_config(
            cfg.playback_display_resolution.as_deref(),
        )?,
        hdr: HdrOverride::from_config(cfg.playback_display_hdr.as_deref())?,
    })
}

#[tauri::command]
pub async fn get_playback_preferences(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<PlaybackPreferencesDto, String> {
    let cfg =
        config::load_config().map_err(|_| "could not read Vela settings".to_string())?;
    let observed = state
        .playback_window_session
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .map(|session| session.observation.display_snapshot());
    let detected_display = display::detect_profile(&app, observed).await;
    let overrides = playback_display_overrides(&cfg)?;
    Ok(PlaybackPreferencesDto {
        policy: crate::selection::PlaybackSourcePolicy::from_config(
            cfg.playback_source_policy.as_deref(),
        )?,
        resolution_override: overrides
            .resolution
            .map(|value| value.as_str().to_string()),
        hdr_override: overrides.hdr.map(|value| value.as_str().to_string()),
        effective_display: display::apply_overrides(&detected_display, overrides),
        detected_display,
    })
}

#[tauri::command]
pub fn set_playback_preferences(
    policy: String,
    resolution_override: Option<String>,
    hdr_override: Option<String>,
) -> Result<(), String> {
    let policy = crate::selection::PlaybackSourcePolicy::parse(&policy)?;
    let resolution = resolution_override
        .as_deref()
        .map(ResolutionOverride::parse)
        .transpose()?;
    let hdr = hdr_override.as_deref().map(HdrOverride::parse).transpose()?;
    config::update(move |cfg| {
        cfg.playback_source_policy =
            (policy != crate::selection::PlaybackSourcePolicy::Best)
                .then(|| policy.as_str().to_string());
        cfg.playback_display_resolution =
            resolution.map(|value| value.as_str().to_string());
        cfg.playback_display_hdr = hdr.map(|value| value.as_str().to_string());
        Ok(())
    })
}

/// Install mpv from inside the app. On Windows we assess the CPU and download a
/// matching prebuilt mpv. On macOS/Linux we run the concrete package-manager
/// method detected for this machine. Returns refreshed mpv status so the UI can
/// clear the prompt.
#[tauri::command]
pub async fn install_mpv() -> Result<MpvInfo, String> {
    #[cfg(target_os = "windows")]
    install_mpv_windows().await?;
    #[cfg(not(target_os = "windows"))]
    tauri::async_runtime::spawn_blocking(run_mpv_installer)
        .await
        .map_err(|e| format!("installer task failed: {e}"))??;
    check_mpv()
}

/// Community Windows mpv builds, in two microarchitecture levels:
/// <https://github.com/zhongfly/mpv-winbuild> (a standard source linked from the
/// mpv wiki). `mpv-x86_64-v3-*` targets x86-64-v3 (AVX2/FMA/BMI2 — Haswell and
/// newer); `mpv-x86_64-*` is the baseline build that runs on any 64-bit CPU.
#[cfg(target_os = "windows")]
const MPV_RELEASE_API: &str = "https://api.github.com/repos/zhongfly/mpv-winbuild/releases/latest";

/// True if this CPU implements the x86-64-v3 feature level the "v3" mpv build
/// needs. AVX2/FMA/BMI2 all arrived together with Haswell, so requiring them
/// keeps us from installing a v3 build that would crash (illegal instruction) on
/// an older CPU — the exact failure that makes a v3 build play nothing.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn cpu_supports_v3() -> bool {
    std::arch::is_x86_feature_detected!("avx2")
        && std::arch::is_x86_feature_detected!("fma")
        && std::arch::is_x86_feature_detected!("bmi2")
}
/// Non-x86_64 Windows (e.g. ARM): no x86 v3 build applies — use the baseline.
#[cfg(all(target_os = "windows", not(target_arch = "x86_64")))]
fn cpu_supports_v3() -> bool {
    false
}

#[cfg(target_os = "windows")]
async fn install_mpv_windows() -> Result<(), String> {
    let want_v3 = cpu_supports_v3();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("couldn't create an HTTP client: {e}"))?;

    // 1. Find the asset matching this CPU in the latest release.
    let release: serde_json::Value = client
        .get(MPV_RELEASE_API)
        .header("User-Agent", "vela")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("couldn't reach GitHub to find an mpv build: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub returned an error finding an mpv build: {e}"))?
        .json()
        .await
        .map_err(|e| format!("couldn't read the mpv release info: {e}"))?;

    let (asset_name, asset_url) = release
        .get("assets")
        .and_then(|a| a.as_array())
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let name = a.get("name")?.as_str()?.to_string();
            let url = a.get("browser_download_url")?.as_str()?.to_string();
            Some((name, url))
        })
        .find(|(name, _)| {
            name.ends_with(".7z")
                && name.starts_with("mpv-x86_64-")
                && name.contains("-v3-") == want_v3
        })
        .ok_or_else(|| {
            format!(
                "couldn't find a {} mpv build in the latest release",
                if want_v3 { "v3 (AVX2)" } else { "baseline" }
            )
        })?;

    // 2. Download the archive.
    let bytes = client
        .get(&asset_url)
        .header("User-Agent", "vela")
        .send()
        .await
        .map_err(|e| format!("couldn't download mpv: {e}"))?
        .error_for_status()
        .map_err(|e| format!("mpv download failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("couldn't read the mpv download: {e}"))?;

    // 3. Extract to %LOCALAPPDATA%\Programs\mpv (a per-user location our discovery
    //    already probes), and record the resulting path. Blocking fs/extraction
    //    work runs off the async runtime.
    let local = std::env::var("LOCALAPPDATA")
        .map_err(|_| "couldn't locate %LOCALAPPDATA% to install mpv".to_string())?;
    let dest = std::path::PathBuf::from(local).join(r"Programs\mpv");
    let archive = std::env::temp_dir().join(&asset_name);

    let dest2 = dest.clone();
    let exe = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        std::fs::write(&archive, &bytes)
            .map_err(|e| format!("couldn't save the mpv download: {e}"))?;
        let res = extract_7z(&archive, &dest2);
        let _ = std::fs::remove_file(&archive); // best-effort temp cleanup
        res?;
        let exe = dest2.join("mpv.exe");
        if !playback::mpv_usable(&exe.to_string_lossy()) {
            return Err("mpv was installed but doesn't run on this PC.".to_string());
        }
        Ok(exe.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| format!("install task failed: {e}"))??;

    // 4. Point the config at the freshly installed mpv so it's used immediately
    //    and shown in Settings.
    config::update(move |cfg| {
        cfg.mpv_path = Some(exe);
        Ok(())
    })?;
    Ok(())
}

/// Extract a `.7z` archive into `dest` using the OS `tar` (libarchive, bundled
/// with Windows 10 1803+), which reads 7-Zip archives. The community mpv archives
/// lay `mpv.exe` out at the archive root, so this drops it straight into `dest`.
#[cfg(target_os = "windows")]
fn extract_7z(archive: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dest)
        .map_err(|e| format!("couldn't create {}: {e}", dest.display()))?;
    let status = std::process::Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .map_err(|e| format!("couldn't run tar to extract mpv: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("couldn't extract the mpv archive.".to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn run_mpv_installer() -> Result<(), String> {
    let installer = mpv_command_installer().ok_or_else(|| {
        "No supported mpv installer was found on this system. Install mpv manually, or point Vela at an existing mpv in Settings -> Player.".to_string()
    })?;
    run_command_installer(installer)
}

#[cfg(not(target_os = "windows"))]
fn run_command_installer(installer: CommandInstaller) -> Result<(), String> {
    let out = Command::new(&installer.program)
        .args(&installer.args)
        .output()
        .map_err(|e| format!("couldn't launch {}: {e}", installer.display_command))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} failed:\n{}",
            installer.display_command,
            command_output(&out)
        ))
    }
}

#[cfg(not(target_os = "windows"))]
fn command_output(out: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    match (stderr.is_empty(), stdout.is_empty()) {
        (false, false) => format!("{stderr}\n{stdout}"),
        (false, true) => stderr,
        (true, false) => stdout,
        (true, true) => format!("process exited with status {}", out.status),
    }
}

/// Open a URL in the user's default browser. Restricted to http(s) so a stray
/// invoke can't launch arbitrary local handlers.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("refusing to open a non-http(s) URL".into());
    }
    open_external(&url)
}

#[cfg(target_os = "macos")]
fn open_external(url: &str) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
#[cfg(target_os = "windows")]
fn open_external(url: &str) -> Result<(), String> {
    // rundll32 takes the URL as a single argument, avoiding cmd's `&`/`#` parsing.
    std::process::Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
#[cfg(all(unix, not(target_os = "macos")))]
fn open_external(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Percent-encode a value for use in the plex.tv auth URL.
fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

fn plex_link_url(code: &str) -> String {
    format!("https://plex.tv/link/?pin={}", enc(code))
}

fn server_choices(servers: &[PlexServer]) -> Vec<PlexServerChoiceDto> {
    servers
        .iter()
        .map(|server| PlexServerChoiceDto {
            machine_identifier: server.machine_identifier.clone(),
            name: server.name.clone(),
        })
        .collect()
}

fn server_for_machine<'a>(
    servers: &'a [PlexServer],
    machine_identifier: &str,
) -> Result<&'a PlexServer, String> {
    servers
        .iter()
        .find(|server| server.machine_identifier == machine_identifier)
        .ok_or_else(|| "that Plex server is not part of this link session".to_string())
}

enum ReachableServerDecision {
    Connect(PlexServer),
    Choose(Vec<PlexServer>),
}

fn decide_reachable_servers(mut servers: Vec<PlexServer>) -> Result<ReachableServerDecision, String> {
    match servers.len() {
        0 => Err("No reachable direct HTTPS Plex server was found. Check Plex Remote Access or connect to the server's network; Plex Relay is not used for HDR playback.".to_string()),
        1 => Ok(ReachableServerDecision::Connect(servers.remove(0))),
        _ => Ok(ReachableServerDecision::Choose(servers)),
    }
}

fn prune_link_sessions(sessions: &mut PlexLinkSessions, now: Instant) {
    sessions.retain(|_, session| {
        now.saturating_duration_since(session.created_at()) <= PLEX_LINK_SESSION_TTL
    });
}

fn insert_link_session(sessions: &mut PlexLinkSessions, pin_id: String, session: PlexLinkSession) {
    prune_link_sessions(sessions, Instant::now());
    if !sessions.contains_key(&pin_id) && sessions.len() >= MAX_PLEX_LINK_SESSIONS {
        if let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.created_at())
            .map(|(id, _)| id.clone())
        {
            sessions.remove(&oldest);
        }
    }
    sessions.insert(pin_id, session);
}

fn plex_source_config(
    token: String,
    client_identifier: String,
    server: &PlexServer,
) -> Result<SourceConfig, String> {
    if server.machine_identifier.trim().is_empty() {
        return Err("Plex server did not report a stable machine identifier".to_string());
    }
    if server.scheme != "https" || server.relay {
        return Err("Plex linking requires a direct HTTPS server connection".to_string());
    }
    let mut library = PlexLibrary::new(token.clone(), client_identifier.clone());
    library.set_server(server.clone());
    let base_url = library
        .server_base()
        .ok_or_else(|| "Plex server did not report a usable endpoint".to_string())?;
    Ok(SourceConfig {
        id: format!("plex-{}", uuid::Uuid::new_v4()),
        kind: "plex".to_string(),
        name: if server.name.trim().is_empty() {
            "Plex".to_string()
        } else {
            server.name.clone()
        },
        base_url,
        access_token: Some(token),
        api_key: None,
        user_id: None,
        device_id: Some(client_identifier),
        machine_identifier: Some(server.machine_identifier.clone()),
    })
}

async fn connect_plex_source(
    state: &AppState,
    token: String,
    client_identifier: String,
    server: &PlexServer,
) -> Result<SourceDto, String> {
    let cfg = plex_source_config(token, client_identifier, server)?;
    let source = plex::build_source(&cfg)?;
    let dto = SourceDto {
        id: source.id(),
        name: source.name(),
        kind: source.kind().to_string(),
    };

    let _source_guard = state.source_lock.lock().await;
    config_store(move || {
        connections::update(move |stored| stored.upsert(cfg))
    })
    .await
    .map_err(|error| format!("authenticated but failed to save config: {error}"))?;
    state.registry.lock().await.upsert(source);
    Ok(dto)
}

/// Poll a pending PIN. Authorization remains backend-only: once plex.tv issues
/// a token, this command either connects the sole reachable physical server or
/// keeps the credentials in an expiring in-memory session while the UI chooses
/// among several server names.
#[tauri::command]
pub async fn link_poll(
    pin_id: String,
    client_identifier: String,
    state: State<'_, AppState>,
) -> Result<LinkPollDto, String> {
    crate::durable::ensure_commands_ready()?;
    {
        let mut sessions = state.plex_link_sessions.lock().await;
        prune_link_sessions(&mut sessions, Instant::now());
        if let Some(session) = sessions.get(&pin_id) {
            if session.client_identifier() != client_identifier {
                return Err("link session does not match this device".to_string());
            }
            return Ok(session.response());
        }
    }

    let client = plextv_client()?;
    let resp = client
        .get(format!("https://plex.tv/api/v2/pins/{}", pin_id))
        .header("X-Plex-Product", PRODUCT)
        .header("X-Plex-Version", VERSION)
        .header("X-Plex-Client-Identifier", &client_identifier)
        .header("Accept", "application/xml")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    // Distinguish a real failure from "still pending" so the UI doesn't poll
    // forever on an expired/rate-limited pin.
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("Link code expired — please restart linking.".into());
    }
    if resp.status().is_server_error() || resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(format!("plex.tv error while linking: {}", resp.status()));
    }
    if !resp.status().is_success() {
        return Err(format!("plex.tv error while linking: {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let token = match attr(&body, "authToken") {
        Some(t) if !t.is_empty() => t,
        _ => return Ok(LinkPollDto::Pending), // 200 with no token yet
    };

    let library = PlexLibrary::new(token.clone(), client_identifier.clone());
    let discovered = library
        .discover_servers()
        .await
        .map_err(|error| format!("could not discover Plex servers: {error}"))?;
    let reachable = library
        .reachable_servers_by_machine(&discovered, false)
        .await;
    let reachable = match decide_reachable_servers(reachable)? {
        ReachableServerDecision::Connect(server) => {
            let source = connect_plex_source(
                &state,
                token,
                client_identifier.clone(),
                &server,
            )
            .await?;
            let response = LinkPollDto::Connected {
                source: source.clone(),
            };
            {
                let mut sessions = state.plex_link_sessions.lock().await;
                insert_link_session(
                    &mut sessions,
                    pin_id,
                    PlexLinkSession::Connected {
                        created_at: Instant::now(),
                        client_identifier,
                        source,
                    },
                );
            }
            return Ok(response);
        }
        ReachableServerDecision::Choose(servers) => servers,
    };

    let response = LinkPollDto::ChooseServer {
        servers: server_choices(&reachable),
    };
    {
        let mut sessions = state.plex_link_sessions.lock().await;
        insert_link_session(
            &mut sessions,
            pin_id,
            PlexLinkSession::ChooseServer {
                created_at: Instant::now(),
                client_identifier,
                token,
                servers: reachable,
            },
        );
    }
    Ok(response)
}

/// Complete a multi-server Plex link without ever sending its auth token to the
/// frontend. Removing the pending session before persistence makes a double
/// click one-shot; a persistence failure restores the same choice for retry.
#[tauri::command]
pub async fn link_select_server(
    pin_id: String,
    client_identifier: String,
    machine_identifier: String,
    state: State<'_, AppState>,
) -> Result<SourceDto, String> {
    crate::durable::ensure_commands_ready()?;
    let pending = {
        let mut sessions = state.plex_link_sessions.lock().await;
        prune_link_sessions(&mut sessions, Instant::now());
        let session = sessions
            .get(&pin_id)
            .ok_or_else(|| "Plex link session expired — please restart linking.".to_string())?;
        if session.client_identifier() != client_identifier {
            return Err("link session does not match this device".to_string());
        }
        if let PlexLinkSession::Connected { source, .. } = session {
            return Ok(source.clone());
        }
        let PlexLinkSession::ChooseServer { servers, .. } = session else {
            unreachable!();
        };
        server_for_machine(servers, &machine_identifier)?;
        sessions
            .remove(&pin_id)
            .expect("link session disappeared while locked")
    };

    let PlexLinkSession::ChooseServer {
        created_at,
        client_identifier,
        token,
        servers,
    } = pending
    else {
        unreachable!();
    };
    let server = server_for_machine(&servers, &machine_identifier)
        .cloned()
        .expect("selected server was validated while locked");

    match connect_plex_source(&state, token.clone(), client_identifier.clone(), &server).await {
        Ok(source) => {
            {
                let mut sessions = state.plex_link_sessions.lock().await;
                insert_link_session(
                    &mut sessions,
                    pin_id,
                    PlexLinkSession::Connected {
                        created_at: Instant::now(),
                        client_identifier,
                        source: source.clone(),
                    },
                );
            }
            Ok(source)
        }
        Err(error) => {
            {
                let mut sessions = state.plex_link_sessions.lock().await;
                insert_link_session(
                    &mut sessions,
                    pin_id,
                    PlexLinkSession::ChooseServer {
                        created_at,
                        client_identifier,
                        token,
                        servers,
                    },
                );
            }
            Err(error)
        }
    }
}

// ---- library browsing ----------------------------------------------------

/// Aggregate hubs across the selected source(s). If a source errors (e.g. a
/// stale server it can't recover), it's skipped rather than failing the whole
/// home screen; an error only surfaces if every selected source fails.
#[tauri::command]
pub async fn get_hubs(
    source_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<HubDto>, String> {
    let sources = state
        .registry
        .lock()
        .await
        .selected(source_id.as_deref())?;
    aggregate(sources, true, |s| async move { s.hubs().await }).await
}

#[tauri::command]
pub async fn get_sections(
    source_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<SectionDto>, String> {
    let sources = state
        .registry
        .lock()
        .await
        .selected(source_id.as_deref())?;
    let mut sections = aggregate(sources, true, |s| async move { s.sections().await }).await?;
    // Stamp each library's persisted sort preference (sources construct
    // `sort: None`). Fail-closed on the value: a stale or hand-edited entry
    // that isn't in the whitelist is ignored, not surfaced.
    let cfg =
        config::load_config().map_err(|_| "could not read Vela settings".to_string())?;
    for s in &mut sections {
        s.sort = cfg.section_sorts.get(&s.key).cloned();
    }
    Ok(sections)
}

/// Persist a library's sort preference (`section_sorts`); the next
/// `get_sections` hands it back on the SectionDto and the frontend applies
/// it when the library is opened.
#[tauri::command]
pub async fn set_section_sort(section_key: String, sort: String) -> Result<(), String> {
    if !ALLOWED_SORTS.contains(&sort.as_str()) {
        return Err("unknown sort".into());
    }
    if section_key.is_empty() || section_key.len() > 512 {
        return Err("bad section key".into());
    }
    config::update(move |cfg| {
        cfg.section_sorts.insert(section_key, sort);
        Ok(())
    })
}

/// Ask a section's backend server to rescan that library for new files.
/// Routed by the namespaced section key; the registry lock is released
/// before the (network) call. Local-family sources reject with a friendly
/// error (their listings re-index on ordinary refresh).
#[tauri::command]
pub async fn scan_section(
    section_key: String,
    provenance: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // `provenance` is the section's own, handed straight back from the list the
    // caller is holding — the source decides what it proves (see
    // `SectionDto::provenance`). Not a trust boundary: the user may scan any
    // library they can already see. It exists so a source whose keys are
    // server-local can refuse to act on a key it no longer issues.
    let (src, raw) = state.registry.lock().await.route(&section_key)?;
    src.scan_library(&raw, provenance.as_deref()).await
}

#[tauri::command]
pub async fn get_items(
    section_key: String,
    section_type: String,
    sort: Option<String>,
    start: usize,
    size: usize,
    state: State<'_, AppState>,
) -> Result<Vec<ItemDto>, String> {
    validate_section_type(&section_type)?;
    let sort = validate_sort(sort)?;
    let size = clamp_page_size(size);
    // A section key is source-namespaced, so route by its prefix.
    let (src, raw) = state.registry.lock().await.route(&section_key)?;
    src.items(&raw, &section_type, sort.as_deref(), start, size)
        .await
}

/// Merged listing for the All view's consolidated Library: every section of
/// `section_type` across every source contributes, sorted and windowed here.
/// Paging is stateless-but-exact: each section supplies its first
/// `start+size` items (already source-sorted), so the merged window is
/// correct; cost grows with scroll depth, acceptable at library scale.
/// No dedup yet — that's the rework's Phase C.
/// The materialized merged listing pages are served from. Pagination over a
/// dynamically re-fetched, deduped, re-sorted union is unstable by
/// construction (review rounds 1–2: titles can be skipped forever or
/// duplicated across pages whenever a deeper fetch re-orders the union), so
/// the listing is built once, in full, and continuation pages window this
/// immutable snapshot.
pub struct MergedSnapshot {
    pub section_type: String,
    pub sort: String,
    pub items: Vec<ItemDto>,
}

/// Immutable page source for one merged show/season drill. The fingerprint
/// includes the parent identity and every source-specific parent key, so a
/// continuation page cannot accidentally window a different hierarchy after
/// navigation or source availability changes.
pub struct MergedChildrenSnapshot {
    pub fingerprint: String,
    pub items: Vec<ItemDto>,
}

#[tauri::command]
pub async fn get_type_listing(
    section_type: String,
    sort: Option<String>,
    start: usize,
    size: usize,
    state: State<'_, AppState>,
) -> Result<Vec<ItemDto>, String> {
    validate_section_type(&section_type)?;
    // Merged ordering can only honor fields items carry on the DTO across
    // sources: title, year (= release date at year granularity), added-at, and
    // last-played. `rating` has no DTO field, so it stays per-source (server-side)
    // only. A source that doesn't populate a field sorts last for that key.
    let sort = match validate_sort(sort)?.as_deref() {
        None => "titleSort:asc".to_string(),
        Some(
            s @ ("titleSort:asc"
            | "year:desc"
            | "originallyAvailableAt:desc"
            | "addedAt:desc"
            | "lastViewedAt:desc"),
        ) => s.to_string(),
        Some(_) => return Err("that sort isn't available in the combined view".into()),
    };
    let size = clamp_page_size(size);

    // Continuation pages (start > 0) must window the same immutable snapshot
    // the listing started with; entering a listing (start == 0) rebuilds.
    if start > 0 {
        let snap = state.merged_snapshot.lock().await;
        if let Some(s) = snap
            .as_ref()
            .filter(|s| s.section_type == section_type && s.sort == sort)
        {
            return Ok(s.items.iter().skip(start).take(size).cloned().collect());
        }
        // No matching snapshot (e.g. app restarted mid-scroll): fall through
        // and rebuild — the windowed result is still correct, merely fresher.
    }

    let sources = state.registry.lock().await.selected(None)?;
    // source id → kind, for the default playback ranking of merged backings.
    let kinds: std::collections::HashMap<String, &'static str> =
        sources.iter().map(|s| (s.id(), s.kind())).collect();
    // Owner's per-title source choices (set via the card's context menu).
    let overrides = config::load_config()
        .map_err(|_| "could not read Vela settings".to_string())?
        .merged_overrides;
    // Collect the contributing sections once; a failing source drops out
    // rather than failing the whole view, matching aggregate()'s stance —
    // but TOTAL failure surfaces as an error, not an empty library (rev-4).
    let mut section_refs: Vec<(std::sync::Arc<dyn crate::source::MediaSource>, String)> =
        Vec::new();
    let mut sections_err: Option<String> = None;
    for src in &sources {
        let sections = match src.sections().await {
            Ok(s) => s,
            Err(e) => {
                sections_err = Some(e);
                continue;
            }
        };
        for sec in sections
            .into_iter()
            .filter(|s| s.section_type == section_type)
        {
            let raw = sec
                .key
                .split_once(':')
                .map(|(_, r)| r.to_string())
                .unwrap_or(sec.key);
            section_refs.push((src.clone(), raw));
        }
    }
    if section_refs.is_empty() {
        if let Some(e) = sections_err {
            // Nothing contributed AND something failed: report it rather
            // than rendering a blank library.
            return Err(e);
        }
        // No sections of this type anywhere: legitimately empty.
    }
    let deduped = fetch_all_merged(&section_refs, &section_type, &sort).await?;
    let ranked = rank_backings(deduped, &kinds, &overrides);
    let items = merge_sort_page(ranked, &sort, 0, usize::MAX);
    let page = items.iter().skip(start).take(size).cloned().collect();
    *state.merged_snapshot.lock().await = Some(MergedSnapshot {
        section_type,
        sort,
        items,
    });
    Ok(page)
}

/// Fetch EVERY item of the type across the given sections: per-section depth
/// doubles until no section returns a full window (`!any_full` — nothing
/// more exists anywhere), then the union is deduped. There is deliberately
/// no early stop and no depth cap: any count-based stop leaves the window's
/// contents unstable across pages (review rounds 1–2), and a cap recreates
/// the paging cliff at its own depth. The full-library fetch cost is paid
/// once per listing entry and amortized by the snapshot above.
async fn fetch_all_merged(
    section_refs: &[(std::sync::Arc<dyn crate::source::MediaSource>, String)],
    section_type: &str,
    sort: &str,
) -> Result<Vec<ItemDto>, String> {
    // Start deep enough that typical libraries resolve in one round trip.
    let mut depth: usize = 512;
    loop {
        let mut merged: Vec<ItemDto> = Vec::new();
        // Whether any section returned a full window — i.e. deepening could
        // still surface more items somewhere.
        let mut any_full = false;
        // A partially failing view stays useful, but when EVERY section
        // failed the caller gets the error, not an empty library (rev-4).
        let mut any_ok = section_refs.is_empty();
        let mut last_err: Option<String> = None;
        for (src, raw) in section_refs {
            match src.items(raw, section_type, Some(sort), 0, depth).await {
                Ok(items) => {
                    any_ok = true;
                    any_full |= items.len() >= depth;
                    merged.extend(items);
                }
                Err(e) => last_err = Some(e),
            }
        }
        if !any_full {
            return if any_ok {
                Ok(dedup_across_sources(merged))
            } else {
                Err(last_err.unwrap_or_else(|| "no sources available".into()))
            };
        }
        depth = depth.saturating_mul(2);
    }
}

/// Stable display-face preference for merged titles. Actual playback source
/// and version selection happens at `play_by_key` under the persisted policy;
/// this legacy kind order only chooses an immediately routable card identity.
fn kind_rank(kind: &str) -> u8 {
    match kind {
        "plex" => 0,
        "jellyfin" | "emby" => 1,
        _ => 2,
    }
}

/// Detail-surface preference for merged titles: the metadata-richest backing
/// first. Also a policy constant; independent of the per-title play override
/// (an override moves playback only — detail routing must not follow it).
fn detail_rank(kind: &str) -> u8 {
    match kind {
        "plex" => 0,
        "jellyfin" | "emby" => 1,
        _ => 2,
    }
}

/// Stable identity a per-title override persists under: the first provider
/// id (sorted, so the same set always yields the same key), else the
/// normalized title + year.
fn canonical_id_of(item: &ItemDto) -> String {
    if let Some(id) = item.provider_ids.iter().min() {
        return id.clone();
    }
    let norm: String = item
        .title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    match item.year {
        Some(y) => format!("title:{norm}|{y}"),
        None => format!("title:{norm}|"),
    }
}

/// Order each merged entry's backing list — owner override first, then the
/// stable display-face ranking — and keep its namespaced face identity valid.
/// The play boundary probes every backing and independently applies Best,
/// Compatible, Fastest, or Ask.
fn rank_backings(
    mut groups: Vec<ItemDto>,
    kinds: &std::collections::HashMap<String, &'static str>,
    overrides: &std::collections::HashMap<String, String>,
) -> Vec<ItemDto> {
    for group in &mut groups {
        let canonical = canonical_id_of(group);
        let Some(backing) = group.backing.as_mut() else {
            group.canonical_id = Some(canonical);
            continue;
        };
        let override_sid = overrides.get(&canonical);
        backing.sort_by_key(|b| {
            let overridden = Some(&b.source_id) == override_sid;
            let rank = kinds.get(&b.source_id).map(|k| kind_rank(k)).unwrap_or(4);
            (!overridden, rank)
        });
        if let Some(face) = backing.first() {
            group.rating_key = face.rating_key.clone();
            group.source_id = face.source_id.clone();
        }
        // Watched-state actions can't route to a local-family face (those
        // sources have no watch state); point them at the first server
        // backing instead (rev-3). Absent when the face itself can take them.
        group.watch_key = backing
            .iter()
            .find(|b| {
                matches!(
                    kinds.get(&b.source_id).copied(),
                    Some("plex" | "jellyfin" | "emby")
                )
            })
            .map(|b| b.rating_key.clone())
            .filter(|k| *k != group.rating_key);
        // The detail surface (and a merged show's children drill) routes to
        // the metadata-richest backing (idv-2/idv-6). Computed by its own
        // rank — the post-sort backing order is play order, which does NOT
        // put the richest first. Absent when it equals the play identity.
        group.detail_key = backing
            .iter()
            .min_by_key(|b| {
                kinds
                    .get(&b.source_id)
                    .map(|k| detail_rank(k))
                    .unwrap_or(u8::MAX)
            })
            .map(|b| b.rating_key.clone())
            .filter(|k| *k != group.rating_key);
        group.canonical_id = Some(canonical);
    }
    groups
}

/// Persist (or clear, with `source_id: None`) the owner's preferred playback
/// source for a merged title.
#[tauri::command]
pub async fn set_merged_override(
    canonical_id: String,
    source_id: Option<String>,
) -> Result<(), String> {
    config::update(move |cfg| {
        match source_id {
            Some(sid) => {
                cfg.merged_overrides.insert(canonical_id.clone(), sid);
            }
            None => {
                cfg.merged_overrides.remove(&canonical_id);
            }
        }
        Ok(())
    })
}

/// Rank a watch state for merged adoption: finished > in-progress (deeper
/// offset wins the tie) > known-unwatched > unknown. Local-family items are
/// unknown (`played: None`), so any server-reported state outranks them.
/// First-Some-wins was order-dependent and hid real progress (rev-5).
fn watch_rank(played: Option<bool>, offset: Option<u64>) -> (u8, u64) {
    match (played, offset) {
        (Some(true), _) => (3, 0),
        (_, Some(o)) if o > 0 => (2, o),
        (Some(false), _) => (1, 0),
        _ => (0, 0),
    }
}

/// Collapse the same title carried by several sources into one entry backed
/// by all of them (rework Phase C). Identity: any shared provider id
/// ("imdb:tt…"), else normalized title + exact year (a missing year only
/// matches another missing year, so remakes and unparsed files don't
/// false-merge). Display fields come from the richest backing — an entry
/// with server metadata (summary + artwork) replaces a bare filename parse —
/// and watch state comes from the first backing that reports any.
fn dedup_across_sources(items: Vec<ItemDto>) -> Vec<ItemDto> {
    use std::collections::HashMap;
    let mut groups: Vec<ItemDto> = Vec::new();
    let mut by_provider: HashMap<String, usize> = HashMap::new();
    let mut by_title: HashMap<String, usize> = HashMap::new();

    fn title_key(item: &ItemDto) -> String {
        let norm: String = item
            .title
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();
        format!("{norm}|{:?}", item.year)
    }
    fn richer(candidate: &ItemDto, current: &ItemDto) -> bool {
        let score = |i: &ItemDto| {
            i.summary.is_some() as u8 + i.poster.is_some() as u8 + i.year.is_some() as u8
        };
        score(candidate) > score(current)
    }

    for item in items {
        let tkey = title_key(&item);
        let hit = item
            .provider_ids
            .iter()
            .find_map(|p| by_provider.get(p).copied())
            .or_else(|| by_title.get(&tkey).copied())
            // Dedup is a cross-source merge only: a colliding item from a
            // source already backing the group stays its own card (rev-2 —
            // same-source versions must remain individually reachable, and
            // duplicate source_ids would make the backing list ambiguous).
            .filter(|gi| {
                groups[*gi]
                    .backing
                    .as_ref()
                    .is_none_or(|b| b.iter().all(|r| r.source_id != item.source_id))
            });
        match hit {
            Some(gi) => {
                for p in &item.provider_ids {
                    by_provider.entry(p.clone()).or_insert(gi);
                }
                by_title.entry(tkey).or_insert(gi);
                let group = &mut groups[gi];
                let backing = group.backing.get_or_insert_with(Vec::new);
                let item_ref = crate::source::backing_ref_of(&item);
                if !backing.contains(&item_ref) {
                    backing.push(item_ref.clone());
                }
                // Adopt the most-progressed watch state across backings
                // before a possible display swap (rev-5).
                if watch_rank(item.played, item.view_offset_ms)
                    > watch_rank(group.played, group.view_offset_ms)
                {
                    group.played = item.played;
                    group.view_offset_ms = item.view_offset_ms;
                }
                // Adopt the most-recent added/last-played across backings so the
                // merged card sorts correctly even when the face backing (e.g. a
                // Jellyfin/local item that doesn't populate these) isn't the one
                // carrying the timestamp. `Option::max` prefers Some over None and
                // the larger ms (sorting slice 3).
                group.added_at_ms = group.added_at_ms.max(item.added_at_ms);
                group.last_watched_at_ms = group.last_watched_at_ms.max(item.last_watched_at_ms);
                if richer(&item, group) {
                    // The richer entry becomes the face (and default play
                    // target): move it to the front of the backing list and
                    // take its display fields, keeping the accumulated
                    // backing/provider state and any adopted watch state.
                    let keep_backing = group.backing.take();
                    let keep_played = group.played.take();
                    let keep_offset = group.view_offset_ms.take();
                    // The accumulated max already folds in the new face's own
                    // timestamps (adopted just above), so it must survive the swap.
                    let keep_added = group.added_at_ms.take();
                    let keep_last_watched = group.last_watched_at_ms.take();
                    let mut ids = std::mem::take(&mut group.provider_ids);
                    for p in &item.provider_ids {
                        if !ids.contains(p) {
                            ids.push(p.clone());
                        }
                    }
                    *group = item;
                    group.provider_ids = ids;
                    group.backing = keep_backing.map(|mut b| {
                        b.retain(|r| *r != item_ref);
                        b.insert(0, item_ref.clone());
                        b
                    });
                    // `keep_*` already holds the most-progressed state seen
                    // (the adopt above ran first); restore it if it outranks
                    // the new face's own state (rev-5).
                    if watch_rank(keep_played, keep_offset)
                        > watch_rank(group.played, group.view_offset_ms)
                    {
                        group.played = keep_played;
                        group.view_offset_ms = keep_offset;
                    }
                    // Restore the accumulated timestamps (they already include the
                    // new face's own values, so this never loses data).
                    group.added_at_ms = keep_added;
                    group.last_watched_at_ms = keep_last_watched;
                } else {
                    for p in &item.provider_ids {
                        if !group.provider_ids.contains(p) {
                            group.provider_ids.push(p.clone());
                        }
                    }
                }
            }
            None => {
                let gi = groups.len();
                for p in &item.provider_ids {
                    by_provider.insert(p.clone(), gi);
                }
                by_title.insert(tkey, gi);
                let mut group = item;
                group.backing = Some(vec![crate::source::backing_ref_of(&group)]);
                groups.push(group);
            }
        }
    }
    groups
}

/// Order the merged union and cut the requested window. Title comparisons
/// fold case; year sorting is newest-first with title tiebreak. (Plex's
/// per-source titleSort strips leading articles; the merged re-sort uses the
/// display title, a small known divergence.)
fn merge_sort_page(mut items: Vec<ItemDto>, sort: &str, start: usize, size: usize) -> Vec<ItemDto> {
    let title = |i: &ItemDto| i.title.to_lowercase();
    match sort {
        // Release date == year granularity, same as year:desc.
        "year:desc" | "originallyAvailableAt:desc" => {
            items.sort_by(|a, b| b.year.cmp(&a.year).then_with(|| title(a).cmp(&title(b))));
        }
        "addedAt:desc" => {
            // None (source didn't populate it) sorts last in a desc order.
            items.sort_by(|a, b| {
                b.added_at_ms
                    .cmp(&a.added_at_ms)
                    .then_with(|| title(a).cmp(&title(b)))
            });
        }
        "lastViewedAt:desc" => {
            items.sort_by(|a, b| {
                b.last_watched_at_ms
                    .cmp(&a.last_watched_at_ms)
                    .then_with(|| title(a).cmp(&title(b)))
            });
        }
        _ => items.sort_by_key(|i| i.title.to_lowercase()),
    }
    items.into_iter().skip(start).take(size).collect()
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    fn item(title: &str, year: Option<u32>, source: &str) -> ItemDto {
        ItemDto {
            rating_key: format!("{source}:{title}"),
            title: title.into(),
            year,
            summary: None,
            duration_ms: None,
            media_type: Some("movie".into()),
            poster: None,
            series_poster: None,
            backdrop: None,
            view_offset_ms: None,
            played: None,
            last_watched_at_ms: None,
            added_at_ms: None,
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            parent_rating_key: None,
            grandparent_rating_key: None,
            provider_ids: vec![],
            backing: None,
            canonical_id: None,
            watch_key: None,
            detail_key: None,
            source_id: source.into(),
        }
    }

    #[test]
    fn merged_page_interleaves_sources_by_title_case_folded() {
        let merged = vec![
            item("delta", Some(2020), "plex"),
            item("Alpha", Some(2021), "plex"),
            item("charlie", Some(2019), "jf-1"),
            item("Bravo", Some(2022), "jf-1"),
        ];
        let page = merge_sort_page(merged, "titleSort:asc", 0, 10);
        let titles: Vec<_> = page.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "Bravo", "charlie", "delta"]);
    }

    #[test]
    fn merged_page_windows_after_sorting() {
        let merged = vec![
            item("c", None, "a"),
            item("a", None, "a"),
            item("d", None, "b"),
            item("b", None, "b"),
        ];
        let page = merge_sort_page(merged, "titleSort:asc", 1, 2);
        let titles: Vec<_> = page.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["b", "c"]);
    }

    #[test]
    fn merged_page_year_desc_with_title_tiebreak() {
        let merged = vec![
            item("older", Some(1999), "a"),
            item("z-new", Some(2024), "b"),
            item("a-new", Some(2024), "a"),
        ];
        let page = merge_sort_page(merged, "year:desc", 0, 10);
        let titles: Vec<_> = page.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["a-new", "z-new", "older"]);
    }

    #[test]
    fn merged_release_date_matches_year_desc() {
        let mk = || {
            vec![
                item("old", Some(1999), "a"),
                item("new", Some(2024), "b"),
                item("undated", None, "a"),
            ]
        };
        // originallyAvailableAt:desc is year:desc granularity; undated sorts last.
        let by_rel = merge_sort_page(mk(), "originallyAvailableAt:desc", 0, 10);
        let by_year = merge_sort_page(mk(), "year:desc", 0, 10);
        let rel: Vec<_> = by_rel.iter().map(|i| i.title.as_str()).collect();
        let yr: Vec<_> = by_year.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(rel, vec!["new", "old", "undated"]);
        assert_eq!(rel, yr);
    }

    #[test]
    fn merged_added_at_desc_missing_sorts_last() {
        let mut a = item("mid", None, "plex");
        a.added_at_ms = Some(200);
        let mut b = item("newest", None, "jf-1");
        b.added_at_ms = Some(500);
        let c = item("unknown", None, "plex"); // added_at_ms None
        let page = merge_sort_page(vec![a, b, c], "addedAt:desc", 0, 10);
        let titles: Vec<_> = page.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["newest", "mid", "unknown"]);
    }

    #[test]
    fn merged_last_played_desc_missing_sorts_last() {
        let mut a = item("watched-old", None, "plex");
        a.last_watched_at_ms = Some(100);
        let mut b = item("watched-recent", None, "plex");
        b.last_watched_at_ms = Some(900);
        let c = item("never", None, "jf-1"); // last_watched_at_ms None
        let page = merge_sort_page(vec![a, b, c], "lastViewedAt:desc", 0, 10);
        let titles: Vec<_> = page.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["watched-recent", "watched-old", "never"]);
    }

    fn with_ids(mut i: ItemDto, ids: &[&str]) -> ItemDto {
        i.provider_ids = ids.iter().map(|s| s.to_string()).collect();
        i
    }

    struct FakeItems {
        items: Vec<ItemDto>,
    }

    #[async_trait::async_trait]
    impl crate::source::MediaSource for FakeItems {
        fn id(&self) -> String {
            "fake".into()
        }
        fn name(&self) -> String {
            "Fake".into()
        }
        fn kind(&self) -> &'static str {
            "plex"
        }
        async fn sections(&self) -> Result<Vec<SectionDto>, String> {
            Ok(vec![])
        }
        async fn hubs(&self) -> Result<Vec<HubDto>, String> {
            Ok(vec![])
        }
        async fn items(
            &self,
            _key: &str,
            _ty: &str,
            _sort: Option<&str>,
            start: usize,
            size: usize,
        ) -> Result<Vec<ItemDto>, String> {
            Ok(self.items.iter().skip(start).take(size).cloned().collect())
        }
        async fn search(&self, _q: &str) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn children(&self, _k: &str, _s: usize, _z: usize) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn resolve_stream(
            &self,
            _k: &str,
            _d: Option<u64>,
            _m: bool,
            _q: &str,
        ) -> Result<crate::source::StreamResolution, String> {
            Err("fake source".into())
        }
    }

    struct FailingSource;

    #[async_trait::async_trait]
    impl crate::source::MediaSource for FailingSource {
        fn id(&self) -> String {
            "down".into()
        }
        fn name(&self) -> String {
            "Down".into()
        }
        fn kind(&self) -> &'static str {
            "plex"
        }
        async fn sections(&self) -> Result<Vec<SectionDto>, String> {
            Err("server offline".into())
        }
        async fn hubs(&self) -> Result<Vec<HubDto>, String> {
            Err("server offline".into())
        }
        async fn items(
            &self,
            _k: &str,
            _t: &str,
            _s: Option<&str>,
            _st: usize,
            _sz: usize,
        ) -> Result<Vec<ItemDto>, String> {
            Err("server offline".into())
        }
        async fn search(&self, _q: &str) -> Result<Vec<ItemDto>, String> {
            Err("server offline".into())
        }
        async fn children(&self, _k: &str, _s: usize, _z: usize) -> Result<Vec<ItemDto>, String> {
            Err("server offline".into())
        }
        async fn resolve_stream(
            &self,
            _k: &str,
            _d: Option<u64>,
            _m: bool,
            _q: &str,
        ) -> Result<crate::source::StreamResolution, String> {
            Err("server offline".into())
        }
    }

    // rev-4 guard: when EVERY contributing section fails, the merged fetch
    // reports the failure instead of masquerading as an empty library; a
    // partially failing view still serves the healthy sources.
    #[test]
    fn merged_fetch_surfaces_total_failure_but_tolerates_partial() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        let all_down: Vec<(std::sync::Arc<dyn crate::source::MediaSource>, String)> =
            vec![(std::sync::Arc::new(FailingSource), "sec".into())];
        let err = rt
            .block_on(fetch_all_merged(&all_down, "movie", "titleSort:asc"))
            .expect_err("total failure must surface as an error");
        assert!(err.contains("offline"));

        let mixed: Vec<(std::sync::Arc<dyn crate::source::MediaSource>, String)> = vec![
            (std::sync::Arc::new(FailingSource), "sec".into()),
            (
                std::sync::Arc::new(FakeItems {
                    items: vec![item("Alpha", Some(2020), "fake")],
                }),
                "sec".into(),
            ),
        ];
        let out = rt
            .block_on(fetch_all_merged(&mixed, "movie", "titleSort:asc"))
            .expect("partial failure still serves healthy sources");
        assert_eq!(out.len(), 1);
    }

    // rev-1 guard: the merged fetch must be EXHAUSTIVE — every unique title
    // is present regardless of duplicates or section size, because pages
    // window an immutable snapshot of this result (any early stop makes
    // pagination skip or duplicate titles across pages; review rounds 1–2).
    #[test]
    fn merged_fetch_is_exhaustive_past_the_initial_depth() {
        // Two duplicates up front, then more items than the initial fetch
        // depth (512), so exhaustiveness requires actually deepening.
        let mut items = vec![
            with_ids(item("Alpha", Some(2020), "fake"), &["imdb:tt1"]),
            with_ids(item("Alpha copy", Some(2020), "fake"), &["imdb:tt1"]),
        ];
        for i in 0..600 {
            items.push(item(&format!("Title {i:04}"), Some(2000), "fake"));
        }
        let refs: Vec<(std::sync::Arc<dyn crate::source::MediaSource>, String)> =
            vec![(std::sync::Arc::new(FakeItems { items }), "sec".into())];
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let out = rt
            .block_on(fetch_all_merged(&refs, "movie", "titleSort:asc"))
            .expect("fetch succeeds");
        // 602: same-source versions stay separate cards (rev-2), and every
        // item past the initial depth must be present.
        assert_eq!(out.len(), 602, "every title must be fetched");
    }

    // The exhaustive loop must terminate once every section is exhausted
    // (!any_full). Same-source versions stay separate cards (rev-2), so all
    // three survive dedup.
    #[test]
    fn merged_fetch_terminates_when_everything_duplicates() {
        let items = vec![
            with_ids(item("Alpha", Some(2020), "fake"), &["imdb:tt1"]),
            with_ids(item("Alpha 4K", Some(2020), "fake"), &["imdb:tt1"]),
            with_ids(item("Alpha DC", Some(2020), "fake"), &["imdb:tt1"]),
        ];
        let refs: Vec<(std::sync::Arc<dyn crate::source::MediaSource>, String)> =
            vec![(std::sync::Arc::new(FakeItems { items }), "sec".into())];
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let out = rt
            .block_on(fetch_all_merged(&refs, "movie", "titleSort:asc"))
            .expect("fetch succeeds");
        assert_eq!(
            out.len(),
            3,
            "same-source versions all kept; loop terminates"
        );
    }

    #[test]
    fn dedup_merges_by_provider_id_despite_title_differences() {
        let a = with_ids(item("The Matrix", Some(1999), "plex"), &["imdb:tt0133093"]);
        let b = with_ids(
            item("Matrix, The", Some(1999), "jf"),
            &["imdb:tt0133093", "tmdb:603"],
        );
        let out = dedup_across_sources(vec![a, b]);
        assert_eq!(out.len(), 1);
        let backing = out[0].backing.as_ref().unwrap();
        assert_eq!(backing.len(), 2);
        // The union of provider ids is retained for later joins.
        assert!(out[0].provider_ids.contains(&"tmdb:603".to_string()));
    }

    #[test]
    fn dedup_adopts_timestamps_from_a_non_face_backing() {
        // The richer face (poster + summary) lacks added/last-played; a plainer
        // backing carries them. The merged card must adopt them or it sorts to
        // the bottom of Recently added / Recently played despite a backing
        // having the timestamp (sorting slice 3 review, finding 3).
        let mut backing = with_ids(item("Dune", Some(2021), "plex"), &["imdb:tt1"]);
        backing.added_at_ms = Some(500);
        backing.last_watched_at_ms = Some(900);
        let mut face = with_ids(item("Dune", Some(2021), "jf"), &["imdb:tt1"]);
        face.poster = Some("p.jpg".into());
        face.summary = Some("sand".into()); // richer → becomes the face
                                            // Backing seen first, then the richer face swaps in — the fragile path.
        let out = dedup_across_sources(vec![backing, face]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].added_at_ms, Some(500));
        assert_eq!(out[0].last_watched_at_ms, Some(900));
    }

    // rev-2 guard: same-source versions stay separate cards (dedup is a
    // cross-source merge only); other sources' copies still merge into the
    // first group.
    #[test]
    fn dedup_keeps_same_source_versions_as_separate_cards() {
        let mut a = item("Dune", Some(2021), "jf-1");
        a.rating_key = "jf-1:/dune.1080p.mkv".into();
        let mut b = item("Dune", Some(2021), "jf-1");
        b.rating_key = "jf-1:/dune.2160p.mkv".into();
        let mut c = item("Dune", Some(2021), "plex");
        c.rating_key = "plex:42".into();

        let out = dedup_across_sources(vec![a, b, c]);
        assert_eq!(
            out.len(),
            2,
            "two cards: merged cross-source + solo version"
        );
        let merged = out
            .iter()
            .find(|g| g.backing.as_ref().unwrap().len() == 2)
            .expect("cross-source merge must still happen");
        assert!(merged
            .backing
            .as_ref()
            .unwrap()
            .iter()
            .any(|r| r.source_id == "plex"));
        // Which of the two smb versions the plex copy attaches to is not
        // pinned; the guarantee is that the other stays its own card.
        let solo = out
            .iter()
            .find(|g| g.backing.as_ref().unwrap().len() == 1)
            .expect("second same-source version stays its own card");
        assert_eq!(solo.source_id, "jf-1");
    }

    #[test]
    fn dedup_merges_by_normalized_title_and_exact_year() {
        let a = item("The Matrix", Some(1999), "plex");
        let b = item("the  matrix!", Some(1999), "jf-1"); // punctuation/case folded
        let c = item("The Matrix", Some(2021), "plex"); // remake: different year
        let out = dedup_across_sources(vec![a, b, c]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].backing.as_ref().unwrap().len(), 2);
        assert_eq!(out[1].backing.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn dedup_prefers_richer_display_and_keeps_watch_state() {
        // Bare local parse first, then the server copy with metadata.
        let mut local = item("Dune", Some(2021), "jf-1");
        local.rating_key = "jf-1:/x/dune.mkv".into();
        let mut server = item("Dune", Some(2021), "plex");
        server.rating_key = "plex:42".into();
        server.summary = Some("desert".into());
        server.poster = Some("p".into());
        server.played = Some(false);
        server.view_offset_ms = Some(1234);

        let out = dedup_across_sources(vec![local, server]);
        assert_eq!(out.len(), 1);
        let g = &out[0];
        // Server copy is the face and the first backing (default play target).
        assert_eq!(g.rating_key, "plex:42");
        assert_eq!(g.summary.as_deref(), Some("desert"));
        assert_eq!(g.view_offset_ms, Some(1234));
        let backing = g.backing.as_ref().unwrap();
        assert_eq!(backing.len(), 2);
        assert_eq!(backing[0].rating_key, "plex:42");
    }

    fn kinds() -> std::collections::HashMap<String, &'static str> {
        [("plex".to_string(), "plex"), ("jf".to_string(), "jellyfin")]
            .into_iter()
            .collect()
    }

    #[test]
    fn ranking_prefers_plex_and_points_play_identity_at_winner() {
        let mut a = item("Dune", Some(2021), "jf");
        a.rating_key = "jf:9".into();
        let mut b = item("Dune", Some(2021), "plex");
        b.rating_key = "plex:42".into();
        let groups = dedup_across_sources(vec![a, b]);
        let ranked = rank_backings(groups, &kinds(), &Default::default());
        let g = &ranked[0];
        // Plex outranks Jellyfin/Emby by default (policy ladder).
        assert_eq!(g.rating_key, "plex:42");
        assert_eq!(g.source_id, "plex");
        assert_eq!(g.backing.as_ref().unwrap()[0].source_id, "plex");
        assert!(g.canonical_id.is_some());
    }

    // rev-3 guard, server-only form: every face is a server that can take
    // watched-state actions itself, so no separate watch key is emitted.
    #[test]
    fn merged_watch_key_absent_when_face_is_a_server() {
        let mut a = item("Dune", Some(2021), "plex");
        a.rating_key = "plex:42".into();
        let mut b = item("Dune", Some(2021), "jf");
        b.rating_key = "jf:9".into();
        let groups = dedup_across_sources(vec![a, b]);
        let ranked = rank_backings(groups, &kinds(), &Default::default());
        assert_eq!(ranked[0].watch_key, None);

        let mut only = item("Solo", Some(2020), "plex");
        only.rating_key = "plex:7".into();
        let ranked = rank_backings(
            dedup_across_sources(vec![only]),
            &kinds(),
            &Default::default(),
        );
        assert_eq!(ranked[0].watch_key, None);
    }

    // rev-5 guard: merged watch state is the most-progressed across
    // backings — real progress must survive an earlier plain-unwatched
    // report, and finished beats in-progress.
    #[test]
    fn dedup_adopts_the_most_progressed_watch_state() {
        let mut a = item("Dune", Some(2021), "plex");
        a.played = Some(false); // plain unwatched, reported first
        let mut b = item("Dune", Some(2021), "jf");
        b.played = Some(false);
        b.view_offset_ms = Some(30 * 60 * 1000); // real progress
        let out = dedup_across_sources(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].view_offset_ms,
            Some(30 * 60 * 1000),
            "progress must not be hidden by an earlier unwatched report"
        );

        let mut c = item("Alien", Some(1979), "jf");
        c.played = Some(false);
        c.view_offset_ms = Some(1000); // barely started, reported first
        let mut d = item("Alien", Some(1979), "plex");
        d.played = Some(true); // finished elsewhere
        let out = dedup_across_sources(vec![c, d]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].played, Some(true), "finished beats in-progress");
    }

    #[test]
    fn per_title_override_beats_the_default_ranking() {
        let mut a = item("Dune", Some(2021), "plex");
        a.rating_key = "plex:42".into();
        let mut b = item("Dune", Some(2021), "jf");
        b.rating_key = "jf:9".into();
        let groups = dedup_across_sources(vec![a, b]);
        let canonical = canonical_id_of(&groups[0]);
        let overrides = [(canonical, "jf".to_string())].into_iter().collect();
        let ranked = rank_backings(groups, &kinds(), &overrides);
        assert_eq!(ranked[0].rating_key, "jf:9");
        assert_eq!(ranked[0].source_id, "jf");
    }

    // idv-2/idv-6 guard: the detail surface routes to the metadata-richest
    // backing via its own rank, independent of the play override — the
    // play-ordered backing list must not be scanned for it (an override
    // puts the overridden source first).
    #[test]
    fn detail_key_stays_on_the_richest_backing_despite_play_override() {
        let mut a = item("Dune", Some(2021), "plex");
        a.rating_key = "plex:42".into();
        let mut b = item("Dune", Some(2021), "jf");
        b.rating_key = "jf:9".into();
        let groups = dedup_across_sources(vec![a, b]);
        let canonical = canonical_id_of(&groups[0]);
        let overrides = [(canonical, "jf".to_string())].into_iter().collect();
        let ranked = rank_backings(groups, &kinds(), &overrides);
        let g = &ranked[0];
        // The override moves playback to Jellyfin; detail stays on Plex.
        assert_eq!(g.rating_key, "jf:9");
        assert_eq!(g.detail_key.as_deref(), Some("plex:42"));
    }

    // detail_key folds into the play identity when they agree, and never
    // appears on an unmerged entry (no backing list) — callers fall back to
    // rating_key in both cases.
    #[test]
    fn detail_key_absent_when_redundant_or_unmerged() {
        // Default ranking: the play face IS the richest backing.
        let mut a = item("Dune", Some(2021), "plex");
        a.rating_key = "plex:42".into();
        let mut b = item("Dune", Some(2021), "jf");
        b.rating_key = "jf:9".into();
        let ranked = rank_backings(
            dedup_across_sources(vec![a, b]),
            &kinds(),
            &Default::default(),
        );
        assert_eq!(ranked[0].rating_key, "plex:42");
        assert_eq!(ranked[0].detail_key, None);

        // No backing list at all (non-merged path).
        let bare = item("Bare", Some(2019), "plex");
        let ranked = rank_backings(vec![bare], &kinds(), &Default::default());
        assert_eq!(ranked[0].detail_key, None);
    }

    fn episode(source: &str, key: &str, season: Option<u32>, index: Option<u32>) -> ItemDto {
        let mut episode = item("Episode", None, source);
        episode.rating_key = format!("{source}:{key}");
        episode.media_type = Some("episode".to_string());
        episode.parent_index = season;
        episode.index = index;
        episode.parent_rating_key = Some(format!("{source}:season"));
        episode.grandparent_rating_key = Some(format!("{source}:show"));
        episode
    }

    #[test]
    fn hierarchy_merges_episode_coordinates_only_inside_the_parent_show() {
        let first = dedup_hierarchy_children(
            vec![
                episode("plex", "ep", Some(2), Some(7)),
                episode("jf", "ep", Some(2), Some(7)),
            ],
            "show:one",
            "season",
        );
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].backing.as_ref().map(Vec::len), Some(2));
        assert_eq!(
            first[0].canonical_id.as_deref(),
            Some("show:one|season:2|episode:7")
        );

        let other_show = dedup_hierarchy_children(
            vec![episode("plex", "ep", Some(2), Some(7))],
            "show:two",
            "season",
        );
        assert_eq!(
            other_show[0].canonical_id.as_deref(),
            Some("show:two|season:2|episode:7")
        );
    }

    #[test]
    fn hierarchy_keeps_ambiguous_or_conflicting_episode_identities_separate() {
        let ambiguous = dedup_hierarchy_children(
            vec![
                episode("plex", "unknown", None, Some(7)),
                episode("jf", "unknown", None, Some(7)),
            ],
            "show:one",
            "season",
        );
        assert_eq!(ambiguous.len(), 2);

        let mut plex = episode("plex", "ep", Some(2), Some(7));
        plex.provider_ids = vec!["tmdb:100".to_string()];
        let mut jellyfin = episode("jf", "ep", Some(2), Some(7));
        jellyfin.provider_ids = vec!["tmdb:200".to_string()];
        let conflicting =
            dedup_hierarchy_children(vec![plex, jellyfin], "show:one", "season");
        assert_eq!(conflicting.len(), 2);
    }

    #[test]
    fn hierarchy_never_collapses_two_items_from_one_source() {
        let groups = dedup_hierarchy_children(
            vec![
                episode("plex", "cut-a", Some(1), Some(1)),
                episode("plex", "cut-b", Some(1), Some(1)),
                episode("jf", "cut", Some(1), Some(1)),
            ],
            "show:one",
            "season",
        );
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|group| {
            let backings = group.backing.as_ref().unwrap();
            let mut sources = backings
                .iter()
                .map(|backing| backing.source_id.as_str())
                .collect::<Vec<_>>();
            sources.sort_unstable();
            sources.dedup();
            sources.len() == backings.len()
        }));
    }

    #[test]
    fn hierarchy_backings_retain_each_sources_parent_path() {
        let groups = dedup_hierarchy_children(
            vec![
                episode("plex", "ep", Some(1), Some(2)),
                episode("jf", "ep", Some(1), Some(2)),
            ],
            "show:one",
            "season",
        );
        let backings = groups[0].backing.as_ref().unwrap();
        assert!(backings.iter().any(|backing| {
            backing.source_id == "plex"
                && backing.parent_rating_key.as_deref() == Some("plex:season")
                && backing.grandparent_rating_key.as_deref() == Some("plex:show")
        }));
        assert!(backings.iter().any(|backing| {
            backing.source_id == "jf"
                && backing.parent_rating_key.as_deref() == Some("jf:season")
                && backing.grandparent_rating_key.as_deref() == Some("jf:show")
        }));
    }

    #[test]
    fn continuation_recovers_every_show_and_season_parent_copy() {
        let mut current = dedup_hierarchy_children(
            vec![
                episode("plex", "ep", Some(1), Some(2)),
                episode("jf", "ep", Some(1), Some(2)),
            ],
            "show:one",
            "season",
        )
        .remove(0);
        current.source_id = "jf".to_string();
        current.rating_key = "jf:ep".to_string();

        let shows = hierarchy_parent_backings(&current, true);
        let seasons = hierarchy_parent_backings(&current, false);
        assert_eq!(
            shows
                .iter()
                .map(|backing| backing.rating_key.as_str())
                .collect::<Vec<_>>(),
            ["jf:show", "plex:show"]
        );
        assert_eq!(
            seasons
                .iter()
                .map(|backing| backing.rating_key.as_str())
                .collect::<Vec<_>>(),
            ["jf:season", "plex:season"]
        );
    }
}

#[tauri::command]
pub async fn search(
    query: String,
    source_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ItemDto>, String> {
    let q = query.trim().to_string();
    if q.len() < 2 {
        return Ok(vec![]);
    }
    if q.len() > MAX_SEARCH_LEN {
        return Err("search query is too long".into());
    }
    let sources = state
        .registry
        .lock()
        .await
        .selected(source_id.as_deref())?;
    // Search: an empty result is a legitimate "no matches", so don't turn it into
    // an error just because one backend hiccuped (error only if all failed).
    aggregate(sources, false, move |s| {
        let q = q.clone();
        async move { s.search(&q).await }
    })
    .await
}

fn merged_children_fingerprint(
    canonical_id: Option<&str>,
    media_type: Option<&str>,
    backings: &[BackingRef],
) -> String {
    let mut identities = backings
        .iter()
        .map(|backing| format!("{}:{}", backing.source_id, backing.rating_key))
        .collect::<Vec<_>>();
    identities.sort();
    format!(
        "{}|{}|{}",
        canonical_id.unwrap_or(""),
        media_type.unwrap_or(""),
        identities.join("|")
    )
}

fn hierarchy_coordinate(item: &ItemDto, parent_media_type: &str) -> Option<(u32, u32)> {
    match parent_media_type {
        "show" if item.media_type.as_deref() == Some("season") => Some((item.index?, 0)),
        "season" if item.media_type.as_deref() == Some("episode") => {
            Some((item.parent_index?, item.index?))
        }
        _ => None,
    }
}

fn hierarchy_items_match(left: &ItemDto, right: &ItemDto, parent_media_type: &str) -> bool {
    if left.source_id == right.source_id {
        return false;
    }
    if !left.provider_ids.is_empty() || !right.provider_ids.is_empty() {
        return !left.provider_ids.is_empty()
            && !right.provider_ids.is_empty()
            && left
                .provider_ids
                .iter()
                .any(|id| right.provider_ids.contains(id));
    }
    hierarchy_coordinate(left, parent_media_type).is_some_and(|coordinate| {
        hierarchy_coordinate(right, parent_media_type) == Some(coordinate)
    })
}

fn hierarchy_canonical_id(
    item: &ItemDto,
    parent_canonical_id: &str,
    parent_media_type: &str,
) -> String {
    if let Some(provider_id) = item.provider_ids.iter().min() {
        return provider_id.clone();
    }
    match hierarchy_coordinate(item, parent_media_type) {
        Some((season, 0)) if parent_media_type == "show" => {
            format!("{parent_canonical_id}|season:{season}")
        }
        Some((season, episode)) => {
            format!("{parent_canonical_id}|season:{season}|episode:{episode}")
        }
        None => format!(
            "{parent_canonical_id}|copy:{}:{}",
            item.source_id, item.rating_key
        ),
    }
}

fn merge_hierarchy_item(group: &mut ItemDto, item: ItemDto) {
    let item_ref = crate::source::backing_ref_of(&item);
    let backing = group.backing.get_or_insert_with(Vec::new);
    if !backing.contains(&item_ref) {
        backing.push(item_ref.clone());
    }

    if watch_rank(item.played, item.view_offset_ms)
        > watch_rank(group.played, group.view_offset_ms)
    {
        group.played = item.played;
        group.view_offset_ms = item.view_offset_ms;
    }
    group.added_at_ms = group.added_at_ms.max(item.added_at_ms);
    group.last_watched_at_ms = group.last_watched_at_ms.max(item.last_watched_at_ms);

    let richness = |candidate: &ItemDto| {
        candidate.summary.is_some() as u8
            + candidate.poster.is_some() as u8
            + candidate.year.is_some() as u8
    };
    if richness(&item) > richness(group) {
        let keep_backing = group.backing.take();
        let keep_played = group.played;
        let keep_offset = group.view_offset_ms;
        let keep_added = group.added_at_ms;
        let keep_last_watched = group.last_watched_at_ms;
        let mut provider_ids = std::mem::take(&mut group.provider_ids);
        for provider_id in &item.provider_ids {
            if !provider_ids.contains(provider_id) {
                provider_ids.push(provider_id.clone());
            }
        }
        *group = item;
        group.provider_ids = provider_ids;
        group.backing = keep_backing.map(|mut backings| {
            backings.retain(|backing| *backing != item_ref);
            backings.insert(0, item_ref);
            backings
        });
        if watch_rank(keep_played, keep_offset)
            > watch_rank(group.played, group.view_offset_ms)
        {
            group.played = keep_played;
            group.view_offset_ms = keep_offset;
        }
        group.added_at_ms = keep_added;
        group.last_watched_at_ms = keep_last_watched;
    } else {
        for provider_id in item.provider_ids {
            if !group.provider_ids.contains(&provider_id) {
                group.provider_ids.push(provider_id);
            }
        }
    }
}

fn dedup_hierarchy_children(
    items: Vec<ItemDto>,
    parent_canonical_id: &str,
    parent_media_type: &str,
) -> Vec<ItemDto> {
    let mut groups: Vec<ItemDto> = Vec::new();
    for item in items {
        let hit = groups.iter().position(|group| {
            group.backing.as_ref().is_none_or(|backings| {
                backings
                    .iter()
                    .all(|backing| backing.source_id != item.source_id)
            }) && hierarchy_items_match(group, &item, parent_media_type)
        });
        if let Some(index) = hit {
            merge_hierarchy_item(&mut groups[index], item);
        } else {
            let mut group = item;
            group.backing = Some(vec![crate::source::backing_ref_of(&group)]);
            groups.push(group);
        }
    }
    for group in &mut groups {
        group.canonical_id = Some(hierarchy_canonical_id(
            group,
            parent_canonical_id,
            parent_media_type,
        ));
    }
    groups
}

fn sort_hierarchy_children(items: &mut [ItemDto]) {
    items.sort_by(|left, right| {
        left.parent_index
            .unwrap_or(u32::MAX)
            .cmp(&right.parent_index.unwrap_or(u32::MAX))
            .then_with(|| {
                left.index
                    .unwrap_or(u32::MAX)
                    .cmp(&right.index.unwrap_or(u32::MAX))
            })
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
            .then_with(|| left.canonical_id.cmp(&right.canonical_id))
            .then_with(|| left.source_id.cmp(&right.source_id))
            .then_with(|| left.rating_key.cmp(&right.rating_key))
    });
}

async fn fetch_merged_children(
    state: &AppState,
    backings: &[BackingRef],
    parent_canonical_id: &str,
    parent_media_type: &str,
) -> Result<Vec<ItemDto>, String> {
    crate::durable::ensure_commands_ready()?;
    let (routes, kinds) = {
        let registry = state.registry.lock().await;
        let kinds = registry
            .all()
            .iter()
            .map(|source| (source.id(), source.kind()))
            .collect::<HashMap<_, _>>();
        let routes = backings
            .iter()
            .filter_map(|backing| registry.route(&backing.rating_key).ok())
            .collect::<Vec<_>>();
        (routes, kinds)
    };
    if routes.is_empty() {
        return Err("none of this title's source copies is still configured".to_string());
    }

    let mut jobs = tokio::task::JoinSet::new();
    for (source, raw) in routes {
        jobs.spawn(async move {
            tokio::time::timeout(Duration::from_secs(20), all_children(&source, &raw)).await
        });
    }
    let mut children = Vec::new();
    let mut succeeded = false;
    while let Some(result) = jobs.join_next().await {
        if let Ok(Ok(Ok(mut items))) = result {
            succeeded = true;
            children.append(&mut items);
        }
    }
    if !succeeded {
        return Err("none of this title's source copies is reachable".to_string());
    }

    let overrides = config::load_config()
        .map_err(|_| "could not read Vela settings".to_string())?
        .merged_overrides;
    let mut merged = rank_backings(
        dedup_hierarchy_children(children, parent_canonical_id, parent_media_type),
        &kinds,
        &overrides,
    );
    sort_hierarchy_children(&mut merged);
    Ok(merged)
}

/// Children of a show (seasons) or season (episodes), for drill-down navigation.
#[tauri::command]
pub async fn get_children(
    rating_key: String,
    backing: Option<Vec<BackingRef>>,
    canonical_id: Option<String>,
    media_type: Option<String>,
    start: usize,
    size: usize,
    state: State<'_, AppState>,
) -> Result<Vec<ItemDto>, String> {
    let size = clamp_page_size(size);
    let mut backings = backing.unwrap_or_default();
    backings.sort_by(|left, right| {
        left.source_id
            .cmp(&right.source_id)
            .then_with(|| left.rating_key.cmp(&right.rating_key))
    });
    backings.dedup_by(|left, right| {
        left.source_id == right.source_id && left.rating_key == right.rating_key
    });
    if backings.len() > 1 {
        let parent_media_type = media_type.as_deref().unwrap_or("");
        if !matches!(parent_media_type, "show" | "season") {
            return Err("merged children require a show or season parent".to_string());
        }
        let fingerprint =
            merged_children_fingerprint(canonical_id.as_deref(), media_type.as_deref(), &backings);
        if start > 0 {
            let snapshot = state.merged_children_snapshot.lock().await;
            if let Some(snapshot) = snapshot
                .as_ref()
                .filter(|snapshot| snapshot.fingerprint == fingerprint)
            {
                return Ok(snapshot
                    .items
                    .iter()
                    .skip(start)
                    .take(size)
                    .cloned()
                    .collect());
            }
        }
        let parent_canonical_id = canonical_id.unwrap_or_else(|| {
            format!(
                "hierarchy:{}",
                backings
                    .iter()
                    .map(|backing| format!("{}:{}", backing.source_id, backing.rating_key))
                    .collect::<Vec<_>>()
                    .join("|")
            )
        });
        let items = fetch_merged_children(
            &state,
            &backings,
            &parent_canonical_id,
            parent_media_type,
        )
        .await?;
        let page = items.iter().skip(start).take(size).cloned().collect();
        *state.merged_children_snapshot.lock().await = Some(MergedChildrenSnapshot {
            fingerprint,
            items,
        });
        return Ok(page);
    }
    let (src, raw) = state.registry.lock().await.route(&rating_key)?;
    src.children(&raw, start, size).await
}

/// Full metadata for one item — the detail / "more info" surface. Routes by the
/// namespaced key; the registry lock is released before the (network) call.
/// Sources that can't enrich the item return an error the caller degrades on.
#[tauri::command]
pub async fn get_item_detail(
    rating_key: String,
    state: State<'_, AppState>,
) -> Result<DetailDto, String> {
    let (src, raw) = state.registry.lock().await.route(&rating_key)?;
    src.item_detail(&raw).await
}

/// Everything in a source's libraries featuring a person — the clickable
/// actor/director/writer browse. Routes by the namespaced person key; the
/// registry lock is released before the (network) call. Unsupported sources
/// return an error the caller degrades on.
#[tauri::command]
pub async fn get_person_items(
    person_key: String,
    kind: String,
    state: State<'_, AppState>,
) -> Result<Vec<ItemDto>, String> {
    let (src, raw) = state.registry.lock().await.route(&person_key)?;
    src.person_items(&raw, &kind).await
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WatchStateMutationDto {
    pub succeeded_sources: usize,
    pub failed_sources: usize,
    pub failed_source_names: Vec<String>,
}

struct WatchMutationTarget {
    source: std::sync::Arc<dyn MediaSource>,
    source_name: String,
    raw_key: String,
}

struct PreparedWatchMutations {
    targets: Vec<WatchMutationTarget>,
    failed_sources: usize,
    failed_source_names: Vec<String>,
}

/// Resolve every still-configured title backing while holding the registry
/// lock, then release it before any provider call. Backings for sources the
/// user removed are no longer mutation targets; malformed identities for a
/// source that still exists count as a safe, named failure.
async fn prepare_watch_mutations(
    source_registry: &tokio::sync::Mutex<crate::source::SourceRegistry>,
    backings: &[BackingRef],
) -> PreparedWatchMutations {
    let registry = source_registry.lock().await;
    let mut targets = Vec::new();
    let mut failed_source_names = Vec::new();
    for backing in backings {
        let Some(configured_source) = registry.get(&backing.source_id) else {
            continue;
        };
        let correctly_namespaced = backing
            .rating_key
            .split_once(':')
            .is_some_and(|(source_id, _)| source_id == backing.source_id);
        if !correctly_namespaced {
            failed_source_names.push(configured_source.name());
            continue;
        }
        match registry.route(&backing.rating_key) {
            Ok((source, raw_key)) => targets.push(WatchMutationTarget {
                source_name: source.name(),
                source,
                raw_key,
            }),
            Err(_) => failed_source_names.push(configured_source.name()),
        }
    }
    let failed_sources = failed_source_names.len();
    PreparedWatchMutations {
        targets,
        failed_sources,
        failed_source_names,
    }
}

/// Run each provider mutation independently. Provider error text is discarded:
/// it may contain request details or credentials and never belongs in an IPC
/// result. Source display names and aggregate counts are the entire public
/// failure surface.
async fn execute_watch_mutations(
    prepared: PreparedWatchMutations,
    played: bool,
) -> WatchStateMutationDto {
    let mut jobs = tokio::task::JoinSet::new();
    for target in prepared.targets {
        jobs.spawn(async move {
            let result = target.source.mark_played(&target.raw_key, played).await;
            (target.source_name, result.is_ok())
        });
    }

    let mut succeeded_sources = 0;
    let mut failed_sources = prepared.failed_sources;
    let mut failed_source_names = prepared.failed_source_names;
    while let Some(joined) = jobs.join_next().await {
        match joined {
            Ok((_source_name, true)) => succeeded_sources += 1,
            Ok((source_name, false)) => {
                failed_sources += 1;
                failed_source_names.push(source_name);
            }
            Err(_) => {
                failed_sources += 1;
                failed_source_names.push("Unavailable source".to_string());
            }
        }
    }
    failed_source_names.sort();
    failed_source_names.dedup();
    WatchStateMutationDto {
        succeeded_sources,
        failed_sources,
        failed_source_names,
    }
}

fn all_watch_mutations_failed(result: &WatchStateMutationDto) -> bool {
    result.succeeded_sources == 0
}

fn total_watch_failure_message(result: &WatchStateMutationDto) -> String {
    if result.failed_source_names.is_empty() {
        return "none of this title's sources is currently configured".to_string();
    }
    format!(
        "watched-state update failed on all {} configured source(s): {}",
        result.failed_sources,
        result.failed_source_names.join(", ")
    )
}

/// Mark an item watched/unwatched on every currently configured backing. All
/// routes are captured before the independent provider calls begin.
#[tauri::command]
pub async fn set_watched(
    item: ItemDto,
    played: bool,
    state: State<'_, AppState>,
) -> Result<WatchStateMutationDto, String> {
    // One edit at a time: overlapping curate-first hides and failure
    // rollbacks must not interleave (undo tokens carry no generation).
    let _edit = state.watch_edit_lock.lock().await;
    let backings = watched_state_backings(&item);
    let prepared = prepare_watch_mutations(&state.registry, &backings).await;
    // Watched-state edits curate Continue Watching in the same op (owner
    // decision 2026-07-10): watched OR reset-to-unwatched, the item is no
    // longer "recently played and not finished". Drop the recents entry AND
    // tombstone its identity set — the frozen local snapshot would otherwise
    // keep masking the new server state in the hero merge, and a lagging
    // server hub copy would resurface it. Playing the item again clears the
    // tombstone; the explicit remove action stays the keep-progress dismiss.
    //
    // Curate BEFORE the server call: `mark_played` can take up to ~15s
    // (client timeouts, Plex rediscover+retry), and a play recorded inside
    // that window would be dropped by a delayed curation — losing a
    // sub-threshold resume position that only Vela's stamp holds (plan
    // review r4). On a failed server edit, the undo token restores the
    // exact pre-curation state; newer play activity, if any landed
    // meanwhile, wins over the restore.
    let key = item
        .watch_key
        .clone()
        .unwrap_or_else(|| item.rating_key.clone());
    let undo = config::update(move |cfg| Ok(crate::recents::hide_with_undo(cfg, &key)))?;
    let result = execute_watch_mutations(prepared, played).await;
    if all_watch_mutations_failed(&result) {
        let error = total_watch_failure_message(&result);
        let _ = config::update(move |cfg| {
            crate::recents::restore_hidden(cfg, undo);
            Ok(())
        });
        return Err(error);
    }
    Ok(result)
}

// ---- playback ------------------------------------------------------------

const MAX_PLAYBACK_SIGNALS: usize = 64;
const MAX_PLAYBACK_CHOICES: usize = 16;
const PLAYBACK_CHOICE_TTL: Duration = Duration::from_secs(120);

#[derive(Default)]
struct PlaybackSignals {
    eof: VecDeque<String>,
    ended: VecDeque<PlaybackCompletion>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackCompletion {
    pub session_id: String,
    pub item_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_key: Option<String>,
    /// Internal ordering evidence for exact-session curation. The continuation
    /// payload remains ids only.
    #[serde(skip)]
    pub started_at_ms: u64,
    /// Immutable title identities captured when this exact playback launched.
    /// They stay backend-only so completion events remain ids-only.
    #[serde(skip)]
    pub watch_backings: Vec<BackingRef>,
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSourceChoiceDto {
    pub source_id: String,
    pub source_name: String,
    pub locality: String,
    pub quality_label: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSourceChoiceRequestDto {
    pub request_id: String,
    pub title: String,
    pub choices: Vec<PlaybackSourceChoiceDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PlayCommandResult {
    Started { session_id: String },
    Superseded,
    SourceChoiceRequired {
        request: PlaybackSourceChoiceRequestDto,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaybackRunKind {
    Series,
    VelaPlaylist,
    ServerPlaylist,
}

#[derive(Debug, Clone)]
pub(crate) struct PlaybackRunState {
    session_id: String,
    kind: PlaybackRunKind,
    affinity_source_id: Option<String>,
}

#[derive(Debug, Clone)]
struct PendingPlaybackChoice {
    request: PlaybackSourceChoiceRequestDto,
    created_at: Instant,
    item: ItemDto,
    start_from_beginning: bool,
    session_id: String,
    playlist: Option<PlaylistLocation>,
    replace_session: Option<String>,
    run_kind: Option<PlaybackRunKind>,
}

#[derive(Default)]
pub(crate) struct PlaybackChoiceRequests {
    entries: VecDeque<PendingPlaybackChoice>,
}

impl PlaybackChoiceRequests {
    fn prune_at(&mut self, now: Instant) {
        self.entries.retain(|entry| {
            now.checked_duration_since(entry.created_at)
                .is_some_and(|age| age <= PLAYBACK_CHOICE_TTL)
        });
    }

    fn insert_at(&mut self, pending: PendingPlaybackChoice, now: Instant) {
        self.prune_at(now);
        self.entries
            .retain(|entry| entry.request.request_id != pending.request.request_id);
        self.entries.push_back(pending);
        while self.entries.len() > MAX_PLAYBACK_CHOICES {
            self.entries.pop_front();
        }
    }

    fn request_at(
        &mut self,
        request_id: &str,
        now: Instant,
    ) -> Option<PlaybackSourceChoiceRequestDto> {
        self.prune_at(now);
        self.entries
            .iter()
            .find(|entry| entry.request.request_id == request_id)
            .map(|entry| entry.request.clone())
    }

    fn take_at(&mut self, request_id: &str, now: Instant) -> Option<PendingPlaybackChoice> {
        self.prune_at(now);
        let index = self
            .entries
            .iter()
            .position(|entry| entry.request.request_id == request_id)?;
        self.entries.remove(index)
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    fn clear_for_session(&mut self, session_id: &str) {
        self.entries.retain(|entry| {
            entry.replace_session.as_deref() != Some(session_id)
                && entry.session_id != session_id
        });
    }

    fn has_manual_pending(&mut self, now: Instant) -> bool {
        self.prune_at(now);
        self.entries
            .iter()
            .any(|entry| entry.replace_session.is_none())
    }
}

/// The observation handle currently authorized for automatic window-state
/// inheritance. The session id keeps delayed or manual work from sampling an
/// unrelated mpv process.
#[derive(Clone)]
pub(crate) struct PlaybackWindowSession {
    session_id: String,
    observation: playback::WindowStateObservation,
}

/// A health sampler's verdict, waiting for something that can act on it.
pub(crate) struct StepDownRequest {
    /// The play that produced the verdict. A relaunch is refused if this is no
    /// longer the active session: by then the verdict describes a player the
    /// user has already replaced.
    pub session_id: String,
    pub position_ms: u64,
    pub reason: crate::automatic::StepDownReason,
    /// Steps already taken in this LOGICAL play. A step-down relaunches mpv, so
    /// without carrying this the cap would reset on every step and Automatic
    /// would walk to the floor two rungs at a time.
    pub steps_taken: u32,
}

/// Carries a verdict from the sampler thread to the async dispatcher that can
/// start a new play, the same shape `PlaybackAdvance` uses for clean EOF.
///
/// Holds ONE request, replaced rather than queued: a second verdict can only
/// describe a play the first is already replacing.
#[derive(Default)]
pub(crate) struct StepDownQueue {
    pending: std::sync::Mutex<Option<StepDownRequest>>,
    changed: tokio::sync::Notify,
}

impl StepDownQueue {
    pub(crate) fn request(&self, request: StepDownRequest) {
        *self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(request);
        self.changed.notify_one();
    }

    pub(crate) async fn next(&self) -> StepDownRequest {
        loop {
            // Register interest BEFORE checking, or a request arriving between
            // the check and the wait would not wake this loop.
            let changed = self.changed.notified();
            if let Some(request) = self
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            {
                return request;
            }
            changed.await;
        }
    }
}

/// Act on one verdict: relaunch the running play one tier lower, at the
/// position it had reached. Returns the tier it stepped to, for the caller's
/// log and for tests; `None` means the verdict was declined, which is ordinary.
pub(crate) async fn apply_step_down(
    state: &AppState,
    request: StepDownRequest,
) -> Option<crate::source::QualityTier> {
    // The verdict describes one exact player. If the user has since started
    // something else, that player is already gone and stepping it down would
    // replace whatever they chose instead.
    let item = {
        let active = state.active_playback_item.lock().await;
        match active.as_ref() {
            Some((session, item)) if *session == request.session_id => item.clone(),
            _ => return None,
        }
    };

    // What this copy can actually be delivered as, asked only now: a verdict is
    // rare, and paying a decision round trip on every play to prepare for one
    // would tax everybody who never steps down.
    let (source, key) = {
        let registry = state.registry.lock().await;
        registry.route(&item.rating_key).ok()?
    };
    let options = source.playback_options(&key, None).await.ok()?;
    // Where the walk has already got to: the stored setting, stepped down once
    // per step already taken. Derived rather than stored, so the current tier
    // cannot drift out of step with the count that bounds it.
    let current = quality_after_steps(
        &current_playback_quality(),
        &options.tiers,
        request.steps_taken,
    );
    let next = crate::source::next_tier_down(&current, &options.tiers)?;

    eprintln!(
        "vela: {} — stepping down to {}",
        match request.reason {
            crate::automatic::StepDownReason::DropStorm =>
                "playback is dropping frames faster than it can decode",
            crate::automatic::StepDownReason::StarvingCache =>
                "playback keeps running out of buffered video",
        },
        next.id
    );

    let session_id = uuid::Uuid::new_v4().to_string();
    let outcome = play_by_key(
        state,
        PlayLaunchRequest {
            item: &item,
            start_from_beginning: false,
            session_id: &session_id,
            playlist: None,
            // This replaces the exact play the verdict came from, and nothing
            // else: a race with the user's own choice must lose.
            replace_session: Some(&request.session_id),
            run_kind: None,
            explicit_source_id: None,
            persist_explicit_choice: false,
            quality_override: Some(next.id),
            resume_override_ms: Some(request.position_ms),
            osd_notice: Some(short_quality_notice(next)),
            steps_taken: request.steps_taken + 1,
        },
    )
    .await;
    match outcome {
        Ok(_) => Some(next),
        Err(failure) => {
            eprintln!("vela: couldn't lower the quality: {}", failure.message);
            None
        }
    }
}

/// The OSD line for a step-down. Deliberately tiny — mpv's OSD is large, and
/// this is an explanation, not an announcement (owner, 2026-07-25).
fn short_quality_notice(tier: crate::source::QualityTier) -> String {
    if tier.bitrate_kbps >= 1000 && tier.bitrate_kbps.is_multiple_of(1000) {
        format!("↓ {} Mbps", tier.bitrate_kbps / 1000)
    } else if tier.bitrate_kbps >= 1000 {
        format!("↓ {:.1} Mbps", tier.bitrate_kbps as f64 / 1000.0)
    } else {
        format!("↓ {} kbps", tier.bitrate_kbps)
    }
}

/// The quality a play is running at after `steps` step-downs from `stored`.
/// Stops at the floor rather than running off the end.
fn quality_after_steps(stored: &str, tiers: &[crate::source::QualityTier], steps: u32) -> String {
    let mut current = stored.to_string();
    for _ in 0..steps {
        match crate::source::next_tier_down(&current, tiers) {
            Some(tier) => current = tier.id.to_string(),
            None => break,
        }
    }
    current
}

/// The quality the running play was launched at. Automatic starts at Original,
/// and a stored tier is where the ladder walk resumes from.
fn current_playback_quality() -> String {
    config::load_config()
        .map(|cfg| config::playback_quality(cfg.playback_quality.as_deref()))
        .unwrap_or_else(|_| config::PLAYBACK_QUALITY_ORIGINAL.to_string())
}

/// A server-side transcode Vela started and is therefore obliged to stop.
///
/// It holds the source itself rather than an id to look up later: the play must
/// remain stoppable even if the user disconnects that server while it is still
/// running. Neither backend has a keep-alive and how an abandoned session
/// expires is unknown, so the DELETE is mandatory, not best-effort.
pub(crate) struct ActiveTranscode {
    source: std::sync::Arc<dyn MediaSource>,
    session: String,
}

/// Where that record lives while the play runs. `AppState` owns it so that app
/// exit — which kills mpv and returns without waiting for any tracker tail —
/// still has something to drain (finding `tr-4`).
pub(crate) type ActiveTranscodeSlot = std::sync::Arc<std::sync::Mutex<Option<ActiveTranscode>>>;

/// Bounded so a shutdown can never hang on an unreachable server. Matches the
/// tracker's own HTTP deadline in `playback.rs`.
const TRANSCODE_TEARDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// Install the record for a play that started a transcode, returning whatever
/// it displaced so the caller can stop that encoder too. Replacement is not
/// left to the superseded play's tail: that tail races this registration, and
/// after it loses the race `take_active_transcode` correctly refuses it.
pub(crate) fn register_active_transcode(
    slot: &ActiveTranscodeSlot,
    source: &std::sync::Arc<dyn MediaSource>,
    session: String,
) -> Option<ActiveTranscode> {
    slot.lock()
        .unwrap_or_else(|e| e.into_inner())
        .replace(ActiveTranscode {
            source: source.clone(),
            session,
        })
}

/// Claim the record for exactly this session. Session-matched so a newer play's
/// encoder survives an older play's end callback, and `take` so exactly one of
/// the tail, the failure path, and the exit sweep can ever stop a given session.
pub(crate) fn take_active_transcode(
    slot: &ActiveTranscodeSlot,
    session: &str,
) -> Option<ActiveTranscode> {
    let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    if guard
        .as_ref()
        .is_some_and(|active| active.session == session)
    {
        guard.take()
    } else {
        None
    }
}

/// Claim whatever is registered, whatever its session. Only the exit sweep may
/// do this: it is the last code that will ever run, so leaving a record behind
/// means leaving an encoder running.
pub(crate) fn take_any_active_transcode(slot: &ActiveTranscodeSlot) -> Option<ActiveTranscode> {
    slot.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// Stop one transcode, awaited to completion or to its deadline.
pub(crate) async fn stop_transcode_record(record: ActiveTranscode) {
    let _ = tokio::time::timeout(
        TRANSCODE_TEARDOWN_TIMEOUT,
        record.source.stop_transcode(&record.session),
    )
    .await;
}

/// The same teardown for callers that are plain threads outside any runtime —
/// the tracker tail and the exit sweep. Both must not return until the DELETE
/// has been attempted, which is the whole point: a detached task there can die
/// with the process. Runs on the app's own runtime so the provider's pooled
/// HTTP connections stay on the reactor that created them.
pub(crate) fn stop_transcode_record_blocking(record: ActiveTranscode) {
    tauri::async_runtime::block_on(stop_transcode_record(record));
}

/// Joins mpv's clean-EOF observation with the matching tracker's completed
/// final write. The dispatcher receives a UUID only after both happened, so
/// auto-advance cannot overtake recents/server progress and a stale EOF cannot
/// advance a newer playlist cursor.
#[derive(Default)]
pub(crate) struct PlaybackAdvance {
    signals: std::sync::Mutex<PlaybackSignals>,
    changed: tokio::sync::Notify,
}

#[derive(Debug, Clone)]
pub(crate) struct PlaylistCursor {
    owner: PlaylistOwner,
    playlist_id: String,
    entry_id: String,
    index: usize,
    session_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaylistOwner {
    Vela,
    Server,
}

#[derive(Debug, Clone)]
struct PlaylistLocation {
    owner: PlaylistOwner,
    playlist_id: String,
    entry_id: String,
    index: usize,
}

#[derive(Debug)]
struct PlayFailure {
    message: String,
    kind: PlayFailureKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayFailureKind {
    Unavailable,
    Superseded,
    Fatal,
}

impl PlayFailure {
    fn unavailable(message: String) -> Self {
        Self {
            message,
            kind: PlayFailureKind::Unavailable,
        }
    }

    fn superseded() -> Self {
        Self {
            message: "playback request was superseded".to_string(),
            kind: PlayFailureKind::Superseded,
        }
    }

    fn fatal(message: String) -> Self {
        Self {
            message,
            kind: PlayFailureKind::Fatal,
        }
    }
}

impl PlaybackAdvance {
    fn push_eof_bounded(queue: &mut VecDeque<String>, session_id: String) {
        if !queue.iter().any(|held| held == &session_id) {
            queue.push_back(session_id);
            while queue.len() > MAX_PLAYBACK_SIGNALS {
                queue.pop_front();
            }
        }
    }

    pub(crate) fn mark_eof(&self, session_id: String) {
        let mut signals = self
            .signals
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::push_eof_bounded(&mut signals.eof, session_id);
        drop(signals);
        self.changed.notify_one();
    }

    fn mark_ended(&self, completion: PlaybackCompletion) {
        let mut signals = self
            .signals
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !signals
            .ended
            .iter()
            .any(|held| held.session_id == completion.session_id)
        {
            signals.ended.push_back(completion);
            while signals.ended.len() > MAX_PLAYBACK_SIGNALS {
                signals.ended.pop_front();
            }
        }
        drop(signals);
        self.changed.notify_one();
    }

    fn take_ready(&self) -> Option<PlaybackCompletion> {
        let mut signals = self
            .signals
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let eof_index = signals.eof.iter().position(|session| {
            signals
                .ended
                .iter()
                .any(|ended| ended.session_id == *session)
        })?;
        let session = signals.eof.remove(eof_index)?;
        let ended_index = signals
            .ended
            .iter()
            .position(|ended| ended.session_id == session)?;
        signals.ended.remove(ended_index)
    }

    pub(crate) async fn next(&self) -> PlaybackCompletion {
        loop {
            let changed = self.changed.notified();
            if let Some(session) = self.take_ready() {
                return session;
            }
            changed.await;
        }
    }
}

fn cursor_matches_session(cursor: Option<&PlaylistCursor>, session_id: &str) -> bool {
    cursor.is_some_and(|current| current.session_id == session_id)
}

fn expected_session_matches(active: Option<&str>, expected: Option<&str>) -> bool {
    expected.is_none_or(|expected| active == Some(expected))
}

fn inherited_window_state(
    current: Option<&PlaybackWindowSession>,
    replace_session: Option<&str>,
) -> playback::PlaybackWindowState {
    let Some(expected) = replace_session else {
        return playback::PlaybackWindowState::default();
    };
    current
        .filter(|current| current.session_id == expected)
        .map(|current| current.observation.snapshot())
        .unwrap_or_default()
}

fn inherited_screen_name(
    current: Option<&PlaybackWindowSession>,
    replace_session: Option<&str>,
) -> Option<String> {
    let expected = replace_session?;
    current
        .filter(|current| current.session_id == expected)
        .and_then(|current| current.observation.display_snapshot().names.into_iter().next())
}

async fn validate_playback_session(state: &AppState, expected_session: Option<&str>) -> bool {
    let active = state.active_playback_session.lock().await;
    expected_session_matches(active.as_deref(), expected_session)
}

async fn install_playback_session(
    state: &AppState,
    new_session: &str,
    item: &ItemDto,
    run_kind: Option<PlaybackRunKind>,
    affinity_source_id: Option<String>,
) {
    *state.active_playback_session.lock().await = Some(new_session.to_string());
    *state.active_playback_item.lock().await = Some((new_session.to_string(), item.clone()));
    *state.playback_run.lock().await = run_kind.map(|kind| PlaybackRunState {
        session_id: new_session.to_string(),
        kind,
        affinity_source_id,
    });
}

async fn clear_playback_session_if(state: &AppState, expected_session: &str) {
    let mut active = state.active_playback_session.lock().await;
    if active.as_deref() == Some(expected_session) {
        *active = None;
        let mut item = state.active_playback_item.lock().await;
        if item
            .as_ref()
            .is_some_and(|(session, _)| session == expected_session)
        {
            *item = None;
        }
        let mut run = state.playback_run.lock().await;
        if run
            .as_ref()
            .is_some_and(|run| run.session_id == expected_session)
        {
            *run = None;
        }
    }
}

async fn active_session_matches(state: &AppState, session_id: &str) -> bool {
    state.active_playback_session.lock().await.as_deref() == Some(session_id)
}

/// Keep only recents whose source still exists. An entry from a removed
/// source (e.g. the local/SMB/SSH family removed 2026-07-08) would render a
/// hero card whose Play can only error — the dead-end the UX rulings forbid.
/// Read-time filtering only: the config entries are preserved untouched, so
/// a rollback build still sees them.
fn filter_live_recents(items: Vec<ItemDto>, live_source_ids: &[String]) -> Vec<ItemDto> {
    items
        .into_iter()
        .filter(|i| live_source_ids.contains(&i.source_id))
        .collect()
}

/// Vela's "recently played and not finished" list, newest first — the hero
/// cover-flow's primary feed. Entries from removed sources are filtered out
/// at read time (see `filter_live_recents`).
#[tauri::command]
pub async fn get_recents(state: State<'_, AppState>) -> Result<Vec<ItemDto>, String> {
    let cfg =
        config::load_config().map_err(|_| "could not read Vela settings".to_string())?;
    let live = state.registry.lock().await.ids();
    Ok(filter_live_recents(crate::recents::list(&cfg), &live))
}

/// Remove an item from the Continue Watching flow: drop the recents entry,
/// tombstone the key (so a server hub that still carries it stays
/// suppressed), and best-effort ask the backend to remove it server-side
/// (Plex). The tombstone clears if the item is played again.
#[tauri::command]
pub async fn remove_from_continue(
    rating_key: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // hide() tombstones the entry's full identity set and reports which key
    // the server actually owns (a merged item's watch key when present).
    let key = rating_key.clone();
    let server_key = config::update(move |cfg| Ok(crate::recents::hide(cfg, &key)))?;
    // Server-side removal is best-effort: the tombstone above already
    // guarantees the UX, and an unroutable key (source removed) is fine.
    // Route in its own statement so the registry guard drops BEFORE the
    // network call — an if-let scrutinee would hold the lock across the
    // await and stall every registry user behind a slow server.
    let routed = state.registry.lock().await.route(&server_key);
    if let Ok((src, raw)) = routed {
        let _ = src.remove_from_continue(&raw).await;
    }
    Ok(())
}

/// Rating keys the user removed from Continue Watching — the hero merge
/// filters these out of both feeds.
#[tauri::command]
pub async fn get_continue_tombstones() -> Result<Vec<String>, String> {
    let cfg =
        config::load_config().map_err(|_| "could not read Vela settings".to_string())?;
    Ok(cfg.hidden_from_continue.clone())
}

/// Internal helper: kill any prior player, route+resolve, and launch the new mpv.
/// All play contexts use the same lock discipline regardless of trigger.
struct PlaybackSelection {
    source_id: String,
    raw_item_key: String,
    version_id: Option<String>,
    item: ItemDto,
}

enum PlaybackSelectionOutcome {
    Ready {
        selection: Box<PlaybackSelection>,
        ask_mode: bool,
    },
    Choice(Vec<PlaybackSourceChoiceDto>),
}

#[derive(Debug, PartialEq, Eq)]
enum AskSourceDecision {
    UseSource(String),
    Prompt,
    NoCandidate,
}

fn playback_backings(item: &ItemDto) -> Vec<BackingRef> {
    let mut backings = item.backing.clone().unwrap_or_default();
    let face = crate::source::backing_ref_of(item);
    if !backings.contains(&face) {
        backings.push(face);
    }
    backings.sort_by(|left, right| {
        left.source_id
            .cmp(&right.source_id)
            .then_with(|| left.rating_key.cmp(&right.rating_key))
    });
    backings.dedup_by(|left, right| {
        left.source_id == right.source_id && left.rating_key == right.rating_key
    });
    backings
}

fn backing_for_namespaced_key(key: &str) -> Option<BackingRef> {
    let (source_id, _) = key.split_once(':')?;
    Some(BackingRef {
        source_id: source_id.to_string(),
        rating_key: key.to_string(),
        parent_rating_key: None,
        grandparent_rating_key: None,
    })
}

fn dedup_watch_backings(backings: &mut Vec<BackingRef>) {
    backings.sort_by(|left, right| {
        left.source_id
            .cmp(&right.source_id)
            .then_with(|| left.rating_key.cmp(&right.rating_key))
    });
    backings.dedup_by(|left, right| {
        left.source_id == right.source_id && left.rating_key == right.rating_key
    });
}

/// Every title identity eligible for a watched-state mutation. Older merged
/// snapshots may carry a distinct watch key without a complete backing list,
/// so preserve it explicitly in addition to the face and backing set.
fn watched_state_backings(item: &ItemDto) -> Vec<BackingRef> {
    let mut backings = playback_backings(item);
    if let Some(watch_backing) = item
        .watch_key
        .as_deref()
        .and_then(backing_for_namespaced_key)
    {
        backings.push(watch_backing);
    }
    dedup_watch_backings(&mut backings);
    backings
}

fn completion_watch_backings(completion: &PlaybackCompletion) -> Vec<BackingRef> {
    let mut backings = completion.watch_backings.clone();
    if let Some(face) = backing_for_namespaced_key(&completion.item_key) {
        backings.push(face);
    }
    if let Some(watch) = completion
        .watch_key
        .as_deref()
        .and_then(backing_for_namespaced_key)
    {
        backings.push(watch);
    }
    dedup_watch_backings(&mut backings);
    backings
}

async fn playback_compatibility_target(
    state: &AppState,
    replace_session: Option<&str>,
    cfg: &config::AppConfig,
) -> Result<Option<crate::selection::CompatibilityTarget>, PlayFailure> {
    let Some(app) = state.app_handle.get() else {
        return Ok(None);
    };
    let observed = {
        let current = state
            .playback_window_session
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        current
            .as_ref()
            .filter(|session| replace_session == Some(session.session_id.as_str()))
            .map(|session| session.observation.display_snapshot())
    };
    let detected = display::detect_profile(app, observed).await;
    let overrides = playback_display_overrides(cfg).map_err(PlayFailure::fatal)?;
    let effective = display::apply_overrides(&detected, overrides);
    Ok(Some(crate::selection::CompatibilityTarget {
        width: effective.width_px,
        height: effective.height_px,
        hdr: effective.hdr,
    }))
}

fn playback_source_choices(
    candidates: &[crate::selection::PlaybackCandidate],
    versions: &[PlaybackVersion],
) -> Vec<PlaybackSourceChoiceDto> {
    let mut source_ids = candidates
        .iter()
        .map(|candidate| candidate.source_id.clone())
        .collect::<Vec<_>>();
    source_ids.sort();
    source_ids.dedup();
    let mut choices = Vec::with_capacity(source_ids.len());
    for source_id in source_ids {
        let mut source_candidates = candidates
            .iter()
            .filter(|candidate| candidate.source_id == source_id)
            .cloned()
            .collect::<Vec<_>>();
        crate::selection::rank_candidates(
            &mut source_candidates,
            crate::selection::PlaybackSourcePolicy::Best,
            None,
        );
        let Some(best) = source_candidates.first() else {
            continue;
        };
        let source_name = versions
            .iter()
            .find(|version| version.source_id == source_id)
            .map(|version| version.source_name.clone())
            .unwrap_or_else(|| source_id.clone());
        let locality = match best.locality {
            crate::locality::EndpointLocality::SameMachine => "same-machine",
            crate::locality::EndpointLocality::Lan => "lan",
            crate::locality::EndpointLocality::Internet => "internet",
        };
        let resolution = if best.width > 0 && best.height > 0 {
            format!("{}×{}", best.width, best.height)
        } else {
            "Unknown resolution".to_string()
        };
        choices.push(PlaybackSourceChoiceDto {
            source_id,
            source_name,
            locality: locality.to_string(),
            quality_label: if best.hdr {
                format!("{resolution} · HDR")
            } else {
                format!("{resolution} · SDR")
            },
        });
    }
    choices.sort_by(|left, right| {
        left.source_name
            .to_lowercase()
            .cmp(&right.source_name.to_lowercase())
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    choices
}

fn ask_source_decision(
    candidates: &[crate::selection::PlaybackCandidate],
    affinity_source_id: Option<&str>,
) -> AskSourceDecision {
    let mut source_ids = candidates
        .iter()
        .map(|candidate| candidate.source_id.as_str())
        .collect::<Vec<_>>();
    source_ids.sort_unstable();
    source_ids.dedup();
    if let Some(affinity) = affinity_source_id.filter(|affinity| source_ids.contains(affinity)) {
        return AskSourceDecision::UseSource(affinity.to_string());
    }
    match source_ids.as_slice() {
        [] => AskSourceDecision::NoCandidate,
        [source_id] => AskSourceDecision::UseSource((*source_id).to_string()),
        [_, _, ..] => AskSourceDecision::Prompt,
    }
}

fn next_playback_affinity(
    ask_mode: bool,
    run_kind: Option<PlaybackRunKind>,
    explicit_source_id: Option<&str>,
    prior_affinity: Option<&str>,
    selected_source_id: &str,
) -> Option<String> {
    (ask_mode
        && run_kind.is_some()
        && (explicit_source_id.is_some() || prior_affinity.is_some()))
    .then(|| selected_source_id.to_string())
}

async fn select_playback_version(
    state: &AppState,
    item: &ItemDto,
    replace_session: Option<&str>,
    explicit_source_id: Option<&str>,
    affinity_source_id: Option<&str>,
    server_owned: bool,
) -> Result<PlaybackSelectionOutcome, PlayFailure> {
    const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

    let cfg = config::load_config().map_err(|_| {
        PlayFailure::fatal("couldn't read the saved playback preferences".to_string())
    })?;
    let policy = crate::selection::PlaybackSourcePolicy::from_config(
        cfg.playback_source_policy.as_deref(),
    )
    .map_err(PlayFailure::fatal)?;
    let override_source = (explicit_source_id.is_none()
        && policy != crate::selection::PlaybackSourcePolicy::Ask)
        .then(|| {
            item.canonical_id
                .as_ref()
                .and_then(|canonical| cfg.merged_overrides.get(canonical))
                .cloned()
        })
        .flatten();
    let preferred_source = explicit_source_id.or(override_source.as_deref());
    let mut backings = if server_owned {
        vec![crate::source::backing_ref_of(item)]
    } else {
        playback_backings(item)
    };
    if let Some(source_id) = preferred_source {
        backings.retain(|backing| backing.source_id == source_id);
        if backings.is_empty() {
            let source_name = state
                .registry
                .lock()
                .await
                .get(source_id)
                .map(|source| source.name())
                .unwrap_or_else(|| source_id.to_string());
            return Err(PlayFailure::unavailable(format!(
                "the preferred source “{source_name}” no longer has this title"
            )));
        }
    }

    let routed = {
        let registry = state.registry.lock().await;
        backings
            .into_iter()
            .filter_map(|backing| {
                registry
                    .route(&backing.rating_key)
                    .ok()
                    .map(|(source, raw)| (backing, source, raw))
            })
            .collect::<Vec<_>>()
    };
    if routed.is_empty() {
        return Err(PlayFailure::unavailable(
            "no configured source can play this title".to_string(),
        ));
    }

    let mut probes = tokio::task::JoinSet::new();
    for (backing, source, raw) in routed {
        probes.spawn(async move {
            let result = tokio::time::timeout(PROBE_TIMEOUT, source.playback_versions(&raw)).await;
            (backing, source, raw, result)
        });
    }

    let mut versions: Vec<PlaybackVersion> = Vec::new();
    let mut legacy = Vec::new();
    let mut probed_sources: HashMap<String, String> = HashMap::new();
    while let Some(joined) = probes.join_next().await {
        let Ok((backing, source, raw, result)) = joined else {
            continue;
        };
        match result {
            Ok(Ok(found)) if found.is_empty() => {
                legacy.push((backing, source, raw));
            }
            Ok(Ok(found)) => {
                probed_sources.insert(source.id(), raw);
                versions.extend(found);
            }
            Ok(Err(_)) | Err(_) => {}
        }
    }

    if versions.is_empty() {
        legacy.sort_by(|left, right| left.0.source_id.cmp(&right.0.source_id));
        let Some((backing, _source, raw)) = legacy.into_iter().next() else {
            let name = if let Some(source_id) = preferred_source {
                state
                    .registry
                    .lock()
                    .await
                    .get(source_id)
                    .map(|source| format!(" on “{}”", source.name()))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            return Err(PlayFailure::unavailable(format!(
                "no reachable playable copy was found{name}"
            )));
        };
        let mut selected_item = item.clone();
        selected_item.rating_key = backing.rating_key;
        selected_item.source_id = backing.source_id.clone();
        return Ok(PlaybackSelectionOutcome::Ready {
            selection: Box::new(PlaybackSelection {
            source_id: backing.source_id,
            raw_item_key: raw,
            version_id: None,
            item: selected_item,
            }),
            ask_mode: policy == crate::selection::PlaybackSourcePolicy::Ask,
        });
    }

    let mut locality_cache: HashMap<(String, bool), crate::locality::EndpointLocality> =
        HashMap::new();
    let mut candidates = Vec::with_capacity(versions.len());
    for version in &versions {
        let key = (
            version.endpoint.as_str().to_string(),
            version.provider_verified_local,
        );
        let locality = if let Some(locality) = locality_cache.get(&key) {
            *locality
        } else {
            let locality = crate::locality::classify_endpoint(
                &version.endpoint,
                version.provider_verified_local,
            )
            .await;
            locality_cache.insert(key, locality);
            locality
        };
        candidates.push(crate::selection::PlaybackCandidate {
            source_id: version.source_id.clone(),
            version_id: version.version_id.clone(),
            width: version.width,
            height: version.height,
            hdr: version.hdr,
            bitrate: version.bitrate,
            direct_play_rank: version.direct_play_rank,
            locality,
        });
    }
    if policy == crate::selection::PlaybackSourcePolicy::Ask && explicit_source_id.is_none() {
        match ask_source_decision(&candidates, affinity_source_id) {
            AskSourceDecision::UseSource(source_id) => {
                candidates.retain(|candidate| candidate.source_id == source_id);
            }
            AskSourceDecision::Prompt => {
                let choices = playback_source_choices(&candidates, &versions);
                return Ok(PlaybackSelectionOutcome::Choice(choices));
            }
            AskSourceDecision::NoCandidate => {}
        }
    }
    let target = if policy == crate::selection::PlaybackSourcePolicy::Compatible {
        playback_compatibility_target(state, replace_session, &cfg).await?
    } else {
        None
    };
    crate::selection::rank_candidates(&mut candidates, policy, target);
    let winner = candidates
        .first()
        .ok_or_else(|| PlayFailure::unavailable("no playable copy was found".to_string()))?;
    let version = versions
        .into_iter()
        .find(|version| {
            version.source_id == winner.source_id && version.version_id == winner.version_id
        })
        .ok_or_else(|| PlayFailure::fatal("playback selection lost its source".to_string()))?;
    let raw_item_key = probed_sources
        .remove(&version.source_id)
        .ok_or_else(|| PlayFailure::fatal("playback selection lost its route".to_string()))?;
    let mut selected_item = item.clone();
    selected_item.rating_key = version.item_key;
    selected_item.source_id = version.source_id.clone();
    Ok(PlaybackSelectionOutcome::Ready {
        selection: Box::new(PlaybackSelection {
            source_id: version.source_id,
            raw_item_key,
            version_id: Some(version.version_id),
            item: selected_item,
        }),
        ask_mode: policy == crate::selection::PlaybackSourcePolicy::Ask,
    })
}

struct PlayLaunchRequest<'a> {
    item: &'a ItemDto,
    start_from_beginning: bool,
    session_id: &'a str,
    playlist: Option<PlaylistLocation>,
    replace_session: Option<&'a str>,
    run_kind: Option<PlaybackRunKind>,
    explicit_source_id: Option<&'a str>,
    persist_explicit_choice: bool,
    /// A one-off quality chosen from the title's own menu. It applies to this
    /// play and is never written to config — the situation changes, not the
    /// title (`.agents/decisions.md`, 2026-07-25). `None` uses the setting.
    quality_override: Option<&'a str>,
    /// Start here exactly, ignoring both resume sources. Only an Automatic
    /// step-down sets it, to resume where the replaced player actually was.
    resume_override_ms: Option<u64>,
    /// A short OSD line explaining a play the user did not start.
    osd_notice: Option<String>,
    /// Steps this logical play has already taken (see `PlaySpec::steps_taken`).
    steps_taken: u32,
}

async fn play_by_key(
    state: &AppState,
    request: PlayLaunchRequest<'_>,
) -> Result<PlayCommandResult, PlayFailure> {
    // Serialize the whole resolve+stop-old+spawn sequence so overlapping triggers
    // can't both spawn an mpv and lose one of the child handles.
    let _play = state.play_lock.lock().await;
    play_by_key_locked(state, request).await
}

async fn play_by_key_locked(
    state: &AppState,
    request: PlayLaunchRequest<'_>,
) -> Result<PlayCommandResult, PlayFailure> {
    let PlayLaunchRequest {
        item,
        start_from_beginning,
        session_id,
        playlist,
        replace_session,
        run_kind,
        explicit_source_id,
        persist_explicit_choice,
        quality_override,
        resume_override_ms,
        osd_notice,
        steps_taken,
    } = request;
    if !validate_playback_session(state, replace_session).await {
        return Err(PlayFailure::superseded());
    }
    if replace_session.is_none() {
        state.playback_choices.lock().await.clear();
    }
    let prior_affinity = if let (Some(expected), Some(kind)) = (replace_session, run_kind) {
        state
            .playback_run
            .lock()
            .await
            .as_ref()
            .filter(|run| run.session_id == expected && run.kind == kind)
            .and_then(|run| run.affinity_source_id.clone())
    } else {
        None
    };
    let server_owned = run_kind == Some(PlaybackRunKind::ServerPlaylist);
    let selection = select_playback_version(
        state,
        item,
        replace_session,
        explicit_source_id,
        prior_affinity.as_deref(),
        server_owned,
    )
    .await?;
    let (selection, ask_mode) = match selection {
        PlaybackSelectionOutcome::Ready {
            selection,
            ask_mode,
        } => (*selection, ask_mode),
        PlaybackSelectionOutcome::Choice(choices) => {
            let request = PlaybackSourceChoiceRequestDto {
                request_id: uuid::Uuid::new_v4().to_string(),
                title: item.title.clone(),
                choices,
            };
            state.playback_choices.lock().await.insert_at(
                PendingPlaybackChoice {
                    request: request.clone(),
                    created_at: Instant::now(),
                    item: item.clone(),
                    start_from_beginning,
                    session_id: session_id.to_string(),
                    playlist,
                    replace_session: replace_session.map(str::to_string),
                    run_kind,
                },
                Instant::now(),
            );
            return Ok(PlayCommandResult::SourceChoiceRequired { request });
        }
    };
    if persist_explicit_choice && !ask_mode {
        if let (Some(source_id), Some(canonical_id)) =
            (explicit_source_id, item.canonical_id.as_deref())
        {
            let canonical_id = canonical_id.to_string();
            let source_id = source_id.to_string();
            config::update(move |cfg| {
                cfg.merged_overrides.insert(canonical_id, source_id);
                Ok(())
            })
            .map_err(PlayFailure::fatal)?;
        }
    }
    let next_affinity = next_playback_affinity(
        ask_mode,
        run_kind,
        explicit_source_id,
        prior_affinity.as_deref(),
        &selection.source_id,
    );
    let src = state
        .registry
        .lock()
        .await
        .get(&selection.source_id)
        .ok_or_else(|| PlayFailure::unavailable("the selected source was removed".to_string()))?;
    let item = &selection.item;
    // Resolve the skip policies before stream resolution: with every kind Off
    // the server is never asked for markers at all.
    let skip_policies = SkipPolicies::load()?;
    let include_markers = skip_policies.any_enabled();
    // The quality for this play, resolved once here so the sources never read
    // config themselves. A one-off menu choice wins for this launch only; it is
    // validated against the same closed set the setting uses, so a value the
    // frontend invented can never reach a source, and it is NEVER persisted.
    let quality = match quality_override.filter(|value| config::is_playback_quality(value)) {
        Some(chosen) => chosen.to_string(),
        None => config::load_config()
            .map(|cfg| config::playback_quality(cfg.playback_quality.as_deref()))
            .unwrap_or_else(|_| config::PLAYBACK_QUALITY_ORIGINAL.to_string()),
    };
    let resolved = match selection.version_id.as_deref() {
        Some(version_id) => {
            src.resolve_stream_version(
                &selection.raw_item_key,
                item.duration_ms,
                version_id,
                include_markers,
                &quality,
            )
            .await
        }
        None => src
            .resolve_stream(
                &selection.raw_item_key,
                item.duration_ms,
                include_markers,
                &quality,
            )
            .await,
    }
    .map_err(PlayFailure::unavailable)?;

    // Resolve first so the completed process can deliver its final IPC events
    // while a slow server prepares the successor. Only an exact automatic
    // replacement may sample the published observation; manual plays (`None`)
    // deliberately start from configured defaults.
    let (inherited_window_state, inherited_screen_name) = {
        let current = state
            .playback_window_session
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        (
            inherited_window_state(current.as_ref(), replace_session),
            inherited_screen_name(current.as_ref(), replace_session),
        )
    };
    let screen_name = if inherited_screen_name.is_some() {
        inherited_screen_name
    } else if let Some(app) = state.app_handle.get() {
        display::current_screen_name(app).await
    } else {
        None
    };
    let window_observation = playback::WindowStateObservation::default();

    // A failed resolve leaves the currently-playing context intact. Only once
    // the replacement has a real stream do we provisionally install its
    // context. The play lock keeps the expected session valid between the
    // check above and this installation.
    let new_cursor = playlist.map(|location| PlaylistCursor {
        owner: location.owner,
        playlist_id: location.playlist_id,
        entry_id: location.entry_id,
        index: location.index,
        session_id: session_id.to_string(),
    });
    let previous_cursor = {
        let mut cursor = state.playlist_cursor.lock().await;
        std::mem::replace(&mut *cursor, new_cursor)
    };
    install_playback_session(state, session_id, item, run_kind, next_affinity).await;

    // Cancel the prior tracker and terminate the prior mpv so we never run two
    // players. The kill is a non-blocking syscall; the reap is handed to the
    // periodic reaper via reap_queue.
    if let Some(prev) = state
        .tracking_stop
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        prev.store(true, Ordering::Relaxed);
    }
    let prev_child = state
        .current_child
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    let mut prior_include_error = None;
    if let Some(mut child) = prev_child {
        if let Err(error) = child.remove_consumed_header_include() {
            prior_include_error = Some(error);
        }
        let _ = child.kill();
        state
            .reap_queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(child);
    }

    // Sources that keep no server-side progress (the local family) resolve
    // resume_ms 0 — fall back to Vela's own stamped position so a Continue
    // Watching click actually continues (2026-07-04 hero decision). A server
    // that supplies a position still wins: it is the watch-state authority.
    let local_resume_ms = if !start_from_beginning && resolved.resume_ms == 0 {
        let cfg = config::load_config()
            .map_err(|_| PlayFailure::fatal("could not read Vela settings".to_string()))?;
        crate::recents::resume_stamp_ms(&cfg, &item.rating_key)
    } else {
        0
    };
    // An Automatic step-down resumes at the position the replaced player had
    // actually reached — the server's stamp lags it by up to a report interval,
    // and resuming even a few seconds back is a visible stutter in what should
    // look like the picture quality changing.
    let resume_ms = match resume_override_ms {
        Some(exact) => exact,
        None => playback_start_ms(start_from_beginning, resolved.resume_ms, local_resume_ms),
    };
    // Resolve the bundled mpv autocrop script here (the command layer holds the
    // AppHandle; `playback::play` does not). Whether it's injected depends on the
    // `mpv_autocrop` config mode, decided in `play`. `resolve` only computes the
    // path; `play` existence-checks before use.
    let resolve_resource = |name: &str| {
        state.app_handle.get().and_then(|app| {
            use tauri::Manager;
            app.path()
                .resolve(
                    format!("mpv-scripts/{name}"),
                    tauri::path::BaseDirectory::Resource,
                )
                .ok()
                .map(|p| p.to_string_lossy().into_owned())
        })
    };
    let autocrop_script = resolve_resource("autocrop.lua");
    // Vela's trigger shim rides next to the stock script (auto mode disables
    // the stock trigger and lets the shim fire detection after a settle
    // delay — the stock trigger breaks on --start resumes; see the shim).
    let autocrop_shim = resolve_resource("vela-autocrop.lua");
    // Grab the teardown handle before `resolved` is consumed below. Whatever
    // started a transcode is obliged to stop it: neither server has a
    // keep-alive, and how an abandoned session expires is unknown.
    let transcode_session = resolved.transcode_session.clone();
    // Own the session in app state from here on. The end callback alone is not
    // enough: app exit kills mpv and returns without waiting for any tracker
    // tail, so a teardown that only exists inside that callback — or in a task
    // detached from it — can be lost with the runtime and leave an encoder
    // running on the user's server (finding `tr-4`). Registering here also
    // covers the launch-failure path below, which never reaches the callback.
    if let Some(session) = transcode_session.clone() {
        if let Some(replaced) = register_active_transcode(&state.active_transcode, &src, session) {
            // A play that supersedes another must stop the encoder it
            // supersedes; the old tracker's tail can no longer claim it.
            stop_transcode_record(replaced).await;
        }
    }
    let markers_script = resolve_resource("vela-markers.lua");
    // Keep only the kinds the user actually enabled: the script must never be
    // handed a range it is not allowed to act on.
    let markers: Vec<crate::source::MediaMarker> = resolved
        .markers
        .into_iter()
        .filter(|marker| !skip_policy_for(marker.kind, &skip_policies).is_off())
        .collect();
    let spec = playback::PlaySpec {
        url: resolved.url,
        title: item.title.clone(),
        http_headers: resolved.http_headers,
        start_seconds: resume_ms as f64 / 1000.0,
        autocrop_script,
        autocrop_shim,
        inherited_window_state,
        screen_name,
        window_observation: window_observation.clone(),
        markers_script,
        markers,
        intro_policy: skip_policies.intro,
        credits_policy: skip_policies.credits,
        commercial_policy: skip_policies.commercial,
        // Automatic only. Whether a lower tier actually exists is NOT checked
        // here: that needs a decision round trip per play, and a verdict is
        // rare — `apply_step_down` resolves the ladder when one actually
        // arrives, and declines if there is nowhere to go.
        step_down: (quality == config::PLAYBACK_QUALITY_AUTOMATIC).then(|| {
            let queue = state.step_down.clone();
            let session = session_id.to_string();
            let already = steps_taken;
            std::sync::Arc::new(move |position_ms: u64, reason| {
                queue.request(StepDownRequest {
                    session_id: session.clone(),
                    position_ms,
                    reason,
                    steps_taken: already,
                });
            }) as playback::StepDownNotify
        }),
        duration: item.duration_ms.map(Duration::from_millis),
        osd_notice,
        steps_taken,
    };
    // The tracker can observe a very fast mpv exit before this async command
    // resumes from spawn_blocking. Hold its final recents stamp until the
    // post-launch record attempt has completed, or finish() could run first
    // and leave a permanently unstamped open entry behind.
    let recents_ready = std::sync::Arc::new(PlayStartGate::default());
    let playback_started_at_ms = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let _release_recents_on_return = OpenPlayStartGateOnDrop(recents_ready.clone());
    // Every tracker tail publishes a refresh after its final sampled server
    // check-in. That keeps quit/error progress current; a joined clean EOF gets
    // a second, authoritative refresh from the async dispatcher after exact-
    // session completion curation and any backend-owned successor start.
    // Payloads carry ids only — never URLs or tokens.
    let on_end: Option<playback::EndNotify> = {
        use tauri::Emitter;
        let app = state.app_handle.get().cloned();
        let source_id = src.id().to_string();
        let item_key = item.rating_key.clone();
        let watch_key = item.watch_key.clone();
        let watch_backings = watched_state_backings(item);
        let media_type = item.media_type.clone();
        let session_id = session_id.to_string();
        let recents_ready = recents_ready.clone();
        let playback_started_at_ms = playback_started_at_ms.clone();
        let advance = state.playback_advance.clone();
        let transcode_slot = state.active_transcode.clone();
        let transcode_session = transcode_session.clone();
        Some(std::sync::Arc::new(move |position_ms: u64| {
            // Stop the server-side transcode first: it costs the user's server
            // real work, and the recents/event bookkeeping below must not be
            // able to skip it by returning early. This runs on the tracker's
            // own thread, outside any runtime, so it BLOCKS until the DELETE
            // lands or its deadline expires rather than detaching a task the
            // process may outlive.
            if let Some(session) = transcode_session.as_deref() {
                if let Some(record) = take_active_transcode(&transcode_slot, session) {
                    stop_transcode_record_blocking(record);
                }
            }
            if !recents_ready.wait_succeeded() {
                return;
            }
            // Stamp Vela's recents BEFORE emitting, so the refresh the event
            // triggers reads the updated list. Runs on the tracker thread —
            // synchronous config I/O is fine there.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let key = item_key.clone();
            let finishing_session = session_id.clone();
            let _ = config::update(move |cfg| {
                crate::recents::finish_session(cfg, &key, &finishing_session, position_ms, now_ms);
                Ok(())
            });
            if let Some(app) = &app {
                let _ = app.emit(
                    "playback-ended",
                    serde_json::json!({ "sourceId": source_id, "itemKey": item_key.clone() }),
                );
            }
            advance.mark_ended(PlaybackCompletion {
                session_id: session_id.clone(),
                item_key: item_key.clone(),
                watch_key: watch_key.clone(),
                started_at_ms: playback_started_at_ms.load(Ordering::SeqCst),
                watch_backings: watch_backings.clone(),
                media_type: media_type.clone(),
            });
        }) as playback::EndNotify)
    };
    let progress = resolved.progress;
    let child_slot = state.current_child.clone();
    let shutting_down = state.shutting_down.clone();
    let advance = state.playback_advance.clone();
    let playback_session = session_id.to_string();
    let played = if let Some(error) = prior_include_error {
        Err(error)
    } else {
        tauri::async_runtime::spawn_blocking(move || {
            playback::play(
                &spec,
                progress,
                &child_slot,
                &shutting_down,
                &advance,
                playback_session,
                on_end,
            )
        })
        .await
        .map_err(|e| format!("playback task failed: {e}"))
        .and_then(|r| r)
    };
    let recent = item.clone();
    let recent_session = session_id.to_string();
    let completion_started_at_ms = playback_started_at_ms.clone();
    let launched_window_session = PlaybackWindowSession {
        session_id: session_id.to_string(),
        observation: window_observation,
    };
    let played = after_successful_play(played, || {
        *state
            .playback_window_session
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(launched_window_session);
        let started_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        completion_started_at_ms.store(started_at_ms, Ordering::SeqCst);
        if let Err(e) = config::update(move |cfg| {
            crate::recents::record_play_start(
                cfg,
                recent,
                start_from_beginning,
                recent_session,
                started_at_ms,
            );
            Ok(())
        }) {
            eprintln!("vela: couldn't record started playback: {e}");
        }
    });
    let stop = match played {
        Ok(stop) => stop,
        Err(message) => {
            // mpv never started, so no tracker tail will ever run: this is the
            // only chance to stop what the resolve already committed us to.
            if let Some(session) = transcode_session.as_deref() {
                if let Some(record) = take_active_transcode(&state.active_transcode, session) {
                    stop_transcode_record(record).await;
                }
            }
            // The old player has already been terminated. Restore its cursor
            // bookkeeping for the established failure contract, but never
            // re-authorize its dead session for delayed continuation work.
            clear_playback_session_if(state, session_id).await;
            let mut cursor = state.playlist_cursor.lock().await;
            if cursor
                .as_ref()
                .is_none_or(|current| current.session_id == session_id)
            {
                *cursor = previous_cursor;
            }
            return Err(PlayFailure::fatal(message));
        }
    };
    *state
        .tracking_stop
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(stop);
    recents_ready.succeed();
    Ok(PlayCommandResult::Started {
        session_id: session_id.to_string(),
    })
}

/// Play one item, either resuming its resolved position or explicitly starting
/// from the beginning. A single item has no persistent sequence context.
#[tauri::command]
pub async fn play_item(
    item: ItemDto,
    start_from_beginning: bool,
    expected_session: Option<String>,
    series_continuation: Option<bool>,
    explicit_source_id: Option<String>,
    // A one-off choice from the title's own quality menu. It governs this
    // play only and is never stored.
    quality: Option<String>,
    state: State<'_, AppState>,
) -> Result<PlayCommandResult, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    match play_by_key(
        &state,
        PlayLaunchRequest {
            item: &item,
            start_from_beginning,
            session_id: &session_id,
            playlist: None,
            replace_session: expected_session.as_deref(),
            run_kind: series_continuation
                .unwrap_or(false)
                .then_some(PlaybackRunKind::Series),
            explicit_source_id: explicit_source_id.as_deref(),
            persist_explicit_choice: explicit_source_id.is_some(),
            quality_override: quality.as_deref(),
            resume_override_ms: None,
            osd_notice: None,
            steps_taken: 0,
        },
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(failure) if failure.kind == PlayFailureKind::Superseded => {
            Ok(PlayCommandResult::Superseded)
        }
        Err(failure) => Err(failure.message),
    }
}

#[tauri::command]
pub async fn get_playback_source_choice(
    request_id: String,
    state: State<'_, AppState>,
) -> Result<PlaybackSourceChoiceRequestDto, String> {
    state
        .playback_choices
        .lock()
        .await
        .request_at(&request_id, Instant::now())
        .ok_or_else(|| "this source choice expired or was superseded".to_string())
}

#[tauri::command]
pub async fn resolve_playback_source_choice(
    request_id: String,
    source_id: String,
    state: State<'_, AppState>,
) -> Result<PlayCommandResult, String> {
    let _play = state.play_lock.lock().await;
    let pending = state
        .playback_choices
        .lock()
        .await
        .take_at(&request_id, Instant::now())
        .ok_or_else(|| "this source choice expired or was superseded".to_string())?;
    if !pending
        .request
        .choices
        .iter()
        .any(|choice| choice.source_id == source_id)
    {
        return Err("that source was not offered for this playback request".to_string());
    }
    match play_by_key_locked(
        &state,
        PlayLaunchRequest {
            item: &pending.item,
            start_from_beginning: pending.start_from_beginning,
            session_id: &pending.session_id,
            playlist: pending.playlist,
            replace_session: pending.replace_session.as_deref(),
            run_kind: pending.run_kind,
            explicit_source_id: Some(&source_id),
            persist_explicit_choice: false,
            quality_override: None,
            resume_override_ms: None,
            osd_notice: None,
            steps_taken: 0,
        },
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(failure) if failure.kind == PlayFailureKind::Superseded => {
            Ok(PlayCommandResult::Superseded)
        }
        Err(failure) => Err(failure.message),
    }
}

#[tauri::command]
pub async fn cancel_playback_source_choice(
    request_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let _play = state.play_lock.lock().await;
    let Some(pending) = state
        .playback_choices
        .lock()
        .await
        .take_at(&request_id, Instant::now())
    else {
        return Ok(false);
    };
    if let Some(expected) = pending.replace_session.as_deref() {
        clear_playlist_cursor_if(&state, expected).await;
        let mut run = state.playback_run.lock().await;
        if run.as_ref().is_some_and(|run| run.session_id == expected) {
            *run = None;
        }
    }
    Ok(true)
}

#[tauri::command]
pub async fn finish_playback_run(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let _play = state.play_lock.lock().await;
    let mut run = state.playback_run.lock().await;
    let cleared = run
        .as_ref()
        .is_some_and(|run| run.session_id == session_id);
    if cleared {
        *run = None;
    }
    drop(run);
    state
        .playback_choices
        .lock()
        .await
        .clear_for_session(&session_id);
    Ok(cleared)
}

const EPISODE_PAGE_SIZE: usize = 200;

async fn all_children(
    source: &std::sync::Arc<dyn MediaSource>,
    raw_key: &str,
) -> Result<Vec<ItemDto>, String> {
    let mut start = 0;
    let mut all = Vec::new();
    let mut previous_signature: Option<(usize, String, String)> = None;
    loop {
        let page = source.children(raw_key, start, EPISODE_PAGE_SIZE).await?;
        let count = page.len();
        if count == 0 {
            break;
        }
        let signature = (
            count,
            page.first()
                .map(|item| item.rating_key.clone())
                .unwrap_or_default(),
            page.last()
                .map(|item| item.rating_key.clone())
                .unwrap_or_default(),
        );
        if start > 0 && previous_signature.as_ref() == Some(&signature) {
            return Err("the server did not advance episode pagination".to_string());
        }
        previous_signature = Some(signature);
        all.extend(page);
        start += count;
        if count < EPISODE_PAGE_SIZE {
            break;
        }
    }
    Ok(all)
}

fn episode_item(item: &ItemDto) -> bool {
    item.media_type.as_deref() == Some("episode")
}

fn choose_next_episode(
    context: &EpisodeContext,
    seasons: &[(ItemDto, Vec<ItemDto>)],
) -> Option<ItemDto> {
    let current_season = seasons
        .iter()
        .position(|(season, _)| context.season_key.as_deref() == Some(&season.rating_key))
        .or_else(|| {
            context.season_index.and_then(|index| {
                seasons
                    .iter()
                    .position(|(season, _)| season.index == Some(index))
            })
        })?;
    let started_in_specials = seasons[current_season].0.index.or(context.season_index) == Some(0);

    for (position, (season, episodes)) in seasons.iter().enumerate().skip(current_season) {
        if position > current_season && !started_in_specials && season.index == Some(0) {
            continue;
        }
        if position == current_season {
            let current_episode = episodes
                .iter()
                .position(|episode| episode.rating_key == context.item_key)
                .or_else(|| {
                    context.episode_index.and_then(|index| {
                        episodes
                            .iter()
                            .position(|episode| episode.index == Some(index))
                    })
                })?;
            if let Some(next) = episodes
                .iter()
                .skip(current_episode + 1)
                .find(|episode| episode_item(episode) && episode.rating_key != context.item_key)
            {
                return Some(next.clone());
            }
        } else if let Some(next) = episodes
            .iter()
            .find(|episode| episode_item(episode) && episode.rating_key != context.item_key)
        {
            return Some(next.clone());
        }
    }
    None
}

fn current_season_position(context: &EpisodeContext, seasons: &[ItemDto]) -> Option<usize> {
    seasons
        .iter()
        .position(|season| context.season_key.as_deref() == Some(&season.rating_key))
        .or_else(|| {
            context.season_index.and_then(|index| {
                seasons
                    .iter()
                    .position(|season| season.index == Some(index))
            })
        })
}

fn hierarchy_parent_backings(item: &ItemDto, show: bool) -> Vec<BackingRef> {
    let children = playback_backings(item);
    let mut parents = children
        .into_iter()
        .filter_map(|child| {
            let rating_key = if show {
                child.grandparent_rating_key.clone()
            } else {
                child.parent_rating_key.clone()
            }?;
            Some(BackingRef {
                source_id: child.source_id,
                rating_key,
                parent_rating_key: if show {
                    None
                } else {
                    child.grandparent_rating_key.clone()
                },
                grandparent_rating_key: None,
            })
        })
        .collect::<Vec<_>>();
    parents.sort_by(|left, right| {
        left.source_id
            .cmp(&right.source_id)
            .then_with(|| left.rating_key.cmp(&right.rating_key))
    });
    parents.dedup_by(|left, right| {
        left.source_id == right.source_id && left.rating_key == right.rating_key
    });
    parents
}

fn synthetic_hierarchy_canonical(media_type: &str, backings: &[BackingRef]) -> String {
    format!(
        "{media_type}:{}",
        backings
            .iter()
            .map(|backing| format!("{}:{}", backing.source_id, backing.rating_key))
            .collect::<Vec<_>>()
            .join("|")
    )
}

async fn next_merged_episode(
    state: &AppState,
    current: &ItemDto,
) -> Result<Option<ItemDto>, String> {
    let show_backings = hierarchy_parent_backings(current, true);
    let season_backings = hierarchy_parent_backings(current, false);
    if show_backings.is_empty() || season_backings.is_empty() {
        return Ok(None);
    }
    let show_canonical = synthetic_hierarchy_canonical("show", &show_backings);
    let mut seasons = fetch_merged_children(state, &show_backings, &show_canonical, "show")
        .await?
        .into_iter()
        .filter(|item| item.media_type.as_deref() == Some("season"))
        .collect::<Vec<_>>();
    sort_hierarchy_children(&mut seasons);

    let current_season = seasons
        .iter()
        .position(|season| {
            season.backing.as_ref().is_some_and(|backings| {
                backings.iter().any(|season_backing| {
                    season_backings.iter().any(|current_backing| {
                        season_backing.source_id == current_backing.source_id
                            && season_backing.rating_key == current_backing.rating_key
                    })
                })
            })
        })
        .or_else(|| {
            current.parent_index.and_then(|index| {
                seasons
                    .iter()
                    .position(|season| season.index == Some(index))
            })
        });
    let Some(current_season) = current_season else {
        return Ok(None);
    };

    let context = EpisodeContext {
        item_key: current.rating_key.clone(),
        season_key: current.parent_rating_key.clone(),
        show_key: current.grandparent_rating_key.clone(),
        episode_index: current.index,
        season_index: current.parent_index,
    };
    let started_in_specials = seasons[current_season].index.or(current.parent_index) == Some(0);
    let mut catalog = Vec::new();
    for (position, season) in seasons.into_iter().enumerate().skip(current_season) {
        if position > current_season && !started_in_specials && season.index == Some(0) {
            continue;
        }
        let backings = playback_backings(&season);
        let canonical = season
            .canonical_id
            .clone()
            .unwrap_or_else(|| synthetic_hierarchy_canonical("season", &backings));
        let episodes = fetch_merged_children(state, &backings, &canonical, "season").await?;
        catalog.push((season, episodes));
        if let Some(next) = choose_next_episode(&context, &catalog) {
            return Ok(Some(next));
        }
    }
    Ok(None)
}

async fn next_episode_impl(state: &AppState, item_key: &str) -> Result<Option<ItemDto>, String> {
    let active_item = state
        .active_playback_item
        .lock()
        .await
        .as_ref()
        .map(|(_, item)| item.clone());
    if let Some(item) = active_item.filter(|item| {
        item.rating_key == item_key
            && item.media_type.as_deref() == Some("episode")
            && item.backing.as_ref().is_some_and(|backings| backings.len() > 1)
    }) {
        return next_merged_episode(state, &item).await;
    }
    let (source, raw_item_key) = state.registry.lock().await.route(item_key)?;
    let Some(context) = source.episode_context(&raw_item_key).await? else {
        return Ok(None);
    };
    let (Some(show_key), Some(_season_key)) =
        (context.show_key.as_deref(), context.season_key.as_deref())
    else {
        return Ok(None);
    };

    let (show_source, raw_show_key) = state.registry.lock().await.route(show_key)?;
    if show_source.id() != source.id() {
        return Err("episode hierarchy crossed media sources".to_string());
    }
    let mut seasons = all_children(&show_source, &raw_show_key)
        .await?
        .into_iter()
        .filter(|item| item.media_type.as_deref() == Some("season"))
        .collect::<Vec<_>>();
    seasons.sort_by_key(|season| season.index.unwrap_or(u32::MAX));
    let Some(current_season) = current_season_position(&context, &seasons) else {
        return Ok(None);
    };
    let started_in_specials = seasons[current_season].index.or(context.season_index) == Some(0);

    let mut catalog = Vec::new();
    for (position, season) in seasons.into_iter().enumerate().skip(current_season) {
        if position > current_season && !started_in_specials && season.index == Some(0) {
            continue;
        }
        let (season_source, raw_season_key) =
            state.registry.lock().await.route(&season.rating_key)?;
        if season_source.id() != source.id() {
            return Err("episode hierarchy crossed media sources".to_string());
        }
        let episodes = all_children(&season_source, &raw_season_key).await?;
        catalog.push((season, episodes));
        if let Some(next) = choose_next_episode(&context, &catalog) {
            return Ok(Some(next));
        }
    }
    Ok(None)
}

#[tauri::command]
pub async fn next_episode(
    item_key: String,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<ItemDto>, String> {
    if let Some(expected) = session_id.as_deref() {
        let active = state.active_playback_session.lock().await;
        if active.as_deref() != Some(expected) {
            return Ok(None);
        }
    }
    next_episode_impl(&state, &item_key).await
}

// ---- read-only server playlists -----------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPlaylistGroupDto {
    pub source_id: String,
    pub source_name: String,
    pub source_kind: String,
    /// False only when this source's playlist discovery failed. A successful
    /// empty list remains available and is distinguishable in the UI.
    pub available: bool,
    pub playlists: Vec<crate::source::PlaylistDto>,
}

/// List each source independently so one offline server leaves an unavailable
/// group in place without hiding healthy servers' playlists. Calls run in
/// parallel: several 15-second source timeouts must not serialize into a long
/// frozen sidebar load.
#[tauri::command]
pub async fn get_server_playlists(
    state: State<'_, AppState>,
) -> Result<Vec<ServerPlaylistGroupDto>, String> {
    crate::durable::ensure_commands_ready()?;
    let sources = state.registry.lock().await.all().to_vec();
    let mut groups: Vec<_> = sources
        .iter()
        .map(|source| ServerPlaylistGroupDto {
            source_id: source.id(),
            source_name: source.name(),
            source_kind: source.kind().to_string(),
            available: false,
            playlists: Vec::new(),
        })
        .collect();
    let mut tasks = tokio::task::JoinSet::new();
    for (index, source) in sources.into_iter().enumerate() {
        tasks.spawn(async move { (index, source.playlists().await) });
    }
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((index, Ok(playlists))) => {
                groups[index].available = true;
                groups[index].playlists = playlists;
            }
            Ok((_index, Err(_))) => {}
            Err(error) => eprintln!("vela: server playlist discovery task failed: {error}"),
        }
    }
    Ok(groups)
}

async fn fetch_server_playlist_items(
    state: &AppState,
    playlist_key: &str,
) -> Result<Vec<ItemDto>, String> {
    let (source, raw) = state.registry.lock().await.route(playlist_key)?;
    source.playlist_items(&raw).await
}

#[tauri::command]
pub async fn get_server_playlist_items(
    key: String,
    state: State<'_, AppState>,
) -> Result<Vec<ItemDto>, String> {
    fetch_server_playlist_items(&state, &key).await
}

// ---- Vela playlists ------------------------------------------------------

fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

async fn playlist_store<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| format!("playlist task failed: {error}"))?
}

#[tauri::command]
pub async fn playlist_list() -> Result<Vec<crate::playlists::PlaylistSummary>, String> {
    playlist_store(crate::playlists::list).await
}

#[tauri::command]
pub async fn playlist_get(
    id: String,
    state: State<'_, AppState>,
) -> Result<crate::playlists::PlaylistView, String> {
    crate::durable::ensure_commands_ready()?;
    let lookup = id.clone();
    let playlist = playlist_store(move || crate::playlists::get(&lookup)).await?;
    let live_sources = state.registry.lock().await.ids().into_iter().collect();
    Ok(crate::playlists::view(playlist, &live_sources))
}

#[tauri::command]
pub async fn playlist_create(name: String) -> Result<crate::playlists::Playlist, String> {
    let now_ms = unix_now_ms();
    playlist_store(move || crate::playlists::create(name, now_ms)).await
}

#[tauri::command]
pub async fn playlist_rename(
    id: String,
    name: String,
) -> Result<crate::playlists::Playlist, String> {
    let now_ms = unix_now_ms();
    playlist_store(move || crate::playlists::rename(id, name, now_ms)).await
}

#[tauri::command]
pub async fn playlist_delete(id: String) -> Result<(), String> {
    playlist_store(move || crate::playlists::delete(id)).await
}

#[tauri::command]
pub async fn playlist_add_items(
    id: String,
    items: Vec<ItemDto>,
    state: State<'_, AppState>,
) -> Result<crate::playlists::Playlist, String> {
    let stored_items = {
        let registry = state.registry.lock().await;
        items
            .into_iter()
            .map(|item| {
                let source_name = registry.get(&item.source_id).map(|source| source.name());
                (item, source_name)
            })
            .collect()
    };
    let now_ms = unix_now_ms();
    playlist_store(move || crate::playlists::add_items(id, stored_items, now_ms)).await
}

#[tauri::command]
pub async fn playlist_remove_item(
    id: String,
    entry_id: String,
) -> Result<crate::playlists::Playlist, String> {
    let now_ms = unix_now_ms();
    playlist_store(move || crate::playlists::remove_item(id, entry_id, now_ms)).await
}

#[tauri::command]
pub async fn playlist_reorder(
    id: String,
    entry_id: String,
    to_index: usize,
) -> Result<crate::playlists::Playlist, String> {
    let now_ms = unix_now_ms();
    playlist_store(move || crate::playlists::reorder(id, entry_id, to_index, now_ms)).await
}

async fn clear_playlist_cursor_if(state: &AppState, expected_session: &str) {
    let mut cursor = state.playlist_cursor.lock().await;
    if cursor_matches_session(cursor.as_ref(), expected_session) {
        *cursor = None;
    }
    drop(cursor);
    let mut run = state.playback_run.lock().await;
    if run.as_ref().is_some_and(|run| {
        run.session_id == expected_session
            && matches!(
                run.kind,
                PlaybackRunKind::VelaPlaylist | PlaybackRunKind::ServerPlaylist
            )
    }) {
        *run = None;
    }
}

/// Atomically prove that no newer play replaced this completed session and,
/// for a playlist, clear only its exact terminal cursor. The active session is
/// deliberately retained so the frontend can conditionally replace it with
/// the Continue Playing successor.
async fn finish_sequence_if_current(state: &AppState, ended_session: &str) -> bool {
    let _play = state.play_lock.lock().await;
    if !active_session_matches(state, ended_session).await {
        return false;
    }
    let mut cursor = state.playlist_cursor.lock().await;
    match cursor.as_ref() {
        None => true,
        Some(current) if current.session_id == ended_session => {
            *cursor = None;
            drop(cursor);
            let mut run = state.playback_run.lock().await;
            if run
                .as_ref()
                .is_some_and(|run| run.session_id == ended_session)
            {
                *run = None;
            }
            true
        }
        Some(_) => false,
    }
}

fn next_playlist_index<'a>(
    mut entry_ids: impl Iterator<Item = &'a str>,
    item_count: usize,
    cursor: &PlaylistCursor,
) -> usize {
    entry_ids
        .position(|entry_id| entry_id == cursor.entry_id)
        .map_or(cursor.index.min(item_count), |index| index + 1)
}

async fn play_playlist_entries(
    state: &AppState,
    playlist_id: String,
    items: Vec<crate::playlists::PlaylistEntry>,
    start_index: usize,
    start_from_beginning: bool,
    replace_session: Option<&str>,
) -> Result<PlayCommandResult, String> {
    if start_index >= items.len() {
        return Err("playlist position is out of range".to_string());
    }

    let mut last_unavailable = None;
    for (index, entry) in items.into_iter().enumerate().skip(start_index) {
        let session_id = uuid::Uuid::new_v4().to_string();
        let location = PlaylistLocation {
            owner: PlaylistOwner::Vela,
            playlist_id: playlist_id.clone(),
            entry_id: entry.id,
            index,
        };
        let beginning = start_from_beginning && index == start_index;
        match play_by_key(
            state,
            PlayLaunchRequest {
                item: &entry.item,
                start_from_beginning: beginning,
                session_id: &session_id,
                playlist: Some(location),
                replace_session,
                run_kind: Some(PlaybackRunKind::VelaPlaylist),
                explicit_source_id: None,
                persist_explicit_choice: false,
                quality_override: None,
                resume_override_ms: None,
                osd_notice: None,
                steps_taken: 0,
            },
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(failure) if failure.kind == PlayFailureKind::Unavailable => {
                last_unavailable = Some(failure.message);
            }
            Err(failure) if failure.kind == PlayFailureKind::Superseded => {
                return Ok(PlayCommandResult::Superseded)
            }
            Err(failure) => return Err(failure.message),
        }
    }
    Err(last_unavailable.unwrap_or_else(|| "playlist has no playable items".to_string()))
}

async fn play_playlist_from(
    state: &AppState,
    playlist_id: String,
    start_index: usize,
    start_from_beginning: bool,
    replace_session: Option<&str>,
) -> Result<PlayCommandResult, String> {
    let lookup = playlist_id.clone();
    let playlist = playlist_store(move || crate::playlists::get(&lookup)).await?;
    play_playlist_entries(
        state,
        playlist_id,
        playlist.items,
        start_index,
        start_from_beginning,
        replace_session,
    )
    .await
}

#[tauri::command]
pub async fn playlist_play(
    id: String,
    start_index: usize,
    start_from_beginning: bool,
    state: State<'_, AppState>,
) -> Result<PlayCommandResult, String> {
    play_playlist_from(&state, id, start_index, start_from_beginning, None).await
}

async fn play_server_playlist_entries(
    state: &AppState,
    playlist_key: String,
    items: Vec<ItemDto>,
    start_index: usize,
    start_from_beginning: bool,
    replace_session: Option<&str>,
) -> Result<PlayCommandResult, String> {
    if start_index >= items.len() {
        return Err("server playlist position is out of range".to_string());
    }
    let mut last_unavailable = None;
    for (index, item) in items.into_iter().enumerate().skip(start_index) {
        let session_id = uuid::Uuid::new_v4().to_string();
        let location = PlaylistLocation {
            owner: PlaylistOwner::Server,
            playlist_id: playlist_key.clone(),
            entry_id: item.rating_key.clone(),
            index,
        };
        let beginning = start_from_beginning && index == start_index;
        match play_by_key(
            state,
            PlayLaunchRequest {
                item: &item,
                start_from_beginning: beginning,
                session_id: &session_id,
                playlist: Some(location),
                replace_session,
                run_kind: Some(PlaybackRunKind::ServerPlaylist),
                explicit_source_id: None,
                persist_explicit_choice: false,
                quality_override: None,
                resume_override_ms: None,
                osd_notice: None,
                steps_taken: 0,
            },
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(failure) if failure.kind == PlayFailureKind::Unavailable => {
                last_unavailable = Some(failure.message);
            }
            Err(failure) if failure.kind == PlayFailureKind::Superseded => {
                return Ok(PlayCommandResult::Superseded)
            }
            Err(failure) => return Err(failure.message),
        }
    }
    Err(last_unavailable.unwrap_or_else(|| "server playlist has no playable items".to_string()))
}

#[tauri::command]
pub async fn server_playlist_play(
    key: String,
    start_index: usize,
    start_from_beginning: bool,
    state: State<'_, AppState>,
) -> Result<PlayCommandResult, String> {
    let items = fetch_server_playlist_items(&state, &key).await?;
    play_server_playlist_entries(
        &state,
        key,
        items,
        start_index,
        start_from_beginning,
        None,
    )
    .await
}

/// Persist local completion for a joined clean EOF. The caller holds
/// `watch_edit_lock`, keeping this admission ordered with explicit watched-
/// state edits and the later best-effort server synchronization.
pub(crate) fn admit_clean_completion(completion: &PlaybackCompletion) -> Result<bool, String> {
    let play_key = completion.item_key.clone();
    let watch_key = completion.watch_key.clone();
    let session_id = completion.session_id.clone();
    let started_at_ms = completion.started_at_ms;
    config::update(move |cfg| {
        Ok(crate::recents::complete_clean_session(
            cfg,
            &play_key,
            watch_key.as_deref(),
            &session_id,
            started_at_ms,
        ))
    })
}

/// Best-effort all-backing half of a locally admitted clean completion. The
/// immutable title identities were captured when this exact session launched.
/// The caller retains `watch_edit_lock` across this await; a later user edit
/// therefore wins. A proven clean EOF is never rolled back when every provider
/// is unavailable.
pub(crate) async fn mark_clean_completion_played(
    state: &AppState,
    completion: &PlaybackCompletion,
) -> WatchStateMutationDto {
    let backings = completion_watch_backings(completion);
    let prepared = prepare_watch_mutations(&state.registry, &backings).await;
    execute_watch_mutations(prepared, true).await
}

fn emit_source_choice_required(state: &AppState, request_id: &str) {
    use tauri::Emitter;
    if let Some(app) = state.app_handle.get() {
        let _ = app.emit(
            "source-choice-required",
            serde_json::json!({ "requestId": request_id }),
        );
    }
}

/// Handle one fully-finished clean EOF. Returns true only when the exact active
/// single item or playlist genuinely ended, authorizing the frontend to apply
/// Continue Playing. The UUID prevents queued older work from replacing a
/// newer manual play.
pub(crate) async fn advance_playlist(state: &AppState, ended_session: &str) -> bool {
    if state
        .playback_choices
        .lock()
        .await
        .has_manual_pending(Instant::now())
    {
        return false;
    }
    let cursor = state.playlist_cursor.lock().await.clone();
    let Some(cursor) = cursor else {
        return finish_sequence_if_current(state, ended_session).await;
    };
    if cursor.session_id != ended_session {
        return false;
    };
    if cursor.owner == PlaylistOwner::Server {
        let items = match fetch_server_playlist_items(state, &cursor.playlist_id).await {
            Ok(items) => items,
            Err(_) => {
                clear_playlist_cursor_if(state, ended_session).await;
                return false;
            }
        };
        let next_index = cursor.index.saturating_add(1);
        if next_index >= items.len() {
            return finish_sequence_if_current(state, ended_session).await;
        }
        let result = match play_server_playlist_entries(
            state,
            cursor.playlist_id,
            items,
            next_index,
            false,
            Some(ended_session),
        )
        .await
        {
            Ok(PlayCommandResult::Started { .. } | PlayCommandResult::Superseded) => false,
            Ok(PlayCommandResult::SourceChoiceRequired { request }) => {
                emit_source_choice_required(state, &request.request_id);
                false
            }
            Err(error) => {
                eprintln!("vela: server playlist auto-advance stopped: {error}");
                clear_playlist_cursor_if(state, ended_session).await;
                false
            }
        };
        return result;
    }
    let lookup = cursor.playlist_id.clone();
    let playlist = match playlist_store(move || crate::playlists::get(&lookup)).await {
        Ok(playlist) => playlist,
        Err(_) => {
            clear_playlist_cursor_if(state, ended_session).await;
            return false;
        }
    };
    let next_index = next_playlist_index(
        playlist.items.iter().map(|entry| entry.id.as_str()),
        playlist.items.len(),
        &cursor,
    );
    if next_index >= playlist.items.len() {
        return finish_sequence_if_current(state, ended_session).await;
    }
    // Use the same freshly-read snapshot that produced `next_index`. A second
    // read here would let an edit between the reads invalidate the stable-entry
    // anchor and turn the derived numeric index into a different item.
    match play_playlist_entries(
        state,
        cursor.playlist_id,
        playlist.items,
        next_index,
        false,
        Some(ended_session),
    )
    .await
    {
        Ok(PlayCommandResult::Started { .. } | PlayCommandResult::Superseded) => false,
        Ok(PlayCommandResult::SourceChoiceRequired { request }) => {
            emit_source_choice_required(state, &request.request_id);
            false
        }
        Err(error) => {
            eprintln!("vela: playlist auto-advance stopped: {error}");
            clear_playlist_cursor_if(state, ended_session).await;
            false
        }
    }
}

// ---- helpers -------------------------------------------------------------

/// Choose the mpv start position. An explicit beginning request overrides
/// both server progress and Vela's local fallback; otherwise the server is the
/// watch-state authority and the local stamp fills only a zero server offset.
fn playback_start_ms(
    start_from_beginning: bool,
    server_resume_ms: u64,
    local_resume_ms: u64,
) -> u64 {
    if start_from_beginning {
        0
    } else if server_resume_ms > 0 {
        server_resume_ms
    } else {
        local_resume_ms
    }
}

/// Run a side effect only when playback fully launched. Keeping this boundary
/// explicit prevents a resolve, spawn, or tracker-setup error from mutating
/// recents or Continue Watching tombstones.
fn after_successful_play<T>(
    result: Result<T, String>,
    on_success: impl FnOnce(),
) -> Result<T, String> {
    if result.is_ok() {
        on_success();
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PlayStartState {
    #[default]
    Pending,
    Succeeded,
    Failed,
}

#[derive(Default)]
struct PlayStartGate {
    state: std::sync::Mutex<PlayStartState>,
    changed: std::sync::Condvar,
}

impl PlayStartGate {
    fn wait_succeeded(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while *state == PlayStartState::Pending {
            state = self.changed.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        *state == PlayStartState::Succeeded
    }

    fn finish(&self, next: PlayStartState) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if *state != PlayStartState::Pending {
            return;
        }
        *state = next;
        drop(state);
        self.changed.notify_all();
    }

    fn succeed(&self) {
        self.finish(PlayStartState::Succeeded);
    }

    fn fail(&self) {
        self.finish(PlayStartState::Failed);
    }
}

/// Cancellation and every early error must release a tracker that is waiting
/// to finish its session, even though no start record will be written.
struct OpenPlayStartGateOnDrop(std::sync::Arc<PlayStartGate>);

impl Drop for OpenPlayStartGateOnDrop {
    fn drop(&mut self) {
        self.0.fail();
    }
}

/// Run `f` over each source, concatenating results. A failing source is always
/// tolerated when some source returns content. When the combined result is
/// empty, behavior depends on `error_on_empty`:
///   * `true` (Home/sections): surface a source error rather than a misleading
///     empty view (e.g. an empty local source masking a failing remote).
///   * `false` (search): an empty result is a legitimate "no matches" as long as
///     *some* source succeeded — only error if every source failed.
async fn aggregate<T, F, Fut>(
    sources: Vec<std::sync::Arc<dyn crate::source::MediaSource>>,
    error_on_empty: bool,
    f: F,
) -> Result<Vec<T>, String>
where
    F: Fn(std::sync::Arc<dyn crate::source::MediaSource>) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<T>, String>>,
{
    if sources.is_empty() {
        return Err("not authenticated".into());
    }
    let mut out = Vec::new();
    let mut last_err = None;
    let mut any_ok = false;
    for s in sources {
        match f(s).await {
            Ok(mut v) => {
                any_ok = true;
                out.append(&mut v);
            }
            Err(e) => last_err = Some(e),
        }
    }
    if !out.is_empty() {
        Ok(out) // have content: show it, tolerating any failed source
    } else if error_on_empty {
        match last_err {
            Some(e) => Err(e), // empty + a source failed → surface it
            None => Ok(out),   // genuinely empty, no errors
        }
    } else if any_ok {
        Ok(out) // search: a source succeeded with no matches → legitimately empty
    } else {
        Err(last_err.unwrap_or_else(|| "no sources available".into())) // all failed
    }
}

fn validate_section_type(section_type: &str) -> Result<(), String> {
    if matches!(section_type, "movie" | "show" | "video") {
        Ok(())
    } else {
        Err("unsupported library section type".into())
    }
}

fn validate_sort(sort: Option<String>) -> Result<Option<String>, String> {
    match sort {
        Some(s) if ALLOWED_SORTS.contains(&s.as_str()) => Ok(Some(s)),
        Some(_) => Err("unsupported sort".into()),
        None => Ok(None),
    }
}

fn clamp_page_size(size: usize) -> usize {
    size.clamp(1, MAX_PAGE_SIZE)
}

/// reqwest client for plex.tv auth calls, with a timeout so linking can't hang.
fn plextv_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

/// Extract a named attribute from the first element that carries it, using a
/// real XML parser (handles whitespace, quote style, and entity escaping that a
/// naive string search would get wrong).
fn attr(xml: &str, name: &str) -> Option<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                for a in e.attributes().flatten() {
                    if a.key.as_ref() == name.as_bytes() {
                        return a
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .ok()
                            .map(|v| v.into_owned());
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linked_server(name: &str, machine_identifier: &str) -> PlexServer {
        PlexServer {
            name: name.to_string(),
            host: format!("{machine_identifier}.plex.direct"),
            port: 32400,
            scheme: "https".to_string(),
            uri: format!("https://{machine_identifier}.plex.direct:32400"),
            local: false,
            relay: false,
            machine_identifier: machine_identifier.to_string(),
            version: "1.0".to_string(),
        }
    }

    fn source_config(id: &str) -> SourceConfig {
        SourceConfig {
            id: id.to_string(),
            kind: "plex".to_string(),
            name: id.to_string(),
            base_url: format!("https://{id}.example:32400"),
            access_token: Some(format!("token-{id}")),
            api_key: None,
            user_id: None,
            device_id: Some(format!("client-{id}")),
            machine_identifier: Some(format!("machine-{id}")),
        }
    }

    fn connected_session(created_at: Instant, id: &str) -> PlexLinkSession {
        PlexLinkSession::Connected {
            created_at,
            client_identifier: format!("client-{id}"),
            source: SourceDto {
                id: id.to_string(),
                name: id.to_string(),
                kind: "plex".to_string(),
            },
        }
    }

    fn completion(session_id: &str, item_key: &str) -> PlaybackCompletion {
        PlaybackCompletion {
            session_id: session_id.to_string(),
            item_key: item_key.to_string(),
            watch_key: None,
            started_at_ms: 10,
            watch_backings: backing_for_namespaced_key(item_key).into_iter().collect(),
            media_type: Some("episode".to_string()),
        }
    }

    fn catalog_item(key: &str, media_type: &str, index: u32, played: bool) -> ItemDto {
        ItemDto {
            rating_key: key.to_string(),
            title: key.to_string(),
            year: None,
            summary: None,
            duration_ms: None,
            media_type: Some(media_type.to_string()),
            poster: None,
            series_poster: None,
            backdrop: None,
            view_offset_ms: None,
            played: Some(played),
            last_watched_at_ms: None,
            added_at_ms: None,
            index: Some(index),
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            parent_rating_key: None,
            grandparent_rating_key: None,
            source_id: "test".to_string(),
            provider_ids: vec![],
            backing: None,
            canonical_id: None,
            watch_key: None,
            detail_key: None,
        }
    }

    struct WatchTestSource {
        id: &'static str,
        name: &'static str,
        fail: bool,
        calls: std::sync::Arc<std::sync::Mutex<Vec<(String, bool)>>>,
        in_flight: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        max_in_flight: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl crate::source::MediaSource for WatchTestSource {
        fn id(&self) -> String {
            self.id.to_string()
        }
        fn name(&self) -> String {
            self.name.to_string()
        }
        fn kind(&self) -> &'static str {
            "plex"
        }
        async fn sections(&self) -> Result<Vec<SectionDto>, String> {
            Ok(vec![])
        }
        async fn hubs(&self) -> Result<Vec<HubDto>, String> {
            Ok(vec![])
        }
        async fn items(
            &self,
            _key: &str,
            _ty: &str,
            _sort: Option<&str>,
            _start: usize,
            _size: usize,
        ) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn search(&self, _query: &str) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn children(
            &self,
            _key: &str,
            _start: usize,
            _size: usize,
        ) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn resolve_stream(
            &self,
            _key: &str,
            _duration_ms: Option<u64>,
            _include_markers: bool,
            _quality: &str,
        ) -> Result<crate::source::StreamResolution, String> {
            Err("not used".to_string())
        }
        async fn mark_played(&self, item_key: &str, played: bool) -> Result<(), String> {
            let active = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_in_flight.fetch_max(active, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(20)).await;
            self.calls
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push((item_key.to_string(), played));
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            if self.fail {
                Err("provider failure https://secret.invalid/?token=do-not-leak".to_string())
            } else {
                Ok(())
            }
        }
    }

    /// Records every teardown it is asked for, and can be made to hang so the
    /// teardown deadline itself is observable.
    struct TranscodeTestSource {
        stopped: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        hang: bool,
    }

    impl TranscodeTestSource {
        /// Returns the source behind the trait object the slot stores, plus the
        /// log of sessions it was asked to stop.
        fn build() -> (
            std::sync::Arc<dyn crate::source::MediaSource>,
            std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        ) {
            let stopped = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let source: std::sync::Arc<dyn crate::source::MediaSource> =
                std::sync::Arc::new(TranscodeTestSource {
                    stopped: stopped.clone(),
                    hang: false,
                });
            (source, stopped)
        }
    }

    #[async_trait::async_trait]
    impl crate::source::MediaSource for TranscodeTestSource {
        fn id(&self) -> String {
            "transcoder".to_string()
        }
        fn name(&self) -> String {
            "Transcoder".to_string()
        }
        fn kind(&self) -> &'static str {
            "plex"
        }
        async fn sections(&self) -> Result<Vec<SectionDto>, String> {
            Ok(vec![])
        }
        async fn hubs(&self) -> Result<Vec<HubDto>, String> {
            Ok(vec![])
        }
        async fn items(
            &self,
            _key: &str,
            _ty: &str,
            _sort: Option<&str>,
            _start: usize,
            _size: usize,
        ) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn search(&self, _query: &str) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn children(
            &self,
            _key: &str,
            _start: usize,
            _size: usize,
        ) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn resolve_stream(
            &self,
            _key: &str,
            _duration_ms: Option<u64>,
            _include_markers: bool,
            _quality: &str,
        ) -> Result<crate::source::StreamResolution, String> {
            Err("not used".to_string())
        }
        async fn stop_transcode(&self, session: &str) {
            if self.hang {
                // Longer than any deadline the caller could reasonably set.
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
            self.stopped
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(session.to_string());
        }
    }

    fn step_request(session: &str, steps: u32) -> StepDownRequest {
        StepDownRequest {
            session_id: session.to_string(),
            position_ms: 61_000,
            reason: crate::automatic::StepDownReason::DropStorm,
            steps_taken: steps,
        }
    }

    /// A verdict has to cross from a plain sampler thread to the async path
    /// that can start a play; this is the handoff.
    #[tokio::test]
    async fn a_verdict_reaches_the_dispatcher() {
        let queue = StepDownQueue::default();
        queue.request(step_request("s1", 0));
        let got = queue.next().await;
        assert_eq!(got.session_id, "s1");
        assert_eq!(got.position_ms, 61_000);
    }

    /// The waiter must be woken by a request that arrives while it is parked,
    /// not only by one already queued — otherwise a step-down lands whenever
    /// the NEXT one happens to arrive.
    #[tokio::test]
    async fn a_parked_dispatcher_is_woken_by_a_later_verdict() {
        let queue = std::sync::Arc::new(StepDownQueue::default());
        let writer = queue.clone();
        let handle = tokio::spawn(async move { queue.next().await });
        tokio::task::yield_now().await;
        writer.request(step_request("later", 1));
        let got = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("the parked dispatcher must wake")
            .expect("join");
        assert_eq!(got.session_id, "later");
        assert_eq!(got.steps_taken, 1, "the step count must survive the handoff");
    }

    /// Two verdicts can only mean the second describes a play the first is
    /// already replacing, so the newer one wins and the older is dropped.
    #[tokio::test]
    async fn a_newer_verdict_replaces_an_unhandled_one() {
        let queue = StepDownQueue::default();
        queue.request(step_request("old", 0));
        queue.request(step_request("new", 1));
        let got = queue.next().await;
        assert_eq!(got.session_id, "new");
        // And nothing is left behind to fire a second relaunch.
        assert!(
            tokio::time::timeout(Duration::from_millis(200), queue.next())
                .await
                .is_err(),
            "the superseded verdict must not still be waiting"
        );
    }

    /// The current tier is DERIVED from the step count rather than stored, so
    /// these two must agree or the walk skips or repeats a rung.
    #[test]
    fn the_current_tier_follows_the_step_count() {
        let tiers = crate::source::tiers_for_source(1080);
        assert_eq!(
            quality_after_steps("original", &tiers, 0),
            "original",
            "an unstepped play is still at its setting"
        );
        assert_eq!(quality_after_steps("original", &tiers, 1), tiers[0].id);
        assert_eq!(quality_after_steps("original", &tiers, 2), tiers[1].id);
        // Automatic starts at Original, so it walks identically.
        assert_eq!(quality_after_steps("automatic", &tiers, 2), tiers[1].id);
    }

    /// More steps than rungs must stop at the floor, never run off the ladder.
    ///
    /// Several counts in a row, not one: a walk that WRAPS to the top instead
    /// of stopping still lands on the floor every `tiers.len() + 1` steps, so a
    /// single large count can pass by coincidence — an earlier version of this
    /// test did exactly that.
    #[test]
    fn the_derived_tier_stops_at_the_floor() {
        let tiers = crate::source::tiers_for_source(1080);
        let floor = tiers.last().expect("non-empty").id;
        let reach_floor = tiers.len() as u32;
        for extra in 0..=(reach_floor + 3) {
            assert_eq!(
                quality_after_steps("original", &tiers, reach_floor + extra),
                floor,
                "{} steps must still be the floor",
                reach_floor + extra
            );
        }
    }

    /// mpv's OSD is large and this is an explanation, not an announcement.
    #[test]
    fn the_step_down_notice_stays_short() {
        for tier in crate::source::QUALITY_TIERS {
            let notice = short_quality_notice(*tier);
            assert!(
                notice.chars().count() <= 12,
                "{notice:?} is too long for an OSD line"
            );
            assert!(notice.starts_with('↓'), "{notice:?} must read as a drop");
            // The bitrate is the whole point — it is what changed.
            assert!(
                notice.contains("Mbps") || notice.contains("kbps"),
                "{notice:?} must name the new bitrate"
            );
        }
    }

    fn transcode_slot() -> ActiveTranscodeSlot {
        std::sync::Arc::new(std::sync::Mutex::new(None))
    }

    /// The whole point of owning the record in `AppState`: exit kills mpv and
    /// returns, so the tracker tail that normally issues the DELETE may never
    /// run. The exit sweep must still find the session and stop it.
    #[tokio::test]
    async fn exit_sweep_stops_a_transcode_whose_end_callback_never_ran() {
        let slot = transcode_slot();
        let (source, stopped) = TranscodeTestSource::build();
        assert!(register_active_transcode(&slot, &source, "abc".to_string()).is_none());

        let record = take_any_active_transcode(&slot).expect("exit must find the live transcode");
        stop_transcode_record(record).await;

        assert_eq!(
            *stopped.lock().unwrap_or_else(|e| e.into_inner()),
            vec!["abc".to_string()],
            "the exit sweep must issue the teardown for the exact live session"
        );
    }

    /// A superseded play's tail runs after the replacement has registered. It
    /// must not tear down the encoder the user is now watching.
    #[tokio::test]
    async fn an_older_plays_tail_cannot_stop_a_newer_plays_transcode() {
        let slot = transcode_slot();
        let (source, _stopped) = TranscodeTestSource::build();
        register_active_transcode(&slot, &source, "old".to_string());
        register_active_transcode(&slot, &source, "new".to_string());

        assert!(
            take_active_transcode(&slot, "old").is_none(),
            "the old session is gone; its tail must claim nothing"
        );
        let survivor =
            take_active_transcode(&slot, "new").expect("the newer transcode must still be owned");
        assert_eq!(survivor.session, "new");
    }

    /// Registration is what stops the displaced encoder — the superseded tail
    /// races this and loses, so it cannot be relied on to do it.
    #[tokio::test]
    async fn superseding_a_play_hands_back_the_encoder_it_displaced() {
        let slot = transcode_slot();
        let (source, stopped) = TranscodeTestSource::build();
        register_active_transcode(&slot, &source, "first".to_string());

        let displaced = register_active_transcode(&slot, &source, "second".to_string())
            .expect("replacing a live transcode must hand back the old record");
        assert_eq!(displaced.session, "first");
        stop_transcode_record(displaced).await;

        assert_eq!(
            *stopped.lock().unwrap_or_else(|e| e.into_inner()),
            vec!["first".to_string()],
            "the displaced encoder must be the one stopped"
        );
    }

    /// Three separate paths may try to stop a session — the tracker tail, the
    /// launch-failure path, and the exit sweep. Claiming it must be exclusive
    /// so it is stopped exactly once and never stopped twice.
    #[tokio::test]
    async fn a_transcode_can_be_claimed_only_once() {
        let slot = transcode_slot();
        let (source, _stopped) = TranscodeTestSource::build();
        register_active_transcode(&slot, &source, "only".to_string());

        assert!(take_active_transcode(&slot, "only").is_some());
        assert!(
            take_active_transcode(&slot, "only").is_none(),
            "a second claimant must get nothing"
        );
        assert!(
            take_any_active_transcode(&slot).is_none(),
            "the exit sweep must not re-stop an already-torn-down session"
        );
    }

    /// The exit sweep blocks on this call, so an unreachable server must cost a
    /// bounded delay rather than a hung shutdown.
    #[tokio::test(start_paused = true)]
    async fn a_hanging_server_cannot_block_teardown_forever() {
        let stopped = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let source: std::sync::Arc<dyn crate::source::MediaSource> =
            std::sync::Arc::new(TranscodeTestSource {
                stopped: stopped.clone(),
                hang: true,
            });
        let slot = transcode_slot();
        register_active_transcode(&slot, &source, "wedged".to_string());
        let record = take_any_active_transcode(&slot).expect("registered");

        // Returns at all: without the deadline this future never completes.
        stop_transcode_record(record).await;

        assert!(
            stopped
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty(),
            "the hung provider never finished; the deadline is what returned"
        );
    }

    fn watch_backing(source_id: &str, rating_key: &str) -> BackingRef {
        BackingRef {
            source_id: source_id.to_string(),
            rating_key: rating_key.to_string(),
            parent_rating_key: None,
            grandparent_rating_key: None,
        }
    }

    #[tokio::test]
    async fn watched_state_fanout_deduplicates_runs_concurrently_and_keeps_partial_success() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let in_flight = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_in_flight = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut registry = crate::source::SourceRegistry::default();
        for (id, name, fail) in [("alpha", "Alpha", false), ("beta", "Beta", true)] {
            registry.upsert(std::sync::Arc::new(WatchTestSource {
                id,
                name,
                fail,
                calls: calls.clone(),
                in_flight: in_flight.clone(),
                max_in_flight: max_in_flight.clone(),
            }));
        }
        let registry = tokio::sync::Mutex::new(registry);
        let mut item = catalog_item("alpha:one", "movie", 0, false);
        item.source_id = "alpha".to_string();
        item.backing = Some(vec![
            watch_backing("alpha", "alpha:one"),
            watch_backing("alpha", "alpha:one"),
            watch_backing("beta", "beta:two"),
            watch_backing("removed", "removed:three"),
        ]);
        item.watch_key = Some("beta:two".to_string());

        let backings = watched_state_backings(&item);
        assert_eq!(
            backings
                .iter()
                .map(|backing| backing.rating_key.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha:one", "beta:two", "removed:three"]
        );
        let prepared = prepare_watch_mutations(&registry, &backings).await;
        assert_eq!(prepared.targets.len(), 2, "removed sources are not targets");
        assert_eq!(prepared.failed_sources, 0);

        let result = execute_watch_mutations(prepared, true).await;
        assert_eq!(result.succeeded_sources, 1);
        assert_eq!(result.failed_sources, 1);
        assert_eq!(result.failed_source_names, vec!["Beta"]);
        assert!(!all_watch_mutations_failed(&result));
        assert!(
            max_in_flight.load(Ordering::SeqCst) >= 2,
            "independent backing writes must overlap"
        );
        let mut calls = calls
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        calls.sort();
        assert_eq!(
            calls,
            vec![("one".to_string(), true), ("two".to_string(), true)]
        );
        let public = serde_json::to_string(&result).unwrap();
        assert!(!public.contains("secret.invalid"));
        assert!(!public.contains("token"));
    }

    #[tokio::test]
    async fn watched_state_total_failure_is_the_only_rollback_outcome() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let in_flight = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_in_flight = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut registry = crate::source::SourceRegistry::default();
        registry.upsert(std::sync::Arc::new(WatchTestSource {
            id: "down",
            name: "Down",
            fail: true,
            calls,
            in_flight,
            max_in_flight,
        }));
        let registry = tokio::sync::Mutex::new(registry);
        let prepared =
            prepare_watch_mutations(&registry, &[watch_backing("down", "down:item")]).await;
        let result = execute_watch_mutations(prepared, false).await;
        assert!(all_watch_mutations_failed(&result));
        assert_eq!(result.succeeded_sources, 0);
        assert_eq!(result.failed_sources, 1);
        assert_eq!(result.failed_source_names, vec!["Down"]);
        let safe_error = total_watch_failure_message(&result);
        assert!(safe_error.contains("Down"));
        assert!(!safe_error.contains("secret.invalid"));
        assert!(!safe_error.contains("token"));
    }

    fn pending_choice(
        request_id: &str,
        created_at: Instant,
        replace_session: Option<&str>,
    ) -> PendingPlaybackChoice {
        PendingPlaybackChoice {
            request: PlaybackSourceChoiceRequestDto {
                request_id: request_id.to_string(),
                title: "Title".to_string(),
                choices: vec![PlaybackSourceChoiceDto {
                    source_id: "source-a".to_string(),
                    source_name: "Source A".to_string(),
                    locality: "lan".to_string(),
                    quality_label: "1920×1080 · SDR".to_string(),
                }],
            },
            created_at,
            item: catalog_item("source-a:item", "movie", 0, false),
            start_from_beginning: false,
            session_id: format!("future-{request_id}"),
            playlist: None,
            replace_session: replace_session.map(str::to_string),
            run_kind: Some(PlaybackRunKind::Series),
        }
    }

    #[test]
    fn playback_choice_requests_are_bounded_expiring_and_single_use() {
        let now = Instant::now();
        let mut requests = PlaybackChoiceRequests::default();
        for index in 0..=MAX_PLAYBACK_CHOICES {
            requests.insert_at(
                pending_choice(&format!("request-{index}"), now, None),
                now,
            );
        }
        assert_eq!(requests.entries.len(), MAX_PLAYBACK_CHOICES);
        assert!(requests.request_at("request-0", now).is_none());
        assert!(requests.request_at("request-16", now).is_some());
        assert!(requests.take_at("request-16", now).is_some());
        assert!(requests.take_at("request-16", now).is_none());

        requests.insert_at(pending_choice("expired", now, None), now);
        assert!(requests
            .request_at("expired", now + PLAYBACK_CHOICE_TTL + Duration::from_millis(1))
            .is_none());
    }

    #[test]
    fn playback_choice_cancellation_is_scoped_to_the_exact_run() {
        let now = Instant::now();
        let mut requests = PlaybackChoiceRequests::default();
        requests.insert_at(pending_choice("old", now, Some("old-session")), now);
        requests.insert_at(pending_choice("new", now, Some("new-session")), now);
        requests.clear_for_session("old-session");
        assert!(requests.request_at("old", now).is_none());
        assert!(requests.request_at("new", now).is_some());
    }

    #[test]
    fn ask_choices_group_by_source_and_show_that_sources_best_version() {
        let candidates = vec![
            crate::selection::PlaybackCandidate {
                source_id: "source-a".to_string(),
                version_id: "1080-hdr".to_string(),
                width: 1920,
                height: 1080,
                hdr: true,
                bitrate: 20,
                direct_play_rank: 0,
                locality: crate::locality::EndpointLocality::Lan,
            },
            crate::selection::PlaybackCandidate {
                source_id: "source-a".to_string(),
                version_id: "4k-sdr".to_string(),
                width: 3840,
                height: 2160,
                hdr: false,
                bitrate: 80,
                direct_play_rank: 0,
                locality: crate::locality::EndpointLocality::Lan,
            },
            crate::selection::PlaybackCandidate {
                source_id: "source-b".to_string(),
                version_id: "720".to_string(),
                width: 1280,
                height: 720,
                hdr: false,
                bitrate: 8,
                direct_play_rank: 0,
                locality: crate::locality::EndpointLocality::Internet,
            },
        ];
        let version = |source_id: &str, source_name: &str| PlaybackVersion {
            source_id: source_id.to_string(),
            source_name: source_name.to_string(),
            item_key: format!("{source_id}:item"),
            version_id: "opaque".to_string(),
            width: 0,
            height: 0,
            hdr: false,
            bitrate: 0,
            direct_play_rank: 0,
            endpoint: url::Url::parse("https://example.test").unwrap(),
            provider_verified_local: false,
        };
        let choices = playback_source_choices(
            &candidates,
            &[
                version("source-a", "Alpha"),
                version("source-b", "Beta"),
            ],
        );
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].source_name, "Alpha");
        assert_eq!(choices[0].quality_label, "3840×2160 · SDR");
        assert_eq!(choices[0].locality, "lan");
        assert_eq!(choices[1].quality_label, "1280×720 · SDR");
        let public = serde_json::to_string(&choices).unwrap();
        assert!(!public.contains("version_id"));
        assert!(!public.contains("endpoint"));
    }

    #[test]
    fn ask_affinity_exists_only_for_the_current_explicit_or_existing_run() {
        assert_eq!(
            next_playback_affinity(
                true,
                Some(PlaybackRunKind::Series),
                None,
                None,
                "source-a",
            ),
            None,
            "a one-source item must not suppress a later first duplicate prompt"
        );
        assert_eq!(
            next_playback_affinity(
                true,
                Some(PlaybackRunKind::Series),
                Some("source-a"),
                None,
                "source-a",
            )
            .as_deref(),
            Some("source-a")
        );
        assert_eq!(
            next_playback_affinity(
                true,
                Some(PlaybackRunKind::VelaPlaylist),
                None,
                Some("offline"),
                "source-b",
            )
            .as_deref(),
            Some("source-b"),
            "a lone reachable fallback becomes the run's new affinity"
        );
        assert_eq!(
            next_playback_affinity(true, None, Some("source-a"), None, "source-a"),
            None,
            "standalone Ask choices never persist in memory"
        );
        assert_eq!(
            next_playback_affinity(
                false,
                Some(PlaybackRunKind::Series),
                Some("source-a"),
                Some("source-a"),
                "source-a",
            ),
            None,
            "automatic policies do not inherit Ask affinity"
        );
    }

    #[test]
    fn ask_source_decision_prompts_only_when_the_run_has_multiple_alternatives() {
        let candidate = |source_id: &str| crate::selection::PlaybackCandidate {
            source_id: source_id.to_string(),
            version_id: format!("{source_id}-version"),
            width: 1920,
            height: 1080,
            hdr: false,
            bitrate: 10,
            direct_play_rank: 0,
            locality: crate::locality::EndpointLocality::Lan,
        };
        let duplicate = vec![candidate("source-a"), candidate("source-b")];
        assert_eq!(
            ask_source_decision(&duplicate, None),
            AskSourceDecision::Prompt,
            "the first duplicate in a run must ask"
        );
        assert_eq!(
            ask_source_decision(&duplicate, Some("source-a")),
            AskSourceDecision::UseSource("source-a".to_string()),
            "a reachable run affinity must be reused"
        );
        assert_eq!(
            ask_source_decision(&[candidate("source-b")], Some("offline")),
            AskSourceDecision::UseSource("source-b".to_string()),
            "one reachable fallback must be selected directly"
        );
        assert_eq!(
            ask_source_decision(
                &[candidate("source-b"), candidate("source-c")],
                Some("offline"),
            ),
            AskSourceDecision::Prompt,
            "multiple fallbacks must replace the affinity through a new prompt"
        );
    }

    fn episode_context(
        item: &str,
        season: &str,
        episode_index: u32,
        season_index: u32,
    ) -> EpisodeContext {
        EpisodeContext {
            item_key: item.to_string(),
            season_key: Some(season.to_string()),
            show_key: Some("test:show".to_string()),
            episode_index: Some(episode_index),
            season_index: Some(season_index),
        }
    }

    #[test]
    fn playback_start_mode_honors_resume_authority_and_forced_beginning() {
        assert_eq!(playback_start_ms(false, 7_000, 3_000), 7_000);
        assert_eq!(playback_start_ms(false, 0, 3_000), 3_000);
        assert_eq!(playback_start_ms(true, 7_000, 3_000), 0);
        assert_eq!(playback_start_ms(true, 0, 3_000), 0);
    }

    #[test]
    fn successful_play_side_effect_runs_only_for_a_completed_launch() {
        let mut calls = 0;
        assert_eq!(after_successful_play(Ok(7_u8), || calls += 1), Ok(7));
        assert_eq!(calls, 1);

        assert_eq!(
            after_successful_play(Err::<u8, _>("spawn failed".into()), || calls += 1),
            Err("spawn failed".into())
        );
        assert_eq!(calls, 1, "a failed launch must not run the side effect");
    }

    #[test]
    fn automatic_window_state_requires_the_exact_replaced_session() {
        let observation = playback::WindowStateObservation::default();
        observation.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "fullscreen",
            "data": true
        }));
        observation.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "window-maximized",
            "data": false
        }));
        observation.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "display-names",
            "data": ["DP-1"]
        }));
        let current = PlaybackWindowSession {
            session_id: "completed".to_string(),
            observation,
        };

        assert_eq!(
            inherited_window_state(Some(&current), Some("completed")),
            playback::PlaybackWindowState {
                fullscreen: Some(true),
                maximized: Some(false),
            }
        );
        assert_eq!(
            inherited_window_state(Some(&current), None),
            playback::PlaybackWindowState::default(),
            "manual play must not inherit the current process"
        );
        assert_eq!(
            inherited_window_state(Some(&current), Some("stale")),
            playback::PlaybackWindowState::default(),
            "a delayed continuation must not inherit a newer process"
        );
        assert_eq!(
            inherited_window_state(None, Some("completed")),
            playback::PlaybackWindowState::default()
        );
        assert_eq!(
            inherited_screen_name(Some(&current), Some("completed")).as_deref(),
            Some("DP-1")
        );
        assert_eq!(inherited_screen_name(Some(&current), None), None);
        assert_eq!(inherited_screen_name(Some(&current), Some("stale")), None);
    }

    #[test]
    fn window_session_publication_stays_behind_successful_launch_boundary() {
        let published = std::sync::Mutex::new(None);
        let failed_record = PlaybackWindowSession {
            session_id: "failed".to_string(),
            observation: playback::WindowStateObservation::default(),
        };
        let result = after_successful_play(Err::<(), _>("spawn failed".to_string()), || {
            *published.lock().unwrap() = Some(failed_record)
        });
        assert_eq!(result, Err("spawn failed".to_string()));
        assert!(published.lock().unwrap().is_none());

        let successful_record = PlaybackWindowSession {
            session_id: "launched".to_string(),
            observation: playback::WindowStateObservation::default(),
        };
        after_successful_play(Ok(()), || {
            *published.lock().unwrap() = Some(successful_record)
        })
        .unwrap();
        assert_eq!(
            published.lock().unwrap().as_ref().unwrap().session_id,
            "launched"
        );
    }

    #[test]
    fn playback_end_waits_until_the_start_record_boundary_opens() {
        let gate = std::sync::Arc::new(PlayStartGate::default());
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (passed_tx, passed_rx) = std::sync::mpsc::channel();
        let waiter_gate = gate.clone();
        let waiter = std::thread::spawn(move || {
            entered_tx.send(()).unwrap();
            passed_tx.send(waiter_gate.wait_succeeded()).unwrap();
        });

        entered_rx.recv().unwrap();
        assert!(
            passed_rx
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err(),
            "the end callback crossed the closed start-record boundary"
        );
        gate.succeed();
        assert!(passed_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("the successful boundary releases the end callback"));
        waiter.join().unwrap();
    }

    #[test]
    fn failed_playback_releases_the_start_gate_without_authorizing_end_work() {
        let gate = PlayStartGate::default();
        gate.fail();
        assert!(!gate.wait_succeeded());
        gate.succeed();
        assert!(
            !gate.wait_succeeded(),
            "a late success cannot revive a failed launch"
        );
    }

    #[test]
    fn playback_advance_joins_only_matching_eof_and_tracker_sessions() {
        let advance = PlaybackAdvance::default();
        advance.mark_eof("old".to_string());
        advance.mark_ended(completion("new", "test:new"));
        assert_eq!(advance.take_ready(), None);

        let mut completed = completion("old", "test:old");
        completed.watch_key = Some("plex:watch-owner".to_string());
        advance.mark_ended(completed.clone());
        assert_eq!(advance.take_ready(), Some(completed));
        assert_eq!(advance.take_ready(), None);
    }

    #[test]
    fn conditional_play_rejects_only_a_stale_expected_session() {
        assert!(expected_session_matches(Some("current"), None));
        assert!(expected_session_matches(Some("current"), Some("current")));
        assert!(!expected_session_matches(Some("newer"), Some("old")));
        assert!(!expected_session_matches(None, Some("old")));
    }

    #[test]
    fn session_comparison_rejects_a_stale_dispatcher() {
        let cursor = PlaylistCursor {
            owner: PlaylistOwner::Vela,
            playlist_id: "p".to_string(),
            entry_id: "entry".to_string(),
            index: 1,
            session_id: "new".to_string(),
        };
        assert!(cursor_matches_session(Some(&cursor), "new"));
        assert!(!cursor_matches_session(Some(&cursor), "old"));
        assert!(!cursor_matches_session(None, "new"));
    }

    #[test]
    fn next_playlist_position_tracks_the_stable_entry_across_edits() {
        let cursor = PlaylistCursor {
            owner: PlaylistOwner::Vela,
            playlist_id: "p".to_string(),
            entry_id: "current".to_string(),
            index: 1,
            session_id: "s".to_string(),
        };
        assert_eq!(
            next_playlist_index(["before", "current", "next"].into_iter(), 3, &cursor),
            2
        );
        assert_eq!(
            next_playlist_index(["current", "inserted", "next"].into_iter(), 3, &cursor),
            1,
            "a fresh store read keeps the stable current entry as its anchor"
        );
        assert_eq!(
            next_playlist_index(["before", "next"].into_iter(), 2, &cursor),
            1,
            "if the current entry was removed, its former index is the fallback"
        );
    }

    #[test]
    fn autocrop_defaults_only_missing_and_rejects_unknown_values() {
        assert_eq!(autocrop_from_config(Some("off")).unwrap(), "off");
        assert_eq!(autocrop_from_config(Some("manual")).unwrap(), "manual");
        assert_eq!(autocrop_from_config(Some("auto")).unwrap(), "auto");
        assert_eq!(autocrop_from_config(None).unwrap(), "off");
        assert!(autocrop_from_config(Some("")).is_err());
        assert!(autocrop_from_config(Some("AUTO")).is_err());
        assert!(autocrop_from_config(Some("on")).is_err());
    }

    #[test]
    fn continue_playing_defaults_only_missing_and_rejects_unknown_values() {
        assert_eq!(continue_playing_from_config(Some("off")).unwrap(), "off");
        assert_eq!(continue_playing_from_config(Some("on")).unwrap(), "on");
        assert_eq!(
            continue_playing_from_config(Some("only-tv")).unwrap(),
            "only-tv"
        );
        assert_eq!(continue_playing_from_config(None).unwrap(), "only-tv");
        assert!(continue_playing_from_config(Some("")).is_err());
        assert!(continue_playing_from_config(Some("ON")).is_err());
        assert!(continue_playing_from_config(Some("future-mode")).is_err());
    }

    #[test]
    fn playback_display_overrides_reject_invalid_saved_values() {
        let cfg = config::AppConfig {
            playback_display_resolution: Some("2160p".to_string()),
            playback_display_hdr: None,
            ..Default::default()
        };
        let normalized = playback_display_overrides(&cfg).unwrap();
        assert_eq!(normalized.resolution, Some(ResolutionOverride::P2160));
        assert_eq!(normalized.hdr, None);

        let invalid = config::AppConfig {
            playback_display_hdr: Some("future-hdr".to_string()),
            ..Default::default()
        };
        assert!(playback_display_overrides(&invalid).is_err());
    }

    #[test]
    fn next_episode_selects_the_following_episode_even_when_watched() {
        let context = episode_context("test:s1e1", "test:s1", 1, 1);
        let catalog = vec![(
            catalog_item("test:s1", "season", 1, false),
            vec![
                catalog_item("test:s1e1", "episode", 1, false),
                catalog_item("test:s1e2", "episode", 2, true),
                catalog_item("test:s1e3", "episode", 3, false),
            ],
        )];
        assert_eq!(
            choose_next_episode(&context, &catalog).map(|item| item.rating_key),
            Some("test:s1e2".to_string())
        );
    }

    #[test]
    fn next_episode_rolls_into_the_next_season() {
        let context = episode_context("test:s1e2", "test:s1", 2, 1);
        let catalog = vec![
            (
                catalog_item("test:s1", "season", 1, false),
                vec![
                    catalog_item("test:s1e1", "episode", 1, false),
                    catalog_item("test:s1e2", "episode", 2, false),
                ],
            ),
            (
                catalog_item("test:s2", "season", 2, false),
                vec![catalog_item("test:s2e1", "episode", 1, false)],
            ),
        ];
        assert_eq!(
            choose_next_episode(&context, &catalog).map(|item| item.rating_key),
            Some("test:s2e1".to_string())
        );
    }

    #[test]
    fn next_episode_stops_at_the_end_of_the_show_without_repeating() {
        let context = episode_context("test:s1e1", "test:s1", 1, 1);
        let catalog = vec![(
            catalog_item("test:s1", "season", 1, false),
            vec![catalog_item("test:s1e1", "episode", 1, false)],
        )];
        assert_eq!(
            choose_next_episode(&context, &catalog).map(|item| item.rating_key),
            None
        );
    }

    #[test]
    fn next_episode_skips_specials_when_the_run_started_in_a_normal_season() {
        let context = episode_context("test:s1e1", "test:s1", 1, 1);
        let catalog = vec![
            (
                catalog_item("test:s1", "season", 1, false),
                vec![catalog_item("test:s1e1", "episode", 1, false)],
            ),
            (
                catalog_item("test:specials", "season", 0, false),
                vec![catalog_item("test:sp1", "episode", 1, false)],
            ),
            (
                catalog_item("test:s2", "season", 2, false),
                vec![catalog_item("test:s2e1", "episode", 1, false)],
            ),
        ];
        assert_eq!(
            choose_next_episode(&context, &catalog).map(|item| item.rating_key),
            Some("test:s2e1".to_string())
        );
    }

    #[test]
    fn next_episode_honors_specials_when_the_run_started_there() {
        let context = episode_context("test:sp2", "test:specials", 2, 0);
        let catalog = vec![
            (
                catalog_item("test:specials", "season", 0, false),
                vec![
                    catalog_item("test:sp1", "episode", 1, false),
                    catalog_item("test:sp2", "episode", 2, false),
                ],
            ),
            (
                catalog_item("test:s1", "season", 1, false),
                vec![catalog_item("test:s1e1", "episode", 1, false)],
            ),
        ];
        assert_eq!(
            choose_next_episode(&context, &catalog).map(|item| item.rating_key),
            Some("test:s1e1".to_string())
        );
    }

    // The play path hands the player only the ranges the user enabled. Without
    // this filter the script would receive a credits range while credits are
    // Off and skip something the user chose to watch.
    #[test]
    fn only_enabled_marker_kinds_reach_the_player() {
        use crate::config::SkipPolicy;
        use crate::source::{MarkerKind, MediaMarker};

        let range = |kind, start_ms| MediaMarker {
            kind,
            start_ms,
            end_ms: start_ms + 1_000,
        };
        let all = [
            range(MarkerKind::Intro, 1_000),
            range(MarkerKind::Credits, 2_000),
            range(MarkerKind::Commercial, 3_000),
        ];

        let policies = SkipPolicies {
            intro: SkipPolicy::Button,
            credits: SkipPolicy::Off,
            commercial: SkipPolicy::Autoskip,
        };
        assert!(policies.any_enabled(), "markers are requested at all");
        let kept: Vec<MarkerKind> = all
            .iter()
            .filter(|marker| !skip_policy_for(marker.kind, &policies).is_off())
            .map(|marker| marker.kind)
            .collect();
        assert_eq!(
            kept,
            vec![MarkerKind::Intro, MarkerKind::Commercial],
            "the disabled kind must not reach the player"
        );

        let none = SkipPolicies {
            intro: SkipPolicy::Off,
            credits: SkipPolicy::Off,
            commercial: SkipPolicy::Off,
        };
        assert!(
            !none.any_enabled(),
            "with every kind off the server is never asked for markers"
        );
        assert!(
            all.iter()
                .all(|marker| skip_policy_for(marker.kind, &none).is_off()),
            "with every kind off nothing survives the filter"
        );
    }

    #[test]
    fn validate_sort_rejects_unknown_values() {
        assert!(validate_sort(Some("titleSort:asc".to_string())).is_ok());
        assert!(validate_sort(Some("unknown:desc".to_string())).is_err());
    }

    #[test]
    fn page_size_is_clamped() {
        assert_eq!(clamp_page_size(0), 1);
        assert_eq!(clamp_page_size(60), 60);
        assert_eq!(clamp_page_size(500), MAX_PAGE_SIZE);
    }

    #[test]
    fn section_type_allowlist_includes_video() {
        assert!(validate_section_type("movie").is_ok());
        assert!(validate_section_type("show").is_ok());
        assert!(validate_section_type("video").is_ok());
        assert!(validate_section_type("photo").is_err());
    }

    #[test]
    fn weak_pin_uses_plex_link_url() {
        assert_eq!(plex_link_url("ABCD"), "https://plex.tv/link/?pin=ABCD");
    }

    #[test]
    fn plex_link_mints_a_fresh_nonlegacy_source_id_each_time() {
        let server = linked_server("Home", "machine-a");
        let first = plex_source_config("token".into(), "client".into(), &server).unwrap();
        let second = plex_source_config("token".into(), "client".into(), &server).unwrap();

        assert!(first.id.starts_with("plex-"));
        assert_ne!(first.id, "plex");
        assert_ne!(first.id, second.id);
    }

    #[test]
    fn plex_link_persists_credentials_endpoint_and_machine_together() {
        let server = linked_server("Home", "machine-a");
        let cfg = plex_source_config("secret-token".into(), "device-a".into(), &server).unwrap();

        assert_eq!(cfg.kind, "plex");
        assert_eq!(cfg.name, "Home");
        assert_eq!(cfg.base_url, "https://machine-a.plex.direct:32400");
        assert_eq!(cfg.access_token.as_deref(), Some("secret-token"));
        assert_eq!(cfg.device_id.as_deref(), Some("device-a"));
        assert_eq!(cfg.machine_identifier.as_deref(), Some("machine-a"));
        assert!(plex::build_source(&cfg).is_ok());
    }

    #[test]
    fn plex_link_refuses_an_unpinned_server() {
        let server = linked_server("Unknown", "");
        assert!(plex_source_config("token".into(), "client".into(), &server).is_err());
    }

    #[test]
    fn plex_link_refuses_a_non_https_server() {
        let mut server = linked_server("Home", "machine-a");
        server.scheme = "http".to_string();
        server.uri = "http://machine-a.example:32400".to_string();
        assert!(plex_source_config("token".into(), "client".into(), &server).is_err());
    }

    #[test]
    fn plex_link_refuses_a_relay_server() {
        let mut server = linked_server("Relay", "machine-a");
        server.relay = true;
        assert!(plex_source_config("token".into(), "client".into(), &server).is_err());
    }

    #[test]
    fn plex_server_picker_returns_names_and_ids_but_never_credentials() {
        let session = PlexLinkSession::ChooseServer {
            created_at: Instant::now(),
            client_identifier: "private-client".to_string(),
            token: "private-token".to_string(),
            servers: vec![linked_server("Home", "machine-a")],
        };

        let value = serde_json::to_value(session.response()).unwrap();
        let rendered = value.to_string();
        assert_eq!(value["status"], "chooseServer");
        assert_eq!(value["servers"][0]["name"], "Home");
        assert_eq!(value["servers"][0]["machineIdentifier"], "machine-a");
        assert!(!rendered.contains("private-token"));
        assert!(!rendered.contains("private-client"));
    }

    #[test]
    fn plex_link_poll_statuses_match_the_frontend_contract() {
        let pending = serde_json::to_value(LinkPollDto::Pending).unwrap();
        let connected = serde_json::to_value(LinkPollDto::Connected {
            source: SourceDto {
                id: "plex-one".to_string(),
                name: "Home".to_string(),
                kind: "plex".to_string(),
            },
        })
        .unwrap();

        assert_eq!(pending["status"], "pending");
        assert_eq!(connected["status"], "connected");
        assert_eq!(connected["source"]["id"], "plex-one");
    }

    #[test]
    fn plex_server_selection_matches_the_exact_machine() {
        let servers = vec![
            linked_server("Home", "machine-a"),
            linked_server("Remote", "machine-b"),
        ];

        assert_eq!(
            server_for_machine(&servers, "machine-b").unwrap().name,
            "Remote"
        );
        assert!(server_for_machine(&servers, "machine-c").is_err());
    }

    #[test]
    fn one_reachable_plex_server_connects_without_a_picker() {
        let decision = decide_reachable_servers(vec![linked_server("Home", "machine-a")]).unwrap();
        let ReachableServerDecision::Connect(server) = decision else {
            panic!("one server must connect directly");
        };
        assert_eq!(server.machine_identifier, "machine-a");
    }

    #[test]
    fn several_reachable_plex_servers_require_a_picker() {
        let decision = decide_reachable_servers(vec![
            linked_server("Home", "machine-a"),
            linked_server("Remote", "machine-b"),
        ])
        .unwrap();
        let ReachableServerDecision::Choose(servers) = decision else {
            panic!("several servers must wait for a choice");
        };
        assert_eq!(servers.len(), 2);
    }

    #[test]
    fn no_reachable_plex_server_fails_linking() {
        assert!(decide_reachable_servers(Vec::new()).is_err());
    }

    #[test]
    fn removing_one_plex_source_preserves_the_other() {
        let mut cfg = ConnectionsConfig {
            sources: vec![source_config("plex-a"), source_config("plex-b")],
        };

        remove_source_config(&mut cfg, "plex-a").unwrap();

        assert_eq!(cfg.sources.len(), 1);
        assert_eq!(cfg.sources[0].id, "plex-b");
        assert_eq!(cfg.sources[0].access_token.as_deref(), Some("token-plex-b"));
    }

    #[test]
    fn plex_link_sessions_expire_in_memory() {
        let now = Instant::now();
        let mut sessions = PlexLinkSessions::new();
        sessions.insert(
            "expired".to_string(),
            connected_session(now - PLEX_LINK_SESSION_TTL - Duration::from_secs(1), "old"),
        );
        sessions.insert("fresh".to_string(), connected_session(now, "new"));

        prune_link_sessions(&mut sessions, now);

        assert!(!sessions.contains_key("expired"));
        assert!(sessions.contains_key("fresh"));
    }

    #[test]
    fn plex_link_sessions_evict_the_oldest_at_the_bound() {
        let now = Instant::now();
        let mut sessions = PlexLinkSessions::new();
        for index in 0..MAX_PLEX_LINK_SESSIONS {
            sessions.insert(
                format!("pin-{index}"),
                connected_session(
                    now - Duration::from_secs((MAX_PLEX_LINK_SESSIONS - index) as u64),
                    &format!("source-{index}"),
                ),
            );
        }

        insert_link_session(
            &mut sessions,
            "new-pin".to_string(),
            connected_session(now, "new-source"),
        );

        assert_eq!(sessions.len(), MAX_PLEX_LINK_SESSIONS);
        assert!(!sessions.contains_key("pin-0"));
        assert!(sessions.contains_key("new-pin"));
    }

    // Recents from a source that no longer exists (e.g. the removed
    // local/SMB/SSH family) must not surface — a hero card whose Play can
    // only error is a forbidden dead-end. The config entries themselves are
    // preserved; this is read-time filtering only.
    #[test]
    fn recents_from_removed_sources_are_filtered_at_read_time() {
        let mk = |key: &str, sid: &str| {
            let mut i = crate::source::ItemDto {
                rating_key: key.to_string(),
                title: key.to_string(),
                year: None,
                summary: None,
                duration_ms: None,
                media_type: Some("movie".into()),
                poster: None,
                series_poster: None,
                backdrop: None,
                view_offset_ms: None,
                played: None,
                last_watched_at_ms: None,
                added_at_ms: None,
                index: None,
                parent_index: None,
                grandparent_title: None,
                parent_title: None,
                parent_rating_key: None,
                grandparent_rating_key: None,
                provider_ids: vec![],
                backing: None,
                canonical_id: None,
                watch_key: None,
                detail_key: None,
                source_id: String::new(),
            };
            i.source_id = sid.to_string();
            i
        };
        let items = vec![
            mk("plex:1", "plex"),
            mk("local-abc:/x.mkv", "local-abc"),
            mk("jf:9", "jf"),
        ];
        let live = vec!["plex".to_string(), "jf".to_string()];
        let out = filter_live_recents(items, &live);
        let keys: Vec<_> = out.iter().map(|i| i.rating_key.as_str()).collect();
        assert_eq!(keys, vec!["plex:1", "jf:9"], "dead-source entry dropped");
    }
}
