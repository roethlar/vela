mod commands;
mod config;
mod playback;
mod plex_api;
mod plex_library;
mod recents;
mod smb;
#[cfg(all(unix, not(target_os = "macos")))]
mod smb_client;
mod source;
mod sshfs;
mod ui_events;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

use plex_library::PlexLibrary;
use source::{plex::PlexSource, SourceRegistry};

/// Stable id for the (single) Plex source. Multi-server support can suffix this.
pub const PLEX_SOURCE_ID: &str = "plex";

/// Human-friendly OS name reported to media servers (X-Plex-Platform, etc.).
pub fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    }
}

/// Shared application state, managed by Tauri and injected into commands.
pub struct AppState {
    /// All configured media sources (Plex, Jellyfin/Emby, and a local source).
    pub registry: AsyncMutex<SourceRegistry>,
    /// Stop flag for the currently-tracked playback (so a new play cancels the old tracker).
    pub tracking_stop: Mutex<Option<Arc<AtomicBool>>>,
    /// The currently-running mpv process, so a new play can terminate the old one.
    /// `Arc` so a background reaper can also try_wait() it (non-blocking) and clear
    /// the slot once it exits on its own, instead of leaving a zombie.
    pub current_child: Arc<Mutex<Option<std::process::Child>>>,
    /// Set as the app exits. Checked under `current_child`'s lock before launching
    /// mpv, so a play that races shutdown either sees this (and doesn't launch) or
    /// has already registered its child for the exit sweep to kill — no orphan.
    pub shutting_down: Arc<AtomicBool>,
    /// Already-killed players handed off for reaping. The periodic reaper drains
    /// this with non-blocking try_wait(), so replacing a player never needs to
    /// spawn a per-child waiter thread (which could fail under thread exhaustion
    /// and drop the child unreaped) nor block on wait() (which could hang on a
    /// wedged player).
    pub reap_queue: Arc<Mutex<Vec<std::process::Child>>>,
    /// Serializes play_item so overlapping clicks can't both spawn and orphan an mpv.
    pub play_lock: AsyncMutex<()>,
    /// Serializes source mutations (add/remove folder, mount/unmount SMB) so they
    /// apply in order without holding the registry lock across config file I/O.
    pub source_lock: AsyncMutex<()>,
    /// In-memory play queue. Cleared+repopulated by a top-level "Play"; mutated by
    /// "Play Next" / "Add to Queue". Not persisted across restarts.
    pub queue: Arc<Mutex<Vec<commands::QueueItem>>>,
    /// Index of the currently-playing item in `queue`, or `None` if nothing is
    /// playing. The auto-advance task moves this forward on natural mpv EOF.
    pub queue_index: Arc<Mutex<Option<usize>>>,
    /// Signaled by the mpv EOF watcher when a file ends naturally (not when the
    /// user closed the window). An async dispatcher in `run()` awaits this and
    /// plays the next queued item, so closing mpv stops playback while watching
    /// to the end continues to the next.
    pub queue_advance: Arc<tokio::sync::Notify>,
    /// The Tauri app handle, set once at setup. Lets non-command code (the
    /// playback tracker tails) emit UI events such as `playback-ended`.
    pub app_handle: std::sync::OnceLock<tauri::AppHandle>,
    /// The materialized merged All-view listing: built in full when a type
    /// listing is entered, windowed immutably by continuation pages so
    /// paging can never skip or duplicate titles (see `get_type_listing`).
    pub merged_snapshot: AsyncMutex<Option<commands::MergedSnapshot>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Build the source registry from saved config so we can serve data immediately.
    // A parse failure starts empty but leaves the file intact (config::update
    // refuses to overwrite an unreadable config), so nothing is wiped.
    let cfg = config::load_config().unwrap_or_else(|e| {
        eprintln!(
            "vela: config unreadable ({e}); starting with no saved sources (file left intact)"
        );
        config::AppConfig::default()
    });
    let mut registry = SourceRegistry::default();
    if let (Some(token), Some(cid)) = (cfg.auth_token.clone(), cfg.client_identifier.clone()) {
        let mut lib = PlexLibrary::new(token, cid);
        if let (Some(host), Some(port), Some(scheme)) = (
            cfg.last_server_host.clone(),
            cfg.last_server_port,
            cfg.last_server_scheme.clone(),
        ) {
            if scheme == "https" {
                lib.set_server_manual(host, port, true, Some("Saved Server".to_string()));
            }
        }
        registry.upsert(Arc::new(PlexSource::new(PLEX_SOURCE_ID, "Plex", lib)));
    }
    // Restore any configured Jellyfin/Emby sources.
    for src_cfg in &cfg.sources {
        if let Some(src) = source::jellyfin::build_source(src_cfg) {
            registry.upsert(src);
        }
    }
    // Restore the local family: plain folders as "Local", plus one named
    // source per SMB/SSH mount so shares aren't presented as "Local".
    let local_family = source::local::local_family(
        &cfg,
        smb_runtime_folders,
        ssh_runtime_folder,
        safe_user_media_root,
    );
    for member in &local_family {
        registry.upsert(member.build());
    }

