// One play, from start to end. `play` has the player started (how a launch is
// described and who starts it lives in `crate::player`) and then supervises it
// over mpv's JSON IPC socket: position is read there and reported back to the
// originating server (Plex or Jellyfin/Emby) so resume works, and the end of
// the file or of the session is noticed there. Nothing below depends on which
// player is on the other end of the socket.

use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// mpv's JSON IPC channel is a Unix domain socket on Linux/macOS and an emulated
// named pipe on Windows. Both expose the same byte-stream Read+Write interface
// with a `try_clone`, so the tracker code below is platform-agnostic once we pick
// the right connector.
#[cfg(unix)]
type IpcStream = std::os::unix::net::UnixStream;
#[cfg(windows)]
type IpcStream = std::fs::File;

#[cfg(unix)]
fn ipc_connect(path: &str) -> std::io::Result<IpcStream> {
    std::os::unix::net::UnixStream::connect(path)
}
#[cfg(windows)]
fn ipc_connect(path: &str) -> std::io::Result<IpcStream> {
    // A Windows named pipe can be opened like a file for duplex read/write.
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
}

/// Everything the progress tracker needs to report back to Plex.
#[derive(Clone)]
pub struct TrackInfo {
    pub server_base: String,
    pub token: String,
    pub client_identifier: String,
    pub rating_key: String,
    pub key: String,
    pub duration_ms: u64,
}

/// What the Jellyfin/Emby progress tracker needs. Auth is pre-built into
/// `headers` by the source so this layer stays agnostic to the (diverging)
/// Jellyfin-vs-Emby header schemes. `media_source_id` and `play_session_id`
/// come from the playback-info negotiation so the server's check-in model
/// (history/dashboard/resume) ties the events to the right session.
#[derive(Clone)]
pub struct JellyfinTrack {
    pub base_url: String,
    pub item_id: String,
    pub media_source_id: String,
    pub play_session_id: Option<String>,
    pub headers: Vec<(String, String)>,
}

/// Where playback progress should be reported. Each media source supplies the
/// variant it knows how to report against; new backends add their own variants.
pub enum ProgressTarget {
    /// Report position to a Plex server's timeline (enables cross-session resume).
    Plex(TrackInfo),
    /// Report position to a Jellyfin/Emby server's playback-progress endpoints.
    Jellyfin(JellyfinTrack),
    /// No progress reporting.
    None,
}

/// Runtime mpv window state that may be carried to an exact automatic
/// successor. `None` means mpv has not reported the property, which is distinct
/// from an explicit `false` and therefore leaves configured launch options
/// authoritative.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlaybackWindowState {
    pub fullscreen: Option<bool>,
    pub maximized: Option<bool>,
}

/// Per-launch observation handle. Each IPC reader owns a fresh handle so a
/// delayed event from an older mpv process cannot overwrite a newer session's
/// published state.
#[derive(Debug, Clone, Default)]
pub struct WindowStateObservation {
    state: Arc<Mutex<PlaybackWindowState>>,
    display: Arc<Mutex<crate::display::ObservedDisplay>>,
}

