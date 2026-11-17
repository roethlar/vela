//! macOS OpenGL drawing, following probe M1 and mpv's gl_layer.swift.
//! Every render API call takes CGL's recursive lock BEFORE the Rust mutex,
//! then makes this context current. Display requests draw synchronously on main.
//! Native views are owned separately by macos_host, on the main thread.
#![allow(deprecated)] // CAOpenGLLayer is the intentionally selected render API.

use super::libmpv::{Player, Renderer};
use super::macos_display::{DisplayFrame, DrawingSignals};
use super::macos_host::PlacementTarget;
use super::session::macos_hdr::{choose_surface, draws_black, Range, SurfaceFormat};
use objc2::{
    define_class, msg_send,
    rc::{Allocated, Retained},
    runtime::AnyObject,
    AnyThread, DefinedClass, MainThreadMarker,
};
use objc2_app_kit::NSColor;
use objc2_core_foundation::CFTimeInterval;
use objc2_core_graphics::{
    kCGColorSpaceDisplayP3_PQ, kCGColorSpaceITUR_2100_PQ, kCGColorSpaceSRGB, CGColorSpace,
};
use objc2_core_video::CVTimeStamp;
use objc2_open_gl::{
    CGLChoosePixelFormat, CGLContextObj, CGLContextParameter, CGLCreateContext,
    CGLDescribePixelFormat, CGLError, CGLErrorString, CGLFlushDrawable, CGLGetCurrentContext,
    CGLLockContext, CGLPixelFormatAttribute, CGLPixelFormatObj, CGLReleaseContext,
    CGLReleasePixelFormat, CGLRetainContext, CGLRetainPixelFormat, CGLSetCurrentContext,
    CGLSetParameter, CGLUnlockContext,
};
use objc2_quartz_core::{
    kCAContentsFormatRGBA16Float, kCAContentsFormatRGBA8Uint, CAAutoresizingMask, CAOpenGLLayer,
    CATransaction,
};
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    marker::PhantomData,
    ptr::{self, NonNull},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

const GL_DRAW_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
const GL_VIEWPORT: u32 = 0x0BA2;
const GL_VENDOR: u32 = 0x1F00;
const GL_RENDERER: u32 = 0x1F01;
const GL_VERSION: u32 = 0x1F02;

// Keep the system OpenGL framework loaded through every CGL/render operation.
fn framework() -> Result<usize, String> {
    static FRAMEWORK: OnceLock<Result<usize, String>> = OnceLock::new();
    FRAMEWORK
        .get_or_init(|| {
            let handle = unsafe {
                libc::dlopen(
                    c"/System/Library/Frameworks/OpenGL.framework/OpenGL".as_ptr(),
                    libc::RTLD_NOW | libc::RTLD_LOCAL,
                )
            };
            if handle.is_null() {
                Err("OpenGL.framework is unavailable".into())
            } else {
                Ok(handle as usize)
            }
        })
        .clone()
}

// Safety: F must be the named function's signature from OpenGL/gl3.h.
unsafe fn gl_function<F: Copy>(name: &CStr) -> Result<F, String> {
    let address = unsafe { libc::dlsym(framework()? as *mut c_void, name.as_ptr()) };
    if address.is_null() {
        return Err("OpenGL.framework is missing a drawing function".into());
    }
    // Each call site supplies the signature from OpenGL/gl3.h.
    Ok(unsafe { super::function_at(address) })
}

unsafe extern "C" fn get_proc_address(_: *mut c_void, name: *const c_char) -> *mut c_void {
    framework().map_or(ptr::null_mut(), |handle| unsafe {
        libc::dlsym(handle as *mut c_void, name)
    })
}

fn check_cgl(code: CGLError, operation: &str) -> Result<(), String> {
    if code != CGLError::NoError {
        // SAFETY: CGL returns a static NUL-terminated error description.
        let message = unsafe { CStr::from_ptr(CGLErrorString(code).as_ptr()) };
        return Err(format!(
            "{operation}: {} ({})",
            message.to_string_lossy(),
            code.0
        ));
    }
    Ok(())
}

struct GlContext {
    format: SurfaceFormat,
    pixel_format: CGLPixelFormatObj,
    context: CGLContextObj,
    get_integer: unsafe extern "C" fn(u32, *mut c_int),
    get_string: unsafe extern "C" fn(u32) -> *const u8,
    bind_framebuffer: unsafe extern "C" fn(u32, u32),
    clear_color: unsafe extern "C" fn(f32, f32, f32, f32),
    clear: unsafe extern "C" fn(u32),
    disable: unsafe extern "C" fn(u32),
}

