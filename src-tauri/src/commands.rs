use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use tauri::State;

use crate::config::{self, AppConfig, LocalFolder, SmbFolder, SmbMount, SourceConfig, SshMount};
use crate::playback;
use crate::plex_library::PlexLibrary;
use crate::source::jellyfin::{self, Flavor, JellyfinClient};
use crate::source::local::{LocalSource, LOCAL_SOURCE_ID};
use crate::source::{plex::PlexSource, HubDto, ItemDto, SectionDto};
use crate::{AppState, PLEX_SOURCE_ID};

const PRODUCT: &str = "Vela";
/// Derived from Cargo.toml's `version` so it can't drift from package metadata.
/// Bumped on EVERY build (see scripts/bump.sh) so each build is uniquely
/// identifiable — in the window footer and in the bundle filename.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// UTC date the build was cut; updated alongside the version by scripts/bump.sh.
const BUILD_DATE: &str = "2026-05-28";

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

/// A configured local folder, for the settings UI (individually removable).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFolderDto {
    id: String,
    name: String,
    path: String,
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
    if id == LOCAL_SOURCE_ID {
        return Err("manage local media via its folder and SMB entries".into());
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

// ---- local folders -------------------------------------------------------

/// Add a local (or OS-mounted SMB) folder to the built-in local source.
/// `kind` is "movie", "show", or empty (auto). Browsing only — no resume.
#[tauri::command]
pub async fn add_local_folder(
    path: String,
    kind: Option<String>,
    name: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SourceDto, String> {
    let p = std::path::Path::new(&path);
    if !p.is_dir() {
        return Err("that path is not a folder".into());
    }
    let kind = kind.unwrap_or_default();
    if !kind.is_empty() && kind != "movie" && kind != "show" {
        return Err("kind must be 'movie', 'show', or empty".into());
    }
    if !safe_user_media_root(&path) {
        return Err("choose a specific media folder, not a filesystem or home root".into());
    }
    // Let the webview load poster images from this folder, after all validation.
    {
        use tauri::Manager;
        let _ = app.asset_protocol_scope().allow_directory(&path, true);
    }
    let folder = LocalFolder {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.unwrap_or_else(|| {
            p.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Local")
                .to_string()
        }),
        path,
        kind,
    };
    mutate_then_rebuild_local(&state, move |cfg| {
        cfg.local_folders.push(folder);
        Ok(())
    })
    .await?;
    Ok(SourceDto {
        id: LOCAL_SOURCE_ID.to_string(),
        name: "Local".to_string(),
        kind: "local".to_string(),
    })
}

/// List configured local folders (for the settings UI).
#[tauri::command]
pub async fn list_local_folders() -> Result<Vec<LocalFolderDto>, String> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    Ok(cfg
        .local_folders
        .into_iter()
        .map(|f| LocalFolderDto {
            id: f.id,
            name: f.name,
            path: f.path,
            kind: f.kind,
        })
        .collect())
}

/// Remove a local folder by id.
#[tauri::command]
pub async fn remove_local_folder(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let id2 = id.clone();
    mutate_then_rebuild_local(&state, move |cfg| {
        // A folder backing a remote mount must be removed via its mount row, or
        // we'd leave an orphaned mount that still remounts on launch with no source.
        if cfg.ssh_mounts.iter().any(|m| m.local_folder_id == id2) {
            return Err(
                "this folder is provided by an SSH/SFTP mount — unmount it instead".to_string(),
            );
        }
        let before = cfg.local_folders.len();
        cfg.local_folders.retain(|f| f.id != id2);
        if cfg.local_folders.len() == before {
            return Err("no such folder".to_string());
        }
        Ok(())
    })
    .await?;
    Ok(())
}

/// A configured SMB mount, for the settings UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmbMountDto {
    id: String,
    name: String,
    server: String,
    share: String,
    mountpoint: String,
    folders: Vec<SmbFolderDto>,
}

/// A selected SMB folder, for the settings UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmbFolderDto {
    id: String,
    name: String,
    path: String,
    relative_path: String,
    kind: String,
}

/// One directory inside a mounted SMB share, for the settings browser.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmbDirectoryDto {
    name: String,
    path: String,
}

/// A configured SSH/SFTP mount, for the settings UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshMountDto {
    id: String,
    name: String,
    host: String,
    port: u16,
    username: String,
    remote_path: String,
    identity_file: String,
    mountpoint: String,
}

