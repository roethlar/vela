//! Vela's window under X11 ("overlay mode", `startup.rs`).
//!
//! The Wayland host (`surface.rs`, `egl.rs`) shows the video on a surface
//! under Vela's window, which needs a Wayland compositor. X11 has nothing
//! like that surface: a child window cannot be seen through a transparent
//! page, and the LGPL mpv Vela ships has no X11 window code of its own. So
//! under X11 GTK composites the video itself: mpv's render API draws into a
//! GL area (`GtkGLArea`), and the page, made transparent, lies over it in a
//! `GtkOverlay` that takes the page's place in the window (probe X1 in
//! `.agents/plans/embedded-player.md`: the page shows over the video and
//! takes every click, and 4K60 plays without a dropped frame). As with the
//! Wayland host, WebKitGTK's GPU ("DMABUF") renderer must be on: without
//! it, what Vela's page had painted stayed under the transparent player view
//! (seen under X11, 2026-11-17). Standard range only: GTK 3 composites in
//! 8-bit sRGB, into which mpv tone-maps HDR.
//!
//! GTK's GL context lives on GTK's thread, and so does everything that
//! touches it here: the GL area, and mpv's renderer for it, which is made
//! there from the play's player, draws there in the area's render signal,
//! and is freed there with the context current before the play's thread
//! lets mpv go (render.h: a render context is freed before its player).

use std::cell::RefCell;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use gtk::prelude::*;
use tauri::Manager;

use super::libmpv::{LibMpv, Player, RawHandle, Renderer};
use crate::player::EmbeddedUnavailable;

/// What `mpv_render_context_create` resolves OpenGL functions with.
type ProcAddress = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;

const GL_DRAW_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;

/// The few OpenGL functions used here, outside mpv.
#[derive(Clone, Copy)]
struct Gl {
    get_integer: unsafe extern "C" fn(u32, *mut i32),
    clear_color: unsafe extern "C" fn(f32, f32, f32, f32),
    clear: unsafe extern "C" fn(u32),
}

impl Gl {
    /// # Safety
    /// `address` must resolve functions of the OpenGL context that is current.
    unsafe fn resolve(address: ProcAddress) -> Option<Self> {
        let find = |name: &CStr| unsafe { address(std::ptr::null_mut(), name.as_ptr()) };
        let (get_integer, clear_color, clear) = (
            find(c"glGetIntegerv"),
            find(c"glClearColor"),
            find(c"glClear"),
        );
        if get_integer.is_null() || clear_color.is_null() || clear.is_null() {
            return None;
        }
        // SAFETY: OpenGL's own signatures for these three names.
        unsafe {
            Some(Gl {
                get_integer: super::function_at(get_integer),
                clear_color: super::function_at(clear_color),
                clear: super::function_at(clear),
            })
        }
    }
}

/// GLX's resolver, for GTK's contexts on X11 (GTK 3 uses GLX there).
fn glx() -> Option<unsafe extern "C" fn(*const c_char) -> *mut c_void> {
    static GLX: OnceLock<Option<usize>> = OnceLock::new();
    let found: Option<usize> = *GLX.get_or_init(|| {
        // SAFETY: plain C strings; a library that is not there is null.
        unsafe {
            let library = libc::dlopen(c"libGL.so.1".as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
            if library.is_null() {
                return None;
            }
            let function = libc::dlsym(library, c"glXGetProcAddressARB".as_ptr());
            (!function.is_null()).then_some(function as usize)
        }
    });
    let address = found?;
    // SAFETY: glXGetProcAddressARB takes a function name and returns its address.
    Some(unsafe { super::function_at(address as *mut c_void) })
}

unsafe extern "C" fn glx_proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    match glx() {
        // SAFETY: mpv passes a valid function name.
        Some(resolve) => unsafe { resolve(name) },
        None => std::ptr::null_mut(),
    }
}

/// How GTK's GL contexts on `display` resolve their functions: GLX on X11,
/// EGL on Wayland. None for any other backend.
fn proc_address_for(display: &gtk::gdk::Display) -> Option<ProcAddress> {
    match display.type_().name() {
        "GdkX11Display" => glx().map(|_| glx_proc_address as ProcAddress),
        "GdkWaylandDisplay" => Some(super::egl::proc_address as ProcAddress),
        _ => None,
    }
}

/// The overlay, once it is in Vela's window. GTK's thread only.
struct Overlay {
    area: gtk::GLArea,
    /// The running play's renderer, while one draws here, with that play's
    /// phase, which the area's unrealize ends if the window goes first.
    renderer: Option<(Renderer, Arc<Mutex<Phase>>)>,
    gl: Option<Gl>,
}

