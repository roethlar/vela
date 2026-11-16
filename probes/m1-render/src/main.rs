//! M1: libmpv's OpenGL render API in a CAOpenGLLayer below a transparent WKWebView.
//! Run with M1_LIBMPV=/absolute/path/libmpv.2.dylib PROBE_OUT=/absolute/path.
//! The CGL/layer arrangement follows mpv 0.42.0's gl_layer.swift and
//! osdep/mac/libmpv_helper.swift; the window, HTML and insertion come from M0.

#![allow(deprecated)] // CAOpenGLLayer/OpenGL are deliberately what this probe tests.

#[cfg(not(target_os = "macos"))]
compile_error!("m1-render is a macOS-only probe");

use libloading::Library;
use objc2::{
    define_class, msg_send,
    rc::{Allocated, Retained},
    runtime::AnyObject,
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSColor, NSView, NSWindowOrderingMode};
use objc2_core_foundation::CFTimeInterval;
use objc2_core_video::CVTimeStamp;
use objc2_open_gl::{
    kCGLRendererGenericFloatID, CGLChoosePixelFormat, CGLContextObj, CGLContextParameter,
    CGLCreateContext, CGLDescribePixelFormat, CGLError, CGLErrorString, CGLFlushDrawable,
    CGLGetCurrentContext, CGLLockContext, CGLOpenGLProfile, CGLPixelFormatAttribute,
    CGLPixelFormatObj, CGLReleaseContext, CGLReleasePixelFormat, CGLRetainContext,
    CGLRetainPixelFormat, CGLSetCurrentContext, CGLSetParameter, CGLUnlockContext,
};
use objc2_quartz_core::{CAAutoresizingMask, CAOpenGLLayer};
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    fs::File,
    io::Write,
    marker::PhantomData,
    path::{Path, PathBuf},
    ptr::{self, NonNull},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use wry::{WebView, WebViewBuilder, WebViewExtMacOS};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const HTML: &str = r#"<!doctype html><html><head><style>
html, body { background: transparent; margin: 0; height: 100%; overflow: hidden; }
body { font: 24px sans-serif; }
header, footer { position: absolute; left: 0; right: 0; display: flex;
  align-items: center; padding: 0 24px; box-sizing: border-box; }