    let state = AppState {
        registry: AsyncMutex::new(registry),
        tracking_stop: Mutex::new(None),
        current_child: Arc::new(Mutex::new(None)),
        shutting_down: Arc::new(AtomicBool::new(false)),
        reap_queue: Arc::new(Mutex::new(Vec::new())),
        play_lock: AsyncMutex::new(()),
        source_lock: AsyncMutex::new(()),
        queue: Arc::new(Mutex::new(Vec::new())),
        queue_index: Arc::new(Mutex::new(None)),
        queue_advance: Arc::new(tokio::sync::Notify::new()),
        app_handle: std::sync::OnceLock::new(),
        merged_snapshot: AsyncMutex::new(None),
    };

    let asset_folders: Vec<String> = local_family
        .iter()
        .flat_map(|m| &m.folders)
        .map(|f| f.path.clone())
        .collect();
    let smb_mounts = cfg.smb_mounts.clone();
    let ssh_mounts = cfg.ssh_mounts.clone();

    // Periodically reap exited mpv processes so they don't sit as zombies: the
    // live player once it exits on its own, plus any killed players handed off via
    // the reap queue when replaced. All non-blocking try_wait() — a wedged player
    // (try_wait → still running) just stays queued rather than stalling anything.
    // Runs on its own OS thread (not the async runtime) and dies with the process.
    let child_slot = state.current_child.clone();
    let reap_queue = state.reap_queue.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        {
            let mut guard = child_slot.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(child) = guard.as_mut() {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    *guard = None;
                }
            }
        }
        // Keep only the ones still running; try_wait() reaps any that exited, and
        // we drop handles we can't query rather than retaining them forever.
        reap_queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain_mut(|c| matches!(c.try_wait(), Ok(None)));
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(move |app| {
            use tauri::Manager;

            // Publish the app handle so playback threads can emit UI events
            // (`playback-ended`). Set-once; a second set can't happen (setup
            // runs once) but would be harmlessly ignored.
            let _ = app
                .handle()
                .state::<AppState>()
                .app_handle
                .set(app.handle().clone());
            // Same handle for fire-and-forget background signals (the listing
            // cache's `listings-updated`).
            ui_events::set_app_handle(app.handle().clone());

            // Auto-advance dispatcher: when the mpv EOF watcher notifies a clean
            // file end, walk the queue cursor forward and play the next item.
            // Lives until the process exits.
            let handle_for_advance = app.handle().clone();
            let (advance_notify, queue_arc, queue_idx_arc) = {
                let s = handle_for_advance.state::<AppState>();
                (
                    s.queue_advance.clone(),
                    s.queue.clone(),
                    s.queue_index.clone(),
                )
            };
            tauri::async_runtime::spawn(async move {
                loop {
                    advance_notify.notified().await;
                    // Pick the next item under the locks, then drop them before
                    // calling play_by_key (which takes its own locks via state).
                    let next = {
                        let q = queue_arc.lock().unwrap_or_else(|e| e.into_inner());
                        let mut idx = queue_idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                        let candidate = match *idx {
                            Some(i) => i + 1,
                            None => 0,
                        };
                        if candidate < q.len() {
                            *idx = Some(candidate);
                            Some(q[candidate].clone())
                        } else {
                            None
                        }
                    };
                    let Some(item) = next else { continue };
                    let state = handle_for_advance.state::<AppState>();
                    if let Err(e) =
                        commands::play_by_key(&state, &item.rating_key, &item.title, item.duration_ms)
                            .await
                    {
                        eprintln!("vela: auto-advance to {:?} failed: {e}", item.title);
                    }
                }
            });

            // Let the webview load poster images from configured local folders.
            for path in &asset_folders {
                let _ = app.asset_protocol_scope().allow_directory(path, true);
            }
            if smb::remount_on_startup() {
                // Re-establish SMB mounts off the main thread so a slow/offline share
                // can't stall launch. Once a share mounts, refresh the local source so
                // selected folders inside that share become browsable in this running
                // app. Uses the bounded blocking pool (one task per share, so a slow
                // one doesn't block the others, and no unbounded native threads /
                // spawn panics). Each task re-checks the share is still configured
                // before mounting, and undoes the mount if it was removed while
                // mounting — so a remove/remount race can't leave an OS mount with no
                // config record.
                // Cap concurrency so a large/pathological config can't queue a huge
                // number of blocking mount attempts at once.
                let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
                let app_handle = app.handle().clone();
                for m in smb_mounts {
                    let sem = sem.clone();
                    let app_handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let Ok(_permit) = sem.acquire_owned().await else {
                            return;
                        };
                        let mounted = tauri::async_runtime::spawn_blocking(move || {
                            let cfg_now = || config::load_config().ok();
                            // Mount only if this specific record is *definitely* still
                            // configured (read error / removed → skip).
                            let still_configured =
                                cfg_now().map(|c| c.smb_mounts.iter().any(|x| x.id == m.id));
                            if still_configured != Some(true) {
                                return false;
                            }
                            if smb::mount(&m).is_ok() {
                                // Undo only if the mountpoint is *definitely* no longer
                                // referenced by ANY current record — so we don't tear down
                                // a connection a re-added record (same UNC) now uses, and a
                                // read error (None) doesn't unmount anything.
                                let mp_referenced = cfg_now().map(|c| {
                                    c.smb_mounts.iter().any(|x| x.mountpoint == m.mountpoint)
                                });
                                if mp_referenced == Some(false) {
                                    smb::unmount_for_removal(&m.mountpoint);
                                    return false;
                                }
                                return true;
                            }
                            false
                        })
                        .await
                        .unwrap_or(false);
                        if mounted {
                            refresh_local_source(&app_handle).await;
                        }
                    });
                }
            }
            // SSH/SFTP mounts are user-space but app-managed, so try to restore
            // them in the background. A missing key/agent should not stall launch;
            // once a mount succeeds, refresh the local source from the current
            // config so the folder becomes browsable in this running app.
            let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
            let app_handle = app.handle().clone();
            for m in ssh_mounts {
                let sem = sem.clone();
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let Ok(_permit) = sem.acquire_owned().await else {
                        return;
                    };
                    let mounted = tauri::async_runtime::spawn_blocking(move || {
                        let cfg_now = || config::load_config().ok();
                        let still_configured =
                            cfg_now().map(|c| c.ssh_mounts.iter().any(|x| x.id == m.id));
                        if still_configured != Some(true) {
                            return false;
                        }
                        sshfs::mount(&m).is_ok()
                    })
                    .await
                    .unwrap_or(false);
                    if mounted {
                        refresh_local_source(&app_handle).await;
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_app_info,
            commands::get_sources,
            commands::connect_jellyfin,
            commands::connect_jellyfin_token,
            commands::remove_source,
            commands::unlink_plex,
            commands::add_local_folder,
            commands::list_local_folders,
            commands::remove_local_folder,
            commands::mount_smb,
            commands::list_smb_mounts,
            commands::list_smb_directories,
            commands::add_smb_folder,
            commands::remove_smb_folder,
            commands::unmount_smb,
            commands::mount_ssh,
            commands::list_ssh_mounts,
            commands::unmount_ssh,
            commands::sshfs_status,
            commands::check_mpv,
            commands::set_mpv_path,
            commands::get_mpv_advanced,
            commands::set_mpv_advanced,
            commands::install_mpv,
            commands::open_url,
            commands::link_begin,
            commands::link_poll,
            commands::get_hubs,
            commands::get_sections,
            commands::get_items,
            commands::get_type_listing,
            commands::set_merged_override,
            commands::record_recent,
            commands::get_recents,
            commands::search,
            commands::get_children,
            commands::set_watched,
            commands::play_item,
            commands::queue_list,
            commands::queue_clear,
            commands::queue_remove,
            commands::queue_play_at,
            commands::queue_play_next,
            commands::queue_append,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the tauri application")
        .run(|app_handle, event| {
            // On quit, stop the progress tracker and kill the player mpv we
            // launched, so it doesn't linger as an orphaned window with no one
            // updating its resume position. Best-effort and non-blocking: we only
            // signal + kill (never wait()), so a player wedged in the kernel
            // (e.g. a hung SMB read) can't stall shutdown.
            if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
                use std::sync::atomic::Ordering;
                use tauri::Manager;
                let state = app_handle.state::<AppState>();
                // Set the shutdown flag and sweep the player under the SAME lock
                // play() takes before launching. A play racing exit therefore
                // either sees the flag and never launches, or has already
                // registered its child here for us to kill — no orphan window.
                {
                    let mut slot = state
                        .current_child
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    state.shutting_down.store(true, Ordering::SeqCst);
                    if let Some(mut child) = slot.take() {
                        let _ = child.kill();
                    }
                }
                let stop = state
                    .tracking_stop
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
                if let Some(stop) = stop {
                    stop.store(true, Ordering::Relaxed);
                }
            }
        });
}

