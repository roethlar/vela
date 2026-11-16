//! Windows-only W0: compare two mpv `wid` parents beneath a transparent WebView2.
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use libloading::Library;
use tao::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::windows::WindowExtWindows,
    window::WindowBuilder,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    Graphics::Gdi::{GetStockObject, BLACK_BRUSH},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, EnumChildWindows, GetClassNameW, GetClientRect, GetParent,
        GetWindow, GetWindowRect, IsWindowVisible, RegisterClassW, SetWindowPos, GW_HWNDFIRST,
        GW_HWNDNEXT, HWND_BOTTOM, SWP_NOACTIVATE, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE,
    },
};
use wry::WebViewBuilder;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><style>
html, body { background: transparent; margin: 0; height: 100%; overflow: hidden; }
body { font: 24px sans-serif; }
header, footer { position: fixed; left: 0; right: 0; display: flex;
                 align-items: center; padding: 0 24px; box-sizing: border-box; }
header { top: 0; height: 60px; background: rgba(0,0,0,0.55); color: white; }
footer { bottom: 0; height: 80px; background: #e08a00; color: black; }
</style></head><body><header>page text over the video</header>
<footer>opaque HTML bar</footer></body></html>"#;

// mpv/client.h: the event payload is borrowed until the next mpv_wait_event.
#[repr(C)]
struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

struct Mpv {
    handle: *mut c_void,
    set_option: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
    initialize: unsafe extern "C" fn(*mut c_void) -> c_int,
    command: unsafe extern "C" fn(*mut c_void, *const *const c_char) -> c_int,
    wait_event: unsafe extern "C" fn(*mut c_void, f64) -> *mut MpvEvent,
    event_name: unsafe extern "C" fn(c_int) -> *const c_char,
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    terminate_destroy: unsafe extern "C" fn(*mut c_void),
    _library: Library,
}

impl Mpv {
    fn new() -> Result<Self> {
        let path = std::env::current_exe()?.with_file_name("libmpv-2.dll");
        // SAFETY: load the probe's downloaded libmpv, with the public client.h ABI
        // below. The library stays owned until after the handle is destroyed.
        let library = unsafe { Library::new(path)? };
        macro_rules! symbol {
            ($name:literal) => {{
                // SAFETY: each inferred function type matches this client.h symbol;
                // copying its pointer is valid while `_library` remains loaded.
                unsafe { *library.get(concat!($name, "\0").as_bytes())? }
            }};
        }
        let create: unsafe extern "C" fn() -> *mut c_void = symbol!("mpv_create");
        let mut mpv = Self {
            handle: null_mut(),
            set_option: symbol!("mpv_set_option_string"),
            initialize: symbol!("mpv_initialize"),
            command: symbol!("mpv_command"),
            wait_event: symbol!("mpv_wait_event"),
            event_name: symbol!("mpv_event_name"),
            error_string: symbol!("mpv_error_string"),
            terminate_destroy: symbol!("mpv_terminate_destroy"),
            _library: library,
        };
        // SAFETY: no arguments; NULL is checked before using the new handle.
        mpv.handle = unsafe { create() };
        if mpv.handle.is_null() {
            return Err("mpv_create returned NULL".into());
        }
        Ok(mpv)
    }

    fn check(&self, action: &str, code: c_int) -> Result<()> {
        if code < 0 {
            // SAFETY: mpv_error_string accepts any code and returns a static C
            // string; the library remains loaded while it is copied here.
            let error = unsafe { CStr::from_ptr((self.error_string)(code)) };
            return Err(format!("{action}: {} ({code})", error.to_string_lossy()).into());
        }
        Ok(())
    }

    fn option(&self, name: &str, value: &str) -> Result<()> {
        let key = CString::new(name)?;
        let value = CString::new(value)?;
        // SAFETY: the handle is live and not initialized yet; both strings live
        // through the call, which copies them.
        let code = unsafe { (self.set_option)(self.handle, key.as_ptr(), value.as_ptr()) };
        self.check(name, code)
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: this worker exclusively owns the handle, all event reads
            // have ended, and the library and host window still exist. This also
            // handles initialization failure. The UI keeps pumping during teardown.
            unsafe { (self.terminate_destroy)(self.handle) };
        }
    }
}