// SAFETY: these CGL objects may migrate between threads. Every use of GL state
// holds CGLLockContext and makes this context current. Retain/release operations
// only manage ownership. CA's own drawing lock is recursive (CGL's contract).
unsafe impl Send for GlContext {}
unsafe impl Sync for GlContext {}

impl GlContext {
    fn new() -> Result<Self, String> {
        // SAFETY: the fields below supply the two OpenGL/gl3.h signatures.
        let get_integer = unsafe { gl_function(c"glGetIntegerv") }?;
        let get_string = unsafe { gl_function(c"glGetString") }?;
        let bind_framebuffer = unsafe { gl_function(c"glBindFramebuffer") }?;
        let clear_color = unsafe { gl_function(c"glClearColor") }?;
        let clear = unsafe { gl_function(c"glClear") }?;
        let disable = unsafe { gl_function(c"glDisable") }?;
        let (format, (pixel_format, context)) = choose_surface(|format| {
            Self::create_context(format).inspect_err(|error| {
                eprintln!("vela: CGL {format:?} surface unavailable: {error}");
            })
        })?;
        eprintln!(
            "vela: embedded macOS surface: {format:?}, {} bits/channel",
            format.depth()
        );
        let gl = Self {
            format,
            pixel_format,
            context,
            get_integer,
            get_string,
            bind_framebuffer,
            clear_color,
            clear,
            disable,
        };
        {
            let _current = gl.enter()?;
            // CGLFlushDrawable runs on main too. Do not wait for vblank here;
            // mpv hands over due frames and Core Animation presents the layer.
            let mut interval = 0;
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

    fn create_context(format: SurfaceFormat) -> Result<(CGLPixelFormatObj, CGLContextObj), String> {
        let pixel_format = Self::choose_pixel_format(format)?;
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

    fn choose_pixel_format(kind: SurfaceFormat) -> Result<CGLPixelFormatObj, String> {
        use CGLPixelFormatAttribute as A;
        let mut attributes: Vec<_> = kind.attributes().into_iter().map(A).collect();
        let mut format = ptr::null_mut();
        let mut count = 0;
        eprintln!(
            "CGLChoosePixelFormat: {kind:?}, OpenGL 3.2 core, double buffered; {attributes:?}"
        );
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
            return Err(format!("no {kind:?} pixel format"));
        }
        if kind == SurfaceFormat::Float {
            let mut float = 0;
            let mut size = 0;
            let float_code = unsafe {
                CGLDescribePixelFormat(format, 0, A::CGLPFAColorFloat, NonNull::from(&mut float))
            };
            let size_code = unsafe {
                CGLDescribePixelFormat(format, 0, A::CGLPFAColorSize, NonNull::from(&mut size))
            };
            if float_code != CGLError::NoError
                || size_code != CGLError::NoError
                || float == 0
                || size != 64
            {
                unsafe { CGLReleasePixelFormat(format) };
                return Err("CGL did not supply 64-bit float colour".into());
            }
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
        eprintln!("selected pixel format={format:p} kind={kind:?} count={count} accelerated={accelerated} renderer_id=0x{renderer:x}");
        Ok(format)
    }

    fn enter(&self) -> Result<CurrentContext<'_>, String> {
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
        eprintln!("vela: released embedded CGL context and pixel format");
    }
}

// This wrapper alone permits migration: every access goes through
// DrawingState's CGL lock/current guard, then its Rust mutex. The common
// Renderer stays non-Send, preserving Linux's single-thread ownership.
struct MacRenderer(Renderer);
unsafe impl Send for MacRenderer {}

struct Drawing {
    renderer: Option<MacRenderer>,
    frame: DisplayFrame,
    last_fbo: i32,
}

unsafe extern "C" fn render_update(context: *mut c_void) {
    // SAFETY: DrawingState's Arc holds this OnceLock allocation until after
    // Renderer::drop unregisters and waits out callbacks. Only signal here:
    // no AppKit, GL, mpv calls or drawing locks, even on a render thread.
    let target = unsafe { &*context.cast::<PlacementTarget>() };
    target.request_display();
}

/// Shared only as an Arc. CA's presentation copies share the exact context.
pub(super) struct DrawingState {
    gl: GlContext,
    drawing: Mutex<Drawing>,
    stopping: AtomicBool,
    failed: AtomicBool,
    display: OnceLock<PlacementTarget>,
    // A nonzero generation means draw/consume mpv frames, but present black.
    // No client API or synchronous main-thread work while drawing is locked.
    signals: DrawingSignals,
    black: AtomicU64,
}

impl DrawingState {
    pub(super) fn new() -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            gl: GlContext::new()?,
            drawing: Mutex::new(Drawing {
                renderer: None,
                frame: DisplayFrame::default(),
                last_fbo: 0,
            }),
            stopping: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            display: OnceLock::new(),
            signals: DrawingSignals::new(),
            black: AtomicU64::new(0),
        }))
    }

    pub(super) fn set_display_target(&self, target: PlacementTarget) {
        assert!(
            self.display.set(target).is_ok(),
            "display target already set"
        );
    }

    pub(super) fn start(&self, player: &Player) -> Result<(), String> {
        let target = self
            .display
            .get()
            .ok_or("the video layer has no display target")?;
        {
            let _current = self.gl.enter()?;
            let mut drawing = self.drawing.lock().unwrap_or_else(|e| e.into_inner());
            // SAFETY: same locked/current CGL context for creation, use and free.
            let renderer = unsafe { Renderer::create(player, get_proc_address) }?;
            // SAFETY: the target is fixed in this Arc-owned DrawingState. It
            // outlives the renderer, whose drop unregisters the callback before
            // freeing the render context. Registration immediately signals;
            // request_display always queues, never enters AppKit inline.
            unsafe {
                renderer.set_update(
                    render_update,
                    (target as *const PlacementTarget).cast_mut().cast(),
                );
            }
            drawing.renderer = Some(MacRenderer(renderer));
        }
        self.redraw();
        Ok(())
    }

    pub(super) fn redraw(&self) {
        self.signals.redraw(|| self.request_display());
    }

    fn request_display(&self) {
        if let Some(target) = self.display.get() {
            target.request_display();
        }
    }

    pub(super) fn float_surface(&self) -> bool {
        self.gl.format == SurfaceFormat::Float
    }

    pub(super) fn hold(&self, generation: u64) {
        self.signals.hold(generation, || self.request_display());
    }

    pub(super) fn black_ready(&self, generation: u64) -> bool {
        self.black.load(Ordering::Acquire) == generation
    }

    pub(super) fn still_holding(&self, generation: u64) -> bool {
        !self.stopping.load(Ordering::Acquire)
            && self.signals.held.load(Ordering::Acquire) == generation
    }

    /// Main only, after the colour transaction was submitted. An obsolete
    /// queued change cannot release a newer hold or touch its completion.
    pub(super) fn release(&self, generation: u64, layer: &VideoLayer, mtm: MainThreadMarker) {
        self.signals.release(generation, || layer.display_now(mtm));
    }

    pub(super) fn applied(&self, generation: u64) -> bool {
        self.signals.applied(generation)
    }

    pub(super) fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    pub(super) fn stop_drawing(&self) {
        self.stopping.store(true, Ordering::Release);
    }

    pub(super) fn shutdown(&self) -> Result<(), String> {
        self.stop_drawing();
        // Acquiring CGL waits out any CA draw. No AppKit operation or wait for
        // main while either lock is held. Renderer::drop unregisters the
        // callback before freeing, with its target still alive in self.display.
        // Free BEFORE Player is destroyed.
        let _current = self.gl.enter()?;
        let mut drawing = self.drawing.lock().unwrap_or_else(|e| e.into_inner());
        drop(drawing.renderer.take());
        Ok(())
    }

    fn begin_display(&self) -> Result<(), String> {
        let _current = self.gl.enter()?;
        let mut drawing = self.drawing.lock().unwrap_or_else(|e| e.into_inner());
        drawing.frame.begin_display();
        Ok(())
    }

    fn skip_undrawn_frame(&self) -> Result<(), String> {
        if self.stopping.load(Ordering::Acquire) || self.failed() {
            return Ok(());
        }
        // display/flush have returned, with neither drawing lock held. Like
        // draw, skip may enter GL: lock CGL, make it current, THEN lock Rust.
        let _current = self.gl.enter()?;
        let mut drawing = self.drawing.lock().unwrap_or_else(|e| e.into_inner());
        if !drawing.frame.needs_flip() {
            return Ok(());
        }
        let Some(renderer) = drawing.renderer.as_ref() else {
            return Ok(());
        };
        let frame = renderer.0.has_new_frame();
        drawing.frame.update(frame);
        if drawing.frame.needs_skip() {
            let renderer = drawing.renderer.as_ref().unwrap();
            renderer.0.skip()?;
            renderer.0.frame_shown();
            drawing.frame.consumed();
            // No black acknowledgement: skipping consumed mpv's frame but
            // did not put black on the layer for a colour-space switch.
        }
        Ok(())
    }

    fn can_draw(&self, context: CGLContextObj) -> Result<bool, String> {
        if self.stopping.load(Ordering::Acquire) || self.failed() {
            return Ok(false);
        }
        if context != self.gl.context {
            return Err("CA supplied a different CGL context".into());
        }
        let _current = self.gl.enter()?;
        let mut drawing = self.drawing.lock().unwrap_or_else(|e| e.into_inner());
        let Some(renderer) = drawing.renderer.as_ref() else {
            return Ok(false);
        };
        // Latch update: CA may ask more than once before draw consumes it.
        let frame = renderer.0.has_new_frame();
        drawing.frame.update(frame);
        Ok(drawing.frame.pending() || self.signals.redraw.load(Ordering::Acquire))
    }

    fn draw(&self, layer: &VideoLayer, context: CGLContextObj) -> Result<(), String> {
        if self.stopping.load(Ordering::Acquire) || self.failed() {
            return Ok(());
        }
        if context != self.gl.context {
            return Err("CA supplied a different CGL context".into());
        }
        let _current = self.gl.enter()?;
        let mut drawing = self.drawing.lock().unwrap_or_else(|e| e.into_inner());
        let Some(renderer) = drawing.renderer.as_ref() else {
            return Ok(());
        };
        let frame = renderer.0.has_new_frame();
        drawing.frame.update(frame);
        let redraw = self.signals.redraw.swap(false, Ordering::AcqRel);
        if !drawing.frame.pending() && !redraw {
            return Ok(());
        }
        let mut fbo = 0;
        let mut viewport = [0; 4];
        unsafe {
            (self.gl.get_integer)(GL_DRAW_FRAMEBUFFER_BINDING, &mut fbo);
            (self.gl.get_integer)(GL_VIEWPORT, viewport.as_mut_ptr());
        }
        let bounds = layer.bounds();
        let scale = layer.contentsScale();
        let (width, height) = if viewport[2] > 0 && viewport[3] > 0 {
            (viewport[2], viewport[3])
        } else {
            (
                (bounds.size.width * scale).round() as i32,
                (bounds.size.height * scale).round() as i32,
            )
        };
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        // CA owns the FBO. As in mpv's Cocoa helper, retain its nonzero binding
        // because libmpv may leave framebuffer zero bound between calls.
        if fbo != 0 {
            drawing.last_fbo = fbo;
        }
        let renderer = drawing.renderer.as_ref().unwrap();
        // Before mpv renders: a release during the render must not show a
        // frame encoded for the previous range (draws_black).
        let held_at_start = self.signals.held.load(Ordering::Acquire);
        renderer
            .0
            .draw_into(drawing.last_fbo, width, height, self.gl.format.depth())?;
        let held = self.signals.held.load(Ordering::Acquire);
        if draws_black(held_at_start, held) {
            // mpv may leave FBO zero bound. Its documented post-render state
            // has colour writes enabled; disable scissor and clear the whole
            // CA target. Black has the same meaning under both colour tags.
            unsafe {
                (self.gl.bind_framebuffer)(0x8D40, drawing.last_fbo as u32); // GL_FRAMEBUFFER
                (self.gl.disable)(0x0C11); // GL_SCISSOR_TEST
                (self.gl.clear_color)(0.0, 0.0, 0.0, 1.0);
                (self.gl.clear)(0x00004000); // GL_COLOR_BUFFER_BIT
            }
        }
        check_cgl(unsafe { CGLFlushDrawable(context) }, "CGLFlushDrawable")?;
        // Flush-time approximation, as in M1; not a presentation timestamp.
        renderer.0.frame_shown();
        if held != 0 {
            self.black.store(held, Ordering::Release);
        }
        drawing.frame.consumed();
        Ok(())
    }

    pub(super) fn fail(&self, error: &str) {
        if !self.failed.swap(true, Ordering::AcqRel) {
            eprintln!("vela: embedded macOS drawing stopped: {error}");
        }
    }
}

