// External-mpv playback. mpv owns its own Vulkan/Wayland window, which is the
// reliable way to get true HDR passthrough (10-bit PQ/BT.2020 negotiated with
// the compositor). Position is read over mpv's JSON IPC socket and reported back
// to the originating server (Plex or Jellyfin/Emby) so resume works.

use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// mpv's JSON IPC channel is a Unix domain socket on Linux/macOS and an emulated
// named pipe on Windows. Both expose the same byte-stream Read+Write interface
// with a `try_clone`, so the tracker code below is platform-agnostic once we pick
// the right connector.
#[cfg(unix)]
type IpcStream = std::os::unix::net::UnixStream;
#[cfg(windows)]
type IpcStream = std::fs::File;

#[cfg(unix)]
fn ipc_connect(path: &str) -> std::io::Result<IpcStream> {
    std::os::unix::net::UnixStream::connect(path)
}
#[cfg(windows)]
fn ipc_connect(path: &str) -> std::io::Result<IpcStream> {
    // A Windows named pipe can be opened like a file for duplex read/write.
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
}

/// Path for mpv's JSON IPC socket. Lives in a process-private, owner-only (0700)
/// directory with an unpredictable name, so a local user can't pre-create
/// (symlink) the socket the way they could in world-writable `/tmp`. The base
/// stays under `/tmp` so the full path keeps well under the unix socket-path
/// limit (notably macOS's 104 bytes), unlike the long per-user config dir.
#[cfg(not(windows))]
fn ipc_socket_path() -> Result<String, String> {
    use std::sync::OnceLock;
    // Cache the *result* of the one-time creation: the path is random per
    // process and the directory is created exactly once, so a creation failure
    // must stay failed (fail closed) rather than be silently retried.
    static DIR: OnceLock<Result<std::path::PathBuf, String>> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let d = std::path::PathBuf::from(format!(
            "/tmp/vela-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        ));
        // Create the leaf NON-recursively and 0700 from the start: non-recursive
        // gives O_EXCL semantics, so if anything already exists at this path (a
        // symlink or a directory an attacker pre-created to redirect mpv's IPC
        // socket) creation fails and we fail closed — we never adopt a path we
        // didn't exclusively create. 0700-at-creation leaves no world-traversable
        // window. Errors are propagated, not discarded.
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&d)
                .map_err(|e| format!("couldn't create a private IPC directory: {e}"))?;
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir(&d)
                .map_err(|e| format!("couldn't create the IPC directory: {e}"))?;
        }
        Ok(d)
    });
    let dir = dir.as_ref().map_err(|e| e.clone())?;
    // Defense in depth: before each use, confirm it's still a real directory
    // (symlink_metadata, so a symlink swapped in later is rejected — is_dir() is
    // false for a symlink) and still private.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
        if !meta.is_dir() {
            return Err("IPC path is not a directory".to_string());
        }
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("IPC directory is not private (expected mode 0700)".to_string());
        }
    }
    #[cfg(not(unix))]
    {
        if !dir.is_dir() {
            return Err("IPC path is not a directory".to_string());
        }
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Ok(dir
        .join(format!("mpv-{}-{}.sock", std::process::id(), ts))
        .to_string_lossy()
        .into_owned())
}