header { top: 0; height: 60px; background: rgba(0,0,0,0.55); color: white; }
footer { bottom: 0; height: 80px; background: #e08a00; color: black; }
</style></head><body><header>page text over the video</header>
<footer>opaque HTML bar</footer></body></html>"#;

// OpenGL/gl3.h. objc2-open-gl intentionally binds CGL, not these portable GL APIs.
const GL_DRAW_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
const GL_VIEWPORT: u32 = 0x0BA2;
const GL_VENDOR: u32 = 0x1F00;
const GL_RENDERER: u32 = 0x1F01;
const GL_VERSION: u32 = 0x1F02;

fn check_cgl(code: CGLError, operation: &str) -> Result<()> {
    if code != CGLError::NoError {
        // SAFETY: CGL returns a static NUL-terminated error description.
        let message = unsafe { CStr::from_ptr(CGLErrorString(code).as_ptr()) };
        return Err(format!("{operation}: {} ({})", message.to_string_lossy(), code.0).into());
    }
    Ok(())
}

struct GlContext {
    pixel_format: CGLPixelFormatObj,
    context: CGLContextObj,
    get_integer: unsafe extern "C" fn(u32, *mut c_int),
    get_string: unsafe extern "C" fn(u32) -> *const u8,
    framework: Library,
}

// SAFETY: these CGL objects may migrate between threads. Every use of GL state
// holds CGLLockContext and makes this context current. Retain/release operations
// only manage ownership. CA's own drawing lock is recursive (CGL's contract).
unsafe impl Send for GlContext {}
unsafe impl Sync for GlContext {}

impl GlContext {
    fn new(force_software: bool) -> Result<Self> {
        // SAFETY: the system framework exports the GL function signatures above.
        // Keep it loaded through all render calls and through CGL destruction.
        let framework =
            unsafe { Library::new("/System/Library/Frameworks/OpenGL.framework/OpenGL")? };
        let get_integer = unsafe { *framework.get(b"glGetIntegerv\0")? };
        let get_string = unsafe { *framework.get(b"glGetString\0")? };
        let (pixel_format, context) = if force_software {
            Self::create_context(true)?
        } else {
            match Self::create_context(false) {
                Ok(pair) => pair,
                Err(error) => {
                    eprintln!("accelerated CGL setup failed: {error}; trying software");
                    Self::create_context(true)?
                }
            }
        };
        let gl = Self {
            pixel_format,
            context,
            get_integer,
            get_string,
            framework,
        };
        {
            let _current = gl.enter()?;
            let mut interval = 1;
            // SAFETY: live, locked current context; parameter points to one GLint.
            check_cgl(
                unsafe {
                    CGLSetParameter(
                        context,
                        CGLContextParameter::CGLCPSwapInterval,
                        NonNull::from(&mut interval),
                    )
                },
                "CGLSetParameter(swap interval)",
            )?;
            eprintln!("CGL context={context:p} pixel_format={pixel_format:p}");
            for (label, name) in [
                ("vendor", GL_VENDOR),
                ("renderer", GL_RENDERER),
                ("version", GL_VERSION),
            ] {
                let value = unsafe { (gl.get_string)(name) };
                if !value.is_null() {
                    eprintln!(
                        "GL {label}={}",
                        unsafe { CStr::from_ptr(value.cast()) }.to_string_lossy()
                    );
                }
            }
        }
        Ok(gl)
    }

    fn create_context(software: bool) -> Result<(CGLPixelFormatObj, CGLContextObj)> {
        let pixel_format = Self::choose_pixel_format(software)?;
        let mut context = ptr::null_mut();
        // SAFETY: valid pixel format and output, no shared context requested.
        let code =
            unsafe { CGLCreateContext(pixel_format, ptr::null_mut(), NonNull::from(&mut context)) };
        if code != CGLError::NoError || context.is_null() {
            unsafe {
                if !context.is_null() {
                    CGLReleaseContext(context);
                }
                CGLReleasePixelFormat(pixel_format);
            }
            check_cgl(code, "CGLCreateContext")?;
            return Err("CGLCreateContext returned NULL".into());
        }
        Ok((pixel_format, context))
    }

    fn choose_pixel_format(software: bool) -> Result<CGLPixelFormatObj> {
        use CGLPixelFormatAttribute as A;
        let mut attributes = vec![
            A::CGLPFAOpenGLProfile,
            A(CGLOpenGLProfile::CGLOGLPVersion_3_2_Core.0),
        ];
        if software {
            attributes.extend([A::CGLPFARendererID, A(kCGLRendererGenericFloatID)]);
        } else {
            attributes.push(A::CGLPFAAccelerated);
        }
        attributes.extend([A::CGLPFADoubleBuffer, A(0)]);
        let mut format = ptr::null_mut();
        let mut count = 0;
        let kind = if software {
            "software (GenericFloat)"
        } else {
            "accelerated"
        };
        eprintln!("CGLChoosePixelFormat: {kind}, OpenGL 3.2 core, double buffered; {attributes:?}");
        // SAFETY: zero-terminated attribute array and valid output pointers.
        let code = unsafe {
            CGLChoosePixelFormat(
                NonNull::new(attributes.as_mut_ptr()).unwrap(),
                NonNull::from(&mut format),
                NonNull::from(&mut count),
            )
        };
        if code != CGLError::NoError || format.is_null() || count == 0 {
            if !format.is_null() {
                unsafe { CGLReleasePixelFormat(format) };
            }
            check_cgl(code, "CGLChoosePixelFormat")?;
            return Err(format!("no {kind} pixel format").into());
        }
        let mut accelerated = 0;
        let mut renderer = 0;
        // Diagnostics identify what CGL actually chose, including a fallback.
        unsafe {
            let _ = CGLDescribePixelFormat(
                format,
                0,
                A::CGLPFAAccelerated,
                NonNull::from(&mut accelerated),
            );
            let _ = CGLDescribePixelFormat(
                format,
                0,
                A::CGLPFARendererID,
                NonNull::from(&mut renderer),
            );
        }
        eprintln!("selected pixel format={format:p} kind={kind} count={count} accelerated={accelerated} renderer_id=0x{renderer:x}");
        Ok(format)
    }

    fn enter(&self) -> Result<CurrentContext<'_>> {
        // Always acquire CGL BEFORE the Rust render-state mutex. CA may already
        // hold the recursive CGL lock when it enters our draw/canDraw methods.
        check_cgl(unsafe { CGLLockContext(self.context) }, "CGLLockContext")?;
        let previous = unsafe { CGLGetCurrentContext() };
        if let Err(error) = check_cgl(
            unsafe { CGLSetCurrentContext(self.context) },
            "CGLSetCurrentContext",
        ) {
            unsafe { CGLUnlockContext(self.context) };
            return Err(error);
        }
        Ok(CurrentContext {
            gl: self,
            previous,
            _not_send: PhantomData,
        })
    }
}

struct CurrentContext<'a> {
    gl: &'a GlContext,
    previous: CGLContextObj,
    _not_send: PhantomData<*mut ()>,
}