define_class!(
    #[unsafe(super(CAOpenGLLayer))]
    #[name = "VelaVideoOpenGLLayer"]
    #[ivars = Arc<DrawingState>]
    pub(super) struct VideoLayer;

    impl VideoLayer {
        #[unsafe(method_id(initWithLayer:))]
        fn init_with_layer(this: Allocated<Self>, source: &AnyObject) -> Option<Retained<Self>> {
            match source.downcast_ref::<Self>() {
                Some(source) => {
                    let this = this.set_ivars(Arc::clone(source.ivars()));
                    unsafe { msg_send![super(this), initWithLayer: source] }
                }
                None => None,
            }
        }

        #[unsafe(method(copyCGLPixelFormatForDisplayMask:))]
        fn copy_pixel_format(&self, _mask: u32) -> CGLPixelFormatObj {
            unsafe { CGLRetainPixelFormat(self.ivars().gl.pixel_format) }
        }
        #[unsafe(method(releaseCGLPixelFormat:))]
        fn release_pixel_format(&self, format: CGLPixelFormatObj) {
            unsafe { CGLReleasePixelFormat(format) };
        }
        #[unsafe(method(copyCGLContextForPixelFormat:))]
        fn copy_context(&self, _format: CGLPixelFormatObj) -> CGLContextObj {
            unsafe { CGLRetainContext(self.ivars().gl.context) }
        }
        #[unsafe(method(releaseCGLContext:))]
        fn release_context(&self, context: CGLContextObj) {
            unsafe { CGLReleaseContext(context) };
        }

        #[unsafe(method(canDrawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn can_draw(&self, context: CGLContextObj, _format: CGLPixelFormatObj,
            _time: CFTimeInterval, _timestamp: *const CVTimeStamp) -> bool {
            match self.ivars().can_draw(context) {
                Ok(ready) => ready,
                Err(error) => { self.ivars().fail(&error); false }
            }
        }

        #[unsafe(method(drawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn draw(&self, context: CGLContextObj, _format: CGLPixelFormatObj,
            _time: CFTimeInterval, _timestamp: *const CVTimeStamp) {
            if let Err(error) = self.ivars().draw(self, context) { self.ivars().fail(&error); }
        }
    }
);