/// Locate the mpv executable. We can't rely on bare `mpv` resolving via `PATH`:
/// GUI apps launched from Finder/Explorer get a minimal `PATH` that excludes
/// Homebrew (`/opt/homebrew/bin`), scoop, winget shims, etc. So we try, in order:
///   1. the user's explicit path from the Settings page (`mpv_path`),
///   2. a copy shipped alongside the app (next to our own exe),
///   3. bare `mpv` on `PATH`,
///   4. a generous list of real-world install locations per OS.
///
/// Returns the command to run (a bare name if found on `PATH`, otherwise an
/// absolute path).
pub fn resolve_mpv() -> Option<String> {
    // 1. Explicit override the user set in Settings → mpv player. Honored only
    //    if it actually resolves to a runnable file (validated on save too).
    if let Some(p) = crate::config::load_config().ok().and_then(|c| c.mpv_path) {
        let p = p.trim().to_string();
        if !p.is_empty() && mpv_usable(&p) {
            return Some(p);
        }
    }
    // 2. A bundled copy next to the app executable (zero-config "just works").
    if let Some(p) = bundled_mpv().filter(|p| mpv_usable(p)) {
        return Some(p);
    }
    // 3. Bare `mpv` on PATH.
    if mpv_runs("mpv") {
        return Some("mpv".to_string());
    }
    // 4. Known install locations.
    mpv_candidates().into_iter().find(|cand| mpv_usable(cand))
}

/// True if `bin` (a bare name or path) is a *working* mpv: it must run
/// `--version` and exit cleanly (status 0). We check `success()` rather than
/// merely "the process started", because an mpv built for CPU features this
/// machine lacks (e.g. an AVX2 build on a pre-Haswell CPU) launches but crashes
/// immediately with an illegal-instruction exit — and that crash must count as
/// "not usable", or we'd accept a build that can never actually play video.
fn mpv_runs(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Validate a user-supplied mpv path: it must exist as a file and actually run.
/// Used both by `resolve_mpv` (config/env override) and by the `set_mpv_path`
/// command so a saved override is known-good rather than a typo.
pub fn mpv_usable(path: &str) -> bool {
    let p = std::path::Path::new(path);
    p.is_file() && mpv_runs(path)
}

/// Split the user's advanced mpv options (Settings → Advanced mpv options) into
/// argv. One option per line — blank lines and `#` comments ignored — so there's no
/// shell-quoting ambiguity around values that contain spaces (paths, filter graphs).
fn parse_extra_mpv_args(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Look for an mpv shipped alongside our own executable — e.g. a packager (or a
/// future bundled installer) dropped `mpv.exe` next to the app or in an `mpv/`
/// subfolder. Resolved relative to the running binary so it works from any
/// install location.
fn bundled_mpv() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    #[cfg(windows)]
    let names = ["mpv.exe", r"mpv\mpv.exe"];
    #[cfg(not(windows))]
    let names = ["mpv", "mpv/mpv"];
    for name in names {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand.to_string_lossy().into_owned());
        }
    }
    None
}

/// Common absolute install locations to probe when `mpv` isn't on `PATH`.
fn mpv_candidates() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        let mut v = vec![
            "/opt/homebrew/bin/mpv".into(),          // Apple Silicon Homebrew
            "/usr/local/bin/mpv".into(),             // Intel Homebrew
            "/opt/local/bin/mpv".into(),             // MacPorts
            "/run/current-system/sw/bin/mpv".into(), // NixOS/nix-darwin
            "/nix/var/nix/profiles/default/bin/mpv".into(),
        ];
        if let Ok(home) = std::env::var("HOME") {
            v.push(format!("{home}/.nix-profile/bin/mpv"));
        }
        v
    }
    #[cfg(target_os = "windows")]
    {
        let mut v = Vec::new();
        if let Ok(home) = std::env::var("USERPROFILE") {
            // scoop: a shim on PATH normally, plus the real binary under apps/.
            v.push(format!(r"{home}\scoop\shims\mpv.exe"));
            v.push(format!(r"{home}\scoop\apps\mpv\current\mpv.exe"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            // winget drops a per-user shim under Links, and the real payload under
            // Packages\<id>\... — scan the latter since the id/version vary.
            v.push(format!(r"{local}\Microsoft\WinGet\Links\mpv.exe"));
            v.extend(scan_winget_packages(&format!(
                r"{local}\Microsoft\WinGet\Packages"
            )));
            v.push(format!(r"{local}\Programs\mpv\mpv.exe"));
        }
        if let Ok(pf) = std::env::var("ProgramFiles") {
            v.push(format!(r"{pf}\mpv\mpv.exe"));
        }
        if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
            v.push(format!(r"{pf86}\mpv\mpv.exe"));
        }
        if let Ok(pd) = std::env::var("ProgramData") {
            // chocolatey shim.
            v.push(format!(r"{pd}\chocolatey\bin\mpv.exe"));
        }
        v
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut v = vec![
            "/usr/bin/mpv".into(),
            "/bin/mpv".into(),
            "/usr/local/bin/mpv".into(),
            "/home/linuxbrew/.linuxbrew/bin/mpv".into(),
            "/snap/bin/mpv".into(),
            "/var/lib/flatpak/exports/bin/io.mpv.Mpv".into(),
            "/run/current-system/sw/bin/mpv".into(),
            "/nix/var/nix/profiles/default/bin/mpv".into(),
        ];
        if let Ok(home) = std::env::var("HOME") {
            v.push(format!(
                "{home}/.local/share/flatpak/exports/bin/io.mpv.Mpv"
            ));
            v.push(format!("{home}/.nix-profile/bin/mpv"));
        }
        v
    }
}

