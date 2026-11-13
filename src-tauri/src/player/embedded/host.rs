//! Where the embedded player meets Vela's window.
//!
//! Three things live here: whether an embedded play can run at all
//! ([`Host::availability`]), starting one ([`Host::launch`]), and telling a
//! running one where the page has put the video ([`Host::place`]).
//!
//! GTK may only be used from the thread that runs the window. A play is
//! started from a worker thread, so everything that asks the window a
//! question does it through [`on_window_thread`].

use std::ffi::{c_void, CStr};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use gtk::prelude::*;
use tauri::{Emitter, Manager};

use super::libmpv::{self, LibMpv};
use super::options;
use super::session::{self, Lifecycle, MarkerPayload, Rect, Session, Start};
use super::startup::{self, WindowMode};
use super::egl;
use super::surface::{Guest, VideoSurface, WlDisplay};
use crate::player::launch::LaunchPlan;
use crate::player::EmbeddedUnavailable;

/// Environment variable naming a directory that holds a shipped library set,
/// for development builds only (see [`libmpv::set_directory`]).
const DEVELOPMENT_SET_VARIABLE: &str = "VELA_LIBMPV_DIR";

/// Environment variable, read by development builds only, that makes an
/// embedded play wait this many milliseconds before giving mpv its file. The
/// E2E suite uses it to show that the player survives the wait.
const DEVELOPMENT_LOAD_DELAY_VARIABLE: &str = "VELA_EMBEDDED_LOAD_DELAY_MS";

/// Environment variable, read by development builds only, that shortens how
/// long the caller waits for an embedded play to start. With the load delay
/// above, the E2E suite uses it to stage a play that outlasts its caller's
/// patience, which otherwise takes a machine in trouble.
const DEVELOPMENT_START_LIMIT_VARIABLE: &str = "VELA_EMBEDDED_START_LIMIT_MS";

/// How long to wait for a play to start: the usual limit, or less where a
/// development build was asked for less. Never more, and never anything but
/// the usual limit in a release build.
pub(crate) fn start_limit(development_build: bool, asked: Option<&std::ffi::OsStr>) -> Duration {
    let usual = session::START_LIMIT;
    if !development_build {
        return usual;
    }
    asked
        .and_then(|value| value.to_str())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|milliseconds| Duration::from_millis(milliseconds).min(usual))
        .unwrap_or(usual)
}

/// The wait a development build was asked for; none in a release build,
/// whatever the environment says, and never more than ten seconds.
pub(crate) fn load_delay(development_build: bool, asked: Option<&std::ffi::OsStr>) -> Duration {
    if !development_build {
        return Duration::ZERO;
    }
    asked
        .and_then(|value| value.to_str())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|milliseconds| Duration::from_millis(milliseconds.min(10_000)))
        .unwrap_or(Duration::ZERO)
}

/// Run `ask` on the window's thread and wait for its answer.
fn on_window_thread<T: Send + 'static>(
    app: &tauri::AppHandle,
    ask: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (answer, answered) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = answer.send(ask());
    })
    .map_err(|_| "Vela's window is not running".to_string())?;
    answered
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| "Vela's window did not answer".to_string())
}

/// A function GDK's Wayland backend exports. Looked up at run time: a GTK
/// built without that backend has no such function, and Vela must still
/// start there.
fn gdk_wayland_function(name: &CStr) -> Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void> {
    // SAFETY: a valid C string; RTLD_DEFAULT searches what is already loaded.
    let address = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    // SAFETY: both functions looked up here take one object pointer and
    // return one pointer.
    (!address.is_null()).then(|| unsafe { super::function_at(address) })
}

/// What the window's thread knows that a play needs.
///
/// `surface` is the window's own Wayland surface, a raw pointer into GTK
/// that is good only while the window lives, and the window can be closed at
/// any moment, on this thread. So it is used on this thread, at once, and
/// never carried away: this type holds a raw pointer and so cannot be sent
/// to another thread, and the line below it makes sure that stays true.
struct WindowFacts {
    display: WlDisplay,
    surface: *mut c_void,
    /// The web view's place in the window, in the window's own units.
    page: Rect,
}

// Fails to compile if `WindowFacts` can be sent to another thread: the two
// implementations below would then both apply and the choice between them
// would be ambiguous.
const _: fn() = || {
    trait AmbiguousIfSend<Marker> {
        fn check() {}
    }
    impl<T: ?Sized> AmbiguousIfSend<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
    let _ = <WindowFacts as AmbiguousIfSend<_>>::check;
};