impl Drop for CurrentContext<'_> {
    fn drop(&mut self) {
        // Restore Core Animation's calling-thread state, then release our lock.
        unsafe {
            let _ = CGLSetCurrentContext(self.previous);
            let _ = CGLUnlockContext(self.gl.context);
        }
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {
        // All layer copies share an Arc owner; the final owner is dropped only
        // after rendering stops and mpv's renderer is freed. CA owns extra retains
        // supplied by copyCGLContext/copyCGLPixelFormat until its release methods.
        unsafe {
            if CGLGetCurrentContext() == self.context {
                let _ = CGLSetCurrentContext(ptr::null_mut());
            }
            CGLReleaseContext(self.context);
            CGLReleasePixelFormat(self.pixel_format);
        }
        eprintln!("released probe's CGL context and pixel format");
    }
}

unsafe extern "C" fn get_proc_address(context: *mut c_void, name: *const c_char) -> *mut c_void {
    // SAFETY: Shared is already in its stable Arc allocation when this address
    // is registered. Its framework outlives the renderer. libloading uses dlsym
    // on this OpenGL.framework handle; no GL calls or mpv calls happen here.
    let framework = unsafe { &*context.cast::<Library>() };
    let name = unsafe { CStr::from_ptr(name) };
    unsafe { framework.get::<*mut c_void>(name.to_bytes_with_nul()) }
        .map(|symbol| *symbol)
        .unwrap_or(ptr::null_mut())
}

// Public libmpv client.h/render.h/render_gl.h ABI (mpv 0.42.0).
#[repr(C)]
struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}
#[repr(C)]
struct RenderParam {
    kind: c_int,
    data: *mut c_void,
}
#[repr(C)]
struct OpenGlInitParams {
    get_proc_address: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    get_proc_address_ctx: *mut c_void,
}
#[repr(C)]
struct OpenGlFbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}
const MPV_EVENT_NONE: c_int = 0;
const MPV_EVENT_SHUTDOWN: c_int = 1;
const MPV_EVENT_FILE_LOADED: c_int = 8;
const MPV_RENDER_PARAM_INVALID: c_int = 0;
const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
const MPV_RENDER_PARAM_OPENGL_FBO: c_int = 3;
const MPV_RENDER_PARAM_FLIP_Y: c_int = 4;
const MPV_RENDER_UPDATE_FRAME: u64 = 1;

struct MpvApi {
    create: unsafe extern "C" fn() -> *mut c_void,
    set_option: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
    initialize: unsafe extern "C" fn(*mut c_void) -> c_int,
    wait_event: unsafe extern "C" fn(*mut c_void, f64) -> *mut MpvEvent,
    event_name: unsafe extern "C" fn(c_int) -> *const c_char,
    command_async: unsafe extern "C" fn(*mut c_void, u64, *mut *const c_char) -> c_int,
    terminate_destroy: unsafe extern "C" fn(*mut c_void),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    render_create: unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut RenderParam) -> c_int,
    render_set_update:
        unsafe extern "C" fn(*mut c_void, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
    render_update: unsafe extern "C" fn(*mut c_void) -> u64,
    render: unsafe extern "C" fn(*mut c_void, *mut RenderParam) -> c_int,
    render_report_swap: unsafe extern "C" fn(*mut c_void),
    render_free: unsafe extern "C" fn(*mut c_void),
    _library: Library,
}

impl MpvApi {
    fn load(path: &Path) -> Result<Arc<Self>> {
        if !path.is_absolute() {
            return Err("M1_LIBMPV must name an absolute dylib path".into());
        }
        // SAFETY: deliberately load the runner's Homebrew libmpv, as in M0.
        let library = unsafe { Library::new(path)? };
        macro_rules! symbol {
            ($name:literal) => {
                // Each field supplies the corresponding public C ABI type.
                unsafe { *library.get(concat!($name, "\0").as_bytes())? }
            };
        }
        eprintln!("loaded libmpv {}", path.display());
        Ok(Arc::new(Self {
            create: symbol!("mpv_create"),
            set_option: symbol!("mpv_set_option_string"),
            initialize: symbol!("mpv_initialize"),
            wait_event: symbol!("mpv_wait_event"),
            event_name: symbol!("mpv_event_name"),
            command_async: symbol!("mpv_command_async"),
            terminate_destroy: symbol!("mpv_terminate_destroy"),
            error_string: symbol!("mpv_error_string"),
            render_create: symbol!("mpv_render_context_create"),
            render_set_update: symbol!("mpv_render_context_set_update_callback"),
            render_update: symbol!("mpv_render_context_update"),
            render: symbol!("mpv_render_context_render"),
            render_report_swap: symbol!("mpv_render_context_report_swap"),
            render_free: symbol!("mpv_render_context_free"),
            _library: library,
        }))
    }

    fn check(&self, code: c_int, operation: &str) -> Result<()> {
        if code < 0 {
            let error = unsafe { CStr::from_ptr((self.error_string)(code)) };
            return Err(format!("{operation}: {} ({code})", error.to_string_lossy()).into());
        }
        Ok(())
    }
}