/// Scan a winget `Packages` directory for `mpv.exe`. winget names each package
/// folder `<PackageId>_<hash>` and lays the payload out flat inside, so we check
/// each immediate subdirectory (and one nested level, since some packages nest a
/// versioned folder) for the binary. Best-effort: returns whatever it finds.
#[cfg(target_os = "windows")]
fn scan_winget_packages(root: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        // Only look at packages that look like mpv to avoid walking everything.
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.contains("mpv") {
            continue;
        }
        let direct = dir.join("mpv.exe");
        if direct.is_file() {
            out.push(direct.to_string_lossy().into_owned());
        }
        // One level deeper (e.g. an extracted versioned subfolder).
        if let Ok(sub) = std::fs::read_dir(&dir) {
            for s in sub.flatten() {
                let cand = s.path().join("mpv.exe");
                if cand.is_file() {
                    out.push(cand.to_string_lossy().into_owned());
                }
            }
        }
    }
    out
}

/// Everything the progress tracker needs to report back to Plex.
#[derive(Clone)]
pub struct TrackInfo {
    pub server_base: String,
    pub token: String,
    pub client_identifier: String,
    pub rating_key: String,
    pub key: String,
    pub duration_ms: u64,
}

/// What the Jellyfin/Emby progress tracker needs. Auth is pre-built into
/// `headers` by the source so this layer stays agnostic to the (diverging)
/// Jellyfin-vs-Emby header schemes. `media_source_id` and `play_session_id`
/// come from the playback-info negotiation so the server's check-in model
/// (history/dashboard/resume) ties the events to the right session.
#[derive(Clone)]
pub struct JellyfinTrack {
    pub base_url: String,
    pub item_id: String,
    pub media_source_id: String,
    pub play_session_id: Option<String>,
    pub headers: Vec<(String, String)>,
}

/// Where playback progress should be reported. Each media source supplies the
/// variant it knows how to report against; new backends add their own variants.
pub enum ProgressTarget {
    /// Report position to a Plex server's timeline (enables cross-session resume).
    Plex(TrackInfo),
    /// Report position to a Jellyfin/Emby server's playback-progress endpoints.
    Jellyfin(JellyfinTrack),
    /// No progress reporting.
    None,
}

