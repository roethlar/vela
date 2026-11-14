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

/// Runtime mpv window state that is carried to the next play in this run of
/// Vela, and, when the player window setting remembers it, to the first play
/// of the next run (`config::MpvWindow`). `None` means mpv has not reported the property, which is distinct
/// from an explicit `false` and therefore leaves configured launch options
/// authoritative.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlaybackWindowState {
    pub fullscreen: Option<bool>,
    pub maximized: Option<bool>,
    /// The window's size, width then height, as mpv reported it
    /// (`osd-dimensions`) while the window was neither fullscreen nor
    /// maximized, or else the size the play was opened with. Kept while the
    /// window is fullscreen or maximized, and from one play to the next when
    /// a play never shows its window at another size, so a play that opens
    /// fullscreen still knows the size to return to. `None` when no usable
    /// size is known.
    pub windowed_size: Option<(u32, u32)>,
}

/// A reported window size is used only inside these bounds, in each
/// dimension. Anything outside is not a window anyone sized: a player with
/// no window reports 0, and a broken report must not open a huge window.
const MIN_WINDOWED_SIZE: (u32, u32) = (160, 90);
const MAX_WINDOWED_DIMENSION: u32 = 16384;

/// How long a reported size must stand, with the window neither fullscreen
/// nor maximized, before it counts as the windowed size. On X11 the size of
/// a maximized window can be reported just before the maximized flag; the
/// wait lets that flag arrive and discard the size.
const WINDOWED_SIZE_SETTLE: Duration = Duration::from_millis(1000);

/// The window state one mpv process reports, and the rule that turns its
/// size reports into a windowed size. Pure: the caller passes the time of
/// each event, so the rule is tested without mpv or a clock.
#[derive(Debug, Clone, Copy, Default)]
struct WindowTracker {
    fullscreen: Option<bool>,
    maximized: Option<bool>,
    /// A size reported while the window was neither fullscreen nor
    /// maximized, and when, not yet long enough ago to be confirmed.
    candidate: Option<((u32, u32), Instant)>,
    /// The last size that was confirmed as the windowed size.
    confirmed: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowChange {
    Flag(WindowProperty, bool),
    /// A width and a height that mpv reported together, in one
    /// `osd-dimensions` event. A size is never put together from two
    /// reports (review finding pw-3): one half of it could be the other
    /// half of a size the window never was.
    Size(u32, u32),
}

impl WindowTracker {
    fn fullscreen_or_maximized(&self) -> bool {
        self.fullscreen == Some(true) || self.maximized == Some(true)
    }

    /// Any event from the player, at `now`. A candidate that has stood for
    /// the settle time with the window neither fullscreen nor maximized
    /// becomes the windowed size; this runs before the event's own change,
    /// so a flag that turns on later than that does not discard it.
    fn tick(&mut self, now: Instant) {
        if let Some((size, at)) = self.candidate {
            if !self.fullscreen_or_maximized()
                && now.saturating_duration_since(at) >= WINDOWED_SIZE_SETTLE
            {
                self.confirmed = Some(size);
                self.candidate = None;
            }
        }
    }

    fn apply(&mut self, change: WindowChange, now: Instant) {
        self.tick(now);
        match change {
            WindowChange::Flag(property, enabled) => {
                match property {
                    WindowProperty::Fullscreen => self.fullscreen = Some(enabled),
                    WindowProperty::Maximized => self.maximized = Some(enabled),
                }
                // The size just reported may be the fullscreen or maximized
                // one, reported before this flag.
                if enabled {
                    self.candidate = None;
                }
            }
            WindowChange::Size(width, height) => self.observe_size(width, height, now),
        }
    }

    fn observe_size(&mut self, width: u32, height: u32, now: Instant) {
        // While fullscreen or maximized the size is the screen's or the
        // work area's, not the window's.
        if self.fullscreen_or_maximized() {
            return;
        }
        // A size outside the bounds (a player with no window reports 0x0)
        // changes nothing: not the candidate, not the confirmed size.
        if !usable_windowed_size(width, height) {
            return;
        }
        if self.candidate.map(|(size, _)| size) != Some((width, height)) {
            self.candidate = Some(((width, height), now));
        }
    }

    /// The state for the next play. A candidate not yet confirmed counts
    /// when the window is neither fullscreen nor maximized: the user resized
    /// it and quit within the settle time.
    fn snapshot(&self) -> PlaybackWindowState {
        let pending = self
            .candidate
            .filter(|_| !self.fullscreen_or_maximized())
            .map(|(size, _)| size);
        PlaybackWindowState {
            fullscreen: self.fullscreen,
            maximized: self.maximized,
            windowed_size: pending.or(self.confirmed),
        }
    }
}

pub(crate) fn usable_windowed_size(width: u32, height: u32) -> bool {
    (MIN_WINDOWED_SIZE.0..=MAX_WINDOWED_DIMENSION).contains(&width)
        && (MIN_WINDOWED_SIZE.1..=MAX_WINDOWED_DIMENSION).contains(&height)
}

/// Per-launch observation handle. Each IPC reader owns a fresh handle so a
/// delayed event from an older mpv process cannot overwrite a newer session's
/// published state.
#[derive(Debug, Clone, Default)]
pub struct WindowStateObservation {
    state: Arc<Mutex<WindowTracker>>,
    display: Arc<Mutex<crate::display::ObservedDisplay>>,
}

impl WindowStateObservation {
    /// A fresh handle for a play whose window opens at `windowed_size`. That
    /// size counts as confirmed until the play reports a windowed size of its
    /// own, so a play that stays fullscreen or maximized, or never shows a
    /// window, hands the same size on instead of none.
    pub fn opened_at(windowed_size: Option<(u32, u32)>) -> Self {
        Self {
            state: Arc::new(Mutex::new(WindowTracker {
                confirmed: windowed_size,
                ..WindowTracker::default()
            })),
            display: Arc::default(),
        }
    }

    pub fn snapshot(&self) -> PlaybackWindowState {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot()
    }