async fn refresh_local_source(app_handle: &tauri::AppHandle) {
    use tauri::Manager;
    let Ok(cfg) = config::load_config() else {
        return;
    };
    let family = source::local::local_family(
        &cfg,
        smb_runtime_folders,
        ssh_runtime_folder,
        safe_user_media_root,
    );
    for folder in family.iter().flat_map(|m| &m.folders) {
        let _ = app_handle
            .asset_protocol_scope()
            .allow_directory(&folder.path, true);
    }
    let state = app_handle.state::<AppState>();
    let mut reg = state.registry.lock().await;
    // Replace the whole family: a mount that went away must drop its source.
    reg.remove_kinds(source::local::LOCAL_FAMILY_KINDS);
    for member in &family {
        reg.upsert(member.build());
    }
}

fn smb_runtime_folders(m: &config::SmbMount) -> Vec<config::LocalFolder> {
    let Some(root) = smb_mount_root(m) else {
        return Vec::new();
    };
    m.folders
        .iter()
        .map(|folder| config::LocalFolder {
            id: folder.id.clone(),
            name: folder.name.clone(),
            path: smb_path_string_for_relative(&root, &folder.path),
            kind: folder.kind.clone(),
        })
        .collect()
}

fn ssh_runtime_folder(m: &config::SshMount) -> Option<config::LocalFolder> {
    if !sshfs::is_active_mount(m) {
        return None;
    }
    Some(config::LocalFolder {
        id: m.local_folder_id.clone(),
        name: m.name.clone(),
        path: m.mountpoint.clone(),
        kind: m.kind.clone(),
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn smb_mount_root(m: &config::SmbMount) -> Option<String> {
    smb::resolved_mountpoint(m).filter(|path| Path::new(path).is_dir())
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn smb_mount_root(m: &config::SmbMount) -> Option<String> {
    if Path::new(&m.mountpoint).is_dir() {
        Some(m.mountpoint.clone())
    } else {
        None
    }
}

fn smb_path_string_for_relative(root: &str, relative: &str) -> String {
    let mut path = PathBuf::from(root);
    for part in relative.split('/').filter(|part| !part.is_empty()) {
        path.push(part);
    }
    path.to_string_lossy().to_string()
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