/// Spawn mpv for `url`, optionally seeking to `start_seconds`, and start
/// background progress reporting that runs until mpv exits. Publishes the child
/// into `child_slot` the instant it's launched (so an app-exit that races tracker
/// setup can still find and kill it), and returns the stop flag so a later play
/// can cancel this one. The caller must have already cleared/killed any previous
/// child, since this overwrites the slot.
pub fn play(
    url: &str,
    start_seconds: f64,
    progress: ProgressTarget,
    child_slot: &Arc<Mutex<Option<std::process::Child>>>,
    shutting_down: &Arc<AtomicBool>,
    advance: &Arc<tokio::sync::Notify>,
) -> Result<Arc<AtomicBool>, String> {
    // mpv emulates the IPC socket with a named pipe under \\.\pipe\ on Windows.
    #[cfg(windows)]
    let ipc_path = {
        let pid = std::process::id();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!(r"\\.\pipe\mpv-plex-{}-{}", pid, ts)
    };
    #[cfg(not(windows))]
    let ipc_path = ipc_socket_path()?;

    let mpv_bin =
        resolve_mpv().ok_or_else(|| "mpv was not found. Install mpv to play video.".to_string())?;

    // Advanced/user mpv config (Settings → Advanced mpv options), loaded once here.
    let cfg = crate::config::load_config().unwrap_or_default();
    let use_own_config = cfg.mpv_use_own_config.unwrap_or(false);

    let mut cmd = Command::new(&mpv_bin);
    // Reproducible launch by default: ignore the user's mpv.conf unless they opted
    // in via "Use my own mpv config".
    if !use_own_config {
        cmd.arg("--no-config");
    }
    cmd.arg("--no-ytdl")
        .arg("--vo=gpu-next,gpu") // gpu-next first: best HDR passthrough
        .arg("--profile=gpu-hq")
        .arg("--hwdec=auto")
        .arg("--hwdec-codecs=all")
        // HDR: let mpv negotiate the display colorspace (PQ/BT.2020) with the
        // compositor (Wayland) or the OS (macOS EDR).
        .arg("--target-colorspace-hint=yes")
        .arg("--hdr-compute-peak=yes");

    // GPU API/context selection is platform-specific.
    //   * Linux: Vulkan via the Wayland/X11 WSI is the reliable HDR path.
    //   * macOS: mpv only exposes Vulkan over Metal (macvk), so let it auto-pick.
    //   * Windows: D3D11 negotiates HDR through the DXGI swapchain.
    #[cfg(target_os = "macos")]
    {
        cmd.arg("--gpu-api=vulkan"); // macvk (Metal-backed) is the only context on macOS
    }
    #[cfg(target_os = "windows")]
    {
        cmd.arg("--gpu-api=d3d11"); // HDR via the DXGI swapchain on Windows 10/11
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Force Vulkan (the HDR-capable backend, always built into modern mpv)
        // but let mpv auto-pick the context. Naming specific backends (e.g.
        // x11vk) risks listing one a given build lacks — mpv validates option
        // values up front and would exit after spawn() succeeded (silent failure).
        cmd.arg("--gpu-api=vulkan");
        // Enables the experimental Vulkan HDR WSI layer where present (KDE/NVIDIA).
        cmd.env("ENABLE_HDR_WSI", "1");
    }

    // User-supplied advanced options. Appended AFTER our render defaults so they
    // override them (mpv applies the last value of a repeated option), but BEFORE
    // the IPC server, resume seek, and `--`/URL below — those are re-asserted next so
    // a user option can't clobber the socket Vela needs for progress/resume/
    // auto-advance, or feed the media URL to mpv as an option.
    if let Some(extra) = cfg.mpv_extra_args.as_deref() {
        for arg in parse_extra_mpv_args(extra) {
            cmd.arg(arg);
        }
    }

    cmd.arg(format!("--input-ipc-server={}", ipc_path));
    if start_seconds > 0.0 {
        cmd.arg(format!("--start={}", start_seconds));
    }
    // `--` terminates option parsing so a URL/path that begins with `-` (e.g. a
    // hostile filename from a local folder or server) can't be read as an mpv
    // option. In practice our URLs are absolute paths / http(s) / edl://, but
    // this closes the option-injection vector regardless.
    cmd.arg("--").arg(url);

    // Launch mpv and publish it into the shared slot while still holding the
    // lock, so launch+register is atomic w.r.t. the app-exit handler: it can
    // never observe a launched-but-unregistered child and orphan it. The caller
    // has already killed any prior child, so the slot is expected to be empty.
    {
        let mut slot = child_slot.lock().unwrap_or_else(|e| e.into_inner());
        // Don't launch into a shutting-down app. The exit handler sets this flag
        // and sweeps the slot under THIS lock, so checking it here (still holding
        // the lock) closes the race where a play starting during exit would
        // register an mpv the sweep had already passed.
        if shutting_down.load(Ordering::SeqCst) {
            return Err("Vela is shutting down.".to_string());
        }
        let child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "mpv was not found. Install mpv to play video.".to_string()
            } else {
                format!("failed to launch mpv: {}", e)
            }
        })?;
        *slot = Some(child);
    }

    // Seed the tracked position with the resume point, so if mpv exits before
    // IPC reports a time-pos the final check-in reports the resume position
    // rather than 0 (which would clobber an existing resume point).
    let start_ms = (start_seconds.max(0.0) * 1000.0) as u64;
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Watch mpv's IPC for a clean EOF — that's the signal to auto-advance the
    // queue. We do NOT advance on user-close (mpv reports reason=quit/stop),
    // only on reason=eof, so closing the window stops playback while letting a
    // file play to the end roll into the next item. Always-on regardless of
    // progress target (works for local playback too). Failure to spawn the
    // watcher is non-fatal — playback still works, just no auto-advance.
    if let Err(e) = spawn_eof_watcher(ipc_path.clone(), stop_flag.clone(), advance.clone()) {
        eprintln!("vela: couldn't spawn mpv EOF watcher: {e}");
    }

    let tracking = match progress {
        ProgressTarget::Plex(info) => {
            start_tracking_plex(ipc_path, info, start_ms, stop_flag.clone())
        }
        ProgressTarget::Jellyfin(track) => {
            start_tracking_jellyfin(ipc_path, track, start_ms, stop_flag.clone())
        }
        ProgressTarget::None => Ok(()),
    };
    if let Err(e) = tracking {
        // mpv launched but we couldn't spawn its tracker threads (e.g. the OS is
        // out of threads). Stop any IPC reader that did start, then kill the child
        // but LEAVE it in the slot: the periodic reaper try_wait()s it (and a
        // later play / app-exit would also reap or kill it). Spawning our own
        // reaper thread here could fail under the very thread exhaustion that
        // caused this, dropping the child unreaped as a zombie; and a synchronous
        // wait() could block play() forever under play_lock if mpv wedged opening
        // a file on a hung mount before honoring kill().
        stop_flag.store(true, Ordering::Relaxed);
        if let Some(child) = child_slot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            let _ = child.kill();
        }
        return Err(format!("couldn't start playback tracking: {e}"));
    }
    Ok(stop_flag)
}

