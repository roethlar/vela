//! M0: mpv's macvk wid path underneath a transparent Wry 0.55.1 WKWebView.
//! Run with M0_LIBMPV=/absolute/path/libmpv.2.dylib PROBE_OUT=/absolute/path.

#[cfg(not(target_os = "macos"))]
compile_error!("m0-embed is a macOS-only probe");

use libloading::Library;
use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSColor, NSView, NSWindowOrderingMode};
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
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
        view.autoresizingMask(),
    )?;
    for child in view.subviews().iter() {
        dump_view(file, &child, depth + 1)?;
    }
    Ok(())
}

fn insert_video_view(webview: &WebView, out: &Path, variant: &str) -> Result<Retained<NSView>> {
    let mtm = MainThreadMarker::new().ok_or("AppKit needs the main thread")?;
    let window = webview.ns_window();
    let wk = webview.webview(); // Retains WKWebView across any contentView replacement.
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
        // SAFETY: on the main thread, with WKWebView retained; no concurrent
        // hierarchy mutation can invalidate its weak superview during this call.
        unsafe { wk_view.superview() }.ok_or("WKWebView has no superview")?
    };
    let video = NSView::initWithFrame(NSView::alloc(mtm), wk_view.frame());
    video.setAutoresizingMask(wk_view.autoresizingMask());
    video.setWantsLayer(true);
    parent.addSubview_positioned_relativeTo(&video, NSWindowOrderingMode::Below, Some(wk_view));
    video
        .layer()
        .ok_or("layer-backed NSView has no CALayer")?
        .setBackgroundColor(Some(&NSColor::blackColor().CGColor()));
    writeln!(
        file,
        "video NSView={:p}; wid={}\nAFTER: NSWindow.contentView",
        &*video,
        Retained::as_ptr(&video) as usize
    )?;
    let after = window.contentView().ok_or("contentView disappeared")?;
    dump_view(&mut file, &after, 0)?;
    Ok(video)
}

// Public libmpv client.h ABI (client API 2): mpv_event and event IDs.
#[repr(C)]
struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}
const MPV_EVENT_NONE: c_int = 0;
const MPV_EVENT_SHUTDOWN: c_int = 1;
const MPV_EVENT_FILE_LOADED: c_int = 8;

struct Mpv {
    handle: *mut c_void,
    wait_event: unsafe extern "C" fn(*mut c_void, f64) -> *mut MpvEvent,
    command_async: unsafe extern "C" fn(*mut c_void, u64, *mut *const c_char) -> c_int,
    terminate_destroy: unsafe extern "C" fn(*mut c_void),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    _library: Library, // Outlives every function pointer and mpv_terminate_destroy.
}

// SAFETY: libmpv's client API allows thread migration. Ownership is transferred
// once after initialization; only the worker uses/destroys the handle thereafter.
// AppKit views stay retained on the main thread until that worker has finished.
unsafe impl Send for Mpv {}

impl Mpv {
    fn new(path: &Path, wid: usize, variant: &str, out: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err("M0_LIBMPV must name an absolute dylib path".into());
        }
        // SAFETY: the runner supplies the Homebrew libmpv dylib, deliberately
        // executing its initializers. It stays loaded until after mpv destruction.
        let library = unsafe { Library::new(path)? };
        macro_rules! symbol {
            ($name:literal, $ty:ty) => {{
                // SAFETY: these NUL-terminated, named client.h symbols have the
                // exact C signatures below; the Library is retained in Mpv.
                unsafe { *library.get::<$ty>(concat!($name, "\0").as_bytes())? }
            }};
        }
        let create = symbol!("mpv_create", unsafe extern "C" fn() -> *mut c_void);
        let set_option = symbol!(
            "mpv_set_option_string",
            unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int
        );
        let initialize = symbol!("mpv_initialize", unsafe extern "C" fn(*mut c_void) -> c_int);
        let wait_event = symbol!(
            "mpv_wait_event",
            unsafe extern "C" fn(*mut c_void, f64) -> *mut MpvEvent
        );
        let command_async = symbol!(
            "mpv_command_async",
            unsafe extern "C" fn(*mut c_void, u64, *mut *const c_char) -> c_int
        );
        let terminate_destroy = symbol!("mpv_terminate_destroy", unsafe extern "C" fn(*mut c_void));
        let error_string = symbol!(
            "mpv_error_string",
            unsafe extern "C" fn(c_int) -> *const c_char
        );
        // SAFETY: mpv_create takes no arguments; window/webview already exist on
        // this main thread. Check its nullable result before any further call.
        let handle = unsafe { create() };
        if handle.is_null() {
            return Err("mpv_create returned NULL".into());
        }
        let player = Self {
            handle,
            wait_event,
            command_async,
            terminate_destroy,
            error_string,
            _library: library,
        };
        let wid = wid.to_string();
        let log = out.join(format!("mpv-{variant}.log"));
        let log = log.to_str().ok_or("log path is not UTF-8")?;
        for (name, value) in [
            ("wid", wid.as_str()),
            ("vo", if variant == "wid" { "gpu-next" } else { "gpu" }),
            ("gpu-api", "vulkan"),
            ("gpu-context", "macvk"),
            ("hwdec", "no"),
            ("idle", "yes"),
            ("keep-open", "yes"),
            ("osc", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("loop-file", "inf"),
            ("msg-level", "all=v"),
            ("log-file", log),
        ] {
            eprintln!("option {name}={value}");
            let key = CString::new(name)?;
            let value = CString::new(value)?;
            // SAFETY: live, uninitialized handle and C strings that outlive the
            // call; mpv copies the option. All options precede mpv_initialize.
            player.check(
                unsafe { set_option(handle, key.as_ptr(), value.as_ptr()) },
                name,
            )?;
        }
        // SAFETY: the handle is live, configured, and not yet initialized.
        player.check(unsafe { initialize(handle) }, "mpv_initialize")?;
        Ok(player)
    }

