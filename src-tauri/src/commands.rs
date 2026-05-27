use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::State;

use crate::config::{self, AppConfig, LocalFolder, SmbMount, SourceConfig, SshMount};
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
const BUILD_DATE: &str = "2026-05-27";

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
    /// Per-OS install command the user can copy (e.g. `brew install mpv`).
    install_command: String,
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
        if cfg.smb_mounts.iter().any(|m| m.local_folder_id == id2) {
            return Err("this folder is provided by an SMB mount — unmount it instead".to_string());
        }
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

/// Mount an SMB share via the OS, then register its mountpoint as a local
/// folder so the local source browses it. Persists the mount for remount on
/// next launch.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri command surface; each is a distinct field.
pub async fn mount_smb(
    server: String,
    share: String,
    username: String,
    password: String,
    domain: Option<String>,
    kind: Option<String>,
    name: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SourceDto, String> {
    if server.trim().is_empty() || share.trim().is_empty() {
        return Err("server and share are required".into());
    }
    let kind = kind.unwrap_or_default();
    if !kind.is_empty() && kind != "movie" && kind != "show" {
        return Err("kind must be 'movie', 'show', or empty".into());
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
    let folder_id = uuid::Uuid::new_v4().to_string();
    let mut mount = SmbMount {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.unwrap_or_else(|| format!("{}/{}", server.trim(), share.trim())),
        server: server.trim().to_string(),
        share: share.trim().to_string(),
        username,
        password,
        domain: domain.unwrap_or_default(),
        kind: kind.clone(),
        mountpoint: String::new(),
        local_folder_id: folder_id.clone(),
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

    let folder = LocalFolder {
        id: folder_id,
        name: mount.name.clone(),
        path: mount.mountpoint.clone(),
        kind,
    };
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
        cfg.local_folders.push(folder);
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
            id: m.id,
            name: m.name,
            server: m.server,
            share: m.share,
            mountpoint: m.mountpoint,
        })
        .collect())
}

/// Unmount an SMB share and drop the local folder it fed.
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
    let folder_id = m.local_folder_id.clone();
    // Remove records + rebuild the live source atomically BEFORE the (possibly
    // slow/hung) OS teardown, so playback can't route through the now-removed
    // folder meanwhile. The lock-held existence check means two concurrent
    // unmounts don't both report success for the same (already-removed) mount.
    mutate_then_rebuild_local(&state, move |cfg| {
        if !cfg.smb_mounts.iter().any(|x| x.id == id) {
            return Err("no such mount".to_string());
        }
        cfg.smb_mounts.retain(|x| x.id != id);
        cfg.local_folders.retain(|f| f.id != folder_id);
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
    let smb_folder_ids: std::collections::HashSet<_> = cfg
        .smb_mounts
        .iter()
        .map(|m| m.local_folder_id.as_str())
        .collect();
    let ssh_folder_ids: std::collections::HashSet<_> = cfg
        .ssh_mounts
        .iter()
        .map(|m| m.local_folder_id.as_str())
        .collect();
    let mut folders: Vec<_> = cfg
        .local_folders
        .iter()
        .filter(|f| !smb_folder_ids.contains(f.id.as_str()))
        .filter(|f| !ssh_folder_ids.contains(f.id.as_str()))
        .filter(|f| safe_user_media_root(&f.path))
        .cloned()
        .collect();
    folders.extend(cfg.smb_mounts.iter().filter_map(smb_live_folder));
    folders.extend(cfg.ssh_mounts.iter().filter_map(ssh_live_folder));
    folders
}

#[cfg(all(unix, not(target_os = "macos")))]
fn smb_live_folder(m: &SmbMount) -> Option<LocalFolder> {
    crate::smb::resolved_mountpoint(m).and_then(|path| {
        if !safe_user_media_root(&path) {
            return None;
        }
        Some(LocalFolder {
            id: m.local_folder_id.clone(),
            name: m.name.clone(),
            path,
            kind: m.kind.clone(),
        })
    })
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn smb_live_folder(m: &SmbMount) -> Option<LocalFolder> {
    if !safe_user_media_root(&m.mountpoint) {
        return None;
    }
    Some(LocalFolder {
        id: m.local_folder_id.clone(),
        name: m.name.clone(),
        path: m.mountpoint.clone(),
        kind: m.kind.clone(),
    })
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

/// Whether mpv is available, plus a per-OS install hint for the UI.
#[tauri::command]
pub fn check_mpv() -> MpvInfo {
    let (install_command, install_url) = if cfg!(target_os = "macos") {
        ("brew install mpv", "https://mpv.io/installation/")
    } else if cfg!(target_os = "windows") {
        ("winget install mpv", "https://mpv.io/installation/")
    } else {
        (
            "sudo apt install mpv   # or your distro's package manager",
            "https://mpv.io/installation/",
        )
    };
    MpvInfo {
        available: playback::resolve_mpv().is_some(),
        install_command: install_command.to_string(),
        install_url: install_url.to_string(),
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

#[tauri::command]
pub async fn play_item(
    rating_key: String,
    duration_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Serialize the whole resolve+stop-old+spawn sequence so overlapping clicks
    // can't both spawn an mpv and lose one of the child handles.
    let _play = state.play_lock.lock().await;
    let (src, raw) = state.registry.lock().await.route(&rating_key)?;
    let resolved = src.resolve_stream(&raw, duration_ms).await?;

    // Cancel the prior tracker and terminate the prior mpv so we never run two
    // players. The kill is a non-blocking syscall; the (possibly unbounded) reap
    // runs on its own native thread (see below).
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
        // Send the kill NOW, synchronously — kill() is a non-blocking syscall, so
        // the old mpv is signalled before we spawn the replacement. Hand the
        // killed child to the reap queue (drained by the periodic reaper) rather
        // than spawning a per-child waiter thread: a thread spawn could fail under
        // thread exhaustion and drop the child unreaped, and a blocking wait()
        // could hang here if the player wedged on a hung mount.
        let _ = child.kill();
        state
            .reap_queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(child);
    }

    // playback::play execs mpv (and probes `mpv --version`), so run it off the
    // async runtime too. It publishes the child into current_child itself, the
    // instant mpv launches, so an app-exit racing tracker setup still finds it.
    let url = resolved.url;
    let start = resolved.resume_ms as f64 / 1000.0;
    let progress = resolved.progress;
    let child_slot = state.current_child.clone();
    let shutting_down = state.shutting_down.clone();
    let stop = tauri::async_runtime::spawn_blocking(move || {
        playback::play(&url, start, progress, &child_slot, &shutting_down)
    })
    .await
    .map_err(|e| format!("playback task failed: {e}"))??;
    *state
        .tracking_stop
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(stop);
    Ok(())
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
    fn weak_pin_uses_plex_link_url() {
        assert_eq!(plex_link_url("ABCD"), "https://plex.tv/link/?pin=ABCD");
    }
}