/// Find the web view inside the window. Tauri puts it in a box under the
/// window; this walks down rather than assuming how deep.
fn find_web_view(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.type_().name() == "WebKitWebView" {
        return Some(widget.clone());
    }
    let container = widget.downcast_ref::<gtk::Container>()?;
    container.children().iter().find_map(find_web_view)
}

/// Must run on the window's thread.
fn window_facts(app: &tauri::AppHandle) -> Result<WindowFacts, EmbeddedUnavailable> {
    use gtk::glib::translate::ToGlibPtr;
    let not_ready = EmbeddedUnavailable::WindowNotReady;
    let window = app
        .webview_windows()
        .into_values()
        .next()
        .ok_or(not_ready)?;
    let gtk_window = window.gtk_window().map_err(|_| not_ready)?;
    let gdk_window = gtk_window.window().ok_or(not_ready)?;
    let display = gdk_window.display();
    if display.type_().name() != "GdkWaylandDisplay" {
        return Err(EmbeddedUnavailable::NotWayland);
    }
    let surface_of = gdk_wayland_function(c"gdk_wayland_window_get_wl_surface")
        .ok_or(EmbeddedUnavailable::NotWayland)?;
    let display_of = gdk_wayland_function(c"gdk_wayland_display_get_wl_display")
        .ok_or(EmbeddedUnavailable::NotWayland)?;
    let gdk_window_ptr: *mut gtk::gdk::ffi::GdkWindow = gdk_window.to_glib_none().0;
    let display_ptr: *mut gtk::gdk::ffi::GdkDisplay = display.to_glib_none().0;
    // SAFETY: live GDK objects of the Wayland backend, on their own thread.
    let (wl_surface, wl_display) = unsafe {
        (
            surface_of(gdk_window_ptr.cast()),
            display_of(display_ptr.cast()),
        )
    };
    if wl_surface.is_null() || wl_display.is_null() {
        return Err(not_ready);
    }
    let whole_window: gtk::Widget = gtk_window.clone().upcast();
    let page_widget = find_web_view(&whole_window).unwrap_or_else(|| whole_window.clone());
    let (x, y) = page_widget
        .translate_coordinates(&gtk_window, 0, 0)
        .unwrap_or((0, 0));
    let size = page_widget.allocation();
    Ok(WindowFacts {
        display: WlDisplay(wl_display),
        surface: wl_surface,
        page: Rect {
            x,
            y,
            width: size.width().max(1),
            height: size.height().max(1),
        },
    })
}

/// Ask the window to draw itself again. A subsurface's place in the window
/// takes effect when the window next commits, which is when it next draws.
fn redraw_window(app: &tauri::AppHandle) {
    if let Some(window) = app.webview_windows().into_values().next() {
        if let Ok(gtk_window) = window.gtk_window() {
            gtk_window.queue_draw();
        }
    }
}

/// A rectangle the page reported, in CSS pixels relative to the page, turned
/// into the window's own units relative to the window.
///
/// The page's CSS pixel is the window's unit as long as the page is not
/// zoomed, and Vela never zooms it. A rectangle that pokes out of the page is
/// cut to it: the video must not cover the window's own frame.
pub(crate) fn rect_in_window(page: Rect, reported: Rect) -> Rect {
    let left = reported.x.clamp(0, page.width);
    let top = reported.y.clamp(0, page.height);
    let right = reported.x.saturating_add(reported.width).clamp(left, page.width);
    let bottom = reported.y.saturating_add(reported.height).clamp(top, page.height);
    Rect {
        x: page.x + left,
        y: page.y + top,
        width: (right - left).max(1),
        height: (bottom - top).max(1),
    }
}

/// What every embedded play in this run of Vela shares, made once.
#[derive(Clone)]
struct Ready {
    lib: Arc<LibMpv>,
    /// Vela's place on the window's connection to the compositor. Joined
    /// here, once: see `surface.rs` for why never per play.
    guest: Arc<Guest>,
}

/// The whole page, as a rectangle relative to the page.
fn whole(page: Rect) -> Rect {
    Rect { x: 0, y: 0, ..page }
}

/// The embedded player's side of the app, for the life of the process.
pub(crate) struct Host {
    /// Whether this machine can run an embedded play, found out once, and
    /// with it what its plays share.
    checked: OnceLock<Result<Ready, EmbeddedUnavailable>>,
    /// Set when an embedded play failed to start, so the next play goes to
    /// the external player instead of failing the same way.
    start_failed: std::sync::atomic::AtomicBool,
    /// The rectangle the page last reported, relative to the page.
    reported: Mutex<Option<Rect>>,
}

