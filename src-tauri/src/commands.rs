use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use tauri::State;

use crate::config::{self, SourceConfig};
use crate::playback;
use crate::plex_library::PlexLibrary;
use crate::source::jellyfin::{self, Flavor, JellyfinClient};
use crate::source::{plex::PlexSource, DetailDto, HubDto, ItemDto, SectionDto};
use crate::{AppState, PLEX_SOURCE_ID};

const PRODUCT: &str = "Vela";
/// Derived from Cargo.toml's `version` so it can't drift from package metadata.
/// Bumped on EVERY build (see scripts/bump.sh) so each build is uniquely
/// identifiable — in the window footer and in the bundle filename.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// UTC date the build was cut; updated alongside the version by scripts/bump.sh.
const BUILD_DATE: &str = "2026-07-10";

/// Project home, shown (and opened) from the build-info footer.
const REPO_URL: &str = "https://github.com/roethlar/vela";
const MAX_PAGE_SIZE: usize = 100;
const MAX_SEARCH_LEN: usize = 200;
const ALLOWED_SORTS: &[&str] = &[
    "titleSort:asc",
    "year:desc",
    "addedAt:desc",
    "originallyAvailableAt:desc",
    "rating:desc",
    "lastViewedAt:desc",
];

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
#[derive(Serialize)]
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

// ---- status & auth -------------------------------------------------------

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<StatusDto, String> {
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
    };
    register_source(&state, cfg).await
}

/// Remove a configured (non-Plex) source by id. The Plex source is managed by
/// the link/unlink flow, not here, so it's explicitly off-limits — and we only
/// touch ids that actually exist in the persisted source list.
#[tauri::command]
pub async fn remove_source(id: String, state: State<'_, AppState>) -> Result<(), String> {
    if id == PLEX_SOURCE_ID {
        return Err("the Plex source can't be removed here".into());
    }
    let id2 = id.clone();
    config::update(move |cfg| {
        if !cfg.sources.iter().any(|s| s.id == id2) {
            return Err("no such source".to_string());
        }
        cfg.sources.retain(|s| s.id != id2);
        Ok(())
    })?;
    state.registry.lock().await.remove(&id);
    Ok(())
}

/// Unlink the Plex account: clear the stored auth token and drop the live Plex
/// source. The client identifier is kept so a later re-link reuses the same device
/// identity. This is the counterpart to the link flow that `remove_source` defers to.
#[tauri::command]
pub async fn unlink_plex(state: State<'_, AppState>) -> Result<(), String> {
    config::update(|cfg| {
        cfg.auth_token = None;
        Ok(())
    })?;
    state.registry.lock().await.remove(PLEX_SOURCE_ID);
    Ok(())
}

/// Persist a source config and add it to the live registry.
async fn register_source(
    state: &State<'_, AppState>,
    cfg: SourceConfig,
) -> Result<SourceDto, String> {
    let source = jellyfin::build_source(&cfg).ok_or("could not build source from config")?;
    let dto = SourceDto {
        id: source.id(),
        name: source.name(),
        kind: source.kind().to_string(),
    };
    config::update(move |stored| {
        stored.upsert_source(cfg);
        Ok(())
    })
    .map_err(|e| format!("connected but failed to save config: {}", e))?;
    state.registry.lock().await.upsert(source);
    Ok(dto)
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

#[derive(Clone)]
struct CommandInstaller {
    program: String,
    args: Vec<String>,
    display_command: String,
    description: String,
}

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
pub fn check_mpv() -> MpvInfo {
    let install = mpv_install_info();
    let resolved = playback::resolve_mpv();
    let configured_path = config::load_config()
        .ok()
        .and_then(|c| c.mpv_path)
        .filter(|s| !s.trim().is_empty());
    MpvInfo {
        available: resolved.is_some(),
        path: resolved,
        configured_path,
        can_auto_install: install.can_auto_install,
        install_command: install.install_command,
        install_description: install.install_description,
        install_url: install.install_url,
    }
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

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

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
    Ok(check_mpv())
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
}

#[tauri::command]
pub fn get_mpv_advanced() -> MpvAdvanced {
    let cfg = config::load_config().unwrap_or_default();
    MpvAdvanced {
        extra_args: cfg.mpv_extra_args.unwrap_or_default(),
        use_own_config: cfg.mpv_use_own_config.unwrap_or(false),
        autocrop: normalize_autocrop(cfg.mpv_autocrop.as_deref()),
    }
}

/// Clamp any stored/incoming autocrop value to the known three-state set,
/// defaulting anything unrecognised (incl. `None`) to `"off"`.
fn normalize_autocrop(value: Option<&str>) -> String {
    match value {
        Some("manual") => "manual",
        Some("auto") => "auto",
        _ => "off",
    }
    .to_string()
}

/// Persist the advanced mpv settings. No validation of `extra_args` — these are the
/// user's own machine and their own call; a bad option just makes mpv refuse to
/// launch, which surfaces as a normal playback error. An empty `extra_args` clears
/// the override. `autocrop` is optional so older frontends that don't send it leave
/// the mode unchanged; when present it is clamped to the known three states.
#[tauri::command]
pub fn set_mpv_advanced(
    extra_args: String,
    use_own_config: bool,
    autocrop: Option<String>,
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
            cfg.mpv_autocrop = match normalize_autocrop(Some(mode)).as_str() {
                "off" => None,
                m => Some(m.to_string()),
            };
        }
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
    Ok(check_mpv())
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

/// Poll a pending PIN. Returns true once linked (and wires up the client).
#[tauri::command]
pub async fn link_poll(
    pin_id: String,
    client_identifier: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
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
        _ => return Ok(false), // 200 with no token yet = still pending
    };

    // Persist and build the client. Surface a save failure so the user isn't
    // silently logged out on the next launch.
    let (token2, cid2) = (token.clone(), client_identifier.clone());
    config::update(move |cfg| {
        cfg.auth_token = Some(token2);
        cfg.client_identifier = Some(cid2);
        Ok(())
    })
    .map_err(|e| format!("authenticated but failed to save config: {}", e))?;

    let lib = PlexLibrary::new(token, client_identifier);
    state
        .registry
        .lock()
        .await
        .upsert(std::sync::Arc::new(PlexSource::new(
            PLEX_SOURCE_ID,
            "Plex",
            lib,
        )));
    Ok(true)
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
    let sources = state.registry.lock().await.selected(source_id.as_deref());
    aggregate(sources, true, |s| async move { s.hubs().await }).await
}

