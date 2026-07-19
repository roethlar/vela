mod commands;
mod config;
mod playback;
mod playlists;
mod plex_api;
mod plex_library;
mod recents;
mod source;
mod storage;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

use source::SourceRegistry;

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
    /// Backend-only Plex authorization sessions. Tokens wait here while the
    /// user chooses among several reachable physical servers; the frontend sees
    /// only server names and stable machine identifiers.
    pub(crate) plex_link_sessions: AsyncMutex<commands::PlexLinkSessions>,
    /// Joins mpv's clean-EOF signal to the matching completed tracker write.
    /// The async dispatcher in `run()` advances only that exact session.
    pub(crate) playback_advance: Arc<commands::PlaybackAdvance>,
    /// Active Vela- or server-playlist location. The cursor is in-memory by
    /// design; neither playlist authority changes merely because it is played.
    pub(crate) playlist_cursor: AsyncMutex<Option<commands::PlaylistCursor>>,
    /// UUID of the latest successfully resolved playback context. Automatic
    /// continuation supplies the completed UUID as an expectation so delayed
    /// work can never replace a newer manual play.
    pub(crate) active_playback_session: AsyncMutex<Option<String>>,
    /// Window-state observer for the latest successfully launched mpv session.
    /// Exact automatic replacements may snapshot it; manual plays never do.
    pub(crate) playback_window_session: Mutex<Option<commands::PlaybackWindowSession>>,
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
    // Restore every configured source. Plex now uses the same per-row model as
    // Jellyfin/Emby; `load_config` has already migrated the old singleton.
    for src_cfg in &cfg.sources {
        if let Some(src) = source::plex::build_source(src_cfg)
            .or_else(|| source::jellyfin::build_source(src_cfg))
        {
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
        plex_link_sessions: AsyncMutex::new(Default::default()),
        playback_advance: Arc::new(commands::PlaybackAdvance::default()),
        playlist_cursor: AsyncMutex::new(None),
        active_playback_session: AsyncMutex::new(None),
        playback_window_session: Mutex::new(None),
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

            // Playback sequence dispatcher: only the joined clean-EOF and
            // final-tracker signal can advance a playlist or authorize Continue
            // Playing. A user-closing mpv emits playback-ended for refresh, but
            // never reaches this loop.
            let app_handle = app.handle().clone();
            let advance_notify = app.handle().state::<AppState>().playback_advance.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let completion = advance_notify.next().await;
                    let state = app_handle.state::<AppState>();
                    // Serialize the whole admitted-completion transaction with
                    // explicit watched-state edits. Local curation and sequence
                    // release happen before the owning server write, so an
                    // offline server cannot stall the next video; retaining the
                    // lock across that write makes any later user edit win.
                    let _edit = state.watch_edit_lock.lock().await;
                    let admitted = match commands::admit_clean_completion(&completion) {
                        Ok(admitted) => admitted,
                        Err(error) => {
                            eprintln!("vela: couldn't persist clean playback completion: {error}");
                            continue;
                        }
                    };
                    if !admitted {
                        // A newer same-key/watch-key session owns the item now.
                        // Skip every clean-completion side effect: no sequence
                        // advance, terminal continuation, refresh, or server mark.
                        continue;
                    }

                    use tauri::Emitter;
                    if commands::advance_playlist(&state, &completion.session_id).await {
                        // Terminal policy reads the literal carousel that was
                        // rendered before this event. Publish continuation before
                        // any refresh or slow server synchronization can replace it.
                        let _ = app_handle.emit("continue-playing", completion.clone());
                    }

                    if let Err(error) =
                        commands::mark_clean_completion_played(&state, &completion).await
                    {
                        eprintln!("vela: automatic played-state update failed: {error}");
                    }

                    // Publish the authoritative post-curation refresh after the
                    // owning server's played-state attempt settles, so newly
                    // eligible hub items are visible without a manual refresh.
                    // This remains unconditional on server success: local
                    // curation and any backend-owned successor are already final.
                    let source_id = completion
                        .item_key
                        .split_once(':')
                        .map(|(source, _)| source)
                        .unwrap_or_default();
                    let _ = app_handle.emit(
                        "playback-ended",
                        serde_json::json!({
                            "sourceId": source_id,
                            "itemKey": completion.item_key.clone(),
                        }),
                    );
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
            commands::check_mpv,
            commands::set_mpv_path,
            commands::get_mpv_advanced,
            commands::set_mpv_advanced,
            commands::get_continue_playing,
            commands::set_continue_playing,
            commands::install_mpv,
            commands::open_url,
            commands::link_begin,
            commands::link_poll,
            commands::link_select_server,
            commands::get_hubs,
            commands::get_sections,
            commands::set_section_sort,
            commands::scan_section,
            commands::get_items,
            commands::get_type_listing,
            commands::set_merged_override,
            commands::get_recents,
            commands::remove_from_continue,
            commands::get_continue_tombstones,
            commands::search,
            commands::get_children,
            commands::get_item_detail,
            commands::get_person_items,
            commands::set_watched,
            commands::play_item,
            commands::next_episode,
            commands::get_server_playlists,
            commands::get_server_playlist_items,
            commands::server_playlist_play,
            commands::playlist_list,
            commands::playlist_get,
            commands::playlist_create,
            commands::playlist_rename,
            commands::playlist_delete,
            commands::playlist_add_items,
            commands::playlist_remove_item,
            commands::playlist_reorder,
            commands::playlist_play,
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