impl Host {
    pub(crate) fn new() -> Self {
        Host {
            checked: OnceLock::new(),
            start_failed: std::sync::atomic::AtomicBool::new(false),
            reported: Mutex::new(None),
        }
    }

    /// Everything that has to be true of this machine, checked in order of
    /// cost and stopping at the first that is not.
    fn check(app: &tauri::AppHandle) -> Result<Ready, EmbeddedUnavailable> {
        let display = on_window_thread(app, {
            let app = app.clone();
            move || window_facts(&app).map(|facts| facts.display)
        })
        .map_err(|_| EmbeddedUnavailable::WindowNotReady)??;
        // SAFETY: the window's live Wayland connection, which GTK keeps for
        // the life of the process.
        let guest = unsafe { Guest::join(display) }.map_err(|detail| {
            eprintln!("vela: the embedded player is unavailable: {detail}");
            EmbeddedUnavailable::CompositorLacks
        })?;
        egl::probe(display.0).map_err(|detail| {
            eprintln!("vela: the embedded player is unavailable: {detail}");
            EmbeddedUnavailable::NoGpuFormat
        })?;
        let resources = app.path().resource_dir().map_err(|_| {
            eprintln!("vela: the embedded player is unavailable: Vela's resources were not found");
            EmbeddedUnavailable::LibraryMissing
        })?;
        let directory = libmpv::set_directory(
            &resources,
            cfg!(debug_assertions),
            std::env::var_os(DEVELOPMENT_SET_VARIABLE).as_deref(),
        );
        let lib = LibMpv::load(&directory.join(libmpv::LIBRARY_FILE))
            .map(Arc::new)
            .map_err(|detail| {
                // The loader's own words, so a damaged or too-old system can
                // be told apart from a missing file. A path, never a secret.
                eprintln!(
                    "vela: the embedded player is unavailable: its mpv library could not be loaded: {detail}"
                );
                EmbeddedUnavailable::LibraryMissing
            })?;
        Ok(Ready {
            lib,
            guest: Arc::new(guest),
        })
    }

    fn library(&self, app: &tauri::AppHandle) -> Result<Ready, EmbeddedUnavailable> {
        let decided = startup::decision();
        if decided.mode != WindowMode::Embedded {
            return Err(decided.refused.unwrap_or(EmbeddedUnavailable::NeedsRestart));
        }
        if self.start_failed.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(EmbeddedUnavailable::StartFailed);
        }
        // A play Vela stopped waiting for and that has still not ended holds
        // its surface and its player. No other is started next to it.
        if session::unfinished_plays() > 0 {
            return Err(EmbeddedUnavailable::StartFailed);
        }
        self.checked.get_or_init(|| Self::check(app)).clone()
    }

    /// Whether an embedded play can be started now. Asks the window, so it
    /// must not be called on the window's own thread.
    pub(crate) fn availability(&self, app: &tauri::AppHandle) -> Result<(), EmbeddedUnavailable> {
        self.library(app).map(|_| ())
    }

    /// The first half of starting a play: everything that asks the window,
    /// done before the caller locks the player slot (the window's thread
    /// takes that lock too). Must not be called on the window's thread.
    pub(crate) fn prepare(
        self: &Arc<Self>,
        app: &tauri::AppHandle,
        session_id: &str,
    ) -> Result<Prepared, String> {
        let Ready { lib, guest } = self.library(app).map_err(|reason| reason.notice())?;
        let reported = *self.reported.lock().unwrap_or_else(|e| e.into_inner());
        // The play's surface is made here, on the window's thread, where the
        // window it is made under cannot be closed in the middle (review
        // finding ec-1). Only the finished surface leaves that thread.
        let surface = on_window_thread(app, {
            let app = app.clone();
            let guest = guest.clone();
            move || {
                let facts = window_facts(&app)?;
                let rect = rect_in_window(facts.page, reported.unwrap_or(whole(facts.page)));
                // SAFETY: on the window's own thread, with the surface of a
                // window that was alive a moment ago on this same thread, on
                // the connection `guest` joined.
                unsafe { VideoSurface::create(&guest, facts.surface, rect.width, rect.height) }
                    .map_err(|detail| {
                        eprintln!("vela: the embedded player could not make its surface: {detail}");
                        EmbeddedUnavailable::WindowNotReady
                    })
            }
        })?
        .map_err(|reason| reason.notice())?;
        Ok(Prepared {
            host: self.clone(),
            app: app.clone(),
            lib,
            guest,
            surface,
            session_id: session_id.to_string(),
        })
    }

    /// What the page last reported, or the whole page until it has: the
    /// player view covers the page.
    fn reported_or_whole(&self, page: Rect) -> Rect {
        self.reported
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unwrap_or(whole(page))
    }

    /// The page says where the video is. Must run on the window's thread.
    /// `session` is the running embedded play, when there is one.
    pub(crate) fn place(&self, app: &tauri::AppHandle, reported: Rect, session: Option<&Session>) {
        *self.reported.lock().unwrap_or_else(|e| e.into_inner()) = Some(reported);
        self.place_again(app, session);
    }

    /// Put the running play where the page last said. Must run on the
    /// window's thread.
    fn place_again(&self, app: &tauri::AppHandle, session: Option<&Session>) {
        let (Some(session), Ok(facts)) = (session, window_facts(app)) else {
            return;
        };
        session.place(rect_in_window(facts.page, self.reported_or_whole(facts.page)));
        redraw_window(app);
    }
}