thread_local! {
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

/// A redraw mpv asked for is queued on GTK's thread and has not run.
static REDRAW_QUEUED: AtomicBool = AtomicBool::new(false);

/// mpv calls this from its own threads when a frame is ready: it may only
/// ask GTK's thread to draw (render.h), at most once at a time.
unsafe extern "C" fn render_update(_context: *mut c_void) {
    if !REDRAW_QUEUED.swap(true, Ordering::AcqRel) {
        gtk::glib::MainContext::default().invoke(|| {
            REDRAW_QUEUED.store(false, Ordering::Release);
            if let Some(area) = area() {
                area.queue_render();
            }
        });
    }
}

fn area() -> Option<gtk::GLArea> {
    OVERLAY.with(|overlay| overlay.borrow().as_ref().map(|overlay| overlay.area.clone()))
}

fn main_window(app: &tauri::AppHandle) -> Option<gtk::ApplicationWindow> {
    app.webview_windows().into_values().next()?.gtk_window().ok()
}

/// Whether a play could draw here: Vela's window is up and GTK can make a
/// GL context for it. GTK's thread only.
pub(super) fn check(app: &tauri::AppHandle) -> Result<(), EmbeddedUnavailable> {
    let window = main_window(app).ok_or(EmbeddedUnavailable::WindowNotReady)?;
    let gdk_window = window.window().ok_or(EmbeddedUnavailable::WindowNotReady)?;
    let context = gdk_window.create_gl_context().map_err(|error| {
        eprintln!("vela: the embedded player is unavailable: GTK made no OpenGL context ({error})");
        EmbeddedUnavailable::NoGpuFormat
    })?;
    context.realize().map_err(|error| {
        eprintln!("vela: the embedded player is unavailable: GTK's OpenGL context did not start ({error})");
        EmbeddedUnavailable::NoGpuFormat
    })?;
    proc_address_for(&gdk_window.display()).ok_or_else(|| {
        eprintln!("vela: the embedded player is unavailable: no OpenGL loader for this display");
        EmbeddedUnavailable::NoGpuFormat
    })?;
    Ok(())
}

/// Clear WebKit's own background, so that wherever the page paints nothing
/// (the video, while the player view is open) what lies under it shows.
fn clear_background(page: &gtk::Widget) -> bool {
    use gtk::glib::translate::ToGlibPtr;
    // SAFETY: a C string; RTLD_DEFAULT searches what is already loaded.
    let address = unsafe {
        libc::dlsym(
            libc::RTLD_DEFAULT,
            c"webkit_web_view_set_background_color".as_ptr(),
        )
    };
    if address.is_null() {
        return false;
    }
    // SAFETY: WebKitGTK's signature for this function.
    let set: unsafe extern "C" fn(*mut c_void, *const gtk::gdk::ffi::GdkRGBA) =
        unsafe { super::function_at(address) };
    let clear = gtk::gdk::ffi::GdkRGBA {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    };
    let pointer: *mut gtk::ffi::GtkWidget = page.to_glib_none().0;
    // SAFETY: a live WebKitWebView, on its own thread.
    unsafe { set(pointer.cast(), &clear) };
    true
}

/// Put the overlay into Vela's window, once per run: the page leaves the box
/// wry packed it into, and an overlay takes its place there, holding the GL
/// area and, over it, the page. GTK's thread only.
///
/// The GL area is the overlay's main child and so fills the page, as in probe
/// X1: the player view, the only thing a play is shown under, always covers
/// the whole page (`PlayerView.svelte`, `.player { position: fixed; inset: 0
/// }`), so the video never needs a place of its own.
pub(super) fn install(app: &tauri::AppHandle) -> Result<(), EmbeddedUnavailable> {
    if OVERLAY.with(|overlay| overlay.borrow().is_some()) {
        return Ok(());
    }
    let not_ready = EmbeddedUnavailable::WindowNotReady;
    let window = main_window(app).ok_or(not_ready)?;
    let whole: gtk::Widget = window.clone().upcast();
    let page = super::host::find_web_view(&whole).ok_or(not_ready)?;
    let holder = page
        .parent()
        .and_then(|parent| parent.downcast::<gtk::Box>().ok())
        .ok_or(not_ready)?;
    if !clear_background(&page) {
        eprintln!("vela: the embedded player is unavailable: WebKit's background cannot be cleared");
        return Err(not_ready);
    }
    let position = holder
        .children()
        .iter()
        .position(|child| child == &page)
        .ok_or(not_ready)? as i32;
    let (expand, fill, padding, pack_type) = holder.query_child_packing(&page);

    let overlay = gtk::Overlay::new();
    let area = gtk::GLArea::new();
    area.set_has_alpha(false);
    area.set_no_show_all(true);
    area.connect_render(render);
    // GTK runs this before it lets the context go: the renderer is freed
    // while it can still be.
    area.connect_unrealize(|area| end_drawing(area, false));
    overlay.add(&area);
    // GTK takes the window's focus off a widget that leaves its container.
    // wry gives the page the focus as it makes it; without it back, keys go
    // nowhere until the page is clicked (seen under X11: the page never had
    // focus, `uifoundation`).
    let page_had_focus = window.focused_widget().as_ref() == Some(&page);
    // The page leaves wry's box (this reference keeps it alive) and lies
    // over the GL area, taking every click and key as before.
    holder.remove(&page);
    overlay.add_overlay(&page);
    overlay.set_overlay_pass_through(&page, false);
    holder.pack_start(&overlay, expand, fill, padding);
    holder.set_child_packing(&overlay, expand, fill, padding, pack_type);
    holder.reorder_child(&overlay, position);
    overlay.show_all();
    if page_had_focus {
        page.grab_focus();
    }
    OVERLAY.with(|slot| {
        *slot.borrow_mut() = Some(Overlay {
            area,
            renderer: None,
            gl: None,
        })
    });
    Ok(())
}

/// Overlay mode: put the overlay into Vela's window as the app starts, so that
/// no play has to move the live page: it is laid out in its place, with
/// WebKit's background transparent, before anything is played. Tauri's
/// setup, on the window's thread.
pub(crate) fn prepare(app: &tauri::AppHandle) {
    if super::startup::decision().mode != super::startup::WindowMode::Overlay {
        return;
    }
    if let Err(reason) = install(app) {
        // A play tries again, and says why it cannot run if it still cannot.
        eprintln!(
            "vela: the embedded player could not prepare Vela's window: {}",
            reason.notice()
        );
    }
}

/// The area's render signal: draw mpv's frame into the framebuffer GTK has
/// bound, or black when no play is drawing.
fn render(area: &gtk::GLArea, _context: &gtk::gdk::GLContext) -> gtk::glib::Propagation {
    OVERLAY.with(|slot| {
        let slot = slot.borrow();
        let Some(overlay) = slot.as_ref() else {
            return;
        };
        let Some(gl) = overlay.gl else {
            return;
        };
        let scale = area.scale_factor();
        let (width, height) = (area.allocated_width() * scale, area.allocated_height() * scale);
        match overlay.renderer.as_ref() {
            Some((renderer, _)) if width > 0 && height > 0 => {
                let new_frame = renderer.has_new_frame();
                let mut framebuffer = 0;
                // SAFETY: GTK made the area's context current for this
                // signal; one integer is written.
                unsafe { (gl.get_integer)(GL_DRAW_FRAMEBUFFER_BINDING, &mut framebuffer) };
                // Never wait here: this is GTK's thread (`video-timing-offset`
                // 0 hands frames over when they are due).
                renderer.draw_into_framebuffer(framebuffer, width, height, 8, false);
                if new_frame {
                    renderer.frame_shown();
                }
            }
            _ => {
                // SAFETY: as above.
                unsafe {
                    (gl.clear_color)(0.0, 0.0, 0.0, 1.0);
                    (gl.clear)(GL_COLOR_BUFFER_BIT);
                }
            }
        }
    });
    gtk::glib::Propagation::Stop
}

/// Free the play's renderer, with the area's context current, and hide the
/// area when `hide`. Run by the play (`Target::end`) and by the area's
/// unrealize, when the window goes away under a running play (which does not
/// hide what is being taken down). GTK's thread only.
fn end_drawing(area: &gtk::GLArea, hide: bool) {
    let taken = OVERLAY.with(|slot| slot.borrow_mut().as_mut().and_then(|overlay| overlay.renderer.take()));
    if let Some((renderer, phase)) = taken {
        if area.is_realized() {
            area.make_current();
        }
        // Renderer::drop unregisters mpv's callback, then frees the render
        // context, which needs this context current.
        drop(renderer);
        *phase.lock().unwrap_or_else(|e| e.into_inner()) = Phase::Done;
    }
    if hide {
        area.hide();
    }
}

/// Make the play's renderer, from its player's handle, and start drawing.
///
/// # Safety
/// GTK's thread; `handle` is the live handle of a player of `lib` that its
/// thread does not let go of before [`end_drawing`] has freed the renderer.
unsafe fn begin_drawing(
    lib: Arc<LibMpv>,
    handle: *mut RawHandle,
    phase: Arc<Mutex<Phase>>,
) -> Result<(), String> {
    let area = area().ok_or("Vela's window has no video area")?;
    if OVERLAY.with(|slot| slot.borrow().as_ref().is_some_and(|overlay| overlay.renderer.is_some())) {
        return Err("another play is still drawing in Vela's window".to_string());
    }
    area.show();
    if !area.is_realized() {
        area.realize();
    }
    area.make_current();
    if let Some(error) = area.error() {
        area.hide();
        return Err(format!("GTK could not set up OpenGL ({error})"));
    }
    let Some(address) = proc_address_for(&area.display()) else {
        area.hide();
        return Err("there is no OpenGL loader for this display".to_string());
    };
    // SAFETY: the area's context is current (above), and `address` resolves
    // its functions.
    let Some(gl) = (unsafe { Gl::resolve(address) }) else {
        area.hide();
        return Err("OpenGL's basic functions are missing".to_string());
    };
    // SAFETY: the area's context is current; the caller keeps the player
    // alive until the renderer is freed. No render node: hardware decoding
    // copies back, as mpv does on GLX (probe X1).
    let renderer = match unsafe { Renderer::create_for(lib, handle, address, -1) } {
        Ok(renderer) => renderer,
        Err(reason) => {
            area.hide();
            return Err(format!("mpv could not draw into Vela's window ({reason})"));
        }
    };
    // SAFETY: the callback only queues work for GTK's thread and keeps no
    // pointer; Renderer::drop unregisters it.
    unsafe { renderer.set_update(render_update, std::ptr::null_mut()) };
    OVERLAY.with(|slot| {
        if let Some(overlay) = slot.borrow_mut().as_mut() {
            overlay.renderer = Some((renderer, phase));
            overlay.gl = Some(gl);
        }
    });
    area.queue_render();
    Ok(())
}

/// Where one play's drawing is, as both threads see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    /// Asked of GTK's thread, not yet begun.
    Waiting,
    /// A renderer exists for the play's player.
    Drawing,
    /// The play stopped waiting before GTK's thread began; it never will.
    Abandoned,
    /// Drawing has ended and the renderer is freed.
    Done,
}