fn play(
    wid: usize,
    variant: &str,
    out: &Path,
    stop: &AtomicBool,
    loaded: &AtomicBool,
) -> Result<()> {
    let mut events = OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join(format!("events-{variant}.log")))?;
    let mpv = Mpv::new()?;
    let wid = wid.to_string();
    let log = out.join(format!("mpv-{variant}.log"));
    for (name, value) in [
        ("wid", wid.as_str()),
        ("vo", "gpu-next"),
        ("gpu-api", "d3d11"),
        ("hwdec", "no"),
        ("idle", "yes"),
        ("keep-open", "yes"),
        ("osc", "no"),
        ("input-default-bindings", "no"),
        ("input-vo-keyboard", "no"),
        ("loop-file", "inf"),
        ("terminal", "yes"),
        ("msg-level", "all=v"),
        ("log-file", log.to_str().ok_or("log path is not UTF-8")?),
    ] {
        mpv.option(name, value)?;
    }
    // SAFETY: live, configured handle; initialize is called exactly once.
    mpv.check("mpv_initialize", unsafe { (mpv.initialize)(mpv.handle) })?;
    let args = [
        c"loadfile".as_ptr(),
        c"av://lavfi:testsrc2=size=1280x720:rate=30".as_ptr(),
        null(),
    ];
    // SAFETY: initialized handle and a NULL-terminated array of valid C strings;
    // mpv_command consumes/copies the arguments before returning.
    mpv.check("loadfile", unsafe {
        (mpv.command)(mpv.handle, args.as_ptr())
    })?;
    while !stop.load(Ordering::Relaxed) {
        // SAFETY: this is the sole event reader; the handle is live until Drop.
        let event = unsafe { (mpv.wait_event)(mpv.handle, 0.1) };
        if event.is_null() {
            return Err("mpv_wait_event returned NULL".into());
        }
        // SAFETY: checked non-NULL and copied before the next wait_event call.
        let id = unsafe { (*event).event_id };
        if id == 0 {
            continue; // MPV_EVENT_NONE is a timeout, not an event.
        }
        if id == 8 {
            loaded.store(true, Ordering::Relaxed); // MPV_EVENT_FILE_LOADED
        }
        // SAFETY: accepts an event ID; returns NULL or a static string owned by
        // the still-loaded library. No event payload is dereferenced.
        let name = unsafe { (mpv.event_name)(id) };
        if name.is_null() {
            writeln!(events, "unknown-event-{id}")?;
        } else {
            // SAFETY: non-NULL, NUL-terminated string returned by mpv_event_name.
            writeln!(
                events,
                "{}",
                unsafe { CStr::from_ptr(name) }.to_string_lossy()
            )?;
        }
        if id == 1 {
            break; // MPV_EVENT_SHUTDOWN
        }
    }
    Ok(()) // Drop calls mpv_terminate_destroy before this worker finishes.
}