/// A play that has everything it needs from the window and has not started.
pub(crate) struct Prepared {
    host: Arc<Host>,
    app: tauri::AppHandle,
    lib: Arc<LibMpv>,
    guest: Arc<Guest>,
    /// The play's surface, already under the window.
    surface: VideoSurface,
    session_id: String,
}

impl Prepared {
    /// Start the described launch. Asks the window nothing.
    pub(crate) fn start(
        self,
        plan: LaunchPlan,
        own_config: bool,
        ipc_path: &str,
    ) -> Result<Session, String> {
        let Prepared {
            host,
            app,
            lib,
            guest,
            surface,
            session_id,
        } = self;
        let on_lifecycle = {
            let app = app.clone();
            let session_id = session_id.clone();
            Arc::new(move |event: Lifecycle| {
                let name = match event {
                    Lifecycle::Opened => "player-opened",
                    Lifecycle::Closed => "player-closed",
                };
                // An id and nothing else: never a URL, a title or a path.
                let _ = app.emit(name, serde_json::json!({ "sessionId": session_id }));
                // Appearing and disappearing both take effect with the
                // window's next draw, and a play that has just appeared is
                // put where the page last said.
                let on_window = app.clone();
                let _ = app.run_on_main_thread(move || {
                    if event == Lifecycle::Opened {
                        place_running_play(&on_window);
                    }
                    redraw_window(&on_window);
                });
            }) as Arc<dyn Fn(Lifecycle) + Send + Sync>
        };
        // Every part of the plan is named: a part added later has to be
        // given an owner here before this compiles. The environment is for a
        // player that is a process of its own.
        let LaunchPlan {
            args,
            env: _,
            url,
            header_include,
            marker_payload,
        } = plan;
        let start = Start {
            lib,
            guest,
            session_id,
            own_config: if own_config {
                options::user_config_files(
                    std::env::var_os("MPV_HOME").as_deref(),
                    std::env::var_os("XDG_CONFIG_HOME").as_deref(),
                    std::env::var_os("HOME").as_deref(),
                )
            } else {
                Vec::new()
            },
            steps: options::steps(&args),
            ipc_path: ipc_path.to_string(),
            url,
            load_delay: load_delay(
                cfg!(debug_assertions),
                std::env::var_os(DEVELOPMENT_LOAD_DELAY_VARIABLE).as_deref(),
            ),
            start_limit: start_limit(
                cfg!(debug_assertions),
                std::env::var_os(DEVELOPMENT_START_LIMIT_VARIABLE).as_deref(),
            ),
            header_include,
            _marker_payload: marker_payload.map(MarkerPayload::new),
            on_lifecycle,
        };
        let started = Session::start(start, surface);
        started.map_err(|reason| {
            // One failed start is enough: the next play uses the external
            // player and says why, instead of failing the same way.
            host.start_failed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            eprintln!("vela: the embedded player could not start: {reason}");
            format!("The embedded player could not start: {reason}")
        })
    }
}

/// Find the running embedded play, if there is one, and put it where the
/// page last said. Must run on the window's thread.
fn place_running_play(app: &tauri::AppHandle) {
    let state = app.state::<crate::AppState>();
    let slot = state.current_child.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(crate::player::PlayerHandle::Embedded(session)) = slot.as_ref() {
        state.embedded.place_again(app, Some(session));
    }
}