#[tauri::command]
pub async fn get_sections(
    source_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<SectionDto>, String> {
    let sources = state.registry.lock().await.selected(source_id.as_deref());
    aggregate(sources, true, |s| async move { s.sections().await }).await
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

    let sources = state.registry.lock().await.selected(None);
    // source id → kind, for the default playback ranking of merged backings.
    let kinds: std::collections::HashMap<String, &'static str> =
        sources.iter().map(|s| (s.id(), s.kind())).collect();
    // Owner's per-title source choices (set via the card's context menu).
    let overrides = config::load_config()
        .map(|c| c.merged_overrides)
        .unwrap_or_default();
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

/// Default playback preference for merged titles. A policy constant, not a
/// heuristic — the per-title override wins over all of it. (Sources are
/// servers only since the 2026-07-08 local-family removal; the ladder kept
/// its Plex-first order.)
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
/// kind ranking (registry order breaks ties via stable sort) — and point the
/// entry's play identity (rating_key/source_id) at the winner. Display
/// fields keep whatever the dedup pass chose as richest.
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
                let item_ref = crate::source::BackingRef {
                    source_id: item.source_id.clone(),
                    rating_key: item.rating_key.clone(),
                };
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
                group.backing = Some(vec![crate::source::BackingRef {
                    source_id: group.source_id.clone(),
                    rating_key: group.rating_key.clone(),
                }]);
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
            items.sort_by(|a, b| b.added_at_ms.cmp(&a.added_at_ms).then_with(|| title(a).cmp(&title(b))));
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
        assert_eq!(out.len(), 3, "same-source versions all kept; loop terminates");
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
        assert_eq!(out.len(), 2, "two cards: merged cross-source + solo version");
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
        [
            ("plex".to_string(), "plex"),
            ("jf".to_string(), "jellyfin"),
        ]
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
    let sources = state.registry.lock().await.selected(source_id.as_deref());
    // Search: an empty result is a legitimate "no matches", so don't turn it into
    // an error just because one backend hiccuped (error only if all failed).
    aggregate(sources, false, move |s| {
        let q = q.clone();
        async move { s.search(&q).await }
    })
    .await
}

/// Children of a show (seasons) or season (episodes), for drill-down navigation.
#[tauri::command]
pub async fn get_children(
    rating_key: String,
    start: usize,
    size: usize,
    state: State<'_, AppState>,
) -> Result<Vec<ItemDto>, String> {
    let size = clamp_page_size(size);
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

/// Mark an item watched/unwatched on its source. Routes by the namespaced key;
/// the registry lock is released before the (network) call.
#[tauri::command]
pub async fn set_watched(
    rating_key: String,
    played: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // One edit at a time: overlapping curate-first hides and failure
    // rollbacks must not interleave (undo tokens carry no generation).
    let _edit = state.watch_edit_lock.lock().await;
    let (src, raw) = state.registry.lock().await.route(&rating_key)?;
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
    let key = rating_key.clone();
    let undo = config::update(move |cfg| Ok(crate::recents::hide_with_undo(cfg, &key)))?;
    if let Err(e) = src.mark_played(&raw, played).await {
        let _ = config::update(move |cfg| {
            crate::recents::restore_hidden(cfg, undo);
            Ok(())
        });
        return Err(e);
    }
    Ok(())
}

// ---- playback ------------------------------------------------------------

/// Display-friendly snapshot of a queued item — what the drawer renders and what
/// the dispatcher needs to drive the next play.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub rating_key: String,
    pub title: String,
    pub duration_ms: Option<u64>,
    /// http(s) URL or asset path; the drawer uses convertFileSrc for non-http.
    pub poster: Option<String>,
    /// e.g. "S5 · E8" for episodes, "2026" for movies — what the frontend would
    /// otherwise compute itself.
    pub subtitle: Option<String>,
}

/// Snapshot an item into Vela's recents as playback starts (frontend passes
/// the full card it played, artwork included). The end notifier stamps the
/// final position and drops finished entries.
#[tauri::command]
pub async fn record_recent(item: ItemDto) -> Result<(), String> {
    config::update(move |cfg| {
        crate::recents::record(cfg, item);
        Ok(())
    })
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
    let cfg = config::load_config().map_err(|e| e.to_string())?;
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
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    Ok(cfg.hidden_from_continue.clone())
}

/// Result of `queue_list` — the queue snapshot plus the cursor, so the drawer
/// can highlight whichever item is currently playing.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot {
    pub items: Vec<QueueItem>,
    pub current_index: Option<usize>,
}