    pub(crate) fn display_snapshot(&self) -> crate::display::ObservedDisplay {
        self.display
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// One event from the player's IPC connection, any event: every one
    /// moves the window-size rule's clock on, not only the window's own.
    pub(crate) fn apply_ipc_event(&self, value: &serde_json::Value) {
        self.apply_ipc_event_at(value, Instant::now());
    }

    fn apply_ipc_event_at(&self, value: &serde_json::Value, now: Instant) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            match window_property_change(value) {
                Some(change) => state.apply(change, now),
                None => state.tick(now),
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

/// Accept only owned property-change events of the exact JSON type: booleans
/// for the two flags, and for the window size an `osd-dimensions` object
/// whose `w` and `h` are both whole numbers that fit a u32. Observe-property
/// command replies, nulls, numeric/string coercions, a size missing either
/// half, and unrelated properties must not invent state.
fn window_property_change(value: &serde_json::Value) -> Option<WindowChange> {
    if value.get("event")?.as_str()? != "property-change" {
        return None;
    }
    let data = value.get("data")?;
    let dimension = |key: &str| {
        data.as_object()?
            .get(key)?
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
    };
    match value.get("name")?.as_str()? {
        "fullscreen" => Some(WindowChange::Flag(
            WindowProperty::Fullscreen,
            data.as_bool()?,
        )),
        "window-maximized" => Some(WindowChange::Flag(
            WindowProperty::Maximized,
            data.as_bool()?,
        )),
        // mpv sends `{"w":800,"h":450,"par":1.0,...}`, and `w` and `h` of 0
        // while there is no video output.
        "osd-dimensions" => Some(WindowChange::Size(dimension("w")?, dimension("h")?)),
        _ => None,
    }
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
    /// Absolute path to Vela's own `vela-crop.lua` (same resolver and
    /// existence check), the cropping of a play in Vela's own window: the
    /// stock script above is never loaded there. `None` when it could not be
    /// resolved — that play is then uncropped.
    pub crop_script: Option<String>,
    /// Actual state sampled from the last play that started, manual or
    /// automatic. The first play of a run gets what the player window setting
    /// says (`config::first_window_of_run`); unknown properties stay `None`.
    pub inherited_window_state: PlaybackWindowState,
    /// The output the last play's window was observed on (for the first play
    /// of a run, the one the setting remembers), or, when there is none, the
    /// native output containing Vela's window.
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
    /// What this play says if it was given the embedded player and has to be
    /// started in the external one after all ([`play`]).
    pub(crate) if_embedded_cannot_start: EmbeddedFallback,
    /// Steps this LOGICAL play has already taken, carried into the new
    /// detector so the cap counts plays rather than mpv processes.
    pub steps_taken: u32,
}

/// What a play given the embedded player shows when that player could not
/// start it and the external one runs it instead: the one line that says so,
/// and what to call once the line has been shown. Both absent for a play
/// that explains nothing (the user has been told already, or the play is an
/// automatic relaunch), and unused for a play given the external player.
#[derive(Clone, Default)]
pub(crate) struct EmbeddedFallback {
    pub(crate) notice: Option<String>,
    pub(crate) on_notice_shown: Option<NoticeShown>,
}

/// A play that has started.
pub struct PlayStarted {
    /// Set to stop this play's trackers when another play replaces it.
    pub stop: Arc<AtomicBool>,
    /// The player running it. It is the one the play was given, except that a
    /// play given the embedded player runs in the external one when the
    /// embedded one could not start it.
    pub(crate) player: crate::player::PlayerKind,
}

/// Start a play's player: the embedded one when the play was given it
/// (`embedded` is then `Some`), otherwise the external one. "Otherwise"
/// includes an embedded player that could not start the play: a play is never
/// failed over the embedded player (decision D3 of
/// `.agents/plans/embedded-player.md`; review finding ec-6). The play fails
/// only if the external player cannot start it either.
///
/// `embedded` fails only after putting back everything it did, so `external`
/// starts as if it had been the first choice. One failure is different:
/// when nothing may be started any more because Vela is exiting, the play
/// ends there and the external player is not tried.
fn start_player<P>(
    embedded: Option<impl FnOnce() -> Result<P, NotStarted>>,
    external: impl FnOnce() -> Result<P, String>,
) -> Result<(P, crate::player::PlayerKind), String> {
    use crate::player::PlayerKind;
    let Some(embedded) = embedded else {
        return external().map(|player| (player, PlayerKind::External));
    };
    match embedded() {
        Ok(player) => Ok((player, PlayerKind::Embedded)),
        Err(NotStarted::MustNot(reason)) => Err(reason),
        Err(NotStarted::CouldNot(reason)) => {
            // The reason is the launcher's own words about its own failure:
            // never a URL, a title or a credential.
            eprintln!("vela: playing in the external player: {reason}");
            external()
                .map(|player| (player, PlayerKind::External))
                .map_err(|other| format!("{reason} The external player could not start either: {other}"))
        }
    }
}

/// Why the embedded player did not start a play.
#[derive(Debug, PartialEq, Eq)]
enum NotStarted {
    /// It could not. The play goes to the external player.
    CouldNot(String),
    /// Nothing may be started any more. The play ends here.
    MustNot(String),
}

/// What `play` answers once Vela has begun to exit.
const SHUTTING_DOWN: &str = "Vela is shutting down.";

/// A test's stand-in for the search for the `mpv` program.
#[cfg(test)]
type FindMpv = Box<dyn Fn() -> Result<String, String>>;

#[cfg(test)]
thread_local! {
    /// In effect on the thread that calls `play`. A test cannot depend on
    /// what is installed on the machine it runs on, and must not start a
    /// real player on it.
    static MPV_FOR_THIS_TEST: std::cell::RefCell<Option<FindMpv>> =
        const { std::cell::RefCell::new(None) };
}

/// A socket path for a second launch of the same play, never the first
/// launch's: an embedded player that failed to start may still own that one.
fn another_ipc_path(taken: &str) -> Result<String, String> {
    loop {
        let path = crate::player::launch::new_ipc_path()?;
        if path != taken {
            return Ok(path);
        }
        // Each path carries a number of its own, so this is not reached;
        // should it be, the next millisecond names another.
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Fired exactly once when a playback session ends — after the final server
/// check-in for tracked sessions, or at mpv exit for untracked ones — with
/// the last observed playback position in ms (0 when none was read). The
/// caller wires this to recents recording + UI notification (a
/// `playback-ended` Tauri event); this module stays UI-framework-free.
pub type EndNotify = Arc<dyn Fn(u64) + Send + Sync>;

/// The longest an end notice waits for the EOF watcher. mpv writes its
/// `end-file` before it closes the socket the watcher reads, and a watcher
/// still trying to connect gives up once the end is known, so the watcher
/// stops within moments of the end; this only bounds one that does not.
const EOF_WATCHER_WAIT: Duration = Duration::from_secs(1);

/// The EOF watcher's half of [`eof_watch`]: `watching` is dropped when the
/// watcher stops, and nothing is ever sent on it.
struct EofWatching {
    watching: std::sync::mpsc::Sender<()>,
    ended: Arc<AtomicBool>,
}

/// The end notice's half of [`eof_watch`].
struct EofWait {
    stopped: std::sync::mpsc::Receiver<()>,
    ended: Arc<AtomicBool>,
}

/// The two halves that let an end notice wait for one EOF watcher: the
/// notice tells the watcher the play has ended (so one still trying to
/// connect gives up), and learns when the watcher has stopped.
fn eof_watch() -> (EofWatching, EofWait) {
    let (watching, stopped) = std::sync::mpsc::channel();
    let ended = Arc::new(AtomicBool::new(false));
    (
        EofWatching {
            watching,
            ended: ended.clone(),
        },
        EofWait { stopped, ended },
    )
}

/// `notify`, called once the EOF watcher of `wait` has stopped, or after
/// [`EOF_WATCHER_WAIT`].
fn after_eof_watcher(notify: EndNotify, wait: EofWait) -> EndNotify {
    let EofWait { stopped, ended } = wait;
    let stopped = Mutex::new(stopped);
    Arc::new(move |position_ms| {
        ended.store(true, Ordering::Relaxed);
        let _ = stopped
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .recv_timeout(EOF_WATCHER_WAIT);
        notify(position_ms);
    })
}

/// See [`PlaySpec::on_notice_shown`].
pub type NoticeShown = Arc<dyn Fn() + Send + Sync>;

/// How long a notice waits after the player's IPC connection is up. Let the
/// picture arrive first: a notice drawn over a black frame during load
/// explains nothing.
const OSD_NOTICE_DELAY: Duration = Duration::from_millis(1500);

/// Where `play` publishes the player it starts, and what it can start one
/// with.
pub struct Players<'a> {
    /// The app's current-player slot. The player is put here the instant it
    /// is started, under the slot's lock.
    pub(crate) slot: &'a Arc<Mutex<Option<crate::player::PlayerHandle>>>,
    /// Set as the app exits, and checked under the slot's lock, so a play
    /// that races the exit either does not start or is found by its sweep.
    pub(crate) shutting_down: &'a Arc<AtomicBool>,
    /// Starts embedded plays. `None` where there is no embedded player; the
    /// external one needs nothing but the `mpv` program.
    pub(crate) embedded: Option<&'a dyn crate::player::EmbeddedLauncher>,
}

/// What `play` answers when handed an embedded play and nothing to start it
/// with.
const NO_EMBEDDED_PLAYER: &str = "No embedded player is available here.";

/// Start a player for `spec.url`, optionally seeking to `spec.start_seconds`,
/// and start background progress reporting that runs until the player ends.
/// Publishes the player into `player_slot` the instant it's launched (so an
/// app-exit that races tracker setup can still find and kill it), and returns
/// the stop flag so a later play can cancel this one. The caller must have
/// already cleared/killed any previous player, since this overwrites the slot.
pub fn play(
    spec: &PlaySpec,
    progress: ProgressTarget,
    players: Players<'_>,
    advance: &Arc<crate::commands::PlaybackAdvance>,
    session_id: String,
    on_end: Option<EndNotify>,
    on_state: Option<crate::player::report::StateNotify>,
) -> Result<PlayStarted, String> {
    let Players {
        slot: player_slot,
        shutting_down,
        embedded,
    } = players;
    // Each player has its own way to be started, and neither may be started
    // as the other: an embedded play is given an embedded launcher or is
    // refused here, before anything has been done. A play is given the
    // embedded kind only when `player::embedded_availability` said it can
    // run, so a user does not reach the refusal.
    //
    // A launcher that cannot prepare the play is another matter: that is the
    // embedded player failing to start, and the play goes on in the external
    // one (`start_player`). So its answer is kept, not returned.
    let embedded_start = match spec.player {
        crate::player::PlayerKind::External => None,
        crate::player::PlayerKind::Embedded => Some(
            embedded
                .ok_or_else(|| NO_EMBEDDED_PLAYER.to_string())?
                .prepare(&session_id),
        ),
    };
    let ipc_path = crate::player::launch::new_ipc_path()?;

    // The external player is a program on the machine; the embedded one is
    // not, and needs none unless it fails.
    let find_mpv = || {
        #[cfg(test)]
        if let Some(found) = MPV_FOR_THIS_TEST.with(|find| find.borrow().as_ref().map(|find| find())) {
            return found;
        }
        crate::player::external::resolve_mpv()?
            .ok_or_else(|| "mpv was not found. Install mpv to play video.".to_string())
    };
    let mpv_bin = match embedded_start {
        Some(_) => None,
        None => Some(find_mpv()?),
    };

    // Advanced/user mpv config (Settings → Advanced mpv options), loaded once here.
    let cfg =
        crate::config::load_config().map_err(|_| "could not read Vela settings".to_string())?;
    let plan = crate::player::launch::build_launch(spec, &cfg, &ipc_path)?;

    // Launch the player and publish it into the shared slot while still holding
    // the lock, so launch+register is atomic w.r.t. the app-exit handler: it can
    // never observe a launched-but-unregistered player and orphan it. The caller
    // has already killed any prior player, so the slot is expected to be empty.
    //
    // Don't launch into a shutting-down app. The exit handler sets this flag
    // and sweeps the slot under THIS lock, so checking it under the lock, each
    // time the lock is taken to start a player, closes the race where a play
    // starting during exit would register an mpv the sweep had already passed.
    //
    // The lock is the one the exit handler waits for on the window's thread, so
    // nothing is done under it but that check and the start itself. Looking for
    // the `mpv` program is not: it runs programs and waits for them. A play
    // given the external player has looked already, above. A play that has to
    // fall back to it looks, and describes its launch, between two holds of the
    // lock (review finding ec-6, reopened for looking inside the first).
    let mut external_ipc_path = None;
    let started_player = match (embedded_start, mpv_bin) {
        (None, None) => return Err(NO_EMBEDDED_PLAYER.to_string()),
        (None, Some(mpv_bin)) => {
            let mut slot = player_slot.lock().unwrap_or_else(|e| e.into_inner());
            if shutting_down.load(Ordering::SeqCst) {
                plan.discard_marker_payload();
                return Err(SHUTTING_DOWN.to_string());
            }
            *slot = Some(crate::player::external::launch(&mpv_bin, plan)?);
            crate::player::PlayerKind::External
        }
        (Some(prepared), _) => {
            let own_config = cfg.mpv_use_own_config.unwrap_or(false);
            let ((), kind) = start_player(
                Some(|| {
                    let mut slot = player_slot.lock().unwrap_or_else(|e| e.into_inner());
                    if shutting_down.load(Ordering::SeqCst) {
                        plan.discard_marker_payload();
                        return Err(NotStarted::MustNot(SHUTTING_DOWN.to_string()));
                    }
                    let player = match prepared {
                        Ok(start) => start(plan, own_config, &ipc_path),
                        Err(reason) => {
                            // Nothing will read what this launch wrote.
                            plan.discard_marker_payload();
                            Err(reason)
                        }
                    }
                    .map_err(NotStarted::CouldNot)?;
                    *slot = Some(player);
                    Ok(())
                }),
                || {
                    // The lock is not held here. The same play, described
                    // again for the other player, on a socket of its own.
                    let mpv_bin = find_mpv()?;
                    let path = another_ipc_path(&ipc_path)?;
                    let plan = crate::player::launch::build_launch_for(
                        spec,
                        crate::player::PlayerKind::External,
                        &cfg,
                        &path,
                    )?;
                    let mut slot = player_slot.lock().unwrap_or_else(|e| e.into_inner());
                    // Vela may have begun to exit while mpv was looked for.
                    if shutting_down.load(Ordering::SeqCst) {
                        plan.discard_marker_payload();
                        return Err(SHUTTING_DOWN.to_string());
                    }
                    *slot = Some(crate::player::external::launch(&mpv_bin, plan)?);
                    external_ipc_path = Some(path);
                    Ok(())
                },
            )?;
            kind
        }
    };
    // The trackers below follow the player that started, on its socket.
    let ipc_path = external_ipc_path.unwrap_or(ipc_path);
    let fell_back = started_player != spec.player;
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
    let state_session = session_id.clone();
    let (eof_watching, eof_wait) = eof_watch();
    if let Err(e) = spawn_eof_watcher(
        ipc_path.clone(),
        stop_flag.clone(),
        advance.clone(),
        session_id,
        eof_watching,
    ) {
        eprintln!("vela: couldn't spawn mpv EOF watcher: {e}");
    }

    // The player view's state, for an embedded play only: one that runs in
    // the external player, from the start or after falling back, has no view
    // and never gets the notifier. Non-fatal like the watcher above: the play
    // goes on, its view without controls. Only Linux has an embedded play.
    #[cfg(target_os = "linux")]
    if started_player == crate::player::PlayerKind::Embedded {
        if let Some(notify) = on_state {
            if let Err(e) = crate::player::report::spawn_reporter(
                ipc_path.clone(),
                stop_flag.clone(),
                state_session,
                spec.duration,
                notify,
            ) {
                eprintln!("vela: couldn't spawn the player state reporter: {e}");
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (on_state, state_session);

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
    // A play that fell back from the embedded player explains that, unless
    // it already carries a notice of its own; that explanation then stays due.
    let (notice, on_notice_shown) = match &spec.osd_notice {
        None if fell_back => (
            spec.if_embedded_cannot_start.notice.clone(),
            spec.if_embedded_cannot_start.on_notice_shown.clone(),
        ),
        carried => (carried.clone(), spec.on_notice_shown.clone()),
    };
    if let Some(notice) = notice {
        if let Err(e) = spawn_osd_notice(
            ipc_path.clone(),
            stop_flag.clone(),
            notice,
            OSD_NOTICE_DELAY,
            on_notice_shown,
        ) {
            eprintln!("vela: couldn't show the playback notice: {e}");
        }
    }

    // An embedded play is told of its end only once the EOF watcher has
    // stopped reading, so whoever is told knows whether the end was a clean
    // end of file: the page leaves the run's fullscreen at an end that is not
    // (pc-2), and a clean end read late would cost a run going on its
    // fullscreen. A play in the external player is told as before.
    let on_end = match started_player {
        crate::player::PlayerKind::Embedded => {
            on_end.map(|notify| after_eof_watcher(notify, eof_wait))
        }
        crate::player::PlayerKind::External => on_end,
    };

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
    Ok(PlayStarted {
        stop: stop_flag,
        player: started_player,
    })
}

/// Tiny IPC listener that signals the playback dispatcher when mpv finishes a
/// file CLEANLY (mpv emits `event=end-file, reason=eof`). User-closed (`quit`),
/// errored (`error`), or otherwise-stopped exits never fire the notifier — so
/// closing the player stops playback. Separate from the progress reader so it
/// runs for every source even when no progress tracker is active.
///
/// `watching` is dropped when the watcher stops, by any way (and at once if
/// it cannot be started); a watcher not yet connected when it is told the
/// play has ended gives up at its next failed try.
fn spawn_eof_watcher(
    socket_path: String,
    stop_flag: Arc<AtomicBool>,
    advance: Arc<crate::commands::PlaybackAdvance>,
    session_id: String,
    watching: EofWatching,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("mpv-eof-watcher".into())
        .spawn(move || {
            let EofWatching {
                watching: _watching,
                ended,
            } = watching;
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
                    // The play is over and its socket gone: no end of file
                    // will come.
                    Err(_) if ended.load(Ordering::Relaxed) => return,
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
                  {\"command\":[\"observe_property\",6,\"display-height\"]}\n\
                  {\"command\":[\"observe_property\",7,\"osd-dimensions\"]}\n",
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
                // above is either non-numeric or a display or window
                // dimension, and a dimension read as a position corrupts the
                // resume point.
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

    // Review finding sel-4. Handed an embedded play and nothing to start it
    // with, `play` must refuse, and refuse before it has done anything:
    // without the refusal an embedded play would be started as an external
    // one, with the options of neither.
    #[test]
    fn play_refuses_a_player_it_cannot_start_and_starts_nothing() {
        let spec = embedded_spec();
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
            Players {
                slot: &slot,
                shutting_down: &Arc::new(AtomicBool::new(false)),
                embedded: None,
            },
            &Arc::new(crate::commands::PlaybackAdvance::default()),
            "refused-session".to_string(),
            Some(on_end),
            None,
        );
        // If the refusal is ever lost, this test has just started a real
        // player on a machine that has one. Stop it before judging anything.
        let started = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        let started_a_player = started.is_some();
        if let Some(mut player) = started {
            let _ = player.kill();
        }
        if let Ok(started) = &result {
            started.stop.store(true, Ordering::Relaxed);
        }
        assert_eq!(result.err().as_deref(), Some(NO_EMBEDDED_PLAYER));
        assert!(!started_a_player, "a refused play must not start a player");
        assert_eq!(
            ended.load(Ordering::SeqCst),
            0,
            "a play that never started has no end to report"
        );
    }

    /// A play for the embedded player with nothing optional.
    fn embedded_spec() -> PlaySpec {
        PlaySpec {
            player: crate::player::PlayerKind::Embedded,
            url: "https://media.invalid/never-played".to_string(),
            title: String::new(),
            http_headers: Vec::new(),
            start_seconds: 0.0,
            autocrop_script: None,
            autocrop_shim: None,
            crop_script: None,
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
            if_embedded_cannot_start: EmbeddedFallback::default(),
            steps_taken: 0,
        }
    }

    /// Stands in for the embedded launcher: writes down what it is asked, in
    /// order, and hands back a handle to something that really runs.
    #[cfg(unix)]
    struct RecordingLauncher {
        asked: Arc<Mutex<Vec<String>>>,
    }

    #[cfg(unix)]
    impl crate::player::EmbeddedLauncher for RecordingLauncher {
        fn prepare(&self, session_id: &str) -> Result<crate::player::EmbeddedStart, String> {
            let asked = self.asked.clone();
            asked
                .lock()
                .unwrap()
                .push(format!("prepare {session_id}"));
            Ok(Box::new(move |plan, _own_config, ipc_path: &str| {
                let mut asked = asked.lock().unwrap();
                asked.push(format!("start {ipc_path}"));
                asked.extend(plan.args.iter().map(|arg| arg.to_cli()));
                asked.push(format!("url {}", plan.url));
                let stand_in = std::process::Command::new("sleep")
                    .arg("30")
                    .spawn()
                    .map_err(|e| e.to_string())?;
                Ok(crate::player::PlayerHandle::External(
                    crate::player::external::ManagedChild::for_test(stand_in),
                ))
            }))
        }
    }

    // The wiring the unit tests of the embedded launch cannot see: that `play`
    // hands an embedded play to the embedded launcher, complete, and to
    // nothing else. It needs no `mpv` program on the machine, which is itself
    // part of the point.
    #[cfg(unix)]
    #[test]
    fn an_embedded_play_is_started_by_the_embedded_launcher_from_its_described_launch() {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let launcher = RecordingLauncher {
            asked: asked.clone(),
        };
        let slot = Arc::new(Mutex::new(None));
        let result = play(
            &embedded_spec(),
            ProgressTarget::None,
            Players {
                slot: &slot,
                shutting_down: &Arc::new(AtomicBool::new(false)),
                embedded: Some(&launcher),
            },
            &Arc::new(crate::commands::PlaybackAdvance::default()),
            "embedded-session".to_string(),
            None,
            None,
        );
        // Whatever happened, leave nothing running.
        let published = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        let was_published = published.is_some();
        if let Some(mut player) = published {
            let _ = player.kill();
        }
        let started = result.unwrap_or_else(|reason| panic!("an embedded play starts through its launcher: {reason}"));
        started.stop.store(true, Ordering::Relaxed);
        assert!(was_published, "the launcher's player is the one published");
        assert_eq!(started.player, crate::player::PlayerKind::Embedded);

        let asked = asked.lock().unwrap().clone();
        assert_eq!(asked[0], "prepare embedded-session");
        let ipc_path = asked[1]
            .strip_prefix("start ")
            .expect("the launch is started second, after the window was asked")
            .to_string();
        let options = &asked[2..asked.len() - 1];
        // The user's own settings on the machine running this test may add
        // options of theirs; what Vela asserts after them is fixed.
        let last = |name: &str| {
            options
                .iter()
                .rev()
                .find(|option| option.starts_with(&format!("--{name}=")))
                .cloned()
        };
        assert_eq!(last("vo").as_deref(), Some("--vo=libmpv"));
        assert_eq!(last("force-window").as_deref(), Some("--force-window=no"));
        assert_eq!(last("idle").as_deref(), Some("--idle=once"));
        assert_eq!(
            last("input-ipc-server"),
            Some(format!("--input-ipc-server={ipc_path}")),
            "the socket the launch names is the one the trackers are given"
        );
        assert!(
            !options.iter().any(|option| option.starts_with("--title=")
                || option.starts_with("--gpu-api=")
                || option.starts_with("--target-colorspace-hint=")),
            "nothing of the external player's launch: {options:?}"
        );
        assert_eq!(
            asked.last().map(String::as_str),
            Some("url https://media.invalid/never-played")
        );
    }

    // ---- an embedded player that cannot start a play (review finding ec-6;
    // decision D3: a play is never failed over the embedded player)

    type Never = fn() -> Result<&'static str, NotStarted>;
    fn could_not(reason: &str) -> Result<&'static str, NotStarted> {
        Err(NotStarted::CouldNot(reason.to_string()))
    }

    #[test]
    fn a_play_given_the_external_player_starts_there() {
        let started = start_player(None::<Never>, || Ok("mpv"));
        assert_eq!(started, Ok(("mpv", crate::player::PlayerKind::External)));
        // And fails with that player's own reason, nothing added.
        let failed = start_player(None::<Never>, || Err::<&str, _>("mpv was not found.".to_string()));
        assert_eq!(failed, Err("mpv was not found.".to_string()));
    }

    #[test]
    fn a_play_the_embedded_player_starts_never_reaches_the_external_one() {
        let mut external_asked = false;
        let started = start_player(Some(|| Ok("embedded")), || {
            external_asked = true;
            Ok("mpv")
        });
        assert_eq!(started, Ok(("embedded", crate::player::PlayerKind::Embedded)));
        assert!(!external_asked);
    }

    #[test]
    fn a_play_the_embedded_player_cannot_start_runs_in_the_external_one() {
        let order = std::cell::RefCell::new(Vec::new());
        let started = start_player(
            Some(|| {
                order.borrow_mut().push("embedded");
                could_not("The embedded player could not start: no window.")
            }),
            || {
                order.borrow_mut().push("external");
                Ok("mpv")
            },
        );
        assert_eq!(
            started,
            Ok(("mpv", crate::player::PlayerKind::External)),
            "the play is not failed; it is the external player's now"
        );
        assert_eq!(
            *order.borrow(),
            ["embedded", "external"],
            "the external player is started only once the embedded one has failed"
        );
    }

    #[test]
    fn a_play_neither_player_can_start_fails_and_says_why_for_both() {
        let failed = start_player(
            Some(|| could_not("The embedded player could not start: no window.")),
            || Err("mpv was not found.".to_string()),
        );
        assert_eq!(
            failed,
            Err("The embedded player could not start: no window. The external player could not start either: mpv was not found.".to_string())
        );
    }

    #[test]
    fn a_play_that_must_not_start_is_not_started_in_the_other_player_either() {
        // Vela is exiting. That is not the embedded player failing, and the
        // external player must not be started in its place.
        let mut external_asked = false;
        let ended = start_player(
            Some(|| Err::<&str, _>(NotStarted::MustNot(SHUTTING_DOWN.to_string()))),
            || {
                external_asked = true;
                Ok("mpv")
            },
        );
        assert_eq!(ended, Err(SHUTTING_DOWN.to_string()), "said plainly, nothing added");
        assert!(!external_asked);
    }

    /// Stands in for an embedded launcher whose player will not start.
    #[cfg(unix)]
    struct FailingLauncher {
        asked: Arc<Mutex<Vec<String>>>,
    }

    #[cfg(unix)]
    impl crate::player::EmbeddedLauncher for FailingLauncher {
        fn prepare(&self, session_id: &str) -> Result<crate::player::EmbeddedStart, String> {
            let asked = self.asked.clone();
            asked.lock().unwrap().push(format!("prepare {session_id}"));
            Ok(Box::new(move |plan, _own_config, ipc_path: &str| {
                asked.lock().unwrap().push(format!("start {ipc_path}"));
                // As the real launcher does on failure: nothing is left of
                // what the launch wrote.
                plan.discard_marker_payload();
                Err("The embedded player could not start: no window in this test.".to_string())
            }))
        }
    }

    /// Have `play`, on this thread, find `found` when it looks for mpv, for
    /// as long as the returned guard lives.
    #[cfg(unix)]
    fn mpv_is(found: impl Fn() -> Result<String, String> + 'static) -> impl Drop {
        struct Restore;
        impl Drop for Restore {
            fn drop(&mut self) {
                MPV_FOR_THIS_TEST.with(|find| find.borrow_mut().take());
            }
        }
        MPV_FOR_THIS_TEST.with(|find| *find.borrow_mut() = Some(Box::new(found)));
        Restore
    }

    /// A program that stands in for mpv: it writes down the arguments it was
    /// started with and then stays alive until it is killed.
    #[cfg(unix)]
    fn stand_in_for_mpv(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("vela-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let program = dir.join("mpv-stand-in");
        let arguments = dir.join("arguments");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexec sleep 30\n",
                arguments.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        (program, arguments)
    }

    // The whole path through `play`, with stand-ins at both ends: an embedded
    // launcher that fails, and a program in place of mpv.
    #[cfg(unix)]
    #[test]
    fn a_play_the_embedded_player_cannot_start_is_started_in_the_external_one_by_play() {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let launcher = FailingLauncher {
            asked: asked.clone(),
        };
        let (program, arguments) = stand_in_for_mpv("fallback-starts");
        let found = program.to_string_lossy().into_owned();
        let _mpv = mpv_is(move || Ok(found.clone()));
        let slot = Arc::new(Mutex::new(None));
        let result = play(
            &embedded_spec(),
            ProgressTarget::None,
            Players {
                slot: &slot,
                shutting_down: &Arc::new(AtomicBool::new(false)),
                embedded: Some(&launcher),
            },
            &Arc::new(crate::commands::PlaybackAdvance::default()),
            "fallback-session".to_string(),
            None,
            None,
        );
        // The stand-in writes its arguments as it starts.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !arguments.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let started_with = std::fs::read_to_string(&arguments).unwrap_or_default();
        // Whatever happened, leave nothing running.
        let published = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        let published_external = matches!(published, Some(crate::player::PlayerHandle::External(_)));
        if let Some(mut player) = published {
            let _ = player.kill();
        }
        let _ = std::fs::remove_dir_all(program.parent().unwrap());
        let started = result.unwrap_or_else(|reason| panic!("the play is not failed: {reason}"));
        started.stop.store(true, Ordering::Relaxed);

        assert_eq!(started.player, crate::player::PlayerKind::External);
        assert!(published_external, "the external player is the one published");
        let asked = asked.lock().unwrap().clone();
        assert_eq!(asked[0], "prepare fallback-session");
        let embedded_socket = asked[1]
            .strip_prefix("start ")
            .expect("the embedded player was tried first")
            .to_string();
        assert_eq!(asked.len(), 2);
        // The external launch is the external player's own, not the embedded
        // one's handed on: its lifecycle option, its own socket, no library
        // output.
        let arguments: Vec<&str> = started_with.lines().collect();
        let last = |name: &str| {
            arguments
                .iter()
                .rev()
                .find(|argument| argument.starts_with(&format!("--{name}=")))
                .copied()
        };
        assert_eq!(last("idle"), Some("--idle=no"), "arguments: {arguments:?}");
        assert!(!arguments.contains(&"--vo=libmpv"));
        let socket = last("input-ipc-server")
            .and_then(|argument| argument.strip_prefix("--input-ipc-server="))
            .expect("the external player is given a socket");
        assert_ne!(socket, embedded_socket, "a socket of its own");
        assert_eq!(
            &arguments[arguments.len() - 2..],
            ["--", "https://media.invalid/never-played"]
        );
    }

    /// A notifier of player states that counts them.
    #[cfg(unix)]
    fn counting_states() -> (
        Arc<Mutex<Vec<crate::player::report::PlayerState>>>,
        crate::player::report::StateNotify,
    ) {
        let states = Arc::new(Mutex::new(Vec::new()));
        let notify: crate::player::report::StateNotify = {
            let states = states.clone();
            Arc::new(move |state: &crate::player::report::PlayerState| {
                states.lock().unwrap().push(state.clone())
            })
        };
        (states, notify)
    }

    /// Stands in for an embedded launcher whose player listens on the socket
    /// the launch names, as mpv does.
    #[cfg(target_os = "linux")]
    struct ListeningLauncher {
        listener: Arc<Mutex<Option<(std::os::unix::net::UnixListener, String)>>>,
    }

    #[cfg(target_os = "linux")]
    impl crate::player::EmbeddedLauncher for ListeningLauncher {
        fn prepare(&self, _session_id: &str) -> Result<crate::player::EmbeddedStart, String> {
            let listener = self.listener.clone();
            Ok(Box::new(move |_plan, _own_config, ipc_path: &str| {
                let bound = std::os::unix::net::UnixListener::bind(ipc_path).map_err(|e| e.to_string())?;
                *listener.lock().unwrap() = Some((bound, ipc_path.to_string()));
                let stand_in = std::process::Command::new("sleep")
                    .arg("30")
                    .spawn()
                    .map_err(|e| e.to_string())?;
                Ok(crate::player::PlayerHandle::External(
                    crate::player::external::ManagedChild::for_test(stand_in),
                ))
            }))
        }
    }

    // The player view's state reaches the page from an embedded play: `play`
    // starts the reporter on the play's own socket, with the play's session.
    #[cfg(target_os = "linux")]
    #[test]
    fn an_embedded_play_reports_its_state_to_its_view() {
        use std::io::{BufRead, Write};
        let listener = Arc::new(Mutex::new(None));
        let launcher = ListeningLauncher {
            listener: listener.clone(),
        };
        let (states, notify) = counting_states();
        let slot = Arc::new(Mutex::new(None));
        let mut spec = embedded_spec();
        spec.duration = Some(Duration::from_secs(1200));
        let result = play(
            &spec,
            ProgressTarget::None,
            Players {
                slot: &slot,
                shutting_down: &Arc::new(AtomicBool::new(false)),
                embedded: Some(&launcher),
            },
            &Arc::new(crate::commands::PlaybackAdvance::default()),
            "state-session".to_string(),
            None,
            Some(notify),
        );
        if let Some(mut player) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = player.kill();
        }
        let started = result.unwrap_or_else(|reason| panic!("the play starts: {reason}"));
        let (listener, path) = listener.lock().unwrap().take().expect("the launch was started");
        listener.set_nonblocking(true).unwrap();
        // Several of the play's threads connect; the reporter is the one
        // that asks for the crop script's state.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut reporter = None;
        while reporter.is_none() && std::time::Instant::now() < deadline {
            let Ok((stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
            // The other threads ask for other things, or nothing.
            let mut asked = String::new();
            let mut reader = std::io::BufReader::new(&stream);
            while !asked.contains("user-data/vela-crop/state") {
                match reader.read_line(&mut asked) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            if asked.contains("user-data/vela-crop/state") {
                reporter = Some(stream);
            }
        }
        let mut reporter = reporter.expect("the reporter connected to the play's socket");
        reporter
            .write_all(b"{\"event\":\"property-change\",\"id\":3,\"name\":\"pause\",\"data\":false}\n")
            .unwrap();
        while states.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        started.stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&path);
        let first = states.lock().unwrap().first().cloned().expect("a state was reported");
        assert_eq!(first.session_id, "state-session");
        assert_eq!(first.paused, Some(false));
        assert_eq!(first.duration, Some(1200.0), "the item's own duration until mpv has one");
    }

    // A play given the embedded player that runs in the external one has no
    // view: nothing reports its state, though the notifier was handed in.
    #[cfg(unix)]
    #[test]
    fn a_play_that_fell_back_reports_no_state() {
        let launcher = FailingLauncher {
            asked: Arc::new(Mutex::new(Vec::new())),
        };
        let (program, arguments) = stand_in_for_mpv("fallback-no-state");
        let found = program.to_string_lossy().into_owned();
        let _mpv = mpv_is(move || Ok(found.clone()));
        let (states, notify) = counting_states();
        let slot = Arc::new(Mutex::new(None));
        let result = play(
            &embedded_spec(),
            ProgressTarget::None,
            Players {
                slot: &slot,
                shutting_down: &Arc::new(AtomicBool::new(false)),
                embedded: Some(&launcher),
            },
            &Arc::new(crate::commands::PlaybackAdvance::default()),
            "fallback-state-session".to_string(),
            None,
            Some(notify),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !arguments.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let started_with = std::fs::read_to_string(&arguments).unwrap_or_default();
        // Listen on the external player's socket, as mpv would, while the
        // play's threads are still trying to connect.
        let socket = started_with
            .lines()
            .rev()
            .find_map(|argument| argument.strip_prefix("--input-ipc-server="))
            .expect("the external player is given a socket")
            .to_string();
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut asked_for_state = false;
        let watch_until = std::time::Instant::now() + Duration::from_millis(1500);
        while std::time::Instant::now() < watch_until {
            match listener.accept() {
                Ok((stream, _)) => {
                    use std::io::Read;
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
                    let mut asked = vec![0u8; 4096];
                    let read = (&stream).read(&mut asked).unwrap_or(0);
                    asked_for_state |= String::from_utf8_lossy(&asked[..read]).contains("vela-crop/state");
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        if let Some(mut player) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = player.kill();
        }
        let _ = std::fs::remove_file(&socket);
        let _ = std::fs::remove_dir_all(program.parent().unwrap());
        let started = result.unwrap_or_else(|reason| panic!("the play is not failed: {reason}"));
        started.stop.store(true, Ordering::Relaxed);
        assert_eq!(started.player, crate::player::PlayerKind::External);
        assert!(!asked_for_state, "no reporter connected to the external player");
        assert!(states.lock().unwrap().is_empty());
    }

    /// An end notifier that writes down whether `advance` knew of a clean end
    /// of file for `session` when it ran.
    #[cfg(target_os = "linux")]
    fn noting_clean_end(
        advance: &Arc<crate::commands::PlaybackAdvance>,
        session: &'static str,
    ) -> (Arc<Mutex<Option<bool>>>, EndNotify) {
        let seen = Arc::new(Mutex::new(None));
        let notify: EndNotify = {
            let seen = seen.clone();
            let advance = advance.clone();
            Arc::new(move |_position_ms| {
                *seen.lock().unwrap() = Some(advance.saw_eof(session));
            })
        };
        (seen, notify)
    }

    /// Plays mpv's side of a play's end on `listener`: the position reader
    /// loses the player first, and the EOF watcher is sent the clean
    /// `end-file` only 400 ms later, as when the watcher's thread is slow to
    /// read it. Returns what the end notifier knew (`noting_clean_end`).
    #[cfg(target_os = "linux")]
    fn end_with_a_late_clean_end_of_file(
        listener: std::os::unix::net::UnixListener,
        seen: &Arc<Mutex<Option<bool>>>,
    ) -> Option<bool> {
        use std::io::{Read, Write};
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let (mut watcher, mut reader) = (None, None);
        while (watcher.is_none() || reader.is_none()) && std::time::Instant::now() < deadline {
            let Ok((stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
            // The position reader asks for properties at once; the EOF
            // watcher only listens.
            let mut asked = [0u8; 256];
            match (&stream).read(&mut asked) {
                Ok(read) if read > 0 => reader = Some(stream),
                _ => watcher = Some(stream),
            }
        }
        let mut watcher = watcher.expect("the EOF watcher connected");
        drop(reader.expect("the position reader connected"));
        std::thread::sleep(Duration::from_millis(400));
        let _ = watcher.write_all(b"{\"event\":\"end-file\",\"reason\":\"eof\"}\n");
        while seen.lock().unwrap().is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let known = *seen.lock().unwrap();
        known
    }

    // pc-2: an embedded play's end is reported once the EOF watcher has
    // read what mpv said, so a clean end read late is still known as clean
    // (the page keeps a run's fullscreen across a clean end, and leaves it
    // at any other).
    #[cfg(target_os = "linux")]
    #[test]
    fn an_embedded_play_reports_its_end_once_a_clean_end_of_file_has_been_read() {
        let listener = Arc::new(Mutex::new(None));
        let launcher = ListeningLauncher {
            listener: listener.clone(),
        };
        let advance = Arc::new(crate::commands::PlaybackAdvance::default());
        let (seen, on_end) = noting_clean_end(&advance, "late-eof-session");
        let slot = Arc::new(Mutex::new(None));
        let result = play(
            &embedded_spec(),
            ProgressTarget::None,
            Players {
                slot: &slot,
                shutting_down: &Arc::new(AtomicBool::new(false)),
                embedded: Some(&launcher),
            },
            &advance,
            "late-eof-session".to_string(),
            Some(on_end),
            None,
        );
        if let Some(mut player) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = player.kill();
        }
        let started = result.unwrap_or_else(|reason| panic!("the play starts: {reason}"));
        let (bound, path) = listener.lock().unwrap().take().expect("the launch was started");
        let known = end_with_a_late_clean_end_of_file(bound, &seen);
        started.stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&path);
        assert_eq!(known, Some(true), "the end was reported, and as clean");
    }

    // pc-2: a play in the external player is told of its end as before, at
    // once, without waiting for the EOF watcher.
    #[cfg(target_os = "linux")]
    #[test]
    fn an_external_play_reports_its_end_without_waiting_for_the_eof_watcher() {
        let (program, arguments) = stand_in_for_mpv("external-end-not-delayed");
        let found = program.to_string_lossy().into_owned();
        let _mpv = mpv_is(move || Ok(found.clone()));
        let advance = Arc::new(crate::commands::PlaybackAdvance::default());
        let (seen, on_end) = noting_clean_end(&advance, "external-end-session");
        let mut spec = embedded_spec();
        spec.player = crate::player::PlayerKind::External;
        let slot = Arc::new(Mutex::new(None));
        let result = play(
            &spec,
            ProgressTarget::None,
            Players {
                slot: &slot,
                shutting_down: &Arc::new(AtomicBool::new(false)),
                embedded: None,
            },
            &advance,
            "external-end-session".to_string(),
            Some(on_end),
            None,
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !arguments.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let started_with = std::fs::read_to_string(&arguments).unwrap_or_default();
        let socket = started_with
            .lines()
            .rev()
            .find_map(|argument| argument.strip_prefix("--input-ipc-server="))
            .expect("the external player is given a socket")
            .to_string();
        let bound = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let known = end_with_a_late_clean_end_of_file(bound, &seen);
        if let Some(mut player) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = player.kill();
        }
        let _ = std::fs::remove_file(&socket);
        let _ = std::fs::remove_dir_all(program.parent().unwrap());
        let started = result.unwrap_or_else(|reason| panic!("the play starts: {reason}"));
        started.stop.store(true, Ordering::Relaxed);
        assert_eq!(started.player, crate::player::PlayerKind::External);
        assert_eq!(known, Some(false), "the end was reported before the late end of file was read");
    }

    /// An end notifier that writes down that it ran, behind an EOF watcher
    /// started on `socket` for a play no one stops.
    #[cfg(target_os = "linux")]
    fn end_behind_an_eof_watcher(socket: &std::path::Path, session: &str) -> (Arc<AtomicBool>, EndNotify) {
        let (watching, wait) = eof_watch();
        spawn_eof_watcher(
            socket.to_string_lossy().into_owned(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(crate::commands::PlaybackAdvance::default()),
            session.to_string(),
            watching,
        )
        .unwrap();
        let reported = Arc::new(AtomicBool::new(false));
        let notify: EndNotify = {
            let reported = reported.clone();
            Arc::new(move |_position_ms| reported.store(true, Ordering::Relaxed))
        };
        (reported, after_eof_watcher(notify, wait))
    }

    // pc-2: an EOF watcher still trying to connect when the play's end is
    // known gives up, so the end is reported at once, not after the wait.
    #[cfg(target_os = "linux")]
    #[test]
    fn an_end_is_not_held_by_an_eof_watcher_that_never_connected() {
        let socket = std::env::temp_dir().join(format!("vela-test-no-mpv-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket);
        let (reported, on_end) = end_behind_an_eof_watcher(&socket, "never-connected");
        let started = std::time::Instant::now();
        on_end(0);
        let took = started.elapsed();
        assert!(reported.load(Ordering::Relaxed), "the end was reported");
        assert!(took < Duration::from_millis(500), "the end was held {took:?}");
    }

    // pc-2: the wait for an EOF watcher that does not stop is bounded, far
    // below what the page would notice as a window kept fullscreen.
    #[cfg(target_os = "linux")]
    #[test]
    fn an_end_waits_at_most_a_second_for_an_eof_watcher_that_does_not_stop() {
        let socket = std::env::temp_dir().join(format!("vela-test-silent-mpv-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket);
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let (reported, on_end) = end_behind_an_eof_watcher(&socket, "silent");
        // Connected, and told nothing: the watcher reads on.
        let (silent, _) = listener.accept().unwrap();
        let started = std::time::Instant::now();
        on_end(0);
        let took = started.elapsed();
        drop(silent);
        let _ = std::fs::remove_file(&socket);
        assert!(reported.load(Ordering::Relaxed), "the end was reported");
        assert!(took < Duration::from_millis(1500), "the end was held {took:?}");
    }

    // The lock on the player slot is the one the exit handler waits for on
    // the window's thread. Looking for mpv runs programs and waits for them.
    #[cfg(unix)]
    #[test]
    fn looking_for_mpv_after_a_failed_embedded_start_does_not_hold_the_player_slot() {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let launcher = FailingLauncher {
            asked: asked.clone(),
        };
        let slot = Arc::new(Mutex::new(None));
        let slot_was_free = Arc::new(Mutex::new(None));
        let _mpv = mpv_is({
            let (slot, slot_was_free) = (slot.clone(), slot_was_free.clone());
            move || {
                *slot_was_free.lock().unwrap() = Some(slot.try_lock().is_ok());
                Err("no mpv in this test".to_string())
            }
        });
        let result = play(
            &embedded_spec(),
            ProgressTarget::None,
            Players {
                slot: &slot,
                shutting_down: &Arc::new(AtomicBool::new(false)),
                embedded: Some(&launcher),
            },
            &Arc::new(crate::commands::PlaybackAdvance::default()),
            "looking-session".to_string(),
            None,
            None,
        );
        assert_eq!(
            result.err().as_deref(),
            Some("The embedded player could not start: no window in this test. The external player could not start either: no mpv in this test")
        );
        assert_eq!(
            *slot_was_free.lock().unwrap(),
            Some(true),
            "mpv was looked for, and the player slot was not locked meanwhile"
        );
        assert!(slot.lock().unwrap().is_none(), "nothing was published");
        assert_eq!(asked.lock().unwrap().len(), 2, "the embedded player was tried first");
    }

    // Vela begins to exit while mpv is being looked for. The exit handler has
    // swept the slot by the time the search is over; a player started now
    // would be one nobody stops.
    #[cfg(unix)]
    #[test]
    fn a_fallback_is_not_started_once_vela_has_begun_to_exit_during_the_search() {
        let launcher = FailingLauncher {
            asked: Arc::new(Mutex::new(Vec::new())),
        };
        let (program, arguments) = stand_in_for_mpv("fallback-during-exit");
        let shutting_down = Arc::new(AtomicBool::new(false));
        let _mpv = mpv_is({
            let (shutting_down, found) = (shutting_down.clone(), program.to_string_lossy().into_owned());
            move || {
                shutting_down.store(true, Ordering::SeqCst);
                Ok(found.clone())
            }
        });
        let slot = Arc::new(Mutex::new(None));
        let result = play(
            &embedded_spec(),
            ProgressTarget::None,
            Players {
                slot: &slot,
                shutting_down: &shutting_down,
                embedded: Some(&launcher),
            },
            &Arc::new(crate::commands::PlaybackAdvance::default()),
            "exit-session".to_string(),
            None,
            None,
        );
        std::thread::sleep(Duration::from_millis(200));
        let was_started = arguments.exists();
        let published = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        let was_published = published.is_some();
        if let Some(mut player) = published {
            let _ = player.kill();
        }
        let _ = std::fs::remove_dir_all(program.parent().unwrap());
        if let Ok(started) = &result {
            started.stop.store(true, Ordering::Relaxed);
        }
        assert_eq!(
            result.err().as_deref(),
            Some("The embedded player could not start: no window in this test. The external player could not start either: Vela is shutting down.")
        );
        assert!(!was_started, "no player was started");
        assert!(!was_published);
    }

    #[cfg(unix)]
    #[test]
    fn a_second_launch_of_a_play_gets_a_socket_of_its_own() {
        // The embedded player that failed may still own the first.
        let first = crate::player::launch::new_ipc_path().unwrap();
        for _ in 0..10 {
            assert_ne!(another_ipc_path(&first).unwrap(), first);
        }
    }

    #[cfg(unix)]
    #[test]
    fn an_embedded_play_is_not_started_once_vela_is_shutting_down() {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let launcher = RecordingLauncher {
            asked: asked.clone(),
        };
        let slot = Arc::new(Mutex::new(None));
        let result = play(
            &embedded_spec(),
            ProgressTarget::None,
            Players {
                slot: &slot,
                shutting_down: &Arc::new(AtomicBool::new(true)),
                embedded: Some(&launcher),
            },
            &Arc::new(crate::commands::PlaybackAdvance::default()),
            "late-session".to_string(),
            None,
            None,
        );
        let published = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        let was_published = published.is_some();
        if let Some(mut player) = published {
            let _ = player.kill();
        }
        assert_eq!(result.err().as_deref(), Some("Vela is shutting down."));
        assert!(!was_published);
        assert_eq!(
            asked.lock().unwrap().clone(),
            ["prepare late-session"],
            "the window may have been asked; nothing was started"
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
    fn a_notice_the_player_did_not_take_is_not_reported_shown() {
        let (listener, socket) = notice_listener("notice-refused");
        let (count, on_shown) = counting();
        let (closed, when_closed) = std::sync::mpsc::channel();
        // A player that goes away during the wait: it accepts the connection
        // and closes it at once.
        let player = std::thread::spawn(move || {
            drop(listener.accept().expect("the notice thread connects"));
            drop(listener);
            let _ = closed.send(());
        });
        spawn_osd_notice(
            socket.clone(),
            Arc::new(AtomicBool::new(false)),
            "A line".to_string(),
            Duration::from_millis(150),
            on_shown,
        )
        .expect("the notice thread starts");
        when_closed
            .recv_timeout(Duration::from_secs(10))
            .expect("the player closes its side");
        player.join().expect("the stand-in player ends");
        // Well past the wait: the write has been tried, and failed.
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(count.load(Ordering::SeqCst), 0, "a failed write tells nobody");
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

    /// The real reader asks mpv for the window size and hands what mpv
    /// reports to the window rule: a size reported before the player closes
    /// is the windowed size of that play, and is not a position.
    #[cfg(unix)]
    #[test]
    fn the_reader_subscribes_to_the_window_size_and_records_it() {
        use std::io::Write as _;
        use std::os::unix::net::UnixListener;

        let dir = crate::player::launch::private_runtime_dir().expect("runtime dir");
        let socket = dir.join(format!("ipc-size-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).expect("bind");

        let observation = WindowStateObservation::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (last_t_ms, done) = spawn_position_reader(
            socket.to_string_lossy().to_string(),
            0,
            stop.clone(),
            observation.clone(),
        )
        .expect("reader");

        let (mut mpv, _) = listener.accept().expect("accept");
        mpv.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        let mut requests = BufReader::new(mpv.try_clone().expect("clone"));
        let mut observed = Vec::new();
        while !observed.iter().any(|name| name == "osd-dimensions") {
            let mut line = String::new();
            requests
                .read_line(&mut line)
                .expect("the reader asks for osd-dimensions");
            let request: serde_json::Value = serde_json::from_str(&line).expect("a command");
            assert_eq!(request["command"][0], "observe_property");
            observed.push(request["command"][2].as_str().expect("a name").to_string());
        }

        // As mpv sends them: a window, then a size that is not usable (the
        // window going away), which must leave the size before it.
        for value in [
            event(
                "osd-dimensions",
                serde_json::json!({ "w": 1024, "h": 576, "par": 1.0, "aspect": 1.777778,
                                    "mt": 0, "mb": 0, "ml": 0, "mr": 0 }),
            ),
            event("time-pos", serde_json::json!(3.0)),
            event(
                "osd-dimensions",
                serde_json::json!({ "w": 1024, "h": 0, "par": 0.0, "aspect": 0.0,
                                    "mt": 0, "mb": 0, "ml": 0, "mr": 0 }),
            ),
        ] {
            writeln!(mpv, "{value}").expect("write");
        }
        mpv.flush().expect("flush");
        drop(requests);
        drop(mpv);
        for _ in 0..200 {
            if done.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&socket);
        assert!(done.load(Ordering::Relaxed), "the reader finished");
        assert_eq!(observation.snapshot().windowed_size, Some((1024, 576)));
        assert_eq!(last_t_ms.load(Ordering::Relaxed), 3000);
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
            event("osd-dimensions", serde_json::json!({ "w": 1280, "h": 720 })),
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
    fn window_property_parser_accepts_only_named_typed_change_events() {
        assert_eq!(
            window_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "fullscreen",
                "data": true
            })),
            Some(WindowChange::Flag(WindowProperty::Fullscreen, true))
        );
        assert_eq!(
            window_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "window-maximized",
                "data": false
            })),
            Some(WindowChange::Flag(WindowProperty::Maximized, false))
        );
        // The size is one report holding both halves, as mpv 0.41 sends it.
        assert_eq!(
            window_property_change(&serde_json::json!({
                "event": "property-change",
                "id": 7,
                "name": "osd-dimensions",
                "data": { "w": 1280, "h": 720, "par": 1.0, "aspect": 1.777778,
                          "mt": 0, "mb": 0, "ml": 0, "mr": 0 }
            })),
            Some(WindowChange::Size(1280, 720))
        );
        // No video output: parsed as it is, and refused by the bounds.
        assert_eq!(
            window_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "osd-dimensions",
                "data": { "w": 0, "h": 0, "par": 0.0, "aspect": 0.0 }
            })),
            Some(WindowChange::Size(0, 0))
        );

        let dimensions = |data: serde_json::Value| {
            serde_json::json!({
                "event": "property-change",
                "name": "osd-dimensions",
                "data": data
            })
        };
        for rejected in [
            serde_json::json!({ "request_id": 7, "data": { "w": 1280, "h": 720 } }),
            // Half a size is never a size (review finding pw-3): the two
            // separate properties are not read at all.
            serde_json::json!({
                "event": "property-change",
                "name": "osd-width",
                "data": 1280
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "osd-height",
                "data": 720
            }),
            dimensions(serde_json::json!({ "w": 1280 })),
            dimensions(serde_json::json!({ "h": 720 })),
            dimensions(serde_json::json!({ "w": "1280", "h": 720 })),
            dimensions(serde_json::json!({ "w": 1280, "h": -720 })),
            dimensions(serde_json::json!({ "w": 1280.5, "h": 720 })),
            dimensions(serde_json::json!({ "w": 1280, "h": 4_294_967_296_u64 })),
            dimensions(serde_json::json!({ "w": null, "h": 720 })),
            dimensions(serde_json::json!([1280, 720])),
            dimensions(serde_json::json!(1280)),
            dimensions(serde_json::Value::Null),
            serde_json::json!({
                "event": "end-file",
                "name": "osd-dimensions",
                "data": { "w": 1280, "h": 720 }
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "fullscreen",
                "data": 1
            }),
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
            assert_eq!(window_property_change(&rejected), None, "{rejected}");
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
                windowed_size: None,
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
                windowed_size: None,
            }
        );
    }

    // The windowed-size rule, driven with explicit times. `t(ms)` is a fixed
    // origin plus `ms`, so nothing here waits or reads a clock.
    mod windowed_size {
        use super::super::*;

        fn t(ms: u64) -> Instant {
            static ORIGIN: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
            *ORIGIN.get_or_init(Instant::now) + Duration::from_millis(ms)
        }

        fn size(tracker: &mut WindowTracker, width: u32, height: u32, at: u64) {
            tracker.apply(WindowChange::Size(width, height), t(at));
        }

        fn flag(tracker: &mut WindowTracker, property: WindowProperty, on: bool, at: u64) {
            tracker.apply(WindowChange::Flag(property, on), t(at));
        }

        #[test]
        fn a_reported_size_becomes_the_candidate() {
            let mut tracker = WindowTracker::default();
            size(&mut tracker, 1280, 720, 10);
            assert_eq!(tracker.candidate, Some(((1280, 720), t(10))));
            assert_eq!(tracker.confirmed, None, "not confirmed yet");
        }

        // Review finding pw-3. After 800x450, a size that is not usable must
        // not lend either of its halves to the size carried on: the size
        // stays the one the window really had.
        #[test]
        fn an_unusable_report_after_a_size_leaves_that_size() {
            for unusable in [
                // The reviewer's two sequences, as one report each now
                // carries both halves: width 1024 with a height of 89; and
                // width 1024 with a height of 0, then width 0.
                vec![(1024, 89)],
                vec![(1024, 0), (0, 0)],
                vec![(0, 0)],
                vec![(0, 450), (1024, 0)],
                vec![(159, 450)],
                vec![(16385, 450), (800, 16385)],
            ] {
                // Confirmed, then the unusable reports.
                let mut tracker = WindowTracker::default();
                size(&mut tracker, 800, 450, 0);
                tracker.tick(t(1000));
                assert_eq!(tracker.confirmed, Some((800, 450)));
                for (width, height) in &unusable {
                    size(&mut tracker, *width, *height, 1500);
                }
                assert_eq!(tracker.candidate, None, "{unusable:?}");
                assert_eq!(tracker.snapshot().windowed_size, Some((800, 450)), "{unusable:?}");
                tracker.tick(t(5000));
                assert_eq!(tracker.confirmed, Some((800, 450)), "{unusable:?}");

                // A candidate not yet confirmed, then the unusable reports:
                // the candidate stands, with its own time.
                let mut tracker = WindowTracker::default();
                size(&mut tracker, 800, 450, 0);
                for (width, height) in &unusable {
                    size(&mut tracker, *width, *height, 100);
                }
                assert_eq!(tracker.candidate, Some(((800, 450), t(0))), "{unusable:?}");
                assert_eq!(tracker.snapshot().windowed_size, Some((800, 450)), "{unusable:?}");
                tracker.tick(t(1000));
                assert_eq!(tracker.confirmed, Some((800, 450)), "{unusable:?}");
            }
        }

        // From an existing size, a usable and an unusable report in either
        // order: the usable one is the size, whole.
        #[test]
        fn usable_and_unusable_reports_in_either_order() {
            for (first, second) in [
                ((1024, 576), (0, 0)),
                ((0, 0), (1024, 576)),
                ((1024, 576), (1280, 89)),
                ((1280, 89), (1024, 576)),
            ] {
                let mut tracker = WindowTracker::default();
                size(&mut tracker, 800, 450, 0);
                tracker.tick(t(1000));
                size(&mut tracker, first.0, first.1, 2000);
                size(&mut tracker, second.0, second.1, 2100);
                assert_eq!(
                    tracker.snapshot().windowed_size,
                    Some((1024, 576)),
                    "{first:?} then {second:?}"
                );
                tracker.tick(t(4000));
                assert_eq!(tracker.confirmed, Some((1024, 576)), "{first:?} then {second:?}");
            }
        }

        #[test]
        fn a_candidate_is_confirmed_by_a_later_event_a_second_after_it() {
            let mut tracker = WindowTracker::default();
            flag(&mut tracker, WindowProperty::Fullscreen, false, 0);
            size(&mut tracker, 800, 450, 100);
            tracker.tick(t(1099));
            assert_eq!(tracker.confirmed, None, "999 ms is not long enough");
            tracker.tick(t(1100));
            assert_eq!(tracker.confirmed, Some((800, 450)));
            assert_eq!(tracker.candidate, None);
            // Confirmed before the window went fullscreen, so it is kept as
            // the size to return to.
            flag(&mut tracker, WindowProperty::Fullscreen, true, 5000);
            assert_eq!(
                tracker.snapshot(),
                PlaybackWindowState {
                    fullscreen: Some(true),
                    maximized: None,
                    windowed_size: Some((800, 450)),
                }
            );
        }

        // mpv reports width and height apart, and may report the same size
        // again: a repeat keeps the time the candidate first stood.
        #[test]
        fn a_repeated_size_keeps_the_candidates_time() {
            let mut tracker = WindowTracker::default();
            size(&mut tracker, 800, 450, 0);
            size(&mut tracker, 800, 450, 900);
            tracker.tick(t(1000));
            assert_eq!(tracker.confirmed, Some((800, 450)));
        }

        #[test]
        fn a_flag_turning_on_discards_the_unconfirmed_candidate() {
            // X11 reports the maximized size just before the flag.
            let mut tracker = WindowTracker::default();
            size(&mut tracker, 800, 450, 0);
            tracker.tick(t(2000));
            assert_eq!(tracker.confirmed, Some((800, 450)));
            size(&mut tracker, 1920, 1050, 3000);
            flag(&mut tracker, WindowProperty::Maximized, true, 3050);
            assert_eq!(tracker.candidate, None);
            // A second later, still maximized: nothing is confirmed.
            tracker.tick(t(5000));
            assert_eq!(tracker.snapshot().windowed_size, Some((800, 450)));

            let mut tracker = WindowTracker::default();
            size(&mut tracker, 800, 450, 0);
            flag(&mut tracker, WindowProperty::Fullscreen, true, 20);
            assert_eq!(tracker.snapshot().windowed_size, None);
        }

        #[test]
        fn sizes_reported_while_fullscreen_or_maximized_are_ignored() {
            for property in [WindowProperty::Fullscreen, WindowProperty::Maximized] {
                let mut tracker = WindowTracker::default();
                flag(&mut tracker, property, true, 0);
                size(&mut tracker, 1920, 1080, 100);
                assert_eq!(tracker.candidate, None);
                tracker.tick(t(5000));
                assert_eq!(tracker.snapshot().windowed_size, None, "{property:?}");
            }
        }

        #[test]
        fn an_unconfirmed_candidate_counts_at_snapshot_unless_a_flag_is_on() {
            // Resized and quit within the second.
            let mut tracker = WindowTracker::default();
            size(&mut tracker, 800, 450, 0);
            size(&mut tracker, 1024, 576, 1500);
            assert_eq!(tracker.confirmed, Some((800, 450)));
            assert_eq!(tracker.snapshot().windowed_size, Some((1024, 576)));

            // The snapshot does not accept a candidate while a flag is on.
            let mut tracker = WindowTracker::default();
            size(&mut tracker, 800, 450, 0);
            tracker.maximized = Some(true);
            assert_eq!(tracker.snapshot().windowed_size, None);
        }

        #[test]
        fn a_size_outside_the_bounds_is_not_recorded() {
            for (width, height) in [
                (0, 0),
                (159, 450),
                (800, 89),
                (16385, 450),
                (800, 16385),
            ] {
                let mut tracker = WindowTracker::default();
                size(&mut tracker, width, height, 0);
                tracker.tick(t(2000));
                assert_eq!(
                    tracker.snapshot().windowed_size,
                    None,
                    "{width}x{height}"
                );
            }
            // A report outside the bounds changes nothing already recorded.
            let mut tracker = WindowTracker::default();
            size(&mut tracker, 800, 450, 0);
            size(&mut tracker, 0, 0, 100);
            assert_eq!(tracker.snapshot().windowed_size, Some((800, 450)));
            for (width, height) in [(160, 90), (16384, 16384)] {
                let mut tracker = WindowTracker::default();
                size(&mut tracker, width, height, 0);
                assert_eq!(tracker.snapshot().windowed_size, Some((width, height)));
            }
        }

        // A play opened at a size, that stays fullscreen and reports only the
        // screen's size, hands that size on; a windowed size it reports
        // itself replaces it.
        #[test]
        fn a_play_hands_on_the_size_it_was_opened_at_until_it_reports_its_own() {
            let observation = WindowStateObservation::opened_at(Some((800, 450)));
            let event = |name: &str, data: serde_json::Value| {
                serde_json::json!({ "event": "property-change", "name": name, "data": data })
            };
            let dimensions = |w: u32, h: u32| {
                event("osd-dimensions", serde_json::json!({ "w": w, "h": h }))
            };
            observation.apply_ipc_event_at(&event("fullscreen", true.into()), t(0));
            observation.apply_ipc_event_at(&dimensions(1920, 1080), t(0));
            observation.apply_ipc_event_at(&event("time-pos", 12.5.into()), t(5000));
            assert_eq!(
                observation.snapshot(),
                PlaybackWindowState {
                    fullscreen: Some(true),
                    maximized: None,
                    windowed_size: Some((800, 450)),
                }
            );
            // A play with no window (it reports 0x0) hands it on too.
            let observation = WindowStateObservation::opened_at(Some((800, 450)));
            observation.apply_ipc_event_at(&dimensions(0, 0), t(0));
            assert_eq!(observation.snapshot().windowed_size, Some((800, 450)));
            // Its own windowed size wins.
            observation.apply_ipc_event_at(&dimensions(1024, 576), t(100));
            assert_eq!(observation.snapshot().windowed_size, Some((1024, 576)));
            observation.apply_ipc_event_at(&event("time-pos", 13.into()), t(1200));
            observation.apply_ipc_event_at(&event("fullscreen", true.into()), t(1300));
            assert_eq!(observation.snapshot().windowed_size, Some((1024, 576)));
            // Opened with no size, nothing is invented.
            assert_eq!(
                WindowStateObservation::opened_at(None).snapshot(),
                PlaybackWindowState::default()
            );
        }

        // Through the IPC event path the reader uses: the observation feeds
        // every event to the rule, the ones that are not the window's too.
        #[test]
        fn the_observation_confirms_on_any_later_event() {
            let observation = WindowStateObservation::default();
            let event = |name: &str, data: serde_json::Value| {
                serde_json::json!({ "event": "property-change", "name": name, "data": data })
            };
            let dimensions = |w: u32, h: u32| {
                event("osd-dimensions", serde_json::json!({ "w": w, "h": h }))
            };
            observation.apply_ipc_event_at(&dimensions(800, 450), t(0));
            observation.apply_ipc_event_at(&event("time-pos", 12.5.into()), t(1000));
            assert_eq!(
                observation.state.lock().expect("tracker").confirmed,
                Some((800, 450)),
                "a time-pos event a second later confirms the size"
            );
            // The maximized size, then the flag: the confirmed size stays.
            observation.apply_ipc_event_at(&dimensions(1920, 1050), t(2000));
            observation.apply_ipc_event_at(&event("window-maximized", true.into()), t(2010));
            assert_eq!(
                observation.snapshot(),
                PlaybackWindowState {
                    fullscreen: None,
                    maximized: Some(true),
                    windowed_size: Some((800, 450)),
                }
            );
        }
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