/// What `playback::play` is given to start embedded plays with.
pub(crate) struct Launcher {
    host: Arc<Host>,
    app: tauri::AppHandle,
}

impl Launcher {
    pub(crate) fn new(host: Arc<Host>, app: tauri::AppHandle) -> Self {
        Launcher { host, app }
    }
}

impl crate::player::EmbeddedLauncher for Launcher {
    fn prepare(&self, session_id: &str) -> Result<crate::player::EmbeddedStart, String> {
        let prepared = self.host.prepare(&self.app, session_id)?;
        Ok(Box::new(move |plan, own_config, ipc_path: &str| {
            prepared
                .start(plan, own_config, ipc_path)
                .map(crate::player::PlayerHandle::Embedded)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: Rect = Rect {
        x: 0,
        y: 37,
        width: 1280,
        height: 763,
    };

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn only_a_development_build_waits_before_loading() {
        use std::ffi::OsStr;
        let asked = |text: &'static str| Some(OsStr::new(text));
        assert_eq!(load_delay(true, asked("2500")), Duration::from_millis(2500));
        assert_eq!(load_delay(true, asked(" 40 ")), Duration::from_millis(40));
        assert_eq!(load_delay(true, None), Duration::ZERO);
        assert_eq!(load_delay(true, asked("soon")), Duration::ZERO);
        assert_eq!(load_delay(true, asked("-5")), Duration::ZERO);
        assert_eq!(
            load_delay(true, asked("999999999")),
            Duration::from_secs(10),
            "a typo cannot hang a play for hours"
        );
        // A released Vela never waits, whatever its environment says.
        assert_eq!(load_delay(false, asked("2500")), Duration::ZERO);
    }

    #[test]
    fn only_a_development_build_can_shorten_the_wait_for_a_start() {
        use std::ffi::OsStr;
        let asked = |text: &'static str| Some(OsStr::new(text));
        let usual = session::START_LIMIT;
        assert_eq!(usual, Duration::from_secs(20));
        assert_eq!(start_limit(true, asked("500")), Duration::from_millis(500));
        assert_eq!(start_limit(true, None), usual);
        assert_eq!(start_limit(true, asked("soon")), usual);
        assert_eq!(start_limit(true, asked("999999999")), usual, "never longer");
        // A released Vela waits the usual time whatever its environment says.
        assert_eq!(start_limit(false, asked("500")), usual);
        assert_eq!(start_limit(false, asked("0")), usual);
    }

    #[test]
    fn a_page_rectangle_is_moved_by_where_the_page_sits_in_the_window() {
        // The whole page.
        assert_eq!(
            rect_in_window(PAGE, rect(0, 0, 1280, 763)),
            rect(0, 37, 1280, 763)
        );
        // A rectangle inside it.
        assert_eq!(
            rect_in_window(PAGE, rect(300, 20, 900, 500)),
            rect(300, 57, 900, 500)
        );
    }

    #[test]
    fn a_rectangle_that_leaves_the_page_is_cut_to_it() {
        assert_eq!(
            rect_in_window(PAGE, rect(-50, -20, 400, 300)),
            rect(0, 37, 350, 280),
            "cut at the top left"
        );
        assert_eq!(
            rect_in_window(PAGE, rect(1000, 600, 900, 900)),
            rect(1000, 637, 280, 163),
            "cut at the bottom right"
        );
        assert_eq!(
            rect_in_window(PAGE, rect(0, 0, 5000, 5000)),
            rect(0, 37, 1280, 763),
            "never larger than the page"
        );
    }

    #[test]
    fn a_rectangle_is_never_empty_or_inside_out() {
        // A page that has not laid out, a negative size, numbers at the edge
        // of what fits: the surface always gets a real size.
        assert_eq!(rect_in_window(PAGE, rect(10, 10, 0, 0)), rect(10, 47, 1, 1));
        assert_eq!(rect_in_window(PAGE, rect(10, 10, -5, -5)), rect(10, 47, 1, 1));
        assert_eq!(
            rect_in_window(PAGE, rect(9000, 9000, 10, 10)),
            rect(1280, 800, 1, 1)
        );
        assert_eq!(
            rect_in_window(PAGE, rect(i32::MAX, i32::MAX, i32::MAX, i32::MAX)),
            rect(1280, 800, 1, 1)
        );
    }
}