/// Internal helper: kill any prior player, route+resolve, and launch the new mpv.
/// Used by `play_item` (top-level play) AND by `queue_play_at` and the auto-
/// advance dispatcher — same lock discipline regardless of trigger.
pub(crate) async fn play_by_key(
    state: &AppState,
    rating_key: &str,
    title: &str,
    duration_ms: Option<u64>,
) -> Result<(), String> {
    // Serialize the whole resolve+stop-old+spawn sequence so overlapping triggers
    // can't both spawn an mpv and lose one of the child handles.
    let _play = state.play_lock.lock().await;
    let (src, raw) = state.registry.lock().await.route(rating_key)?;
    let resolved = src.resolve_stream(&raw, duration_ms).await?;

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
    if let Some(mut child) = prev_child {
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
    let resume_ms = if resolved.resume_ms > 0 {
        resolved.resume_ms
    } else {
        config::load_config()
            .map(|cfg| crate::recents::resume_stamp_ms(&cfg, rating_key))
            .unwrap_or(0)
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
    let spec = playback::PlaySpec {
        url: resolved.url,
        title: title.to_string(),
        http_headers: resolved.http_headers,
        start_seconds: resume_ms as f64 / 1000.0,
        autocrop_script,
        autocrop_shim,
    };
    // End-of-session notifier → the `playback-ended` UI event, emitted after
    // the final server check-in so a re-fetch it triggers sees the new watch
    // state. Payload carries ids only — never URLs or tokens.
    let on_end: Option<playback::EndNotify> = state.app_handle.get().map(|app| {
        use tauri::Emitter;
        let app = app.clone();
        let source_id = src.id().to_string();
        let item_key = rating_key.to_string();
        std::sync::Arc::new(move |position_ms: u64| {
            // Stamp Vela's recents BEFORE emitting, so the refresh the event
            // triggers reads the updated list. Runs on the tracker thread —
            // synchronous config I/O is fine there.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let key = item_key.clone();
            let _ = config::update(move |cfg| {
                crate::recents::finish(cfg, &key, position_ms, now_ms);
                Ok(())
            });
            let _ = app.emit(
                "playback-ended",
                serde_json::json!({ "sourceId": source_id, "itemKey": item_key }),
            );
        }) as playback::EndNotify
    });
    let progress = resolved.progress;
    let child_slot = state.current_child.clone();
    let shutting_down = state.shutting_down.clone();
    let advance = state.queue_advance.clone();
    let played = tauri::async_runtime::spawn_blocking(move || {
        playback::play(&spec, progress, &child_slot, &shutting_down, &advance, on_end)
    })
    .await
    .map_err(|e| format!("playback task failed: {e}"))
    .and_then(|r| r);
    let stop = played?;
    *state
        .tracking_stop
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(stop);
    // A play is the explicit opposite of "stop suggesting it": clear this
    // key's Continue Watching tombstone. Direct plays already do this via
    // record_recent (frontend); this covers queue plays and auto-advance,
    // which record no snapshot. Deliberately post-spawn — a FAILED play must
    // not clear a tombstone (the record_recent rule) — and best-effort: a
    // config hiccup must not fail a playback that already started.
    let key = rating_key.to_string();
    let _ = config::update(move |cfg| {
        crate::recents::untombstone(cfg, &key);
        Ok(())
    });
    Ok(())
}

/// Top-level Play: replace the queue with just this item and play it. The user
/// asked to start over from here, so the existing queue (if any) is cleared.
#[tauri::command]
pub async fn play_item(item: QueueItem, state: State<'_, AppState>) -> Result<(), String> {
    let rating_key = item.rating_key.clone();
    let title = item.title.clone();
    let duration_ms = item.duration_ms;
    {
        let mut q = state.queue.lock().unwrap_or_else(|e| e.into_inner());
        *q = vec![item];
    }
    *state.queue_index.lock().unwrap_or_else(|e| e.into_inner()) = Some(0);
    play_by_key(&state, &rating_key, &title, duration_ms).await
}

/// Insert an item right after the currently-playing one ("Play Next"). If the
/// queue is empty / nothing is playing, it goes to position 0 — i.e. the very
/// next thing the dispatcher will play.
#[tauri::command]
pub async fn queue_play_next(item: QueueItem, state: State<'_, AppState>) -> Result<(), String> {
    let mut q = state.queue.lock().unwrap_or_else(|e| e.into_inner());
    let cursor = *state.queue_index.lock().unwrap_or_else(|e| e.into_inner());
    let pos = match cursor {
        Some(i) => (i + 1).min(q.len()),
        None => 0,
    };
    q.insert(pos, item);
    Ok(())
}

/// Append an item to the end of the queue ("Add to Queue").
#[tauri::command]
pub async fn queue_append(item: QueueItem, state: State<'_, AppState>) -> Result<(), String> {
    state
        .queue
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(item);
    Ok(())
}

/// Snapshot of the queue + cursor for the drawer.
#[tauri::command]
pub async fn queue_list(state: State<'_, AppState>) -> Result<QueueSnapshot, String> {
    let items = state
        .queue
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let current_index = *state.queue_index.lock().unwrap_or_else(|e| e.into_inner());
    Ok(QueueSnapshot {
        items,
        current_index,
    })
}

/// Clear the queue. Does NOT stop the currently-playing item — closing the mpv
/// window does that; "Clear" is for tidying what's queued up next.
#[tauri::command]
pub async fn queue_clear(state: State<'_, AppState>) -> Result<(), String> {
    state
        .queue
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    *state.queue_index.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

/// Remove the item at `index`. Adjusts the cursor: items before it shift the
/// cursor back; removing the currently-playing one detaches the cursor (it keeps
/// playing in mpv, but isn't tracked in the queue any more — next auto-advance
/// will start from queue[0]).
#[tauri::command]
pub async fn queue_remove(index: usize, state: State<'_, AppState>) -> Result<(), String> {
    let mut q = state.queue.lock().unwrap_or_else(|e| e.into_inner());
    if index >= q.len() {
        return Err("queue index out of range".into());
    }
    q.remove(index);
    let mut idx = state.queue_index.lock().unwrap_or_else(|e| e.into_inner());
    *idx = match *idx {
        Some(c) if c == index => None,
        Some(c) if c > index => Some(c - 1),
        other => other,
    };
    Ok(())
}

/// Jump to and play the item at `index`. Sets the cursor so subsequent advances
/// continue from there.
#[tauri::command]
pub async fn queue_play_at(index: usize, state: State<'_, AppState>) -> Result<(), String> {
    let item = {
        let q = state.queue.lock().unwrap_or_else(|e| e.into_inner());
        q.get(index)
            .cloned()
            .ok_or_else(|| "queue index out of range".to_string())?
    };
    *state.queue_index.lock().unwrap_or_else(|e| e.into_inner()) = Some(index);
    play_by_key(&state, &item.rating_key, &item.title, item.duration_ms).await
}

// ---- helpers -------------------------------------------------------------

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
                        return a.unescape_value().ok().map(|v| v.into_owned());
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

    #[test]
    fn normalize_autocrop_clamps_to_known_states() {
        assert_eq!(normalize_autocrop(Some("off")), "off");
        assert_eq!(normalize_autocrop(Some("manual")), "manual");
        assert_eq!(normalize_autocrop(Some("auto")), "auto");
        // Anything unrecognised (incl. None and garbage) must fall back to off, so a
        // corrupt/stale config value can never enable cropping unexpectedly.
        assert_eq!(normalize_autocrop(None), "off");
        assert_eq!(normalize_autocrop(Some("")), "off");
        assert_eq!(normalize_autocrop(Some("AUTO")), "off");
        assert_eq!(normalize_autocrop(Some("on")), "off");
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
