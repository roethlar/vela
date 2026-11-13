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
    let surface = unsafe {
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
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
    header_include: Option<HeaderInclude>,
}

impl Session {
    /// Start a play and wait until mpv has been given its file, or until
    /// something on the way there failed.
    pub(crate) fn start(mut start: Start) -> Result<Self, String> {
        // SAFETY: plain flags; a negative result is an error.
        let wake = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if wake < 0 {
            return Err("couldn't create the player's wake-up channel".to_string());
        }
        let shared = Arc::new(Shared {
            // SAFETY: a descriptor just created and owned by nothing else.
            wake: unsafe { OwnedFd::from_raw_fd(wake) },
            stop: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            resize_to: Mutex::new(None),
            subsurface: Mutex::new(None),
        });
        let header_include = start.header_include.take();
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

    /// Ask the play to stop. Does not wait.
    pub(crate) fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.wake();
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
    /// now rather than when the play is reaped.
    pub(crate) fn remove_consumed_header_include(&mut self) -> Result<(), String> {
        if let Some(mut include) = self.header_include.take() {
            include
                .remove_now()
                .map_err(|_| "couldn't remove the previous mpv auth config".to_string())?;
        }
        Ok(())
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
