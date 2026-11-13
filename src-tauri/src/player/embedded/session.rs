//! One embedded play.
//!
//! A play is one thread that owns everything it draws with: the video
//! surface, the OpenGL window on it, the mpv player and mpv's renderer. The
//! rest of Vela reaches the player the way it reaches the external one,
//! through mpv's IPC socket; it reaches this thread only to stop it and to
//! tell it where the video rectangle is.
//!
//! Order matters twice here and is written into the types rather than
//! remembered: [`Live`]'s fields are declared in the order they must be torn
//! down (the renderer needs the OpenGL context and the player; the context
//! needs the surface), and Rust drops fields in declaration order.

use std::ffi::c_void;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wayland_client::protocol::wl_subsurface::WlSubsurface;

use super::egl;
use super::hdr::{self, Capabilities, Tagging};
use super::libmpv::{LibMpv, Player, Renderer, EVENT_NONE, EVENT_SHUTDOWN};
use super::options::Step;
use super::surface::{Guest, VideoSurface};
use crate::player::launch::HeaderInclude;

/// Where the video is, in the window's own units, relative to the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// What a play tells the rest of Vela about its own life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lifecycle {
    /// The video surface exists and the file has been handed to mpv.
    Opened,
    /// The player has shut down and its surface is gone.
    Closed,
}

/// The file of marker ranges a launch wrote for mpv's script to read once
/// and delete. The script deletes it when it has read it; this deletes it
/// when the play's start request goes, which is when the play has failed to
/// start or has ended. So no play leaves one behind, whether or not the
/// script ever ran (review finding ec-8: a start that failed before mpv
/// existed left it).
pub(crate) struct MarkerPayload(PathBuf);

impl MarkerPayload {
    pub(crate) fn new(path: PathBuf) -> Self {
        MarkerPayload(path)
    }
}

