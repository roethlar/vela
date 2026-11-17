"""Probe M2 only: give a debug Vela a way to run probes/m2-vela/drive.js, and
count what the macOS embedded player's layer does.

Edits the probe's own checkout on the runner, never the repository:
- src-tauri/src/lib.rs: with VELA_PROBE_SCRIPT naming a file, the app
  evaluates it in the main window's page three seconds after start-up and
  prints every "vela-probe" event the page emits as "probe: <payload>" on
  stderr. The page may emit events under Vela's default capability.
- src-tauri/src/player/embedded/macos_gl.rs and macos_host.rs: "probe-gl:"
  and "probe-host:" lines on stderr: Core Animation's canDraw and draw calls
  (sizes, framebuffer, the colour hold), and each placement of the video view.

usage: hook.py <checkout root>
"""
import pathlib
import sys

root = pathlib.Path(sys.argv[1])


def patch(rel, pairs):
    path = root / rel
    source = path.read_text()
    for old, new in pairs:
        assert source.count(old) == 1, f"{rel}: an anchor moved; update probes/m2-vela/hook.py: {old[:60]!r}"
        source = source.replace(old, new)
    path.write_text(source)
    print("probe hook:", rel)


patch("src-tauri/src/lib.rs", [(
"""        .setup(move |app| {
            use tauri::Manager;
""",
"""        .setup(move |app| {
            use tauri::Manager;

            // PROBE M2 ONLY (probes/m2-vela/hook.py); never committed.
            if let Some(script) = std::env::var_os("VELA_PROBE_SCRIPT")
                .and_then(|path| std::fs::read_to_string(path).ok())
            {
                use tauri::Listener;
                app.handle().listen_any("vela-probe", |event| {
                    eprintln!("probe: {}", event.payload());
                });
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    match handle.get_webview_window("main") {
                        Some(window) => {
                            if let Err(error) = window.eval(script.as_str()) {
                                eprintln!("probe: eval failed: {error}");
                            }
                        }
                        None => eprintln!("probe: no main window"),
                    }
                });
            }
""")])

COUNTER = "static PROBE_{0}: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);"

patch("src-tauri/src/player/embedded/macos_gl.rs", [
("""        // Latch update: CA may ask more than once before draw consumes it.
        let frame = renderer.0.has_new_frame();
        drawing.frame.update(frame);
        Ok(drawing.frame.pending() || self.signals.redraw.load(Ordering::Acquire))""",
"""        // Latch update: CA may ask more than once before draw consumes it.
        let frame = renderer.0.has_new_frame();
        drawing.frame.update(frame);
        let ready = drawing.frame.pending() || self.signals.redraw.load(Ordering::Acquire);
        {
            """ + COUNTER.format("CAN") + """
            let n = PROBE_CAN.fetch_add(1, Ordering::Relaxed) + 1;
            if n <= 6 || n % 40 == 0 {
                eprintln!("probe-gl: canDraw #{n} frame={frame} ready={ready}");
            }
        }
        Ok(ready)"""),
("""        let redraw = self.signals.redraw.swap(false, Ordering::AcqRel);
        if !drawing.frame.pending() && !redraw {
            return Ok(());
        }""",
"""        let redraw = self.signals.redraw.swap(false, Ordering::AcqRel);
        {
            """ + COUNTER.format("DRAW") + """
            let n = PROBE_DRAW.fetch_add(1, Ordering::Relaxed) + 1;
            if n <= 6 || n % 20 == 0 {
                eprintln!("probe-gl: draw call #{n} frame={frame} pending={} redraw={redraw}", drawing.frame.pending());
            }
        }
        if !drawing.frame.pending() && !redraw {
            return Ok(());
        }"""),
("""        if width <= 0 || height <= 0 {
            return Ok(());
        }
        // CA owns the FBO.""",
"""        {
            """ + COUNTER.format("SIZE") + """
            let n = PROBE_SIZE.fetch_add(1, Ordering::Relaxed) + 1;
            if n <= 6 || n % 20 == 0 {
                eprintln!(
                    "probe-gl: render #{n} fbo={fbo} last_fbo={} viewport={viewport:?} bounds={}x{} scale={scale} -> {width}x{height} held={}",
                    drawing.last_fbo, bounds.size.width, bounds.size.height, self.signals.held.load(Ordering::Acquire)
                );
            }
        }
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        // CA owns the FBO."""),
])

patch("src-tauri/src/player/embedded/macos_host.rs", [
("""        self.video.setFrame(in_parent);
        self.video
            .setHidden(frame.backing_width <= 0 || frame.backing_height <= 0);""",
"""        self.video.setFrame(in_parent);
        self.video
            .setHidden(frame.backing_width <= 0 || frame.backing_height <= 0);
        eprintln!(
            "probe-host: page bounds={}x{} flipped={} scale={scale} reported={reported:?} placement={frame:?} in_parent=({}, {}, {}x{}) hidden={}",
            bounds.size.width, bounds.size.height, self.page.isFlipped(),
            in_parent.origin.x, in_parent.origin.y, in_parent.size.width, in_parent.size.height,
            self.video.isHidden()
        );"""),
])