/// Mount an SMB share via the OS and persist it for browsing/selection. Library
/// folders are added separately with `add_smb_folder`.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri command surface; each is a distinct field.
pub async fn mount_smb(
    server: String,
    share: String,
    username: String,
    password: String,
    domain: Option<String>,
    name: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SourceDto, String> {
    if server.trim().is_empty() || share.trim().is_empty() {
        return Err("server and share are required".into());
    }
    // Reject a duplicate up front, before doing the (blocking, credentialed) OS
    // mount. Fail if config can't even be read, so we don't mount on an
    // unreadable config and risk an unrecoverable orphan if the later persist
    // fails too. The authoritative check is still in the persist closure below.
    let existing = config::load_config().map_err(|e| format!("could not read config: {e}"))?;
    if existing.smb_mounts.iter().any(|x| {
        x.server.eq_ignore_ascii_case(server.trim()) && x.share.eq_ignore_ascii_case(share.trim())
    }) {
        return Err("that share is already added".into());
    }
    let mut mount = SmbMount {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.unwrap_or_else(|| format!("{}/{}", server.trim(), share.trim())),
        server: server.trim().to_string(),
        share: share.trim().to_string(),
        username,
        password,
        domain: domain.unwrap_or_default(),
        mountpoint: String::new(),
        folders: Vec::new(),
        kind: String::new(),
        local_folder_id: String::new(),
    };
    // Hold source_lock across the whole mount→persist→rollback. Otherwise a
    // concurrent unmount_smb could, between our OS mount and our persist, see no
    // record referencing this mountpoint and tear it down — leaving our
    // just-persisted record pointing at a dead connection (the shared Windows UNC
    // makes this concrete). The registry/play locks are NOT held here, so
    // browsing and playback proceed while the (possibly slow) mount runs.
    let _ops = state.source_lock.lock().await;

    // The OS mount can block for seconds (or hang), so run it off the async runtime.
    let m_for_mount = mount.clone();
    mount = tauri::async_runtime::spawn_blocking(move || {
        let mut mounted = m_for_mount;
        crate::smb::prepare_mount(&mut mounted)?;
        Ok::<SmbMount, String>(mounted)
    })
    .await
    .map_err(|e| format!("mount task failed: {e}"))??;

    {
        use tauri::Manager;
        let _ = app
            .asset_protocol_scope()
            .allow_directory(&mount.mountpoint, true);
    }

    // Roll back the OS mount if we can't persist it, so we don't leave a live
    // mount with no app-managed record. rebuild_local_locked assumes source_lock
    // is held (it is, above) — so the persist and the rollback both run inside the
    // same critical section as the mount.
    let mountpoint = mount.mountpoint.clone();
    if let Err(e) = rebuild_local_locked(&state, move |cfg| {
        // Reject a duplicate of an already-configured share (same server/share),
        // so we never persist two records over one shared connection/mountpoint.
        if cfg.smb_mounts.iter().any(|x| {
            x.server.eq_ignore_ascii_case(&mount.server)
                && x.share.eq_ignore_ascii_case(&mount.share)
        }) {
            return Err("that share is already added".to_string());
        }
        cfg.smb_mounts.push(mount);
        Ok(())
    })
    .await
    {
        // Persisting failed: roll back the OS mount we just created — unless
        // another record *positively* still references this mountpoint (the
        // concurrent-duplicate case), in which case unmounting would disconnect
        // it. We still hold source_lock, so no concurrent op can persist this
        // mountpoint between the check and the unmount. On a read error we still
        // clean up: dropping our just-created mount beats orphaning a credentialed
        // mount with no record. Run the unmount off the async runtime.
        if mountpoint_referenced(&mountpoint) != Some(true) {
            let mp = mountpoint.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || crate::smb::unmount(&mp)).await;
        }
        return Err(e);
    }
    Ok(SourceDto {
        id: LOCAL_SOURCE_ID.to_string(),
        name: "Local".to_string(),
        kind: "local".to_string(),
    })
}

/// List configured SMB mounts (for the settings UI).
#[tauri::command]
pub async fn list_smb_mounts() -> Result<Vec<SmbMountDto>, String> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    Ok(cfg
        .smb_mounts
        .into_iter()
        .map(|m| SmbMountDto {
            folders: smb_folders_for_ui(&m),
            id: m.id,
            name: m.name,
            server: m.server,
            share: m.share,
            mountpoint: m.mountpoint,
        })
        .collect())
}