impl VideoLayer {
    pub(super) fn new(
        state: Arc<DrawingState>,
        mtm: MainThreadMarker,
    ) -> Result<Retained<Self>, String> {
        let float = state.float_surface();
        let this = Self::alloc().set_ivars(state);
        let layer: Retained<Self> = unsafe { msg_send![super(this), init] };
        layer.setAutoresizingMask(
            CAAutoresizingMask::LayerWidthSizable | CAAutoresizingMask::LayerHeightSizable,
        );
        layer.setBackgroundColor(Some(&NSColor::blackColor().CGColor()));
        layer.setOpaque(true);
        layer.setContentsFormat(unsafe {
            if float {
                kCAContentsFormatRGBA16Float
            } else {
                kCAContentsFormatRGBA8Uint
            }
        });
        // As in mpv's Cocoa layer, display() synchronously runs canDraw/draw.
        // Vela has no live-resize signal; keep this false during resize too.
        layer.setAsynchronous(false);
        layer.set_range(Range::Sdr, mtm)?;
        Ok(layer)
    }

    pub(super) fn display_now(&self, _mtm: MainThreadMarker) {
        let state = self.ivars();
        if let Err(error) = state.begin_display() {
            state.fail(&error);
            return;
        }
        // begin_display released both locks. CA may now acquire CGL and
        // reenter our canDraw/draw methods on this main thread.
        self.display();
        CATransaction::flush();
        if let Err(error) = state.skip_undrawn_frame() {
            state.fail(&error);
        }
    }

    pub(super) fn redraw(&self, mtm: MainThreadMarker) {
        self.ivars().signals.redraw(|| self.display_now(mtm));
    }

    pub(super) fn set_range(&self, range: Range, mtm: MainThreadMarker) -> Result<(), String> {
        let name = unsafe {
            match range {
                Range::Sdr => kCGColorSpaceSRGB,
                Range::Pq2020 => kCGColorSpaceITUR_2100_PQ,
                Range::PqP3 => kCGColorSpaceDisplayP3_PQ,
            }
        };
        let space =
            CGColorSpace::with_name(Some(name)).ok_or("the layer colour space is unavailable")?;
        self.setColorspace(Some(&space));
        self.setWantsExtendedDynamicRangeContent(range != Range::Sdr);
        self.redraw(mtm);
        Ok(())
    }
}