/// Spawn the IPC reader thread. It connects to mpv's JSON IPC socket, observes
/// `time-pos`, and continuously drains the socket (so mpv never blocks on a full
/// Tiny IPC listener that signals the queue dispatcher when mpv finishes a file
/// CLEANLY (mpv emits `event=end-file, reason=eof`). User-closed (`quit`),
/// errored (`error`), or otherwise-stopped exits never fire the notifier — so
/// closing the player stops playback, while watching to the end auto-advances.
/// Separate from the progress reader so it runs for every backend, including
/// local files where no progress tracker is active.
fn spawn_eof_watcher(
    socket_path: String,
    stop_flag: Arc<AtomicBool>,
    advance: Arc<tokio::sync::Notify>,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("mpv-eof-watcher".into())
        .spawn(move || {
            // Brief connect retries: mpv may not have opened the socket yet.
            let mut stream = None;
            for _ in 0..50 {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                match ipc_connect(&socket_path) {
                    Ok(s) => {
                        stream = Some(s);
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            let Some(stream) = stream else { return };
            let reader = BufReader::new(stream);
            for line in reader.lines() {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                let Ok(line) = line else { return };
                // mpv emits one JSON event per line. Substring match is fine here
                // (the event/reason keys are fixed, no escaping concerns).
                if line.contains("\"event\":\"end-file\"") && line.contains("\"reason\":\"eof\"") {
                    advance.notify_one();
                }
            }
        })?;
    Ok(())
}

/// send buffer), publishing the latest position into the returned counter. The
/// `done` flag is set when mpv exits or the connection drops. Does NO network I/O,
/// so it's shared by every backend's poster.
fn spawn_position_reader(
    socket_path: String,
    initial_ms: u64,
    stop_flag: Arc<AtomicBool>,
) -> std::io::Result<(Arc<AtomicU64>, Arc<AtomicBool>)> {
    let last_t_ms = Arc::new(AtomicU64::new(initial_ms));
    let done = Arc::new(AtomicBool::new(false));
    let last_t_ms_r = last_t_ms.clone();
    let done_r = done.clone();
    std::thread::Builder::new()
        .name("mpv-ipc-reader".into())
        .spawn(move || {
            let mut stream = None;
            for _ in 0..50 {
                if stop_flag.load(Ordering::Relaxed) {
                    done_r.store(true, Ordering::Relaxed);
                    return;
                }
                match ipc_connect(&socket_path) {
                    Ok(s) => {
                        stream = Some(s);
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            let mut stream = match stream {
                Some(s) => s,
                None => {
                    done_r.store(true, Ordering::Relaxed);
                    return;
                }
            };
            let reader_clone = match stream.try_clone() {
                Ok(s) => s,
                Err(_) => {
                    done_r.store(true, Ordering::Relaxed);
                    return;
                }
            };
            let mut reader = BufReader::new(reader_clone);

            // Ask mpv to push time-pos updates; then just drain every line.
            let _ = stream.write_all(b"{\"command\":[\"observe_property\",1,\"time-pos\"]}\n");

            let mut line = String::new();
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break, // EOF: mpv exited
                    Ok(_) => {}
                    Err(_) => break,
                }
                // Update position from either a property-change event or a response.
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(d) = v.get("data").and_then(|d| d.as_f64()) {
                        if d >= 0.0 {
                            last_t_ms_r.store((d * 1000.0) as u64, Ordering::Relaxed);
                        }
                    }
                }
            }
            done_r.store(true, Ordering::Relaxed);
        })?;
    Ok((last_t_ms, done))
}

/// Sleep until the next ~5s report tick, returning false if playback ended.
fn wait_tick(stop_flag: &AtomicBool, done: &AtomicBool) -> bool {
    for _ in 0..50 {
        if stop_flag.load(Ordering::Relaxed) || done.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !(stop_flag.load(Ordering::Relaxed) || done.load(Ordering::Relaxed))
}

/// Plex: report position on a timer, then post the authoritative final position
/// when mpv exits (this is what makes resume work).
fn start_tracking_plex(
    socket_path: String,
    info: TrackInfo,
    start_ms: u64,
    stop_flag: Arc<AtomicBool>,
) -> std::io::Result<()> {
    if info.server_base.is_empty() {
        return Ok(());
    }
    let (last_t_ms, done) = spawn_position_reader(socket_path, start_ms, stop_flag.clone())?;

    std::thread::Builder::new()
        .name("mpv-tracker-plex".into())
        .spawn(move || {
            // A single current-thread runtime: the tracker only does sequential
            // block_on HTTP posts, so a full multi-thread runtime (its own thread
            // pool) per playback session would be wasteful.
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(_) => return,
            };
            // Bail rather than fall back to a default client with no timeout — a hung
            // check-in post would otherwise pin this tracker thread indefinitely.
            let client = match reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
            {
                Ok(c) => c,
                Err(_) => return,
            };

            while wait_tick(&stop_flag, &done) {
                let t = last_t_ms.load(Ordering::Relaxed);
                if let Err(e) = rt.block_on(crate::plex_api::update_timeline(
                    &client,
                    &info.server_base,
                    &info.token,
                    &info.client_identifier,
                    &info.rating_key,
                    &info.key,
                    "playing",
                    t,
                    info.duration_ms,
                )) {
                    eprintln!("plex: timeline update failed: {}", e);
                }
            }

            // Authoritative final position. Skip if we never read a real position
            // (mpv failed to start / exited immediately) so we don't clobber an
            // existing resume point with 0.
            let t = last_t_ms.load(Ordering::Relaxed);
            if t > 0 {
                let _ = rt.block_on(crate::plex_api::update_timeline(
                    &client,
                    &info.server_base,
                    &info.token,
                    &info.client_identifier,
                    &info.rating_key,
                    &info.key,
                    "stopped",
                    t,
                    info.duration_ms,
                ));
                if let Err(e) = rt.block_on(crate::plex_api::update_progress(
                    &client,
                    &info.server_base,
                    &info.token,
                    &info.client_identifier,
                    &info.rating_key,
                    t,
                )) {
                    eprintln!("plex: final progress update failed: {}", e);
                }
            }
        })?;
    Ok(())
}

/// Jellyfin/Emby: POST to the `/Sessions/Playing*` endpoints — Start on the first
/// tick, Progress on each tick, Stopped (authoritative) when mpv exits.
fn start_tracking_jellyfin(
    socket_path: String,
    track: JellyfinTrack,
    start_ms: u64,
    stop_flag: Arc<AtomicBool>,
) -> std::io::Result<()> {
    if track.base_url.is_empty() {
        return Ok(());
    }
    let (last_t_ms, done) = spawn_position_reader(socket_path, start_ms, stop_flag.clone())?;

    std::thread::Builder::new()
        .name("mpv-tracker-jellyfin".into())
        .spawn(move || {
            // A single current-thread runtime: the tracker only does sequential
            // block_on HTTP posts, so a full multi-thread runtime (its own thread
            // pool) per playback session would be wasteful.
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(_) => return,
            };
            // Bail rather than fall back to a default client with no timeout — a hung
            // check-in post would otherwise pin this tracker thread indefinitely.
            let client = match reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
            {
                Ok(c) => c,
                Err(_) => return,
            };

            // Build a PlaybackProgressInfo-shaped body (Emby/Jellyfin check-in model)
            // so history/dashboard/resume all tie to the right session.
            let post = |endpoint: &str, position_ms: u64, event: Option<&str>| {
                let url = format!("{}/Sessions/Playing{}", track.base_url, endpoint);
                let mut body = serde_json::json!({
                    "ItemId": track.item_id,
                    "MediaSourceId": track.media_source_id,
                    "PositionTicks": position_ms.saturating_mul(10_000),
                    "PlayMethod": "DirectPlay",
                    "CanSeek": true,
                    "IsPaused": false,
                });
                if let Some(ps) = &track.play_session_id {
                    body["PlaySessionId"] = serde_json::Value::String(ps.clone());
                }
                if let Some(ev) = event {
                    body["EventName"] = serde_json::Value::String(ev.to_string());
                }
                let mut rb = client.post(&url).json(&body);
                for (k, v) in &track.headers {
                    rb = rb.header(k, v);
                }
                if let Err(e) =
                    rt.block_on(async { rb.send().await.and_then(|r| r.error_for_status()) })
                {
                    eprintln!("jellyfin: progress update failed: {}", e);
                }
            };

            post("", last_t_ms.load(Ordering::Relaxed), None); // Start
            while wait_tick(&stop_flag, &done) {
                post(
                    "/Progress",
                    last_t_ms.load(Ordering::Relaxed),
                    Some("TimeUpdate"),
                );
            }
            // Always post Stopped — including at position 0 — since we posted Start.
            // Otherwise a fast mpv exit (before any IPC position arrives) would leave
            // a stale "now playing" session on the server.
            post("/Stopped", last_t_ms.load(Ordering::Relaxed), None);
        })?;
    Ok(())
}