struct Mpv {
    handle: *mut c_void,
    api: Arc<MpvApi>,
}

// SAFETY: client.h permits migration. After initialization only the event worker
// owns this handle, and it waits for the main thread to free the render context
// before calling terminate_destroy. AppKit owners remain on the main thread.
unsafe impl Send for Mpv {}

impl Mpv {
    fn new(path: &Path, variant: &str, out: &Path) -> Result<Self> {
        let api = MpvApi::load(path)?;
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err("mpv_create returned NULL".into());
        }
        let player = Self { handle, api };
        eprintln!("created mpv handle={handle:p}");
        let log = out.join(format!("mpv-{variant}.log"));
        let log = log.to_str().ok_or("log path is not UTF-8")?;
        for (name, value) in [
            ("vo", "libmpv"),
            ("hwdec", "no"),
            ("idle", "yes"),
            ("keep-open", "yes"),
            ("loop-file", "inf"),
            ("osc", "no"),
            ("input-default-bindings", "no"),
            ("msg-level", "all=v"),
            ("log-file", log),
        ] {
            eprintln!("option {name}={value}");
            let key = CString::new(name)?;
            let value = CString::new(value)?;
            // SAFETY: live uninitialized handle; mpv copies both C strings.
            player.api.check(
                unsafe { (player.api.set_option)(handle, key.as_ptr(), value.as_ptr()) },
                name,
            )?;
        }
        player
            .api
            .check(unsafe { (player.api.initialize)(handle) }, "mpv_initialize")?;
        Ok(player)
    }

    fn watch(&self, stop: &AtomicBool, proxy: &EventLoopProxy<ProbeEvent>) -> bool {
        let mut args = [
            c"loadfile".as_ptr(),
            c"av://lavfi:testsrc2=size=1280x720:rate=30".as_ptr(),
            ptr::null(),
        ];
        // SAFETY: initialized handle, NULL-terminated argv copied by mpv.
        let code = unsafe { (self.api.command_async)(self.handle, 1, args.as_mut_ptr()) };
        if let Err(error) = self.api.check(code, "loadfile") {
            eprintln!("{error}");
        }
        let mut loaded = false;
        let mut shutdown = false;
        while !stop.load(Ordering::Acquire) {
            if shutdown {
                // A shutdown event must not drop mpv before render_free finishes.
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            // SAFETY: this is the sole event reader; all borrowed data is
            // consumed before the next wait_event call. Never under a GL lock.
            let event = unsafe { &*(self.api.wait_event)(self.handle, 0.1) };
            if event.event_id != MPV_EVENT_NONE {
                let name = unsafe { (self.api.event_name)(event.event_id) };
                let name = if name.is_null() {
                    "unknown".into()
                } else {
                    unsafe { CStr::from_ptr(name) }.to_string_lossy()
                };
                eprintln!(
                    "mpv event={} ({name}) error={} reply={}",
                    event.event_id, event.error, event.reply_userdata
                );
                if let Err(error) = self.api.check(event.error, "mpv event") {
                    eprintln!("{error}");
                }
            }
            if event.event_id == MPV_EVENT_FILE_LOADED {
                loaded = true;
                eprintln!("MPV_EVENT_FILE_LOADED");
            }
            if event.event_id == MPV_EVENT_SHUTDOWN {
                shutdown = true;
                let _ = proxy.send_event(ProbeEvent::Shutdown);
            }
        }
        loaded
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // Startup errors occur before a renderer exists; otherwise the worker is
        // released only after render_free, while the AppKit loop still pumps.
        eprintln!("mpv_terminate_destroy begin");
        unsafe { (self.api.terminate_destroy)(self.handle) };
        eprintln!("mpv destroyed");
    }
}

#[derive(Debug)]
enum ProbeEvent {
    RenderUpdate,
    Shutdown,
    Finished(bool),
}

struct Wakeup {
    proxy: EventLoopProxy<ProbeEvent>,
    queued: AtomicBool,
    enabled: AtomicBool,
}

unsafe extern "C" fn render_update_callback(context: *mut c_void) {
    // SAFETY: Shared owns this Arc allocation until after renderer AND mpv
    // destruction. No AppKit, GL, mpv calls, or renderer locks in this callback.
    let wakeup = unsafe { &*context.cast::<Wakeup>() };
    if wakeup.enabled.load(Ordering::Acquire) && !wakeup.queued.swap(true, Ordering::AcqRel) {
        let _ = wakeup.proxy.send_event(ProbeEvent::RenderUpdate);
    }
}

#[derive(Clone, Copy, Default)]
struct Counts {
    calls: u64,
    successful: u64,
}

struct Renderer {
    raw: *mut c_void,
    api: Arc<MpvApi>,
    pending: bool,
    last_fbo: c_int,
    counts: Counts,
}