impl Drop for MarkerPayload {
    fn drop(&mut self) {
        // Usually gone already. A path of marker times, never a secret.
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Everything a play needs to start.
pub(crate) struct Start {
    pub(crate) lib: Arc<LibMpv>,
    /// Vela's place on the window's connection to the compositor.
    pub(crate) guest: Arc<Guest>,
    /// The playback session this play belongs to.
    pub(crate) session_id: String,
    /// The user's own mpv configuration files, when they asked for them.
    /// Applied before `steps`, so Vela's protected options still win.
    pub(crate) own_config: Vec<PathBuf>,
    /// The launch's options, in order (`super::options::steps`).
    pub(crate) steps: Vec<Step>,
    /// The IPC socket the options name. Read back after start-up as proof
    /// that mpv really took the options: it reports no error for a file it
    /// could not read.
    pub(crate) ipc_path: String,
    pub(crate) url: String,
    /// How long to wait between starting mpv and giving it its file. Zero
    /// outside tests. A player that is created first and handed its file
    /// later has to survive that gap whatever its length (plan review ep-1),
    /// and a test can only show that with a gap worth the name.
    pub(crate) load_delay: Duration,
    /// How long the caller waits for this play to start: [`START_LIMIT`],
    /// except where a test has asked for less.
    pub(crate) start_limit: Duration,
    pub(crate) header_include: Option<HeaderInclude>,
    /// Held, not read: see [`MarkerPayload`].
    pub(crate) _marker_payload: Option<MarkerPayload>,
    pub(crate) on_lifecycle: Arc<dyn Fn(Lifecycle) + Send + Sync>,
}

/// How long the caller waits for a play to start before reporting that it
/// did not.
pub(crate) const START_LIMIT: Duration = Duration::from_secs(20);

/// How long a play that is being let go of is waited for, having been asked
/// to stop, before it is handed to [`Leftovers`] instead.
const STOP_PATIENCE: Duration = Duration::from_secs(5);

/// Who decides whether a starting play has started: the play's own thread,
/// once mpv has its file, or the caller that is waiting for it, when it
/// stops waiting. Exactly one of them wins, and the other finds out.
///
/// Without this a play that outlasted its caller's patience could come back
/// later, hand mpv its file and announce itself, for a play that had already
/// been reported as failed (review finding ec-2).
struct StartRace(AtomicU8);

impl StartRace {
    const STARTING: u8 = 0;
    const STARTED: u8 = 1;
    const GIVEN_UP: u8 = 2;

    fn new() -> Self {
        StartRace(AtomicU8::new(Self::STARTING))
    }

    fn settle(&self, as_what: u8) -> bool {
        self.0
            .compare_exchange(Self::STARTING, as_what, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// The play's thread: "I have started." `false` when the caller had
    /// already stopped waiting; the play must then go without a trace.
    fn claim_started(&self) -> bool {
        self.settle(Self::STARTED)
    }

    /// The caller: "I am not waiting any longer." `false` when the play had
    /// started in the meantime; it is then a started play like any other.
    fn give_up(&self) -> bool {
        self.settle(Self::GIVEN_UP)
    }

    /// Whether the caller has stopped waiting.
    fn given_up(&self) -> bool {
        self.0.load(Ordering::SeqCst) == Self::GIVEN_UP
    }
}

/// How the last step of a start ended, on the play's thread.
#[derive(Debug, PartialEq, Eq)]
enum Started {
    Yes,
    /// The caller had stopped waiting. The play was reported as failed: it
    /// must not announce itself or draw, and goes away at once.
    TooLate,
}

/// The last step of a start, with everything built: hand mpv its file
/// (`load`) and claim the start. A play its caller has given up on is not
/// loaded when that is already known, and does not count as started when it
/// becomes known a moment later.
fn finish_start(
    race: &StartRace,
    load: impl FnOnce() -> Result<(), String>,
) -> Result<Started, String> {
    if race.given_up() {
        return Ok(Started::TooLate);
    }
    load()?;
    Ok(if race.claim_started() {
        Started::Yes
    } else {
        Started::TooLate
    })
}

/// Plays whose thread was still running when Vela stopped waiting for it.
///
/// A thread cannot be made to stop from outside. One that is stuck (in mpv,
/// in the graphics driver) still holds its surface and its player, and may
/// still come back. Letting go of its handle would leave it with nothing
/// that knows it exists (review finding ec-2). So it is kept here until it
/// has ended: while any is kept no new embedded play starts, and Vela gives
/// them one more moment as it exits.
struct Leftovers(Mutex<Vec<(Arc<Shared>, std::thread::JoinHandle<()>)>>);

/// The leftovers of this run of Vela.
static LEFTOVERS: Leftovers = Leftovers::new();

impl Leftovers {
    const fn new() -> Self {
        Leftovers(Mutex::new(Vec::new()))
    }

    fn keep(&self, shared: Arc<Shared>, thread: std::thread::JoinHandle<()>) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((shared, thread));
    }

    /// How many are still running. Those that have ended since the last
    /// call are let go of.
    fn still_running(&self) -> usize {
        let ended: Vec<_>;
        let running = {
            let mut kept = self.0.lock().unwrap_or_else(|e| e.into_inner());
            let (done, running): (Vec<_>, Vec<_>) = std::mem::take(&mut *kept)
                .into_iter()
                .partition(|(shared, _)| shared.ended.load(Ordering::SeqCst));
            ended = done;
            *kept = running;
            kept.len()
        };
        for (_, thread) in ended {
            let _ = thread.join();
        }
        running
    }

    /// Wait until none is running, for at most `limit`.
    fn wait(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while self.still_running() > 0 {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }
}

/// How many embedded plays Vela has stopped waiting for are still running.
pub(crate) fn unfinished_plays() -> usize {
    LEFTOVERS.still_running()
}

/// As Vela exits: give plays it had stopped waiting for one more moment to
/// end, since they draw on the window's connection.
pub(crate) fn wait_for_unfinished_plays(limit: Duration) -> bool {
    LEFTOVERS.wait(limit)
}

struct Shared {
    /// Written to wake the thread: by mpv (a frame or an event is waiting)
    /// and by Vela (stop, or the rectangle changed).
    wake: OwnedFd,
    stop: AtomicBool,
    ended: AtomicBool,
    /// Whether the play started, or its caller stopped waiting first.
    race: StartRace,
    /// The size the page last reported, when the thread has not applied it.
    resize_to: Mutex<Option<(i32, i32)>>,
    /// Set once the surface exists. Used from the window's thread only.
    subsurface: Mutex<Option<WlSubsurface>>,
}

impl Shared {
    fn new() -> Result<Arc<Self>, String> {
        // SAFETY: plain flags; a negative result is an error.
        let wake = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if wake < 0 {
            return Err("couldn't create the player's wake-up channel".to_string());
        }
        Ok(Arc::new(Shared {
            // SAFETY: a descriptor just created and owned by nothing else.
            wake: unsafe { OwnedFd::from_raw_fd(wake) },
            stop: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            race: StartRace::new(),
            resize_to: Mutex::new(None),
            subsurface: Mutex::new(None),
        }))
    }

    fn wake(&self) {
        let one: u64 = 1;
        // SAFETY: a valid eventfd and eight readable bytes. A full counter
        // (EAGAIN) already means "wake up".
        unsafe { libc::write(self.wake.as_raw_fd(), (&one as *const u64).cast(), 8) };
    }

    fn drain_wake(&self) {
        let mut count: u64 = 0;
        // SAFETY: a valid non-blocking eventfd and eight writable bytes.
        unsafe { libc::read(self.wake.as_raw_fd(), (&mut count as *mut u64).cast(), 8) };
    }
}

/// mpv calls this from its own threads. It may do nothing but wake the owner.
unsafe extern "C" fn wake_from_mpv(context: *mut c_void) {
    // SAFETY: `context` is the `Shared` of a play whose player and renderer,
    // the only callers, are destroyed before the play releases it.
    unsafe { (*(context as *const Shared)).wake() };
}

/// What exists while a play is running. Field order is teardown order.
struct Live {
    renderer: Renderer,
    player: Player,
    window: egl::Window,
    surface: VideoSurface,
    /// Kept open for as long as mpv decodes with it.
    _render_node: Option<OwnedFd>,
    /// What can be done about HDR here, and what is in effect.
    capabilities: Capabilities,
    tagging: Tagging,
}

/// How many exchanges with the compositor to wait, when a play starts, for
/// its description of the output and its answer to Vela's. Normally three:
/// the description is announced, its contents arrive, the answer arrives.
/// If they take longer the play starts in standard range and switches when
/// they arrive.
const COLOUR_EXCHANGES_AT_START: usize = 6;

/// Act on whatever the compositor has said about colour since the last call:
/// the one way, at the start of a play and during it, by which mpv and the
/// surface are told what the display can do (see `hdr`). Returns whether a
/// frame has to be drawn for it to be seen.
fn follow_colour(
    player: &Player,
    surface: &mut VideoSurface,
    capabilities: Capabilities,
    tagging: &mut Tagging,
) -> bool {
    let mut redraw = false;
    // The display can change under a running play: another monitor, HDR
    // switched off, the brightness slider (which moves the reference white
    // and so the peak).
    if surface.output_changed() && capabilities.hdr_possible() {
        surface.ask_about_output();
    }
    if let Some(output) = surface.output_described() {
        let step = tagging.output_described(hdr::presentation(Some(output), capabilities));
        redraw |= carry_out(player, surface, tagging, step);
    }
    if let Some(made) = surface.description_answer() {
        let step = if made {
            tagging.description_ready()
        } else {
            tagging.description_failed()
        };
        redraw |= carry_out(player, surface, tagging, step);
    }
    redraw
}

fn carry_out(
    player: &Player,
    surface: &mut VideoSurface,
    tagging: &mut Tagging,
    step: hdr::Step,
) -> bool {
    let change = hdr::carry_out(step, tagging, player, surface);
    match change {
        hdr::Change::Nothing => {}
        hdr::Change::Hdr(peak) => eprintln!("vela: embedded player: HDR output, peak {peak} nits"),
        hdr::Change::Sdr => eprintln!("vela: embedded player: standard-range output"),
        hdr::Change::HdrRefused => {
            eprintln!("vela: embedded player: HDR could not be set up; standard range")
        }
    }
    change.needs_a_frame()
}

fn private_options_file(text: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    let name = format!(
        "mpv-options-{}-{}.conf",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    let path = crate::player::launch::private_runtime_dir()?.join(name);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let written = options
        .open(&path)
        .and_then(|mut file| file.write_all(text.as_bytes()));
    if written.is_err() {
        let _ = std::fs::remove_file(&path);
        return Err("couldn't write the player's options".to_string());
    }
    Ok(path)
}

/// Hand the launch's options to a player that has not been started yet.
fn configure(player: &Player, start: &Start) -> Result<(), String> {
    for file in &start.own_config {
        if file.is_file() {
            // mpv reports no error for a line it cannot use, here as on its
            // own; the user's file is theirs to get right.
            let _ = player.set_option("include", &file.to_string_lossy());
        }
    }
    for step in &start.steps {
        match step {
            Step::Options(text) => {
                let file = private_options_file(text)?;
                let applied = player.set_option("include", &file.to_string_lossy());
                let _ = std::fs::remove_file(&file);
                applied.map_err(|_| "mpv did not accept the player's options".to_string())?;
            }
            Step::Include(path) => {
                // The stream-auth include is one of these. Its path and
                // mpv's answer stay out of every message.
                player
                    .set_option("include", path)
                    .map_err(|_| "mpv did not accept an included options file".to_string())?;
            }
        }
    }
    Ok(())
}

/// Build a play around `surface`, which the window's thread made for it
/// (`VideoSurface::create`). Nothing here touches Vela's window.
fn set_up(start: &Start, mut surface: VideoSurface, shared: &Arc<Shared>) -> Result<Live, String> {
    // The waiting the window's thread was spared: learn the surface's scale.
    surface.settle()?;
    let (width, height) = surface.buffer_size();
    // SAFETY: the connection the surface is on, which lives as long as the
    // process, and a surface that `Live` drops after the window that draws
    // into it.
    let window = unsafe {
        egl::Window::create(start.guest.display().0, surface.wl_surface(), width, height)
    }?;
    // Something to show from the first moment: black, not whatever the
    // compositor has behind the window.
    window.show_black();
    surface.flush();

    let capabilities = Capabilities {
        can_describe: surface.colour_support(),
        ten_bit_window: window.depth == 10,
    };
    let mut tagging = Tagging::new();

    let player = Player::create(start.lib.clone())?;
    configure(&player, start)?;
    player
        .initialize()
        .map_err(|reason| format!("mpv could not start ({reason})"))?;
    // mpv says "success" for an options file it could not read or parse.
    // These two are near the end of Vela's list and nothing works without
    // them, so reading them back shows the list arrived.
    if player.property("input-ipc-server").as_deref() != Some(start.ipc_path.as_str())
        || player.property("vo").as_deref() != Some("libmpv")
    {
        return Err("mpv did not take the player's options".to_string());
    }
    // What the display can do, settled before the first picture so that the
    // first picture is already right, and after the user's options and
    // Vela's own, so that in HDR this is what mpv draws for whatever either
    // of them said.
    if capabilities.hdr_possible() {
        surface.ask_about_output();
        for _ in 0..COLOUR_EXCHANGES_AT_START {
            surface.roundtrip()?;
            follow_colour(&player, &mut surface, capabilities, &mut tagging);
            if tagging.settled() {
                break;
            }
        }
    }

    let render_node = window.render_node();
    let render_fd = render_node.as_ref().map_or(-1, |fd| fd.as_raw_fd());
    // SAFETY: this thread's OpenGL context is current (the window above),
    // `egl::proc_address` resolves its functions, and the render node stays
    // open in `Live` for as long as the renderer exists.
    let renderer = unsafe { Renderer::create(&player, egl::proc_address, render_fd) }
        .map_err(|reason| format!("mpv could not draw into the window ({reason})"))?;
    let context = Arc::as_ptr(shared) as *mut c_void;
    // SAFETY: `shared` is held by this thread until after `Live`, and with
    // it the renderer and the player, has been dropped.
    unsafe {
        renderer.set_update(wake_from_mpv, context);
        player.set_wakeup(wake_from_mpv, context);
    }

    *shared.subsurface.lock().unwrap_or_else(|e| e.into_inner()) = Some(surface.subsurface());
    Ok(Live {
        renderer,
        player,
        window,
        surface,
        _render_node: render_node,
        capabilities,
        tagging,
    })
}

impl Live {
    /// Hand mpv its file.
    fn load(&self, url: &str) -> Result<(), String> {
        self.player
            .command(&["loadfile", url])
            .map_err(|reason| format!("mpv would not load the stream ({reason})"))
    }

    /// Draw frames and follow the rectangle until the player shuts down by
    /// itself or the play is stopped.
    fn run(&mut self, shared: &Shared) {
        loop {
            if self.surface.wait(shared.wake.as_fd(), 100).is_err() {
                // The connection to the compositor is gone; so is the window.
                return;
            }
            shared.drain_wake();
            if shared.stop.load(Ordering::SeqCst) {
                return;
            }
            let mut redraw = false;
            let resize_to = shared
                .resize_to
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if let Some((width, height)) = resize_to {
                if let Some((width, height)) = self.surface.resize(width, height) {
                    self.window.resize(width, height);
                    redraw = true;
                }
            }
            if let Some((width, height)) = self.surface.rescale() {
                self.window.resize(width, height);
                redraw = true;
            }
            redraw |= follow_colour(
                &self.player,
                &mut self.surface,
                self.capabilities,
                &mut self.tagging,
            );
            loop {
                match self.player.next_event() {
                    EVENT_NONE => break,
                    // The file ended, or mpv was told to quit over its socket.
                    EVENT_SHUTDOWN => return,
                    _ => {}
                }
            }
            if self.renderer.has_new_frame() || redraw {
                let (width, height) = self.surface.buffer_size();
                self.renderer.draw(width, height, self.window.depth);
                self.window.swap();
                self.renderer.frame_shown();
            }
        }
    }
}

/// A started embedded play, as the rest of the app holds it.
pub(crate) struct Session {
    /// The playback session this play belongs to. A player is told apart
    /// from the one that replaced it by this and nothing else.
    session_id: String,
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
    header_include: Option<HeaderInclude>,
    /// How long to wait for the thread when this is dropped, and where the
    /// thread goes if it has not ended by then.
    patience: Duration,
    leftovers: &'static Leftovers,
}

impl Session {
    /// Start a play on `surface` and wait until mpv has been given its file,
    /// or until something on the way there failed.
    pub(crate) fn start(mut start: Start, surface: VideoSurface) -> Result<Self, String> {
        let shared = Shared::new()?;
        let header_include = start.header_include.take();
        let session_id = std::mem::take(&mut start.session_id);
        let start_limit = start.start_limit;
        let (ready, started) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("vela-embedded-player".into())
            .spawn(move || {
                let shared = thread_shared;
                // The window's thread must not keep a handle to a surface
                // that is about to go; this takes it away first.
                let forget_surface = || {
                    shared
                        .subsurface
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                };
                match set_up(&start, surface, &shared) {
                    Err(reason) => {
                        forget_surface();
                        let _ = ready.send(Err(reason));
                    }
                    Ok(mut live) => {
                        if !start.load_delay.is_zero() {
                            std::thread::sleep(start.load_delay);
                        }
                        match finish_start(&shared.race, || live.load(&start.url)) {
                            Ok(Started::Yes) => {
                                let _ = ready.send(Ok(()));
                                (start.on_lifecycle)(Lifecycle::Opened);
                                live.run(&shared);
                                forget_surface();
                                drop(live);
                                (start.on_lifecycle)(Lifecycle::Closed);
                            }
                            // Nobody is waiting for this play any more, and
                            // nobody was told it opened: it goes unannounced.
                            Ok(Started::TooLate) => {
                                forget_surface();
                                drop(live);
                            }
                            Err(reason) => {
                                forget_surface();
                                drop(live);
                                let _ = ready.send(Err(reason));
                            }
                        }
                    }
                }
                shared.ended.store(true, Ordering::SeqCst);
            })
            .map_err(|e| format!("couldn't start the embedded player's thread: {e}"))?;
        let session = Session {
            session_id,
            shared,
            thread: Some(thread),
            header_include,
            patience: STOP_PATIENCE,
            leftovers: &LEFTOVERS,
        };
        // On every failure below the session is dropped, which stops the
        // play, waits for it, and keeps it if it will not end (`Drop`).
        let late = || Err("the embedded player did not start in time".to_string());
        match started.recv_timeout(start_limit) {
            Ok(Ok(())) => Ok(session),
            Ok(Err(reason)) => Err(reason),
            Err(_) if session.shared.race.give_up() => late(),
            // It started in the instant the wait ran out; its word that it
            // did is on its way.
            Err(_) => match started.recv_timeout(STOP_PATIENCE) {
                Ok(Ok(())) => Ok(session),
                Ok(Err(reason)) => Err(reason),
                Err(_) => late(),
            },
        }
    }

    /// A play that was never started, for tests of what holds one.
    #[cfg(test)]
    pub(crate) fn for_test(session_id: &str) -> Self {
        Self::for_test_holding(session_id, None)
    }

    /// The same, owning a stream-auth include.
    #[cfg(test)]
    pub(crate) fn for_test_holding(session_id: &str, header_include: Option<HeaderInclude>) -> Self {
        Session {
            session_id: session_id.to_string(),
            shared: Shared::new().expect("an eventfd"),
            thread: None,
            header_include,
            patience: STOP_PATIENCE,
            leftovers: &LEFTOVERS,
        }
    }

    /// A play whose thread runs `body`, for tests of what happens to a
    /// thread that does or does not end. `body` is given a way to ask
    /// whether the play has been told to stop.
    #[cfg(test)]
    fn for_test_running(
        patience: Duration,
        leftovers: &'static Leftovers,
        body: impl FnOnce(&dyn Fn() -> bool) + Send + 'static,
    ) -> Self {
        let shared = Shared::new().expect("an eventfd");
        let thread_shared = shared.clone();
        let thread = std::thread::spawn(move || {
            body(&|| thread_shared.stop.load(Ordering::SeqCst));
            thread_shared.ended.store(true, Ordering::SeqCst);
        });
        Session {
            session_id: "test".to_string(),
            shared,
            thread: Some(thread),
            header_include: None,
            patience,
            leftovers,
        }
    }

    /// Whether this is the play of that playback session.
    pub(crate) fn is_play(&self, session_id: &str) -> bool {
        self.session_id == session_id
    }

    /// Ask the play to stop. Does not wait.
    pub(crate) fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.wake();
    }

    #[cfg(test)]
    pub(crate) fn stop_requested(&self) -> bool {
        self.shared.stop.load(Ordering::SeqCst)
    }

    /// Whether the play has ended and released everything it held.
    pub(crate) fn has_ended(&mut self) -> bool {
        if !self.shared.ended.load(Ordering::SeqCst) {
            return false;
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        true
    }

    /// Stop the play and wait for it to release the window's surface, for at
    /// most `limit`. Used where the window itself is about to go away: the
    /// play draws into a child of that window and must be gone first.
    pub(crate) fn stop_and_wait(&mut self, limit: Duration) -> bool {
        self.stop();
        let deadline = std::time::Instant::now() + limit;
        while !self.has_ended() {
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    /// Move and size the video. Call on the window's own thread, and have
    /// the window redraw afterwards: the position takes effect with the
    /// window's next redraw, the size with the next video frame.
    pub(crate) fn place(&self, rect: Rect) {
        if let Some(subsurface) = self
            .shared
            .subsurface
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            subsurface.set_position(rect.x, rect.y);
        }
        *self.shared.resize_to.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((rect.width, rect.height));
        self.shared.wake();
    }

    /// mpv read the stream-auth include when it was configured. Remove it
    /// now rather than when the play is reaped. A failed removal leaves the
    /// include with this play, to be tried again when the play is reaped.
    pub(crate) fn remove_consumed_header_include(&mut self) -> Result<(), String> {
        HeaderInclude::remove_consumed(&mut self.header_include)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // A play is normally reaped after it has ended. If it is dropped
        // while running, it must still let go of the window: it is told to
        // stop and waited for. One that has not ended by then is not let go
        // of either; its thread is kept (`Leftovers`).
        if self.thread.is_some() && !self.stop_and_wait(self.patience) {
            if let Some(thread) = self.thread.take() {
                eprintln!("vela: an embedded play did not stop in time and is kept until it does");
                self.leftovers.keep(self.shared.clone(), thread);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::launch::write_header_include_at;

    // The embedded play must hold on to a stream-auth file it could not
    // remove, as the external player's handle does, so that reaping the play
    // removes it (review finding ec-7: it used to let go of the file on the
    // first failure, and nothing then knew the file existed).
    #[test]
    fn a_play_keeps_a_stream_auth_file_it_could_not_remove_and_removes_it_when_reaped() {
        use std::os::unix::fs::PermissionsExt;
        // SAFETY: takes nothing and cannot fail.
        if unsafe { libc::geteuid() } == 0 {
            // Root removes from a read-only directory; nothing to show.
            return;
        }
        let dir = std::env::temp_dir().join(format!(
            "vela-test-{}-embedded-include-kept",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lock = |mode: u32| {
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap()
        };
        let path = dir.join("headers.conf");
        let headers = vec![("X-Emby-Token".to_string(), "synthetic".to_string())];
        let include = write_header_include_at(&path, &headers).unwrap();
        let mut play = Session::for_test_holding("play", Some(include));

        // The removal before a replacement launch fails...
        lock(0o500);
        assert!(play.remove_consumed_header_include().is_err());
        lock(0o700);
        assert!(path.exists());
        // ...and the play is reaped later, when it can be done.
        drop(play);
        assert!(
            !path.exists(),
            "the play still owned the file and removed it when it was reaped"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- a start that outlasts its caller (review finding ec-2)

    #[test]
    fn a_start_is_settled_once_by_whoever_gets_there_first() {
        // The play starts; the caller's wait runs out a moment later.
        let race = StartRace::new();
        assert!(!race.given_up());
        assert!(race.claim_started());
        assert!(!race.give_up(), "a started play is not given up on");
        assert!(!race.given_up());
        // The caller stops waiting; the play comes back later.
        let race = StartRace::new();
        assert!(race.give_up());
        assert!(race.given_up());
        assert!(
            !race.claim_started(),
            "a play given up on never counts as started"
        );
        assert!(race.given_up());
    }

    #[test]
    fn of_a_play_and_its_caller_racing_exactly_one_wins() {
        for _ in 0..300 {
            let race = Arc::new(StartRace::new());
            let together = Arc::new(std::sync::Barrier::new(2));
            let play = std::thread::spawn({
                let (race, together) = (race.clone(), together.clone());
                move || {
                    together.wait();
                    race.claim_started()
                }
            });
            let caller = std::thread::spawn({
                let (race, together) = (race.clone(), together.clone());
                move || {
                    together.wait();
                    race.give_up()
                }
            });
            let (started, gave_up) = (play.join().unwrap(), caller.join().unwrap());
            assert_ne!(started, gave_up, "one of them, never both and never neither");
            assert_eq!(race.given_up(), gave_up);
        }
    }

    #[test]
    fn a_play_that_is_built_in_time_is_loaded_and_started() {
        let race = StartRace::new();
        let mut loaded = false;
        let finished = finish_start(&race, || {
            loaded = true;
            Ok(())
        });
        assert_eq!(finished, Ok(Started::Yes));
        assert!(loaded);
        assert!(!race.give_up(), "the caller's wait running out now changes nothing");
    }

    #[test]
    fn a_play_its_caller_gave_up_on_is_not_loaded() {
        let race = StartRace::new();
        assert!(race.give_up());
        let mut loaded = false;
        let finished = finish_start(&race, || {
            loaded = true;
            Ok(())
        });
        assert_eq!(finished, Ok(Started::TooLate));
        assert!(
            !loaded,
            "mpv is not handed the file of a play already reported as failed"
        );
    }

    #[test]
    fn a_play_given_up_on_while_it_loads_does_not_count_as_started() {
        let race = StartRace::new();
        let finished = finish_start(&race, || {
            // The caller's wait runs out in the middle of the load.
            assert!(race.give_up());
            Ok(())
        });
        assert_eq!(finished, Ok(Started::TooLate));
    }

    #[test]
    fn a_play_whose_file_mpv_refuses_says_so_and_claims_nothing() {
        let race = StartRace::new();
        assert_eq!(
            finish_start(&race, || Err("refused".to_string())),
            Err("refused".to_string())
        );
        assert!(race.give_up(), "the start is still the caller's to end");
    }

    /// A list of leftovers of a test's own, so tests cannot see each other's.
    fn leftovers() -> &'static Leftovers {
        Box::leak(Box::new(Leftovers::new()))
    }

    #[test]
    fn a_play_that_will_not_stop_is_kept_not_let_go_of() {
        let kept = leftovers();
        let release = Arc::new(AtomicBool::new(false));
        let stuck = release.clone();
        // A play stuck somewhere that does not look at the request to stop.
        // (It frees itself after ten seconds whatever happens, so that a
        // mistake here that waits for it fails this test instead of hanging
        // the whole run.)
        let play = Session::for_test_running(Duration::from_millis(50), kept, move |_stop| {
            let began = Instant::now();
            while !stuck.load(Ordering::SeqCst) && began.elapsed() < Duration::from_secs(10) {
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let began = Instant::now();
        drop(play);
        assert!(
            began.elapsed() < Duration::from_secs(2),
            "dropping it waits its patience, not for ever"
        );
        assert_eq!(kept.still_running(), 1, "its thread is still known");
        assert!(!kept.wait(Duration::from_millis(50)), "and still running");
        assert!(
            began.elapsed() < Duration::from_secs(2),
            "asking about it does not wait for it"
        );
        // It comes unstuck.
        release.store(true, Ordering::SeqCst);
        assert!(kept.wait(Duration::from_secs(5)));
        assert_eq!(kept.still_running(), 0, "and is let go of once it has ended");
    }

    #[test]
    fn a_play_that_stops_when_asked_is_not_kept() {
        let kept = leftovers();
        let play = Session::for_test_running(Duration::from_secs(5), kept, |stop| {
            while !stop() {
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let began = Instant::now();
        drop(play);
        assert!(began.elapsed() < Duration::from_secs(2));
        assert_eq!(kept.still_running(), 0);
    }

    #[test]
    fn a_marker_payload_goes_when_what_holds_it_goes() {
        let path = std::env::temp_dir().join(format!(
            "vela-test-{}-embedded-marker-payload.json",
            std::process::id()
        ));
        std::fs::write(&path, b"[]").unwrap();
        let held = Some(MarkerPayload::new(path.clone()));
        assert!(path.exists(), "holding it does not touch it");
        drop(held);
        assert!(!path.exists());
        // The script got there first: nothing to remove, and no complaint.
        drop(MarkerPayload::new(path.clone()));
        assert!(!path.exists());
    }

    #[test]
    fn a_play_removes_a_consumed_stream_auth_file_at_once_when_it_can() {
        let path = std::env::temp_dir().join(format!(
            "vela-test-{}-embedded-include-removed.conf",
            std::process::id()
        ));
        let headers = vec![("X-Emby-Token".to_string(), "synthetic".to_string())];
        let include = write_header_include_at(&path, &headers).unwrap();
        let mut play = Session::for_test_holding("play", Some(include));
        assert!(path.exists());
        assert!(play.remove_consumed_header_include().is_ok());
        assert!(!path.exists(), "gone before the replacement launch writes its own");
        // A play with none has nothing to remove.
        assert!(Session::for_test("other").remove_consumed_header_include().is_ok());
    }
}
