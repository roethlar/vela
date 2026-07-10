mod commands;
mod config;
mod playback;
mod plex_api;
mod plex_library;
mod recents;
mod source;

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
    /// All configured media sources (Plex and Jellyfin/Emby servers).
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
    /// Serializes set_watched so overlapping edits can't interleave their
    /// curate-first hides and failure rollbacks (an undo token has no
    /// generation; two in-flight edits on one item could otherwise strip a
    /// tombstone the other's success relies on).
    pub watch_edit_lock: AsyncMutex<()>,
    /// Serializes source mutations (add/remove) so they apply in order
    /// without holding the registry lock across config file I/O.
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

    let state = AppState {
        registry: AsyncMutex::new(registry),
        tracking_stop: Mutex::new(None),
        current_child: Arc::new(Mutex::new(None)),
        shutting_down: Arc::new(AtomicBool::new(false)),
        reap_queue: Arc::new(Mutex::new(Vec::new())),
        play_lock: AsyncMutex::new(()),
        watch_edit_lock: AsyncMutex::new(()),
        source_lock: AsyncMutex::new(()),
        queue: Arc::new(Mutex::new(Vec::new())),
        queue_index: Arc::new(Mutex::new(None)),
        queue_advance: Arc::new(tokio::sync::Notify::new()),
        app_handle: std::sync::OnceLock::new(),
        merged_snapshot: AsyncMutex::new(None),
    };

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

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state);

    builder
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
            commands::set_section_sort,
            commands::get_items,
            commands::get_type_listing,
            commands::set_merged_override,
            commands::record_recent,
            commands::get_recents,
            commands::remove_from_continue,
            commands::get_continue_tombstones,
            commands::search,
            commands::get_children,
            commands::get_item_detail,
            commands::get_person_items,
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