// SAFETY: Renderer is only accessed inside Shared's render mutex, with its CGL
// context locked and current. No destructor issues an implicit render API call.
unsafe impl Send for Renderer {}

struct Shared {
    gl: GlContext,
    renderer: Mutex<Option<Renderer>>,
    wakeup: Arc<Wakeup>,
    stopping: AtomicBool,
    failed: AtomicBool,
}

impl Shared {
    fn start(&self, mpv: &Mpv) -> Result<()> {
        let _current = self.gl.enter()?;
        let mut state = self.renderer.lock().unwrap_or_else(|e| e.into_inner());
        let mut init = OpenGlInitParams {
            get_proc_address,
            get_proc_address_ctx: (&self.gl.framework as *const Library).cast_mut().cast(),
        };
        let mut params = [
            RenderParam {
                kind: MPV_RENDER_PARAM_API_TYPE,
                data: c"opengl".as_ptr().cast_mut().cast(),
            },
            RenderParam {
                kind: MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: (&mut init as *mut OpenGlInitParams).cast(),
            },
            RenderParam {
                kind: MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];
        let mut raw = ptr::null_mut();
        // SAFETY: initialized mpv, stable resolver context, live parameters,
        // serialized with GL current. No loadfile has been sent yet.
        mpv.api.check(
            unsafe { (mpv.api.render_create)(&mut raw, mpv.handle, params.as_mut_ptr()) },
            "mpv_render_context_create",
        )?;
        if raw.is_null() {
            return Err("mpv_render_context_create returned NULL".into());
        }
        *state = Some(Renderer {
            raw,
            api: Arc::clone(&mpv.api),
            pending: false,
            last_fbo: 0,
            counts: Counts::default(),
        });
        unsafe {
            (mpv.api.render_set_update)(
                raw,
                Some(render_update_callback),
                Arc::as_ptr(&self.wakeup).cast_mut().cast(),
            )
        };
        eprintln!(
            "created mpv render context={raw:p} with current CGL={:p}; update callback installed",
            self.gl.context
        );
        Ok(())
    }

    fn can_draw(&self, context: CGLContextObj) -> Result<bool> {
        if self.stopping.load(Ordering::Acquire) || self.failed.load(Ordering::Acquire) {
            return Ok(false);
        }
        if context != self.gl.context {
            return Err("CA supplied a different CGL context to canDraw".into());
        }
        let _current = self.gl.enter()?;
        let mut state = self.renderer.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(renderer) = state.as_mut() {
            // Latch the flag: CA may ask canDraw more than once before draw.
            renderer.pending |= unsafe { (renderer.api.render_update)(renderer.raw) }
                & MPV_RENDER_UPDATE_FRAME
                != 0;
            Ok(renderer.pending)
        } else {
            Ok(false)
        }
    }

    fn draw(&self, layer: &MpvLayer, context: CGLContextObj) -> Result<()> {
        if self.stopping.load(Ordering::Acquire) || self.failed.load(Ordering::Acquire) {
            return Ok(());
        }
        if context != self.gl.context {
            return Err("CA supplied a different CGL context to draw".into());
        }
        let _current = self.gl.enter()?;
        let mut state = self.renderer.lock().unwrap_or_else(|e| e.into_inner());
        let Some(renderer) = state.as_mut() else {
            return Ok(());
        };
        renderer.pending |=
            unsafe { (renderer.api.render_update)(renderer.raw) } & MPV_RENDER_UPDATE_FRAME != 0;
        if !renderer.pending {
            return Ok(());
        }
        let mut bound_fbo = 0;
        let mut viewport = [0; 4];
        unsafe {
            (self.gl.get_integer)(GL_DRAW_FRAMEBUFFER_BINDING, &mut bound_fbo);
            (self.gl.get_integer)(GL_VIEWPORT, viewport.as_mut_ptr());
        }
        // Follow gl_layer.swift: prefer CA's viewport; fall back to backing
        // pixels from bounds * contentsScale if the viewport has no size.
        let bounds = layer.bounds();
        let scale = layer.contentsScale();
        let backing = [
            (bounds.size.width * scale).round() as c_int,
            (bounds.size.height * scale).round() as c_int,
        ];
        let [w, h] = if viewport[2] > 0 && viewport[3] > 0 {
            [viewport[2], viewport[3]]
        } else {
            backing
        };
        if w <= 0 || h <= 0 {
            return Ok(());
        }
        // cocoa-cb remembers a nonzero CA FBO because mpv can leave zero bound.
        // Do not invent FBO 1 if CA has not supplied a nonzero framebuffer yet.
        if bound_fbo != 0 {
            renderer.last_fbo = bound_fbo;
        }
        let mut fbo = OpenGlFbo {
            fbo: renderer.last_fbo,
            w,
            h,
            internal_format: 0,
        };
        let mut flip: c_int = 1;
        let mut params = [
            RenderParam {
                kind: MPV_RENDER_PARAM_OPENGL_FBO,
                data: (&mut fbo as *mut OpenGlFbo).cast(),
            },
            RenderParam {
                kind: MPV_RENDER_PARAM_FLIP_Y,
                data: (&mut flip as *mut c_int).cast(),
            },
            RenderParam {
                kind: MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];
        renderer.counts.calls += 1;
        let code = unsafe { (renderer.api.render)(renderer.raw, params.as_mut_ptr()) };
        renderer.api.check(code, "mpv_render_context_render")?;
        renderer.counts.successful += 1;
        renderer.pending = false;
        if renderer.counts.successful == 1 || renderer.counts.successful % 60 == 0 {
            eprintln!("render call={} ok={} CGL={context:p} bound_fbo={bound_fbo} used_fbo={} viewport={viewport:?} bounds={bounds:?} scale={scale} backing={backing:?} target={w}x{h} flip_y=1 thread={:?}", renderer.counts.calls, renderer.counts.successful, fbo.fbo, thread::current().id());
        }
        // mpv's Cocoa helper uses CGLFlushDrawable after drawing. report_swap
        // here is a flush-time approximation, not a measurement of presentation.
        check_cgl(unsafe { CGLFlushDrawable(context) }, "CGLFlushDrawable")?;
        unsafe { (renderer.api.render_report_swap)(renderer.raw) };
        Ok(())
    }

    fn fail(&self, error: &dyn std::fmt::Display) {
        if !self.failed.swap(true, Ordering::AcqRel) {
            eprintln!("rendering stopped after error: {error}");
        }
    }

    fn shutdown(&self) -> Result<Counts> {
        self.stopping.store(true, Ordering::Release);
        self.wakeup.enabled.store(false, Ordering::Release);
        // Wait for any in-flight CA draw, using the same CGL -> Rust lock order.
        let _current = self.gl.enter()?;
        let mut state = self.renderer.lock().unwrap_or_else(|e| e.into_inner());
        let Some(renderer) = state.take() else {
            return Ok(Counts::default());
        };
        eprintln!(
            "freeing mpv render context={:p}, CGL={:p} current",
            renderer.raw, self.gl.context
        );
        unsafe {
            (renderer.api.render_set_update)(renderer.raw, None, ptr::null_mut());
            (renderer.api.render_free)(renderer.raw);
        }
        eprintln!("mpv render context freed");
        Ok(renderer.counts)
    }
}

define_class!(
    // CA may draw on its own thread. Only the immutable Arc is in the ivars;
    // Shared serializes GL/render state, while view mutations stay on main.
    #[unsafe(super(CAOpenGLLayer))]
    #[name = "VelaM1OpenGLLayer"]
    #[ivars = Arc<Shared>]
    struct MpvLayer;

    impl MpvLayer {
        // Core Animation copies layers for its presentation tree/screen changes.
        // Copies must use the SAME CGL context and the SAME mpv renderer.
        #[unsafe(method_id(initWithLayer:))]
        fn init_with_layer(
            this: Allocated<Self>,
            source: &AnyObject,
        ) -> Option<Retained<Self>> {
            // One expression: define_class! wraps the body's value, so an
            // early `return` would bypass that and not compile.
            match source.downcast_ref::<Self>() {
                Some(source_layer) => {
                    let this = this.set_ivars(Arc::clone(source_layer.ivars()));
                    unsafe { msg_send![super(this), initWithLayer: source] }
                }
                None => {
                    eprintln!("initWithLayer received a non-M1 layer");
                    None
                }
            }
        }

        #[unsafe(method(copyCGLPixelFormatForDisplayMask:))]
        fn copy_pixel_format(&self, mask: u32) -> CGLPixelFormatObj {
            eprintln!("copyCGLPixelFormatForDisplayMask mask=0x{mask:x}");
            // The copy/release pairs give CA its own +1, separate from Shared.
            unsafe { CGLRetainPixelFormat(self.ivars().gl.pixel_format) }
        }

        #[unsafe(method(releaseCGLPixelFormat:))]
        fn release_pixel_format(&self, format: CGLPixelFormatObj) {
            unsafe { CGLReleasePixelFormat(format) };
        }

        #[unsafe(method(copyCGLContextForPixelFormat:))]
        fn copy_context(&self, format: CGLPixelFormatObj) -> CGLContextObj {
            eprintln!(
                "copyCGLContextForPixelFormat format={format:p} context={:p}",
                self.ivars().gl.context
            );
            unsafe { CGLRetainContext(self.ivars().gl.context) }
        }

        #[unsafe(method(releaseCGLContext:))]
        fn release_context(&self, context: CGLContextObj) {
            unsafe { CGLReleaseContext(context) };
        }

        #[unsafe(method(canDrawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn can_draw(
            &self,
            context: CGLContextObj,
            _format: CGLPixelFormatObj,
            _time: CFTimeInterval,
            _timestamp: *const CVTimeStamp,
        ) -> bool {
            match self.ivars().can_draw(context) {
                Ok(ready) => ready,
                Err(error) => {
                    self.ivars().fail(&error);
                    false
                }
            }
        }

        #[unsafe(method(drawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn draw(
            &self,
            context: CGLContextObj,
            _format: CGLPixelFormatObj,
            _time: CFTimeInterval,
            _timestamp: *const CVTimeStamp,
        ) {
            if let Err(error) = self.ivars().draw(self, context) {
                self.ivars().fail(&error);
            }
        }
    }
);