/// List directories inside a configured SMB share, relative to the mounted
/// share root. Used by Settings to choose one or more library folders after the
/// share is mounted.
#[tauri::command]
pub async fn list_smb_directories(
    id: String,
    path: Option<String>,
) -> Result<Vec<SmbDirectoryDto>, String> {
    let relative = normalize_smb_relative_path(path.as_deref().unwrap_or(""))?;
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let mount = cfg
        .smb_mounts
        .into_iter()
        .find(|m| m.id == id)
        .ok_or("no such SMB mount")?;
    let root = smb_mount_root(&mount).ok_or_else(|| {
        format!(
            "SMB share //{}/{} is not mounted or readable",
            mount.server, mount.share
        )
    })?;
    let dir = smb_pathbuf_for_relative(&root, &relative);
    if !dir.is_dir() {
        return Err("that SMB folder is not readable".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut dirs = Vec::new();
        for entry in std::fs::read_dir(&dir).map_err(|e| format!("could not read folder: {e}"))? {
            let entry = entry.map_err(|e| format!("could not read folder entry: {e}"))?;
            if !entry.path().is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            dirs.push(SmbDirectoryDto {
                path: append_smb_relative_path(&relative, &name),
                name,
            });
        }
        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok::<_, String>(dirs)
    })
    .await
    .map_err(|e| format!("SMB browse task failed: {e}"))?
}

/// Add one selected folder inside a mounted SMB share to the local source.
#[tauri::command]
pub async fn add_smb_folder(
    id: String,
    path: String,
    kind: Option<String>,
    name: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SourceDto, String> {
    let kind = kind.unwrap_or_default();
    if !kind.is_empty() && kind != "movie" && kind != "show" {
        return Err("kind must be 'movie', 'show', or empty".into());
    }
    let relative = normalize_smb_relative_path(&path)?;
    let _ops = state.source_lock.lock().await;
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    let mount = cfg
        .smb_mounts
        .iter()
        .find(|m| m.id == id)
        .ok_or("no such SMB mount")?;
    let root = smb_mount_root(mount).ok_or_else(|| {
        format!(
            "SMB share //{}/{} is not mounted or readable",
            mount.server, mount.share
        )
    })?;
    let folder_path = smb_path_string_for_relative(&root, &relative);
    if !Path::new(&folder_path).is_dir() {
        return Err("that SMB folder is not readable".into());
    }
    if !safe_user_media_root(&folder_path) {
        return Err("choose a specific media folder, not a filesystem or home root".into());
    }
    {
        use tauri::Manager;
        let _ = app
            .asset_protocol_scope()
            .allow_directory(&folder_path, true);
    }
    let folder = SmbFolder {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.unwrap_or_else(|| smb_folder_display_name(mount, &relative)),
        path: relative.clone(),
        kind,
    };
    let mount_id = id.clone();
    let relative_for_check = relative.clone();
    rebuild_local_locked(&state, move |cfg| {
        let mount = cfg
            .smb_mounts
            .iter_mut()
            .find(|m| m.id == mount_id)
            .ok_or_else(|| "no such SMB mount".to_string())?;
        if selected_smb_folders(mount)
            .iter()
            .any(|existing| existing.path == relative_for_check)
        {
            return Err("that SMB folder is already added".to_string());
        }
        mount.folders.push(folder);
        Ok(())
    })
    .await?;
    Ok(SourceDto {
        id: LOCAL_SOURCE_ID.to_string(),
        name: "Local".to_string(),
        kind: "local".to_string(),
    })
}

/// Remove one selected folder from an SMB share without unmounting the share.
#[tauri::command]
pub async fn remove_smb_folder(
    id: String,
    folder_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    mutate_then_rebuild_local(&state, move |cfg| {
        let mount = cfg
            .smb_mounts
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or_else(|| "no such SMB mount".to_string())?;
        let before = mount.folders.len();
        mount.folders.retain(|folder| folder.id != folder_id);
        let removed = mount.folders.len() != before;
        if !removed {
            return Err("no such SMB folder".to_string());
        }
        Ok(())
    })
    .await
}

fn smb_folders_for_ui(m: &SmbMount) -> Vec<SmbFolderDto> {
    let root = smb_mount_root(m);
    selected_smb_folders(m)
        .iter()
        .map(|folder| {
            let relative_path = folder.path.clone();
            let path = root
                .as_ref()
                .map(|root| smb_path_string_for_relative(root, &relative_path))
                .unwrap_or_else(|| relative_path.clone());
            SmbFolderDto {
                id: folder.id.clone(),
                name: folder.name.clone(),
                path,
                relative_path,
                kind: folder.kind.clone(),
            }
        })
        .collect()
}

fn selected_smb_folders(m: &SmbMount) -> &[SmbFolder] {
    &m.folders
}

fn normalize_smb_relative_path(path: &str) -> Result<String, String> {
    let raw = path.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }
    if raw.starts_with('/') || raw.starts_with('\\') || Path::new(raw).is_absolute() {
        return Err("SMB folder path must be relative to the share".into());
    }
    let normalized = raw.replace('\\', "/");
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let mut parts = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .ok_or_else(|| "SMB folder path must be valid UTF-8".to_string())?;
                if !part.is_empty() {
                    parts.push(part.to_string());
                }
            }
            Component::CurDir => {}
            _ => return Err("SMB folder path must stay inside the share".into()),
        }
    }
    Ok(parts.join("/"))
}