    fn check(&self, code: c_int, context: &str) -> Result<()> {
        if code < 0 {
            // SAFETY: mpv_error_string returns a static, NUL-terminated string.
            let error = unsafe { CStr::from_ptr((self.error_string)(code)) };
            return Err(format!("{context}: {} ({code})", error.to_string_lossy()).into());
        }
        Ok(())
    }

    fn watch(&self, stop: &AtomicBool, deadline: Instant) -> bool {
        let mut args = [
            c"loadfile".as_ptr(),
            c"av://lavfi:testsrc2=size=1280x720:rate=30".as_ptr(),
            ptr::null(),
        ];
        // SAFETY: initialized handle; args is NULL-terminated, contains live C
        // strings, and is copied before this asynchronous command call returns.
        let code = unsafe { (self.command_async)(self.handle, 1, args.as_mut_ptr()) };
        if let Err(error) = self.check(code, "loadfile") {
            eprintln!("{error}");
            return false;
        }
        let mut loaded = false;
        while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
            // SAFETY: this is the sole event reader, with a live handle. Consume
            // the borrowed event before the next wait_event; no data escapes.
            let event = unsafe { &*(self.wait_event)(self.handle, 0.1) };
            if event.event_id != MPV_EVENT_NONE {
                eprintln!("mpv event={} error={}", event.event_id, event.error);
                if let Err(error) = self.check(event.error, "mpv event") {
                    eprintln!("{error}");
                }
            }
            if event.event_id == MPV_EVENT_FILE_LOADED {
                loaded = true;
                eprintln!("MPV_EVENT_FILE_LOADED");
            }
            if event.event_id == MPV_EVENT_SHUTDOWN {
                break;
            }
        }
        loaded
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: exclusive ownership, no event calls in flight, library still
        // loaded. With a VO, this runs on the worker while AppKit keeps pumping.
        // Startup errors destroy on main before any loadfile/VO exists.
        unsafe { (self.terminate_destroy)(self.handle) };
        eprintln!("mpv destroyed");
    }
}

fn run() -> Result<u8> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 || !matches!(args[1].as_str(), "wid" | "wid-gpu") {
        return Err("usage: m0-embed <wid|wid-gpu> (requires PROBE_OUT and M0_LIBMPV)".into());
    }
    let variant = &args[1];
    let out = PathBuf::from(std::env::var_os("PROBE_OUT").ok_or("missing PROBE_OUT")?);
    let library = PathBuf::from(std::env::var_os("M0_LIBMPV").ok_or("missing M0_LIBMPV")?);
    let mut event_loop = EventLoopBuilder::<bool>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title(format!("M0 {variant}"))
        .with_inner_size(LogicalSize::new(1000.0, 600.0))
        .with_transparent(true)
        .with_visible(false)
        .build(&event_loop)?;
    // Wry 0.55.1's `transparent` feature enables the macOS drawsBackground KVC
    // path; with_transparent alone is insufficient (see its transparent example).
    let webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(HTML)
        .build(&window)?;
    let video = insert_video_view(&webview, &out, variant)?;
    window.set_visible(true);
    webview.ns_window().makeKeyAndOrderFront(None);
    let deadline = Instant::now() + Duration::from_secs(14);
    let stop = Arc::new(AtomicBool::new(false));
    let mut started = false;
    let mut finished = false;
    let mut loaded = false;
    let mut closing = false;
    let mut worker = None;
    let proxy = event_loop.create_proxy();
    event_loop.run_return(|event, _, flow| {
        match event {
            Event::MainEventsCleared if !started => {
                started = true; // Main thread, shown window, active AppKit loop.
                match Mpv::new(&library, Retained::as_ptr(&video) as usize, variant, &out) {
                    Ok(mpv) => {
                        let stop = Arc::clone(&stop);
                        let proxy = proxy.clone();
                        worker = Some(thread::spawn(move || {
                            let loaded = mpv.watch(&stop, deadline);
                            // Do not block the main thread in terminate_destroy:
                            // macvk may synchronously dispatch AppKit cleanup there.
                            drop(mpv);
                            let _ = proxy.send_event(loaded);
                        }));
                    }
                    Err(error) => {
                        eprintln!("mpv startup: {error}");
                        finished = true; // Keep the HTML visible until capture/deadline.
                    }
                }
            }
            Event::UserEvent(saw_file_loaded) => {
                loaded = saw_file_loaded;
                finished = true;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => closing = true,
            _ => {}
        }
        if closing || Instant::now() >= deadline {
            stop.store(true, Ordering::Relaxed);
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
    // The main-thread view owners are released only after mpv has been destroyed.
    drop(webview);
    drop(video);
    drop(window);
    eprintln!("FILE_LOADED={loaded}");
    Ok(if loaded { 0 } else { 3 })
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("m0-embed: {error}");
            std::process::ExitCode::from(3)
        }
    }
}