impl MpvLayer {
    fn new(shared: Arc<Shared>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(shared);
        // SAFETY: ivars initialized before the superclass can call overrides.
        let layer: Retained<Self> = unsafe { msg_send![super(this), init] };
        layer.setAutoresizingMask(
            CAAutoresizingMask::LayerWidthSizable | CAAutoresizingMask::LayerHeightSizable,
        );
        layer.setBackgroundColor(Some(&NSColor::blackColor().CGColor()));
        layer.setOpaque(true);
        // canDraw polls mpv, with callback-driven invalidation on main as well.
        layer.setAsynchronous(true);
        eprintln!("created VelaM1OpenGLLayer={:p}, asynchronous=true", &*layer);
        layer
    }
}

fn dump_view(file: &mut File, view: &NSView, depth: usize) -> Result<()> {
    writeln!(
        file,
        "{}{} {:p} frame={:?} isOpaque={} wantsLayer={} autoresizingMask={:?}",
        "  ".repeat(depth),
        view.class().name().to_string_lossy(),
        view,
        view.frame(),
        view.isOpaque(),
        view.wantsLayer(),
        view.autoresizingMask()
    )?;
    for child in view.subviews().iter() {
        dump_view(file, &child, depth + 1)?;
    }
    Ok(())
}

fn insert_video_view(
    webview: &WebView,
    layer: &MpvLayer,
    out: &Path,
    variant: &str,
) -> Result<Retained<NSView>> {
    let mtm = MainThreadMarker::new().ok_or("AppKit needs the main thread")?;
    let window = webview.ns_window();
    let wk = webview.webview(); // Retain across a possible contentView replacement.
    let wk_view: &NSView = &wk;
    let content = window.contentView().ok_or("NSWindow has no contentView")?;
    let mut file = File::create(out.join(format!("views-{variant}.txt")))?;
    writeln!(
        file,
        "WKWebView={wk_view:p}\nBEFORE: NSWindow.contentView (children back to front)"
    )?;
    dump_view(&mut file, &content, 0)?;
    let parent = if ptr::eq(&*content, wk_view) {
        writeln!(
            file,
            "WKWebView IS contentView; installing an NSView container."
        )?;
        let container = NSView::initWithFrame(NSView::alloc(mtm), content.frame());
        window.setContentView(Some(&container));
        wk_view.setFrame(container.bounds());
        wk_view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        container.addSubview(wk_view);
        window.makeFirstResponder(Some(&wk));
        container
    } else {
        writeln!(
            file,
            "WKWebView is inside a parent; inserting a sibling in that same parent."
        )?;
        // SAFETY: main thread, retained WKWebView, no concurrent hierarchy change.
        unsafe { wk_view.superview() }.ok_or("WKWebView has no superview")?
    };
    let video = NSView::initWithFrame(NSView::alloc(mtm), wk_view.frame());
    video.setAutoresizingMask(wk_view.autoresizingMask());
    layer.setFrame(video.bounds());
    layer.setContentsScale(window.backingScaleFactor());
    // Supply the custom root layer before enabling layer hosting on this view.
    video.setLayer(Some(layer));
    video.setWantsLayer(true);
    parent.addSubview_positioned_relativeTo(&video, NSWindowOrderingMode::Below, Some(wk_view));
    let installed = video.layer().ok_or("video view has no layer")?;
    let expected: &objc2_quartz_core::CALayer = layer;
    if !ptr::eq(&*installed, expected) {
        return Err("AppKit replaced the custom CAOpenGLLayer".into());
    }
    eprintln!(
        "inserted video NSView={:p} layer={layer:p} below WKWebView={wk_view:p}, scale={}",
        &*video,
        layer.contentsScale()
    );
    writeln!(
        file,
        "video NSView={:p}; layer={} {layer:p}\nAFTER: NSWindow.contentView",
        &*video,
        layer.class().name().to_string_lossy()
    )?;
    let after = window.contentView().ok_or("contentView disappeared")?;
    dump_view(&mut file, &after, 0)?;
    Ok(video)
}