/// A player handle carried to GTK's thread for the length of one call.
struct SendHandle(*mut RawHandle);
// SAFETY: mpv's client handles may be used from any thread; the player's own
// thread keeps it alive until the renderer made from it has been freed.
unsafe impl Send for SendHandle {}

/// The play's hold on the overlay, from the play's own thread. Every call is
/// carried out on GTK's thread, and waited for.
pub(crate) struct Target {
    app: tauri::AppHandle,
    phase: Arc<Mutex<Phase>>,
}

impl Target {
    pub(super) fn new(app: tauri::AppHandle) -> Self {
        Target {
            app,
            phase: Arc::new(Mutex::new(Phase::Waiting)),
        }
    }

    /// Start drawing `player` in Vela's window.
    pub(super) fn begin(&self, player: &Player) -> Result<(), String> {
        let lib = player.lib().clone();
        let handle = SendHandle(player.raw());
        let phase = self.phase.clone();
        let asked = super::host::on_window_thread(&self.app, move || {
            let handle = handle;
            let mut state = phase.lock().unwrap_or_else(|e| e.into_inner());
            if *state != Phase::Waiting {
                return Err("the play stopped waiting for Vela's window".to_string());
            }
            // SAFETY: GTK's thread; the play's thread holds the player until
            // `end` has freed what is made here (`OverlayLive`'s drop).
            let begun = unsafe { begin_drawing(lib, handle.0, phase.clone()) };
            if begun.is_ok() {
                *state = Phase::Drawing;
            }
            begun
        });
        match asked {
            Ok(begun) => begun,
            Err(not_answered) => {
                // GTK's thread may still run the request later; from now on
                // it finds the play abandoned. Unless it has begun already:
                // then the drawing is ended before the player can go.
                let mut state = self.phase.lock().unwrap_or_else(|e| e.into_inner());
                if *state == Phase::Waiting {
                    *state = Phase::Abandoned;
                    return Err(not_answered);
                }
                drop(state);
                self.end()?;
                Err(not_answered)
            }
        }
    }

    /// Stop drawing and free the renderer. `Err` only when a renderer may
    /// still exist: then the player must not be let go of.
    pub(super) fn end(&self) -> Result<(), String> {
        if *self.phase.lock().unwrap_or_else(|e| e.into_inner()) != Phase::Drawing {
            return Ok(());
        }
        let phase = self.phase.clone();
        let asked = super::host::on_window_thread(&self.app, move || {
            if *phase.lock().unwrap_or_else(|e| e.into_inner()) == Phase::Drawing {
                if let Some(area) = area() {
                    end_drawing(&area, true);
                }
            }
        });
        let state = *self.phase.lock().unwrap_or_else(|e| e.into_inner());
        match asked {
            Ok(()) if state != Phase::Drawing => Ok(()),
            Ok(()) => Err("Vela's window did not free the video's renderer".to_string()),
            Err(_) if state != Phase::Drawing => Ok(()),
            Err(not_answered) => Err(not_answered),
        }
    }

    /// Whether drawing has ended under a running play (the window went).
    pub(super) fn ended(&self) -> bool {
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) == Phase::Done
    }

    /// Whether a renderer made from the play's player exists.
    pub(super) fn drawing(&self) -> bool {
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) == Phase::Drawing
    }
}