fn append_smb_relative_path(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else {
        format!("{base}/{name}")
    }
}

fn smb_pathbuf_for_relative(root: &str, relative: &str) -> PathBuf {
    let mut path = PathBuf::from(root);
    for part in relative.split('/').filter(|part| !part.is_empty()) {
        path.push(part);
    }
    path
}

fn smb_path_string_for_relative(root: &str, relative: &str) -> String {
    smb_pathbuf_for_relative(root, relative)
        .to_string_lossy()
        .to_string()
}

fn smb_folder_display_name(m: &SmbMount, relative: &str) -> String {
    relative
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(&m.name)
        .to_string()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn smb_mount_root(m: &SmbMount) -> Option<String> {
    crate::smb::resolved_mountpoint(m).filter(|path| Path::new(path).is_dir())
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn smb_mount_root(m: &SmbMount) -> Option<String> {
    if Path::new(&m.mountpoint).is_dir() {
        Some(m.mountpoint.clone())
    } else {
        None
    }
}

/// Unmount an SMB share and drop its selected folders from the local source.
#[tauri::command]
pub async fn unmount_smb(id: String, state: State<'_, AppState>) -> Result<(), String> {
    // Remove the config records and rebuild the local source FIRST, then tear down
    // the OS mount best-effort: config is the source of truth, so we never leave a
    // record pointing at an unmounted folder, and a failed/slow OS unmount just
    // means the connection lingers rather than corrupting state.
    let m = config::load_config()
        .map_err(|e| e.to_string())?
        .smb_mounts
        .into_iter()
        .find(|m| m.id == id)
        .ok_or("no such mount")?;
    // Remove the records first — config is the source of truth, written
    // atomically under lock. If this fails, nothing else changes (no dangling
    // record, no orphaned mount). Only then tear down the OS mount, best-effort:
    // a failure just means the connection lingers until closed/rebooted, but we
    // never leave a record pointing at an unmounted folder, never force-close an
    // in-use mount, and never resurrect one.
    // Remove records + rebuild the live source atomically BEFORE the (possibly
    // slow/hung) OS teardown, so playback can't route through the now-removed
    // folder meanwhile. The lock-held existence check means two concurrent
    // unmounts don't both report success for the same (already-removed) mount.
    mutate_then_rebuild_local(&state, move |cfg| {
        if !cfg.smb_mounts.iter().any(|x| x.id == id) {
            return Err("no such mount".to_string());
        }
        cfg.smb_mounts.retain(|x| x.id != id);
        Ok(())
    })
    .await?;
    // Only tear down the OS mount if no remaining record *positively* references
    // it (duplicate records can share a mountpoint, esp. the Windows UNC). Fail
    // closed: on a read error (None) keep the mount rather than risk
    // disconnecting a share another record still uses. Hold source_lock across
    // the reference check + teardown so a concurrent mount_smb can't persist this
    // mountpoint between them and have its fresh connection torn down.
    {
        let _ops = state.source_lock.lock().await;
        if mountpoint_referenced(&m.mountpoint) == Some(false) {
            // umount / net use can block, so run it off the async runtime.
            let mp = m.mountpoint.clone();
            let _ =
                tauri::async_runtime::spawn_blocking(move || crate::smb::unmount_for_removal(&mp))
                    .await;
        }
    }
    Ok(())
}

/// Mount an SSH/SFTP folder via sshfs, then register its mountpoint as a local
/// folder so the local source browses it. Authentication is handled by OpenSSH
/// keys, agent, and config; Vela stores no SSH password.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri command surface; each is a distinct field.
pub async fn mount_ssh(
    host: String,
    port: Option<u16>,
    username: String,
    remote_path: String,
    identity_file: Option<String>,
    kind: Option<String>,
    name: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SourceDto, String> {
    if host.trim().is_empty() || remote_path.trim().is_empty() {
        return Err("host and remote path are required".into());
    }
    let port = port.unwrap_or(22);
    if port == 0 {
        return Err("port must be between 1 and 65535".into());
    }
    let kind = kind.unwrap_or_default();
    if !kind.is_empty() && kind != "movie" && kind != "show" {
        return Err("kind must be 'movie', 'show', or empty".into());
    }
    let existing = config::load_config().map_err(|e| format!("could not read config: {e}"))?;
    if existing.ssh_mounts.iter().any(|x| {
        x.host.eq_ignore_ascii_case(host.trim())
            && x.port == port
            && x.username == username.trim()
            && x.remote_path == remote_path.trim()
    }) {
        return Err("that SSH/SFTP folder is already added".into());
    }

    let folder_id = uuid::Uuid::new_v4().to_string();
    let mut mount = SshMount {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.unwrap_or_else(|| format!("{}:{}", host.trim(), remote_path.trim())),
        host: host.trim().to_string(),
        port,
        username: username.trim().to_string(),
        remote_path: remote_path.trim().to_string(),
        identity_file: identity_file.unwrap_or_default().trim().to_string(),
        kind: kind.clone(),
        mountpoint: String::new(),
        local_folder_id: folder_id.clone(),
    };

    let _ops = state.source_lock.lock().await;

    let m_for_mount = mount.clone();
    mount = tauri::async_runtime::spawn_blocking(move || {
        let mut mounted = m_for_mount;
        crate::sshfs::prepare_mount(&mut mounted)?;
        Ok::<SshMount, String>(mounted)
    })
    .await
    .map_err(|e| format!("mount task failed: {e}"))??;

    {
        use tauri::Manager;
        let _ = app
            .asset_protocol_scope()
            .allow_directory(&mount.mountpoint, true);
    }

    let folder = LocalFolder {
        id: folder_id,
        name: mount.name.clone(),
        path: mount.mountpoint.clone(),
        kind,
    };
    let mountpoint = mount.mountpoint.clone();
    if let Err(e) = rebuild_local_locked(&state, move |cfg| {
        if cfg.ssh_mounts.iter().any(|x| {
            x.host.eq_ignore_ascii_case(&mount.host)
                && x.port == mount.port
                && x.username == mount.username
                && x.remote_path == mount.remote_path
        }) {
            return Err("that SSH/SFTP folder is already added".to_string());
        }
        cfg.local_folders.push(folder);
        cfg.ssh_mounts.push(mount);
        Ok(())
    })
    .await
    {
        if ssh_mountpoint_referenced(&mountpoint) != Some(true) {
            let mp = mountpoint.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || crate::sshfs::unmount(&mp)).await;
        }
        return Err(e);
    }
    Ok(SourceDto {
        id: LOCAL_SOURCE_ID.to_string(),
        name: "Local".to_string(),
        kind: "local".to_string(),
    })
}

/// List configured SSH/SFTP mounts (for the settings UI).
#[tauri::command]
pub async fn list_ssh_mounts() -> Result<Vec<SshMountDto>, String> {
    let cfg = config::load_config().map_err(|e| e.to_string())?;
    Ok(cfg
        .ssh_mounts
        .into_iter()
        .map(|m| SshMountDto {
            id: m.id,
            name: m.name,
            host: m.host,
            port: m.port,
            username: m.username,
            remote_path: m.remote_path,
            identity_file: m.identity_file,
            mountpoint: m.mountpoint,
        })
        .collect())
}

/// Unmount an SSH/SFTP folder and drop the local folder it fed.
#[tauri::command]
pub async fn unmount_ssh(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let m = config::load_config()
        .map_err(|e| e.to_string())?
        .ssh_mounts
        .into_iter()
        .find(|m| m.id == id)
        .ok_or("no such mount")?;
    let folder_id = m.local_folder_id.clone();
    mutate_then_rebuild_local(&state, move |cfg| {
        if !cfg.ssh_mounts.iter().any(|x| x.id == id) {
            return Err("no such mount".to_string());
        }
        cfg.ssh_mounts.retain(|x| x.id != id);
        cfg.local_folders.retain(|f| f.id != folder_id);
        Ok(())
    })
    .await?;
    {
        let _ops = state.source_lock.lock().await;
        if ssh_mountpoint_referenced(&m.mountpoint) == Some(false) {
            let mp = m.mountpoint.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                crate::sshfs::unmount_for_removal(&mp)
            })
            .await;
        }
    }
    Ok(())
}