# The display chain on macOS, link by link: mpv's update callbacks, the
# requests that queue a main-thread task (and those coalesced), each task with
# whether it found its play's view, each display_now, each skip render.
patch("src-tauri/src/player/embedded/macos_gl.rs", [
("""    let target = unsafe { &*context.cast::<PlacementTarget>() };
    target.request_display();""",
"""    let target = unsafe { &*context.cast::<PlacementTarget>() };
    {
        """ + COUNTER.format("CB") + """
        let n = PROBE_CB.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if n <= 5 || n % 10 == 0 {
            eprintln!("probe-chain: mpv update callback #{n}");
        }
    }
    target.request_display();"""),
("""        self.display();
        CATransaction::flush();""",
"""        {
            """ + COUNTER.format("DN") + """
            let n = PROBE_DN.fetch_add(1, Ordering::Relaxed) + 1;
            if n <= 5 || n % 10 == 0 {
                eprintln!("probe-chain: display_now #{n} asynchronous={}", self.isAsynchronous());
            }
        }
        self.display();
        CATransaction::flush();"""),
("""        if drawing.frame.needs_skip() {""",
"""        if drawing.frame.needs_skip() {
            {
                """ + COUNTER.format("SKIP") + """
                let n = PROBE_SKIP.fetch_add(1, Ordering::Relaxed) + 1;
                if n <= 5 || n % 10 == 0 {
                    eprintln!("probe-chain: skip render #{n}");
                }
            }"""),
])

patch("src-tauri/src/player/embedded/macos_host.rs", [
("""    pub(super) fn request_display(&self) {
        if !self.display_queue.request() {
            return;
        }""",
"""    pub(super) fn request_display(&self) {
        if !self.display_queue.request() {
            """ + COUNTER.format("COAL") + """
            let n = PROBE_COAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if n <= 5 || n % 10 == 0 {
                eprintln!("probe-chain: request coalesced #{n}");
            }
            return;
        }
        {
            """ + COUNTER.format("REQ") + """
            let n = PROBE_REQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if n <= 5 || n % 10 == 0 {
                eprintln!("probe-chain: request queued #{n}");
            }
        }"""),
("""            queue.begin();
            VIEWS.with_borrow(|views| {
                if let Some(view) = views.get(&id) {""",
"""            queue.begin();
            VIEWS.with_borrow(|views| {
                {
                    """ + COUNTER.format("TASK") + """
                    let n = PROBE_TASK.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if n <= 5 || n % 10 == 0 {
                        eprintln!("probe-chain: main task #{n} id={id} found={}", views.contains_key(&id));
                    }
                }
                if let Some(view) = views.get(&id) {"""),
])

# Bisection variants (probes/m2-nofmt, probes/m2-nocs set M2_VARIANT): leave
# out the layer's contents format, or its colour space and EDR flag, which
# the HDR slice added and probe M1's layer (which showed frames) never set.
import os
variants = set(filter(None, os.environ.get("M2_VARIANT", "").split(",")))
if "nofmt" in variants:
    patch("src-tauri/src/player/embedded/macos_gl.rs", [(
"""        layer.setContentsFormat(unsafe {
            if float {
                kCAContentsFormatRGBA16Float
            } else {
                kCAContentsFormatRGBA8Uint
            }
        });""",
"""        let _ = (float, unsafe { (kCAContentsFormatRGBA16Float, kCAContentsFormatRGBA8Uint) });
        eprintln!("probe-variant: no contents format");""")])
if "red" in variants:
    # Clear the layer's framebuffer to solid red after every draw: red on
    # screen means Core Animation presents what Vela draws (so a black
    # picture would be mpv's output); black means it does not.
    patch("src-tauri/src/player/embedded/macos_gl.rs", [
("""        if draws_black(held_at_start, held) {""",
"""        if draws_black(held_at_start, held) || true {"""),
("""                (self.gl.clear_color)(0.0, 0.0, 0.0, 1.0);""",
"""                (self.gl.clear_color)(1.0, 0.0, 0.0, 1.0);"""),
])
if "nocs" in variants:
    patch("src-tauri/src/player/embedded/macos_gl.rs", [(
"""        self.setColorspace(Some(&space));
        self.setWantsExtendedDynamicRangeContent(range != Range::Sdr);""",
"""        let _ = (&space, range);
        eprintln!("probe-variant: no colour space or EDR flag");""")])

# Why a settings or connections file is "unavailable" at start (the page shows
# only a generic sentence), and, on Windows, the ACL the privacy check read
# back (probe W2 found Vela refusing its own settings on a hosted runner).
patch("src-tauri/src/durable.rs", [(
"""fn status_for_error(error: &io::Error, layout: DurableLayout) -> DurableFileStatus {
    DurableFileStatus {""",
"""fn status_for_error(error: &io::Error, layout: DurableLayout) -> DurableFileStatus {
    eprintln!("probe-durable: {:?}: {error}", error.kind());
    DurableFileStatus {""")])

patch("src-tauri/src/storage.rs", [(
"""        let descriptor = read_security_descriptor(path)?;
        let (protected, aces) = inspect_dacl(&descriptor, user.sid())?;""",
"""        let descriptor = read_security_descriptor(path)?;
        {
            use windows::Win32::Security::Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW;
            let sid = current_user_sid()?;
            let mut text = PWSTR::null();
            // SAFETY: the descriptor is live and text receives a LocalAlloc string.
            unsafe {
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor.0,
                    SDDL_REVISION_1,
                    DACL_SECURITY_INFORMATION,
                    &mut text,
                    None,
                )
            }.map_err(|error| io_error("probe could not encode Windows ACL", error))?;
            // SAFETY: the conversion returned a NUL-terminated LocalAlloc string.
            let actual = unsafe { text.to_string() };
            unsafe { LocalFree(Some(HLOCAL(text.as_ptr().cast::<c_void>()))) };
            let actual = actual.map_err(|error| io_error("probe got invalid Windows ACL text", error))?;
            eprintln!("probe-acl: {} sid={sid} dacl={actual}", path.display());
        }
        let (protected, aces) = inspect_dacl(&descriptor, user.sid())?;""")])