impl WindowStateObservation {
    pub fn snapshot(&self) -> PlaybackWindowState {
        *self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn display_snapshot(&self) -> crate::display::ObservedDisplay {
        self.display
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(crate) fn apply_ipc_event(&self, value: &serde_json::Value) {
        if let Some((property, enabled)) = window_property_change(value) {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            match property {
                WindowProperty::Fullscreen => state.fullscreen = Some(enabled),
                WindowProperty::Maximized => state.maximized = Some(enabled),
            }
        }
        if let Some(change) = display_property_change(value) {
            let mut display = self
                .display
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            match change {
                DisplayPropertyChange::Names(names) => display.names = names,
                DisplayPropertyChange::Width(width) => display.width_px = Some(width),
                DisplayPropertyChange::Height(height) => display.height_px = Some(height),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowProperty {
    Fullscreen,
    Maximized,
}

/// Accept only owned boolean property-change events. Observe-property command
/// replies, nulls, numeric/string coercions, and unrelated properties must not
/// invent state.
fn window_property_change(value: &serde_json::Value) -> Option<(WindowProperty, bool)> {
    if value.get("event")?.as_str()? != "property-change" {
        return None;
    }
    let property = match value.get("name")?.as_str()? {
        "fullscreen" => WindowProperty::Fullscreen,
        "window-maximized" => WindowProperty::Maximized,
        _ => return None,
    };
    Some((property, value.get("data")?.as_bool()?))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DisplayPropertyChange {
    Names(Vec<String>),
    Width(u32),
    Height(u32),
}

/// mpv's playback position in milliseconds, and ONLY that.
///
/// This used to take the `data` of ANY event, so every observed property that
/// happens to be numeric read as a position: a `display-width` of 3840 became a
/// position of 3,840,000 ms. Booleans and arrays coerce to nothing, so only the
/// two display dimensions leaked — but mpv emits an observed property's initial
/// value the moment it is registered, before the first `time-pos`. A play that
/// ENDED inside that window therefore defeated the tracker's `t > 0` guard (the
/// one written to avoid clobbering a resume point when mpv fails to start) and
/// wrote a fabricated "stopped at 36 minutes" to the user's server.
///
/// Same strictness as `window_property_change` and `display_property_change`:
/// the exact event, the exact name, the exact JSON type.
fn position_property_change(value: &serde_json::Value) -> Option<u64> {
    if value.get("event")?.as_str()? != "property-change" {
        return None;
    }
    if value.get("name")?.as_str()? != "time-pos" {
        return None;
    }
    let seconds = value.get("data")?.as_f64()?;
    // A negative or non-finite time-pos is not a position. mpv reports null
    // here before playback starts, which `as_f64` already rejects.
    (seconds.is_finite() && seconds >= 0.0).then_some((seconds * 1000.0) as u64)
}

/// mpv reports actual output identity and pixel dimensions as property-change
/// events. Accept only the exact JSON types: malformed replies must not invent
/// compatibility evidence.
fn display_property_change(value: &serde_json::Value) -> Option<DisplayPropertyChange> {
    if value.get("event")?.as_str()? != "property-change" {
        return None;
    }
    match value.get("name")?.as_str()? {
        "display-names" => Some(DisplayPropertyChange::Names(
            value
                .get("data")?
                .as_array()?
                .iter()
                .map(|name| name.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()?,
        )),
        "display-width" => value
            .get("data")?
            .as_u64()
            .and_then(|width| u32::try_from(width).ok())
            .filter(|width| *width > 0)
            .map(DisplayPropertyChange::Width),
        "display-height" => value
            .get("data")?
            .as_u64()
            .and_then(|height| u32::try_from(height).ok())
            .filter(|height| *height > 0)
            .map(DisplayPropertyChange::Height),
        _ => None,
    }
}

/// What to play, as opposed to the supervision plumbing `play` also takes:
/// the stream URL, the human title for mpv's window/OSD, the auth headers the
/// stream needs (written to a private include; see `crate::player::launch`),
/// and the resume offset.
pub struct PlaySpec {
    /// The player chosen for this play (`crate::player::choose`).
    pub(crate) player: crate::player::PlayerKind,
    pub url: String,
    pub title: String,
    pub http_headers: Vec<(String, String)>,
    pub start_seconds: f64,
    /// Absolute path to the bundled mpv `autocrop.lua`, resolved by the caller
    /// via Tauri's resource resolver (the caller has the `AppHandle`; `play`
    /// does not). `None` when it could not be resolved — autocrop is then
    /// skipped even if enabled. Whether it's actually injected depends on the
    /// `mpv_autocrop` config mode, read when the launch is described.
    pub autocrop_script: Option<String>,
    /// Absolute path to Vela's `vela-autocrop.lua` trigger shim (same resolver
    /// and existence check). Auto mode loads the stock script with its own
    /// auto trigger DISABLED and lets the shim fire detection after a settle
    /// delay — the stock trigger skips its delay on `--start` resumes and
    /// races hwdec init (see the shim header + `.agents/plans/autocrop-resume.md`).
    pub autocrop_shim: Option<String>,
    /// Actual state sampled from the exact automatic predecessor. Manual plays
    /// and unknown properties use the default (both `None`).
    pub inherited_window_state: PlaybackWindowState,
    /// Native output containing Vela's window for a manual play, or the exact
    /// observed predecessor output for an automatic successor.
    pub screen_name: Option<String>,
    /// Fresh handle populated by this launch's IPC reader and published by the
    /// command layer only after the full launch succeeds.
    pub window_observation: WindowStateObservation,
    /// Absolute path to Vela's bundled `vela-markers.lua`, resolved by the
    /// caller through the same resource resolver as autocrop. `None` when it
    /// could not be resolved — skipping is then simply absent, never an error.
    pub markers_script: Option<String>,
    /// Marker ranges for this exact launch, already normalized AND already
    /// filtered to the kinds whose policy is enabled. Empty means nothing is
    /// injected at all.
    pub markers: Vec<crate::source::MediaMarker>,
    /// Resolved per-kind policies. The launch passes these to the script; it never
    /// reads config to derive them and never interprets a policy string.
    pub intro_policy: crate::config::SkipPolicy,
    pub credits_policy: crate::config::SkipPolicy,
    pub commercial_policy: crate::config::SkipPolicy,
    /// Set ONLY when the resolved quality is `Automatic` AND a lower tier is
    /// actually available for this copy. `None` leaves the sampler unspawned —
    /// a play that cannot step down must not pay for watching.
    pub step_down: Option<StepDownNotify>,
    /// This item's runtime, for the sampler's end-of-file grace. `None` holds
    /// the cache signal entirely (see `automatic::AutomaticDetector`).
    pub duration: Option<Duration>,
    /// Shown once over mpv's OSD when this launch starts, for a play the user
    /// did not ask for — an Automatic step-down. Kept very short on purpose:
    /// mpv's OSD is large, and the notice must not become the thing the user
    /// notices. Never set `--osd-font-size` to compensate; it is global and
    /// would override the user's own mpv config.
    pub osd_notice: Option<String>,
    /// Called once when `osd_notice` has actually been handed to the player.
    /// The notice is sent a moment after the player starts and is dropped if
    /// the play is replaced first, so "the player started" does not mean "the
    /// user was told" (review finding sel-3).
    pub on_notice_shown: Option<NoticeShown>,
    /// Steps this LOGICAL play has already taken, carried into the new
    /// detector so the cap counts plays rather than mpv processes.
    pub steps_taken: u32,
}

/// Fired exactly once when a playback session ends — after the final server
/// check-in for tracked sessions, or at mpv exit for untracked ones — with
/// the last observed playback position in ms (0 when none was read). The
/// caller wires this to recents recording + UI notification (a
/// `playback-ended` Tauri event); this module stays UI-framework-free.
pub type EndNotify = Arc<dyn Fn(u64) + Send + Sync>;

/// See [`PlaySpec::on_notice_shown`].
pub type NoticeShown = Arc<dyn Fn() + Send + Sync>;

/// How long a notice waits after the player's IPC connection is up. Let the
/// picture arrive first: a notice drawn over a black frame during load
/// explains nothing.
const OSD_NOTICE_DELAY: Duration = Duration::from_millis(1500);

/// Start a player for `spec.url`, optionally seeking to `spec.start_seconds`,
/// and start background progress reporting that runs until the player ends.
/// Publishes the player into `player_slot` the instant it's launched (so an
/// app-exit that races tracker setup can still find and kill it), and returns
/// the stop flag so a later play can cancel this one. The caller must have
/// already cleared/killed any previous player, since this overwrites the slot.
pub fn play(
    spec: &PlaySpec,
    progress: ProgressTarget,
    player_slot: &Arc<Mutex<Option<crate::player::PlayerHandle>>>,
    shutting_down: &Arc<AtomicBool>,
    advance: &Arc<crate::commands::PlaybackAdvance>,
    session_id: String,
    on_end: Option<EndNotify>,
) -> Result<Arc<AtomicBool>, String> {
    // Only the external player can be started here so far. A play is given the
    // embedded kind only when `player::embedded_availability` says it can run,
    // and it does not yet, so this refuses nothing a user can reach; it is here
    // so that an embedded play can never be started as an external one by
    // accident once that changes.
    if spec.player != crate::player::PlayerKind::External {
        return Err("the embedded player is not part of this build".to_string());
    }
    let ipc_path = crate::player::launch::new_ipc_path()?;

    let mpv_bin = crate::player::external::resolve_mpv()?
        .ok_or_else(|| "mpv was not found. Install mpv to play video.".to_string())?;

    // Advanced/user mpv config (Settings → Advanced mpv options), loaded once here.
    let cfg =
        crate::config::load_config().map_err(|_| "could not read Vela settings".to_string())?;
    let plan = crate::player::launch::build_launch(spec, &cfg, &ipc_path)?;

    // Launch the player and publish it into the shared slot while still holding
    // the lock, so launch+register is atomic w.r.t. the app-exit handler: it can
    // never observe a launched-but-unregistered player and orphan it. The caller
    // has already killed any prior player, so the slot is expected to be empty.
    {
        let mut slot = player_slot.lock().unwrap_or_else(|e| e.into_inner());
        // Don't launch into a shutting-down app. The exit handler sets this flag
        // and sweeps the slot under THIS lock, so checking it here (still holding
        // the lock) closes the race where a play starting during exit would
        // register an mpv the sweep had already passed.
        if shutting_down.load(Ordering::SeqCst) {
            plan.discard_marker_payload();
            return Err("Vela is shutting down.".to_string());
        }
        *slot = Some(crate::player::external::launch(&mpv_bin, plan)?);
    }
    // Seed the tracked position with the resume point, so if mpv exits before
    // IPC reports a time-pos the final check-in reports the resume position
    // rather than 0 (which would clobber an existing resume point).
    let start_ms = (spec.start_seconds.max(0.0) * 1000.0) as u64;
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Watch mpv's IPC for a clean EOF — that's the signal for whichever
    // playback context owns the sequence. We do NOT advance on user-close (mpv
    // reports reason=quit/stop), only on reason=eof. Always-on regardless of
    // progress target. Failure to spawn the watcher is non-fatal — playback
    // still works, just without sequence advancement.
    if let Err(e) = spawn_eof_watcher(
        ipc_path.clone(),
        stop_flag.clone(),
        advance.clone(),
        session_id,
    ) {
        eprintln!("vela: couldn't spawn mpv EOF watcher: {e}");
    }

    // Automatic only. Like the EOF watcher, failing to spawn is non-fatal:
    // playback still works, it just cannot step itself down.
    if let Some(step_down) = spec.step_down.clone() {
        if let Err(e) = spawn_health_sampler(
            ipc_path.clone(),
            stop_flag.clone(),
            spec.duration,
            spec.steps_taken,
            step_down,
        ) {
            eprintln!("vela: couldn't spawn mpv health sampler: {e}");
        }
    }

    // Explain a play the user did not ask for. Best-effort and fire-and-forget:
    // a notice that fails to render must never affect playback.
    if let Some(notice) = spec.osd_notice.clone() {
        if let Err(e) = spawn_osd_notice(
            ipc_path.clone(),
            stop_flag.clone(),
            notice,
            OSD_NOTICE_DELAY,
            spec.on_notice_shown.clone(),
        ) {
            eprintln!("vela: couldn't show the playback notice: {e}");
        }
    }

    // Route the end-of-session notifier: tracked sessions fire it from their
    // tracker tail (after the final server write, so a refresh triggered by it
    // sees the new state); untracked sessions fire it when mpv exits. The match
    // guards keep degenerate track targets (empty server base) on the untracked
    // path so the notifier still fires exactly once per session.
    let tracking = match progress {
        ProgressTarget::Plex(info) if !info.server_base.is_empty() => start_tracking_plex(
            ipc_path,
            info,
            start_ms,
            stop_flag.clone(),
            spec.window_observation.clone(),
            on_end,
        ),
        ProgressTarget::Jellyfin(track) if !track.base_url.is_empty() => start_tracking_jellyfin(
            ipc_path,
            track,
            start_ms,
            stop_flag.clone(),
            spec.window_observation.clone(),
            on_end,
        ),
        _ => spawn_end_watcher(
            ipc_path,
            stop_flag.clone(),
            spec.window_observation.clone(),
            on_end,
        ),
    };
    if let Err(e) = tracking {
        // mpv launched but we couldn't spawn its tracker threads (e.g. the OS is
        // out of threads). Stop any IPC reader that did start, then kill the child
        // but LEAVE it in the slot: the periodic reaper try_wait()s it (and a
        // later play / app-exit would also reap or kill it). Spawning our own
        // reaper thread here could fail under the very thread exhaustion that
        // caused this, dropping the child unreaped as a zombie; and a synchronous
        // wait() could block play() forever under play_lock if mpv wedged opening
        // a file on a hung mount before honoring kill().
        stop_flag.store(true, Ordering::Relaxed);
        if let Some(player) = player_slot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            let _ = player.kill();
        }
        return Err(format!("couldn't start playback tracking: {e}"));
    }
    Ok(stop_flag)
}

/// Tiny IPC listener that signals the playback dispatcher when mpv finishes a
/// file CLEANLY (mpv emits `event=end-file, reason=eof`). User-closed (`quit`),
/// errored (`error`), or otherwise-stopped exits never fire the notifier — so
/// closing the player stops playback. Separate from the progress reader so it
/// runs for every source even when no progress tracker is active.
fn spawn_eof_watcher(
    socket_path: String,
    stop_flag: Arc<AtomicBool>,
    advance: Arc<crate::commands::PlaybackAdvance>,
    session_id: String,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("mpv-eof-watcher".into())
        .spawn(move || {
            // Brief connect retries: mpv may not have opened the socket yet.
            let mut stream = None;
            for _ in 0..50 {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                match ipc_connect(&socket_path) {
                    Ok(s) => {
                        stream = Some(s);
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            let Some(stream) = stream else { return };
            let reader = BufReader::new(stream);
            for line in reader.lines() {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                let Ok(line) = line else { return };
                // mpv emits one JSON event per line. Substring match is fine here
                // (the event/reason keys are fixed, no escaping concerns).
                if line.contains("\"event\":\"end-file\"") && line.contains("\"reason\":\"eof\"") {
                    advance.mark_eof(session_id);
                    return;
                }
            }
        })?;
    Ok(())
}

/// send buffer), publishing the latest position into the returned counter. The
/// `done` flag is set when mpv exits or the connection drops. Does NO network I/O,
/// so it's shared by every backend's poster.
fn spawn_position_reader(
    socket_path: String,
    initial_ms: u64,
    stop_flag: Arc<AtomicBool>,
    window_observation: WindowStateObservation,
) -> std::io::Result<(Arc<AtomicU64>, Arc<AtomicBool>)> {
    let last_t_ms = Arc::new(AtomicU64::new(initial_ms));
    let done = Arc::new(AtomicBool::new(false));
    let last_t_ms_r = last_t_ms.clone();
    let done_r = done.clone();
    std::thread::Builder::new()
        .name("mpv-ipc-reader".into())
        .spawn(move || {
            let mut stream = None;
            for _ in 0..50 {
                if stop_flag.load(Ordering::Relaxed) {
                    done_r.store(true, Ordering::Relaxed);
                    return;
                }
                match ipc_connect(&socket_path) {
                    Ok(s) => {
                        stream = Some(s);
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            let mut stream = match stream {
                Some(s) => s,
                None => {
                    done_r.store(true, Ordering::Relaxed);
                    return;
                }
            };
            let reader_clone = match stream.try_clone() {
                Ok(s) => s,
                Err(_) => {
                    done_r.store(true, Ordering::Relaxed);
                    return;
                }
            };
            let mut reader = BufReader::new(reader_clone);

            // Ask mpv to push position and window-state updates; then drain every
            // line. Distinct ids keep command replies unambiguous during IPC
            // diagnostics, while parsing below trusts only named property events.
            // Note for anyone adding a subscription here: mpv sends an observed
            // property's initial value at once, and every consumer below matches
            // on the property NAME. Adding a numeric property is safe; widening
            // a consumer to accept any `data` is not.
            let _ = stream.write_all(
                b"{\"command\":[\"observe_property\",1,\"time-pos\"]}\n\
                  {\"command\":[\"observe_property\",2,\"fullscreen\"]}\n\
                  {\"command\":[\"observe_property\",3,\"window-maximized\"]}\n\
                  {\"command\":[\"observe_property\",4,\"display-names\"]}\n\
                  {\"command\":[\"observe_property\",5,\"display-width\"]}\n\
                  {\"command\":[\"observe_property\",6,\"display-height\"]}\n",
            );

            let mut line = String::new();
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break, // EOF: mpv exited
                    Ok(_) => {}
                    Err(_) => break,
                }
                // Only `time-pos` is a position. Every other observed property
                // above is either non-numeric or a display dimension, and a
                // dimension read as a position corrupts the resume point.
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(ms) = position_property_change(&v) {
                        last_t_ms_r.store(ms, Ordering::Relaxed);
                    }
                    window_observation.apply_ipc_event(&v);
                }
            }
            done_r.store(true, Ordering::Relaxed);
        })?;
    Ok((last_t_ms, done))
}

/// Called when Automatic decides this play needs a lower tier. Carries the
/// position to resume at and why the step happened.
pub type StepDownNotify = Arc<dyn Fn(u64, crate::automatic::StepDownReason) + Send + Sync>;

/// Watch one play for the two Automatic signals and report the first verdict.
///
/// Its own IPC connection, like the EOF watcher: mpv's JSON IPC accepts
/// concurrent clients, and a separate connection keeps sampling off the
/// position reader's hot path, where a slow consumer would delay the resume
/// point everything else depends on.
///
/// Sampling is pull, not push. `demuxer-cache-duration` changes continuously,
/// so observing it would flood the socket for values only read once per tick;
/// asking on a fixed tick is both quieter and what the thresholds are expressed
/// in.
/// Show one short OSD line on this player, then stop. Its own connection and
/// its own thread so a slow or refused write cannot delay the trackers.
/// `on_shown` runs once, and only when the line was really handed to the
/// player: not when the play was replaced during the wait, and not when the
/// player could not be reached.
fn spawn_osd_notice(
    socket_path: String,
    stop_flag: Arc<AtomicBool>,
    notice: String,
    delay: Duration,
    on_shown: Option<NoticeShown>,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("mpv-osd-notice".into())
        .spawn(move || {
            let mut stream = None;
            for _ in 0..50 {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                match ipc_connect(&socket_path) {
                    Ok(s) => {
                        stream = Some(s);
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            let Some(mut stream) = stream else { return };
            std::thread::sleep(delay);
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }
            // JSON-encoded so a title or unit can never break the command.
            let text = serde_json::Value::String(notice);
            let sent = stream.write_all(
                format!("{{\"command\":[\"show-text\",{text},2000]}}\n").as_bytes(),
            );
            if sent.is_ok() {
                if let Some(on_shown) = on_shown {
                    on_shown();
                }
            }
        })?;
    Ok(())
}

fn spawn_health_sampler(
    socket_path: String,
    stop_flag: Arc<AtomicBool>,
    duration: Option<Duration>,
    steps_taken: u32,
    on_step_down: StepDownNotify,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("mpv-health-sampler".into())
        .spawn(move || {
            let mut stream = None;
            for _ in 0..50 {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                match ipc_connect(&socket_path) {
                    Ok(s) => {
                        stream = Some(s);
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            let Some(mut stream) = stream else { return };
            let Ok(read_half) = stream.try_clone() else {
                return;
            };
            let mut reader = BufReader::new(read_half);

            let mut detector = crate::automatic::AutomaticDetector::resuming(duration, steps_taken);
            let started = Instant::now();
            let mut last_position = Duration::ZERO;
            // Where the position was one tick ago, for spotting a seek.
            let mut previous_position = Duration::ZERO;

            loop {
                std::thread::sleep(crate::automatic::SAMPLE_TICK);
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                // One request per property, distinct ids so replies can be told
                // apart from each other and from mpv's own event stream.
                if stream
                    .write_all(
                        b"{\"command\":[\"get_property\",\"time-pos\"],\"request_id\":101}\n\
                          {\"command\":[\"get_property\",\"decoder-frame-drop-count\"],\"request_id\":102}\n\
                          {\"command\":[\"get_property\",\"demuxer-cache-duration\"],\"request_id\":103}\n\
                          {\"command\":[\"get_property\",\"pause\"],\"request_id\":104}\n",
                    )
                    .is_err()
                {
                    return; // mpv is gone
                }
                let mut sample = crate::automatic::HealthSample {
                    at: started.elapsed(),
                    position: last_position,
                    decoder_drops: 0,
                    cache_seconds: None,
                    paused: false,
                };
                let mut drops = None;
                // Drain until every reply has landed, or mpv stops talking.
                let mut line = String::new();
                let mut outstanding = 4;
                while outstanding > 0 {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => return,
                        Ok(_) => {}
                    }
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                        continue;
                    };
                    // Events interleave with replies; only replies carry an id.
                    let Some(id) = v.get("request_id").and_then(|i| i.as_u64()) else {
                        continue;
                    };
                    let data = v.get("data");
                    match id {
                        101 => {
                            if let Some(seconds) = data.and_then(|d| d.as_f64()) {
                                if seconds.is_finite() && seconds >= 0.0 {
                                    last_position = Duration::from_secs_f64(seconds);
                                    sample.position = last_position;
                                }
                            }
                        }
                        102 => drops = data.and_then(|d| d.as_u64()),
                        103 => sample.cache_seconds = data.and_then(|d| d.as_f64()),
                        104 => sample.paused = data.and_then(|d| d.as_bool()).unwrap_or(false),
                        _ => continue,
                    }
                    outstanding -= 1;
                }
                // Without a drop count there is no drop signal to evaluate, and
                // a missing value must not read as "back to zero" — that would
                // look like the counter resetting rather than a gap.
                let Some(drops) = drops else { continue };
                sample.decoder_drops = drops;

                // A seek refills the cache and bursts the decoder, which reads
                // exactly like the failures below. The sampler is what notices,
                // because it is the only thing here watching the position.
                if crate::automatic::looks_like_a_seek(
                    previous_position,
                    sample.position,
                    sample.paused,
                ) {
                    detector.note_seek(sample.at);
                    previous_position = sample.position;
                    continue;
                }
                previous_position = sample.position;

                if let Some(reason) = detector.observe(sample) {
                    // One verdict per sampler: the caller replaces this play, so
                    // this thread's mpv is on its way out either way.
                    on_step_down(last_position.as_millis() as u64, reason);
                    return;
                }
            }
        })?;
    Ok(())
}

/// Sleep until the next ~5s report tick, returning false if playback ended.
fn wait_tick(stop_flag: &AtomicBool, done: &AtomicBool) -> bool {
    for _ in 0..50 {
        if stop_flag.load(Ordering::Relaxed) || done.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !(stop_flag.load(Ordering::Relaxed) || done.load(Ordering::Relaxed))
}

/// Plex: report position on a timer, then post the authoritative final position
/// when mpv exits (this is what makes resume work). The caller guarantees a
/// non-empty `info.server_base` (see the match guards in [`play`]).
fn start_tracking_plex(
    socket_path: String,
    info: TrackInfo,
    start_ms: u64,
    stop_flag: Arc<AtomicBool>,
    window_observation: WindowStateObservation,
    on_end: Option<EndNotify>,
) -> std::io::Result<()> {
    let (last_t_ms, done) =
        spawn_position_reader(socket_path, start_ms, stop_flag.clone(), window_observation)?;

    std::thread::Builder::new()
        .name("mpv-tracker-plex".into())
        .spawn(move || {
            // Every exit funnels past the notifier below, so it fires exactly
            // once per session even on early bails inside this closure.
            (|| {
                // A single current-thread runtime: the tracker only does sequential
                // block_on HTTP posts, so a full multi-thread runtime (its own thread
                // pool) per playback session would be wasteful.
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(r) => r,
                    Err(_) => return,
                };
                // Bail rather than fall back to a default client with no timeout — a hung
                // check-in post would otherwise pin this tracker thread indefinitely.
                let client = match reqwest::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .build()
                {
                    Ok(c) => c,
                    Err(_) => return,
                };

                while wait_tick(&stop_flag, &done) {
                    let t = last_t_ms.load(Ordering::Relaxed);
                    if let Err(e) = rt.block_on(crate::plex_api::update_timeline(
                        &client,
                        &info.server_base,
                        &info.token,
                        &info.client_identifier,
                        &info.rating_key,
                        &info.key,
                        "playing",
                        t,
                        info.duration_ms,
                    )) {
                        eprintln!("plex: timeline update failed: {}", e);
                    }
                }

                // Authoritative final position. Skip if we never read a real position
                // (mpv failed to start / exited immediately) so we don't clobber an
                // existing resume point with 0.
                let t = last_t_ms.load(Ordering::Relaxed);
                if t > 0 {
                    let _ = rt.block_on(crate::plex_api::update_timeline(
                        &client,
                        &info.server_base,
                        &info.token,
                        &info.client_identifier,
                        &info.rating_key,
                        &info.key,
                        "stopped",
                        t,
                        info.duration_ms,
                    ));
                    if let Err(e) = rt.block_on(crate::plex_api::update_progress(
                        &client,
                        &info.server_base,
                        &info.token,
                        &info.client_identifier,
                        &info.rating_key,
                        t,
                    )) {
                        eprintln!("plex: final progress update failed: {}", e);
                    }
                }
            })();
            if let Some(on_end) = on_end {
                on_end(last_t_ms.load(Ordering::Relaxed));
            }
        })?;
    Ok(())
}

/// Jellyfin/Emby: POST to the `/Sessions/Playing*` endpoints — Start on the first
/// tick, Progress on each tick, Stopped (authoritative) when mpv exits. The
/// caller guarantees a non-empty `track.base_url` (see the match guards in
/// [`play`]).
fn start_tracking_jellyfin(
    socket_path: String,
    track: JellyfinTrack,
    start_ms: u64,
    stop_flag: Arc<AtomicBool>,
    window_observation: WindowStateObservation,
    on_end: Option<EndNotify>,
) -> std::io::Result<()> {
    let (last_t_ms, done) =
        spawn_position_reader(socket_path, start_ms, stop_flag.clone(), window_observation)?;

    std::thread::Builder::new()
        .name("mpv-tracker-jellyfin".into())
        .spawn(move || {
            // Every exit funnels past the notifier below, so it fires exactly
            // once per session even on early bails inside this closure.
            (|| {
                // A single current-thread runtime: the tracker only does sequential
                // block_on HTTP posts, so a full multi-thread runtime (its own thread
                // pool) per playback session would be wasteful.
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(r) => r,
                    Err(_) => return,
                };
                // Bail rather than fall back to a default client with no timeout — a hung
                // check-in post would otherwise pin this tracker thread indefinitely.
                let client = match reqwest::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .build()
                {
                    Ok(c) => c,
                    Err(_) => return,
                };

                // Build a PlaybackProgressInfo-shaped body (Emby/Jellyfin check-in model)
                // so history/dashboard/resume all tie to the right session.
                let post = |endpoint: &str, position_ms: u64, event: Option<&str>| {
                    let url = format!("{}/Sessions/Playing{}", track.base_url, endpoint);
                    let mut body = serde_json::json!({
                        "ItemId": track.item_id,
                        "MediaSourceId": track.media_source_id,
                        "PositionTicks": position_ms.saturating_mul(10_000),
                        "PlayMethod": "DirectPlay",
                        "CanSeek": true,
                        "IsPaused": false,
                    });
                    if let Some(ps) = &track.play_session_id {
                        body["PlaySessionId"] = serde_json::Value::String(ps.clone());
                    }
                    if let Some(ev) = event {
                        body["EventName"] = serde_json::Value::String(ev.to_string());
                    }
                    let mut rb = client.post(&url).json(&body);
                    for (k, v) in &track.headers {
                        rb = rb.header(k, v);
                    }
                    if let Err(e) =
                        rt.block_on(async { rb.send().await.and_then(|r| r.error_for_status()) })
                    {
                        eprintln!("jellyfin: progress update failed: {}", e);
                    }
                };

                post("", last_t_ms.load(Ordering::Relaxed), None); // Start
                while wait_tick(&stop_flag, &done) {
                    post(
                        "/Progress",
                        last_t_ms.load(Ordering::Relaxed),
                        Some("TimeUpdate"),
                    );
                }
                // Always post Stopped — including at position 0 — since we posted Start.
                // Otherwise a fast mpv exit (before any IPC position arrives) would leave
                // a stale "now playing" session on the server.
                post("/Stopped", last_t_ms.load(Ordering::Relaxed), None);
            })();
            if let Some(on_end) = on_end {
                on_end(last_t_ms.load(Ordering::Relaxed));
            }
        })?;
    Ok(())
}

/// For sessions with no progress tracker (local/SMB files, or a degenerate
/// track target): observe mpv's position over IPC and fire the end notifier
/// with the last seen position when mpv exits (or the session is replaced).
/// Tracked sessions instead notify from their tracker tail, after the final
/// server write. No-op without a notifier.
fn spawn_end_watcher(
    socket_path: String,
    stop_flag: Arc<AtomicBool>,
    window_observation: WindowStateObservation,
    on_end: Option<EndNotify>,
) -> std::io::Result<()> {
    // Reuse the shared position reader: its `done` flag doubles as the EOF
    // signal (set when mpv exits / the socket drops).
    let (last_t_ms, done) =
        spawn_position_reader(socket_path, 0, stop_flag.clone(), window_observation)?;
    let Some(on_end) = on_end else {
        return Ok(());
    };
    std::thread::Builder::new()
        .name("mpv-end-watcher".into())
        .spawn(move || {
            while !done.load(Ordering::Relaxed) && !stop_flag.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(100));
            }
            // Fire even when the session was replaced or mpv exited before a
            // position arrived (position 0): the session is over either way,
            // and a spurious refresh is harmless.
            on_end(last_t_ms.load(Ordering::Relaxed));
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Review finding sel-4. `play` can start one player today, the external
    // one. Handed any other it must refuse, and refuse before it has done
    // anything: without the refusal an embedded play would be started as an
    // external one, with the options of neither.
    #[test]
    fn play_refuses_a_player_it_cannot_start_and_starts_nothing() {
        let spec = PlaySpec {
            player: crate::player::PlayerKind::Embedded,
            url: "https://media.invalid/never-played".to_string(),
            title: String::new(),
            http_headers: Vec::new(),
            start_seconds: 0.0,
            autocrop_script: None,
            autocrop_shim: None,
            inherited_window_state: PlaybackWindowState::default(),
            screen_name: None,
            window_observation: WindowStateObservation::default(),
            markers_script: None,
            markers: Vec::new(),
            intro_policy: crate::config::SkipPolicy::Off,
            credits_policy: crate::config::SkipPolicy::Off,
            commercial_policy: crate::config::SkipPolicy::Off,
            step_down: None,
            duration: None,
            osd_notice: None,
            on_notice_shown: None,
            steps_taken: 0,
        };
        let slot = Arc::new(Mutex::new(None));
        let ended = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let on_end: EndNotify = {
            let ended = ended.clone();
            Arc::new(move |_position_ms| {
                ended.fetch_add(1, Ordering::SeqCst);
            })
        };
        let result = play(
            &spec,
            ProgressTarget::None,
            &slot,
            &Arc::new(AtomicBool::new(false)),
            &Arc::new(crate::commands::PlaybackAdvance::default()),
            "refused-session".to_string(),
            Some(on_end),
        );
        // If the refusal is ever lost, this test has just started a real
        // player on a machine that has one. Stop it before judging anything.
        let started = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        let started_a_player = started.is_some();
        if let Some(mut player) = started {
            let _ = player.kill();
        }
        if let Ok(stop) = &result {
            stop.store(true, Ordering::Relaxed);
        }
        assert_eq!(
            result.err().as_deref(),
            Some("the embedded player is not part of this build")
        );
        assert!(!started_a_player, "a refused play must not start a player");
        assert_eq!(
            ended.load(Ordering::SeqCst),
            0,
            "a play that never started has no end to report"
        );
    }

    // Review finding sel-3: "the player started" is not "the user was told".
    #[cfg(unix)]
    fn notice_listener(name: &str) -> (std::os::unix::net::UnixListener, String) {
        let dir = crate::player::launch::private_runtime_dir().expect("the private directory");
        let path = dir.join(format!("{name}-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("a socket to listen on");
        (listener, path.to_string_lossy().into_owned())
    }

    #[cfg(unix)]
    fn counting() -> (Arc<std::sync::atomic::AtomicUsize>, Option<NoticeShown>) {
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = count.clone();
        let shown: NoticeShown = Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        (count, Some(shown))
    }

    #[cfg(unix)]
    #[test]
    fn a_notice_that_reaches_the_player_is_reported_shown_once() {
        use std::io::{BufRead, BufReader, Read};
        let (listener, socket) = notice_listener("notice-shown");
        let (count, on_shown) = counting();
        spawn_osd_notice(
            socket.clone(),
            Arc::new(AtomicBool::new(false)),
            "A \"quoted\" line".to_string(),
            Duration::from_millis(30),
            on_shown,
        )
        .expect("the notice thread starts");
        let (stream, _) = listener.accept().expect("the notice thread connects");
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("the command arrives");
        assert_eq!(
            line,
            "{\"command\":[\"show-text\",\"A \\\"quoted\\\" line\",2000]}\n"
        );
        // The thread reports and then ends, closing its side.
        let mut rest = String::new();
        reader.read_to_string(&mut rest).expect("the thread ends");
        assert_eq!(rest, "", "one command and nothing else");
        assert_eq!(count.load(Ordering::SeqCst), 1, "reported exactly once");
        let _ = std::fs::remove_file(&socket);
    }

    #[cfg(unix)]
    #[test]
    fn a_notice_dropped_because_the_play_was_replaced_is_not_reported_shown() {
        use std::io::Read;
        let (listener, socket) = notice_listener("notice-dropped");
        let (count, on_shown) = counting();
        let replaced = Arc::new(AtomicBool::new(false));
        spawn_osd_notice(
            socket.clone(),
            replaced.clone(),
            "A line".to_string(),
            Duration::from_millis(200),
            on_shown,
        )
        .expect("the notice thread starts");
        // Connected, and now waiting for the picture. The play is replaced.
        let (mut stream, _) = listener.accept().expect("the notice thread connects");
        replaced.store(true, Ordering::Relaxed);
        let mut received = String::new();
        stream
            .read_to_string(&mut received)
            .expect("the thread ends and closes its side");
        assert_eq!(received, "", "nothing was sent to the player");
        assert_eq!(count.load(Ordering::SeqCst), 0, "and nothing was reported");
        let _ = std::fs::remove_file(&socket);
    }

    #[cfg(unix)]
    #[test]
    fn a_notice_for_a_player_that_cannot_be_reached_is_not_reported_shown() {
        let dir = crate::player::launch::private_runtime_dir().expect("the private directory");
        let nowhere = dir.join(format!("notice-nowhere-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&nowhere);
        let (count, on_shown) = counting();
        // Stop the connection attempts after the first few instead of
        // waiting out all fifty.
        let gave_up = Arc::new(AtomicBool::new(false));
        spawn_osd_notice(
            nowhere.to_string_lossy().into_owned(),
            gave_up.clone(),
            "A line".to_string(),
            Duration::from_millis(1),
            on_shown,
        )
        .expect("the notice thread starts");
        std::thread::sleep(Duration::from_millis(250));
        gave_up.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    fn event(name: &str, data: serde_json::Value) -> serde_json::Value {
        serde_json::json!({"event": "property-change", "id": 1, "name": name, "data": data})
    }

    /// Drive the REAL reader over a real socket, feeding it `events` in order,
    /// and return the position it ends up reporting as authoritative. A test of
    /// `position_property_change` alone would not have caught the original
    /// defect, which lived in the reader loop rather than in any classifier.
    #[cfg(unix)]
    fn position_reported_after(events: &[serde_json::Value]) -> u64 {
        use std::io::Write as _;
        use std::os::unix::net::UnixListener;

        let dir = crate::player::launch::private_runtime_dir().expect("runtime dir");
        let socket = dir.join(format!("ipc-test-{}.sock", events.len()));
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).expect("bind");

        let stop = Arc::new(AtomicBool::new(false));
        let (last_t_ms, done) = spawn_position_reader(
            socket.to_string_lossy().to_string(),
            0,
            stop.clone(),
            WindowStateObservation::default(),
        )
        .expect("reader");

        let (mut mpv, _) = listener.accept().expect("accept");
        for value in events {
            writeln!(mpv, "{value}").expect("write");
        }
        mpv.flush().expect("flush");
        // Closing the socket is mpv exiting; the reader hits EOF and finishes,
        // which is exactly when the tracker reads the final position.
        drop(mpv);
        for _ in 0..200 {
            if done.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&socket);
        last_t_ms.load(Ordering::Relaxed)
    }

    /// The end-to-end shape of the bug: mpv announces the display before it
    /// announces a position, and the play dies there. The tracker only skips
    /// its write when the position is still zero, so this must stay zero.
    #[cfg(unix)]
    #[test]
    fn a_play_that_dies_before_its_first_position_reports_none() {
        let reported = position_reported_after(&[
            event("display-width", serde_json::json!(3840)),
            event("display-height", serde_json::json!(2160)),
            event("fullscreen", serde_json::json!(true)),
        ]);
        assert_eq!(
            reported, 0,
            "a play with no time-pos must report no position, or the tracker \
             clobbers the user's real resume point"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_display_event_after_a_position_does_not_replace_it() {
        let reported = position_reported_after(&[
            event("time-pos", serde_json::json!(61.0)),
            event("display-width", serde_json::json!(3840)),
        ]);
        assert_eq!(
            reported, 61_000,
            "the last real position must survive a later display event"
        );
    }

    /// The defect: `display-width` 3840 became a position of 3,840,000 ms. mpv
    /// emits it before the first `time-pos`, so a play that ended in that window
    /// wrote a fabricated resume point over the user's real one.
    #[test]
    fn a_display_dimension_is_not_a_playback_position() {
        assert_eq!(
            position_property_change(&event("display-width", serde_json::json!(3840))),
            None,
            "a display width must never be read as a position"
        );
        assert_eq!(
            position_property_change(&event("display-height", serde_json::json!(2160))),
            None,
            "a display height must never be read as a position"
        );
    }

    /// Slice 5's own signals are numeric and continuous. If any numeric property
    /// can be a position, Automatic corrupts every position it observes.
    #[test]
    fn no_other_numeric_property_is_a_playback_position() {
        for property in [
            "decoder-frame-drop-count",
            "demuxer-cache-duration",
            "cache-speed",
            "percent-pos",
        ] {
            assert_eq!(
                position_property_change(&event(property, serde_json::json!(12))),
                None,
                "{property} must never be read as a position"
            );
        }
    }

    #[test]
    fn a_real_time_pos_is_still_a_playback_position() {
        assert_eq!(
            position_property_change(&event("time-pos", serde_json::json!(42.5))),
            Some(42_500)
        );
        assert_eq!(
            position_property_change(&event("time-pos", serde_json::json!(0))),
            Some(0)
        );
        // Values mpv really sends around startup and seeks, none of them a position.
        assert_eq!(
            position_property_change(&event("time-pos", serde_json::Value::Null)),
            None,
            "mpv reports null before playback starts"
        );
        assert_eq!(
            position_property_change(&event("time-pos", serde_json::json!(-1.0))),
            None,
            "a negative time-pos is not a position"
        );
        // Only property-change events count; a command reply carries no name.
        assert_eq!(
            position_property_change(&serde_json::json!({"request_id": 1, "error": "success"})),
            None
        );
    }

    #[test]
    fn window_property_parser_accepts_only_named_boolean_change_events() {
        assert_eq!(
            window_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "fullscreen",
                "data": true
            })),
            Some((WindowProperty::Fullscreen, true))
        );
        assert_eq!(
            window_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "window-maximized",
                "data": false
            })),
            Some((WindowProperty::Maximized, false))
        );

        for rejected in [
            serde_json::json!({ "request_id": 2, "data": true }),
            serde_json::json!({
                "event": "property-change",
                "name": "fullscreen",
                "data": null
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "fullscreen",
                "data": "yes"
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "window-minimized",
                "data": true
            }),
            serde_json::json!({
                "event": "end-file",
                "name": "fullscreen",
                "data": true
            }),
        ] {
            assert_eq!(window_property_change(&rejected), None);
        }
    }

    #[test]
    fn window_observation_keeps_true_false_and_unknown_independent() {
        let observation = WindowStateObservation::default();
        assert_eq!(observation.snapshot(), PlaybackWindowState::default());

        observation.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "fullscreen",
            "data": true
        }));
        assert_eq!(
            observation.snapshot(),
            PlaybackWindowState {
                fullscreen: Some(true),
                maximized: None,
            }
        );

        observation.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "fullscreen",
            "data": false
        }));
        observation.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "window-maximized",
            "data": true
        }));
        assert_eq!(
            observation.snapshot(),
            PlaybackWindowState {
                fullscreen: Some(false),
                maximized: Some(true),
            }
        );
    }

    #[test]
    fn display_property_parser_accepts_only_owned_typed_change_events() {
        assert_eq!(
            display_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "display-names",
                "data": ["DP-1", "HDMI-A-1"]
            })),
            Some(DisplayPropertyChange::Names(vec![
                "DP-1".to_string(),
                "HDMI-A-1".to_string()
            ]))
        );
        assert_eq!(
            display_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "display-width",
                "data": 3840
            })),
            Some(DisplayPropertyChange::Width(3840))
        );
        assert_eq!(
            display_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "display-height",
                "data": 2160
            })),
            Some(DisplayPropertyChange::Height(2160))
        );
        for rejected in [
            serde_json::json!({ "request_id": 4, "data": ["DP-1"] }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-names",
                "data": ["DP-1", 2]
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-width",
                "data": "3840"
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-height",
                "data": 0
            }),
        ] {
            assert_eq!(display_property_change(&rejected), None);
        }
    }

    #[test]
    fn display_observation_is_complete_and_isolated_per_launch() {
        let old = WindowStateObservation::default();
        let successor = WindowStateObservation::default();
        for event in [
            serde_json::json!({
                "event": "property-change",
                "name": "display-names",
                "data": ["DP-1"]
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-width",
                "data": 2560
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-height",
                "data": 1440
            }),
        ] {
            old.apply_ipc_event(&event);
        }
        assert_eq!(
            old.display_snapshot(),
            crate::display::ObservedDisplay {
                names: vec!["DP-1".to_string()],
                width_px: Some(2560),
                height_px: Some(1440),
            }
        );
        assert_eq!(
            successor.display_snapshot(),
            crate::display::ObservedDisplay::default(),
            "an older IPC reader cannot publish into a successor's handle"
        );
    }

    #[test]
    fn playback_window_observation_handles_are_isolated_per_launch() {
        let old = WindowStateObservation::default();
        let successor = WindowStateObservation::default();
        old.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "fullscreen",
            "data": true
        }));
        successor.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "fullscreen",
            "data": false
        }));

        assert_eq!(old.snapshot().fullscreen, Some(true));
        assert_eq!(successor.snapshot().fullscreen, Some(false));

        old.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "window-maximized",
            "data": true
        }));
        assert_eq!(successor.snapshot().maximized, None);
    }
}