/// Whether another persisted SMB record references `mountpoint`. `None` = the
/// config couldn't be read; the caller decides how to treat that uncertainty —
/// teardown of an existing record fails closed (keep a maybe-shared mount, since
/// records can share a mountpoint, notably the Windows UNC), while rollback of a
/// just-created mount fails open (clean it up rather than orphan a credentialed
/// mount with no record).
fn mountpoint_referenced(mountpoint: &str) -> Option<bool> {
    config::load_config()
        .ok()
        .map(|c| c.smb_mounts.iter().any(|x| x.mountpoint == mountpoint))
}

fn ssh_mountpoint_referenced(mountpoint: &str) -> Option<bool> {
    config::load_config()
        .ok()
        .map(|c| c.ssh_mounts.iter().any(|x| x.mountpoint == mountpoint))
}

fn live_local_folders(cfg: &AppConfig) -> Vec<LocalFolder> {
    let ssh_folder_ids: std::collections::HashSet<_> = cfg
        .ssh_mounts
        .iter()
        .map(|m| m.local_folder_id.as_str())
        .collect();
    let mut folders: Vec<_> = cfg
        .local_folders
        .iter()
        .filter(|f| !ssh_folder_ids.contains(f.id.as_str()))
        .filter(|f| safe_user_media_root(&f.path))
        .cloned()
        .collect();
    folders.extend(cfg.smb_mounts.iter().flat_map(smb_live_folders));
    folders.extend(cfg.ssh_mounts.iter().filter_map(ssh_live_folder));
    folders
}