fn run() -> Result<u8> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 || !matches!(args[1].as_str(), "gl" | "gl-sw") {
        return Err("usage: m1-render <gl|gl-sw> (requires PROBE_OUT and M1_LIBMPV)".into());
    }
    let variant = &args[1];
    let out = PathBuf::from(std::env::var_os("PROBE_OUT").ok_or("missing PROBE_OUT")?);
    let library = PathBuf::from(std::env::var_os("M1_LIBMPV").ok_or("missing M1_LIBMPV")?);
    let mut event_loop = EventLoopBuilder::<ProbeEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title(format!("M1 {variant}"))
        .with_inner_size(LogicalSize::new(1000.0, 600.0))
        .with_transparent(true)
        .with_visible(false)
        .build(&event_loop)?;
    // Wry's transparent feature enables the macOS drawsBackground KVC path.
    let webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(HTML)
        .build(&window)?;
    let proxy = event_loop.create_proxy();
    let shared = Arc::new(Shared {
        gl: GlContext::new(variant == "gl-sw")?,
        renderer: Mutex::new(None),
        wakeup: Arc::new(Wakeup {
            proxy: proxy.clone(),
            queued: AtomicBool::new(false),
            enabled: AtomicBool::new(true),
        }),
        stopping: AtomicBool::new(false),
        failed: AtomicBool::new(false),
    });
    let layer = MpvLayer::new(Arc::clone(&shared));
    let video = insert_video_view(&webview, &layer, &out, variant)?;
    window.set_visible(true);
    webview.ns_window().makeKeyAndOrderFront(None);
    let deadline = Instant::now() + Duration::from_secs(12);
    let worker_stop = Arc::new(AtomicBool::new(false));
    let mut worker = None;
    let mut started = false;
    let mut finished = false;
    let mut closing = false;
    let mut stopping = false;
    let mut loaded = false;
    let mut counts = Counts::default();
    event_loop.run_return(|event, _, flow| {
        match event {
            Event::MainEventsCleared if !started && !stopping => {
                started = true;
                match Mpv::new(&library, variant, &out).and_then(|mpv| {
                    shared.start(&mpv)?;
                    Ok(mpv)
                }) {
                    Ok(mpv) => {
                        let stop = Arc::clone(&worker_stop);
                        let proxy = proxy.clone();
                        worker = Some(thread::spawn(move || {
                            let loaded = mpv.watch(&stop, &proxy);
                            drop(mpv);
                            let _ = proxy.send_event(ProbeEvent::Finished(loaded));
                        }));
                        layer.setNeedsDisplay();
                    }
                    Err(error) => {
                        eprintln!("mpv startup: {error}");
                        finished = true; // Keep HTML visible through the captures.
                    }
                }
            }
            Event::UserEvent(ProbeEvent::RenderUpdate) => {
                shared.wakeup.queued.store(false, Ordering::Release);
                if !stopping {
                    // The mpv callback only enqueues; AppKit/CA invalidation is main-thread work.
                    layer.setNeedsDisplay();
                }
            }
            Event::UserEvent(ProbeEvent::Finished(saw_file_loaded)) => {
                loaded = saw_file_loaded;
                finished = true;
            }
            Event::UserEvent(ProbeEvent::Shutdown) => closing = true,
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => closing = true,
            _ => {}
        }
        if closing || Instant::now() >= deadline {
            if !stopping {
                stopping = true;
                shared.stopping.store(true, Ordering::Release);
                layer.setAsynchronous(false);
                counts = match shared.shutdown() {
                    Ok(counts) => counts,
                    Err(error) => {
                        // Without a current CGL context we cannot safely free the
                        // renderer or destroy mpv. Exit as a failed probe instead.
                        eprintln!("fatal renderer teardown: {error}");
                        std::process::exit(3);
                    }
                };
                worker_stop.store(true, Ordering::Release);
                if worker.is_none() {
                    finished = true;
                }
            }
            *flow = if finished {
                ControlFlow::Exit
            } else {
                ControlFlow::Wait
            };
        } else {
            *flow = ControlFlow::WaitUntil(deadline);
        }
    });
    if let Some(worker) = worker {
        worker.join().map_err(|_| "mpv event thread panicked")?;
    }
    // Callback storage and native views survive until AFTER mpv termination.
    drop(webview);
    drop(video);
    drop(window);
    drop(layer);
    eprintln!("summary variant={variant} render_calls={} successful_renders={} FILE_LOADED={loaded} render_error={}", counts.calls, counts.successful, shared.failed.load(Ordering::Acquire));
    Ok(if counts.successful > 0 { 0 } else { 3 })
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("m1-render: {error}");
            std::process::ExitCode::from(3)
        }
    }
}