fn fit_child(parent: HWND, child: HWND) -> io::Result<()> {
    let mut rect = RECT::default();
    // SAFETY: both windows belong to this UI thread and remain alive; rect is
    // writable. HWND_BOTTOM reorders only this child, without activating it.
    unsafe {
        if GetClientRect(parent, &mut rect) == 0
            || SetWindowPos(
                child,
                HWND_BOTTOM,
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOACTIVATE,
            ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn create_child(parent: HWND) -> io::Result<HWND> {
    let name: Vec<u16> = "W0_VIDEO_HOST\0".encode_utf16().collect();
    // SAFETY: NULL selects this executable's module; the class string lives
    // through registration/creation, DefWindowProcW has the WNDPROC ABI, and
    // BLACK_BRUSH is a process-independent stock brush (never deleted by us).
    let child = unsafe {
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(io::Error::last_os_error());
        }
        let class = WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            hInstance: instance,
            hbrBackground: GetStockObject(BLACK_BRUSH),
            lpszClassName: name.as_ptr(),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(io::Error::last_os_error());
        }
        CreateWindowExW(
            0,
            name.as_ptr(),
            name.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            parent,
            null_mut(),
            instance,
            null(),
        )
    };
    if child.is_null() {
        return Err(io::Error::last_os_error());
    }
    fit_child(parent, child)?;
    Ok(child) // Windows destroys this child with the tao parent.
}

unsafe extern "system" fn collect_window(hwnd: HWND, data: LPARAM) -> i32 {
    // SAFETY: dump_windows passes its exclusively borrowed Vec; EnumChildWindows
    // invokes callbacks synchronously and never retains this pointer.
    unsafe { (&mut *(data as *mut Vec<HWND>)).push(hwnd) };
    1
}

fn dump_windows(root: HWND, wid: HWND, path: &Path) -> io::Result<()> {
    let mut windows = vec![root];
    // SAFETY: root is live and the Vec remains exclusively available until the
    // synchronous enumeration (which includes all descendants) returns.
    unsafe { EnumChildWindows(root, Some(collect_window), &mut windows as *mut _ as LPARAM) };
    let mut file = File::create(path)?;
    writeln!(
        file,
        "tao_hwnd={:#x} mpv_wid={:#x}",
        root as usize, wid as usize
    )?;
    writeln!(
        file,
        "rect: screen pixels; z: sibling order, 0=top; live snapshot"
    )?;
    for hwnd in windows {
        let mut class = [0u16; 256];
        let mut rect = RECT::default();
        // SAFETY: the output buffers are valid and sized as passed. Descendants
        // may disappear; Win32 then returns failure, which the listing preserves.
        let (len, rect_ok, parent, visible, z) = unsafe {
            let len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
            let rect_ok = GetWindowRect(hwnd, &mut rect) != 0;
            let mut sibling = GetWindow(hwnd, GW_HWNDFIRST);
            let mut z = None;
            // Bound the walk in case another thread continuously changes z-order.
            for index in 0..10_000 {
                if sibling.is_null() {
                    break;
                }
                if sibling == hwnd {
                    z = Some(index);
                    break;
                }
                sibling = GetWindow(sibling, GW_HWNDNEXT);
            }
            (len, rect_ok, GetParent(hwnd), IsWindowVisible(hwnd) != 0, z)
        };
        let class = String::from_utf16_lossy(&class[..len as usize]);
        let rect = if rect_ok {
            format!(
                "[{},{},{},{}]",
                rect.left, rect.top, rect.right, rect.bottom
            )
        } else {
            "unavailable".into()
        };
        writeln!(
            file,
            "hwnd={:#x} parent={:#x} class={class:?} rect={rect} visible={visible} z={}",
            hwnd as usize,
            parent as usize,
            z.map_or_else(|| "unavailable".into(), |z| z.to_string()),
        )?;
    }
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let variant = match args.as_slice() {
        [arg] if arg == "toplevel" => "toplevel",
        [arg] if arg == "child" => "child",
        _ => return Err("usage: w0-embed <toplevel|child>".into()),
    };
    let out =
        std::path::PathBuf::from(std::env::var_os("PROBE_OUT").ok_or("PROBE_OUT is required")?);
    std::fs::create_dir_all(&out)?;
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(format!("W0 {variant}"))
        .with_inner_size(PhysicalSize::new(1000, 600))
        .with_position(PhysicalPosition::new(0, 0))
        .with_transparent(true)
        .with_visible(false)
        .build(&event_loop)?;
    // wry 0.55.1 docs + webview2/mod.rs: transparent=true calls
    // ICoreWebView2Controller2::SetDefaultBackgroundColor with RGBA (0,0,0,0).
    // with_background_color would be ignored; build(&window) auto-resizes on Windows.
    let webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(HTML)
        .build(&window)?;
    let root = window.hwnd() as HWND;
    let child = if variant == "child" {
        Some(create_child(root)?)
    } else {
        None
    };
    let wid = child.unwrap_or(root);
    window.set_visible(true);
    window.set_focus();
    let started = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    let loaded = Arc::new(AtomicBool::new(false));
    let mut worker: Option<JoinHandle<()>> = None;
    let mut dumped = false;
    let mut closing = false;

    event_loop.run(move |event, _, flow| {
        // Keep both UI owners alive until mpv has completely terminated.
        let _keep_alive = (&window, &webview);
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50));
        match event {
            Event::NewEvents(StartCause::Init) => {
                let (out, stop, loaded) = (out.clone(), stop.clone(), loaded.clone());
                // Only an integer window ID crosses threads. The library, handle,
                // event reads and destruction all stay on this single worker.
                let wid = wid as usize;
                worker = Some(thread::spawn(move || {
                    if let Err(error) = play(wid, variant, &out, &stop, &loaded) {
                        eprintln!("mpv {variant}: {error}");
                    }
                }));
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    if let Some(child) = child {
                        if let Err(error) = fit_child(root, child) {
                            eprintln!("resize child: {error}");
                        }
                    }
                }
                WindowEvent::CloseRequested => closing = true,
                _ => {}
            },
            Event::LoopDestroyed => return,
            _ => {}
        }
        if !dumped && started.elapsed() >= Duration::from_secs(3) {
            if let Err(error) = dump_windows(root, wid, &out.join(format!("windows-{variant}.txt")))
            {
                eprintln!("window listing: {error}");
            }
            dumped = true;
        }
        if closing || started.elapsed() >= Duration::from_secs(14) {
            stop.store(true, Ordering::Relaxed);
            // Do not join a running worker: mpv's teardown may need this UI to
            // service window messages. Keep pumping until Drop has finished.
            if worker.as_ref().is_none_or(JoinHandle::is_finished) {
                if let Some(worker) = worker.take() {
                    if worker.join().is_err() {
                        eprintln!("mpv worker panicked");
                    }
                }
                *flow =
                    ControlFlow::ExitWithCode(if loaded.load(Ordering::Relaxed) { 0 } else { 3 });
            }
        }
    })
}

fn main() {
    if let Err(error) = run() {
        eprintln!("W0: {error}");
        std::process::exit(3);
    }
}