fn smb_live_folders(m: &SmbMount) -> Vec<LocalFolder> {
    let Some(root) = smb_mount_root(m) else {
        return Vec::new();
    };
    selected_smb_folders(m)
        .iter()
        .filter_map(|folder| {
            let path = smb_path_string_for_relative(&root, &folder.path);
            if !safe_user_media_root(&path) {
                return None;
            }
            Some(LocalFolder {
                id: folder.id.clone(),
                name: folder.name.clone(),
                path,
                kind: folder.kind.clone(),
            })
        })
        .collect()
}

fn ssh_live_folder(m: &SshMount) -> Option<LocalFolder> {
    if !crate::sshfs::is_active_mount(m) || !safe_user_media_root(&m.mountpoint) {
        return None;
    }
    Some(LocalFolder {
        id: m.local_folder_id.clone(),
        name: m.name.clone(),
        path: m.mountpoint.clone(),
        kind: m.kind.clone(),
    })
}

/// Apply a config mutation, then rebuild the local source from what was just
/// persisted. Serializes source mutations via `source_lock` (so they apply in
/// order) without holding the registry lock across config file I/O.
async fn mutate_then_rebuild_local<F>(state: &State<'_, AppState>, f: F) -> Result<(), String>
where
    F: FnOnce(&mut AppConfig) -> Result<(), String>,
{
    // Serialize source mutations with their own lock so they apply in order —
    // WITHOUT holding the registry lock across config::update's file I/O (and its
    // cross-process flock), which would block browsing commands meanwhile.
    let _ops = state.source_lock.lock().await;
    rebuild_local_locked(state, f).await
}

/// The persist + registry-swap half of `mutate_then_rebuild_local`, WITHOUT
/// acquiring `source_lock`. For callers that already hold it because they need a
/// wider critical section (e.g. `mount_smb`, which must keep the OS mount and the
/// persist atomic against a concurrent teardown). The registry lock is still
/// taken only briefly, for the swap-in.
async fn rebuild_local_locked<F>(state: &State<'_, AppState>, f: F) -> Result<(), String>
where
    F: FnOnce(&mut AppConfig) -> Result<(), String>,
{
    let folders = config::update(|cfg| {
        f(cfg)?;
        Ok(live_local_folders(cfg))
    })?;
    let mut reg = state.registry.lock().await;
    if folders.is_empty() {
        reg.remove(LOCAL_SOURCE_ID);
    } else {
        reg.upsert(std::sync::Arc::new(LocalSource::new(folders)));
    }
    Ok(())
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
}

#[tauri::command]
pub fn get_mpv_advanced() -> MpvAdvanced {
    let cfg = config::load_config().unwrap_or_default();
    MpvAdvanced {
        extra_args: cfg.mpv_extra_args.unwrap_or_default(),
        use_own_config: cfg.mpv_use_own_config.unwrap_or(false),
    }
}

/// Persist the advanced mpv settings. No validation here — these are the user's own
/// machine and their own call; a bad option just makes mpv refuse to launch, which
/// surfaces as a normal playback error. An empty `extra_args` clears the override.
#[tauri::command]
pub fn set_mpv_advanced(extra_args: String, use_own_config: bool) -> Result<(), String> {
    let trimmed = extra_args.trim().to_string();
    config::update(move |cfg| {
        cfg.mpv_extra_args = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        cfg.mpv_use_own_config = Some(use_own_config);
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

/// Mark an item watched/unwatched on its source. Routes by the namespaced key;
/// the registry lock is released before the (network) call.
#[tauri::command]
pub async fn set_watched(
    rating_key: String,
    played: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (src, raw) = state.registry.lock().await.route(&rating_key)?;
    src.mark_played(&raw, played).await
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

    let url = resolved.url;
    let start = resolved.resume_ms as f64 / 1000.0;
    let progress = resolved.progress;
    let child_slot = state.current_child.clone();
    let shutting_down = state.shutting_down.clone();
    let advance = state.queue_advance.clone();
    let stop = tauri::async_runtime::spawn_blocking(move || {
        playback::play(&url, start, progress, &child_slot, &shutting_down, &advance)
    })
    .await
    .map_err(|e| format!("playback task failed: {e}"))??;
    *state
        .tracking_stop
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(stop);
    Ok(())
}

/// Top-level Play: replace the queue with just this item and play it. The user
/// asked to start over from here, so the existing queue (if any) is cleared.
#[tauri::command]
pub async fn play_item(item: QueueItem, state: State<'_, AppState>) -> Result<(), String> {
    let rating_key = item.rating_key.clone();
    let duration_ms = item.duration_ms;
    {
        let mut q = state.queue.lock().unwrap_or_else(|e| e.into_inner());
        *q = vec![item];
    }
    *state.queue_index.lock().unwrap_or_else(|e| e.into_inner()) = Some(0);
    play_by_key(&state, &rating_key, duration_ms).await
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
    play_by_key(&state, &item.rating_key, item.duration_ms).await
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

fn safe_user_media_root(path: &str) -> bool {
    let Ok(canon) = std::fs::canonicalize(path) else {
        return false;
    };
    if canon.parent().is_none() {
        return false;
    }
    if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
        if std::fs::canonicalize(home).ok().as_ref() == Some(&canon) {
            return false;
        }
    }
    true
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
    fn smb_relative_paths_are_normalized() {
        assert_eq!(normalize_smb_relative_path("").unwrap(), "");
        assert_eq!(
            normalize_smb_relative_path("Movies\\4K").unwrap(),
            "Movies/4K"
        );
        assert_eq!(
            normalize_smb_relative_path("Shows/Season 01").unwrap(),
            "Shows/Season 01"
        );
    }

    #[test]
    fn smb_relative_paths_cannot_escape_share() {
        assert!(normalize_smb_relative_path("/Movies").is_err());
        assert!(normalize_smb_relative_path("../Movies").is_err());
        assert!(normalize_smb_relative_path("Movies/../Shows").is_err());
    }

    #[test]
    fn weak_pin_uses_plex_link_url() {
        assert_eq!(plex_link_url("ABCD"), "https://plex.tv/link/?pin=ABCD");
    }
}
