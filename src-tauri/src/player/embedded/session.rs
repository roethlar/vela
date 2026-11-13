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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wayland_client::protocol::wl_subsurface::WlSubsurface;

use super::egl;
use super::hdr::{self, Capabilities, Tagging};
use super::libmpv::{LibMpv, Player, Renderer, EVENT_NONE, EVENT_SHUTDOWN};
use super::options::Step;
use super::surface::VideoSurface;
use crate::player::launch::HeaderInclude;

/// The window's Wayland connection and surface, as raw pointers GTK owns.
#[derive(Clone, Copy)]
pub(crate) struct WindowHandles {
    pub(crate) wl_display: *mut c_void,
    pub(crate) wl_surface: *mut c_void,
}

// The pointers are used through libwayland, which is thread-safe, and only
// while the window they belong to exists (see `Session::stop_and_wait`).
unsafe impl Send for WindowHandles {}

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

/// Everything a play needs to start.
pub(crate) struct Start {
    pub(crate) lib: Arc<LibMpv>,
    /// The playback session this play belongs to.
    pub(crate) session_id: String,
    pub(crate) window: WindowHandles,
    pub(crate) rect: Rect,
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
    pub(crate) header_include: Option<HeaderInclude>,
    pub(crate) on_lifecycle: Arc<dyn Fn(Lifecycle) + Send + Sync>,
}

struct Shared {
    /// Written to wake the thread: by mpv (a frame or an event is waiting)
    /// and by Vela (stop, or the rectangle changed).
    wake: OwnedFd,
    stop: AtomicBool,
    ended: AtomicBool,
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

fn set_up(start: &Start, shared: &Arc<Shared>) -> Result<Live, String> {
    // SAFETY: `start.window` holds the live display and surface of Vela's
    // window, which outlives this play (`Session::stop_and_wait`).
    let mut surface = unsafe {
        VideoSurface::create(
            start.window.wl_display,
            start.window.wl_surface,
            start.rect.width,
            start.rect.height,
        )
    }?;
    let (width, height) = surface.buffer_size();
    // SAFETY: the same display, and a surface that `Live` drops after the
    // window that draws into it.
    let window =
        unsafe { egl::Window::create(start.window.wl_display, surface.wl_surface(), width, height) }?;
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
    if !start.load_delay.is_zero() {
        std::thread::sleep(start.load_delay);
    }
    player
        .command(&["loadfile", &start.url])
        .map_err(|reason| format!("mpv would not load the stream ({reason})"))?;
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
}

impl Session {
    /// Start a play and wait until mpv has been given its file, or until
    /// something on the way there failed.
    pub(crate) fn start(mut start: Start) -> Result<Self, String> {
        let shared = Shared::new()?;
        let header_include = start.header_include.take();
        let session_id = std::mem::take(&mut start.session_id);
        let (ready, started) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("vela-embedded-player".into())
            .spawn(move || {
                let shared = thread_shared;
                match set_up(&start, &shared) {
                    Err(reason) => {
                        // Whatever was built on the way has been dropped
                        // again; the window's thread must not keep a handle
                        // to a surface that is gone.
                        shared
                            .subsurface
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .take();
                        let _ = ready.send(Err(reason));
                    }
                    Ok(mut live) => {
                        let _ = ready.send(Ok(()));
                        (start.on_lifecycle)(Lifecycle::Opened);
                        live.run(&shared);
                        // The window's thread must not use the surface after
                        // this point; take its handle away first.
                        shared
                            .subsurface
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .take();
                        drop(live);
                        (start.on_lifecycle)(Lifecycle::Closed);
                    }
                }
                shared.ended.store(true, Ordering::SeqCst);
            })
            .map_err(|e| format!("couldn't start the embedded player's thread: {e}"))?;
        let mut session = Session {
            session_id,
            shared,
            thread: Some(thread),
            header_include,
        };
        match started.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(())) => Ok(session),
            Ok(Err(reason)) => {
                session.stop_and_wait(Duration::from_secs(5));
                Err(reason)
            }
            Err(_) => {
                session.stop_and_wait(Duration::from_secs(5));
                Err("the embedded player did not start in time".to_string())
            }
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
        // while running, it must still let go of the window.
        if self.thread.is_some() {
            self.stop_and_wait(Duration::from_secs(5));
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
