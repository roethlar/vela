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

/// Process-private, owner-only (0700) runtime directory with an unpredictable
/// name, so a local user can't pre-create (symlink) the files we put here the
/// way they could in world-writable `/tmp`. Holds mpv's IPC socket and the
/// auth-header include file. The base stays under `/tmp` so socket paths keep
/// well under the unix socket-path limit (notably macOS's 104 bytes), unlike
/// the long per-user config dir.
#[cfg(not(windows))]
fn private_runtime_dir() -> Result<std::path::PathBuf, String> {
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
    Ok(dir.clone())
}

/// Path for mpv's JSON IPC socket, inside [`private_runtime_dir`].
#[cfg(not(windows))]
fn ipc_socket_path() -> Result<String, String> {
    let dir = private_runtime_dir()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Ok(dir
        .join(format!("mpv-{}-{}.sock", std::process::id(), ts))
        .to_string_lossy()
        .into_owned())
}

/// Where the per-launch mpv auth include lives. Unix: inside the same
/// process-private 0700 runtime dir as the IPC socket. Windows: the per-user
/// temp dir, which sits inside the user profile (private by default ACLs) —
/// the same protection class as Vela's config file, which stores this token
/// durably anyway.
fn header_conf_path() -> Result<std::path::PathBuf, String> {
    #[cfg(not(windows))]
    {
        Ok(private_runtime_dir()?.join(format!("mpv-headers-{}.conf", std::process::id())))
    }
    #[cfg(windows)]
    {
        Ok(std::env::temp_dir().join(format!("vela-mpv-headers-{}.conf", std::process::id())))
    }
}

/// Write the mpv include file that carries stream auth headers
/// (`http-header-fields`). An include file — not a command-line option —
/// because argv is world-readable (`/proc/<pid>/cmdline`) while this file is
/// owner-only; and not the URL, because mpv renders `${path}` in its title,
/// stats overlay (Shift+I), and playlist. mpv honors `--include` even under
/// `--no-config`, and an include asserted after the user's extra args
/// overrides any `--http-header-fields` they set — both verified against
/// mpv 0.41 (the header reaches the wire exactly once). One file per Vela
/// process, overwritten on each launch; the previous mpv (killed before a new
/// play starts) has long finished reading its config by then.
fn write_header_include(headers: &[(String, String)]) -> Result<String, String> {
    let path = header_conf_path()?;
    write_header_include_at(&path, headers)?;
    Ok(path.to_string_lossy().into_owned())
}

/// The testable core of [`write_header_include`]: validates and writes to an
/// explicit path. Header names/values are restricted to characters that cannot
/// escape mpv's quoted conf value or smuggle extra list entries or config
/// lines — anything else fails closed, and the error text never echoes the
/// offending value (it may be a credential).
fn write_header_include_at(
    path: &std::path::Path,
    headers: &[(String, String)],
) -> Result<(), String> {
    for (name, value) in headers {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err("invalid stream auth header name".to_string());
        }
        // `"` would close mpv's value quoting, `,` separates list entries, and
        // control characters could inject config lines. `#` (mpv's comment
        // marker) is excluded defensively too.
        if !value
            .chars()
            .all(|c| (c.is_ascii_graphic() || c == ' ') && !['"', ',', '#'].contains(&c))
        {
            return Err("invalid stream auth header value".to_string());
        }
    }
    let joined = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join(",");
    let content = format!("http-header-fields=\"{joined}\"\n");
    // Remove the previous launch's file, then create exclusively with
    // owner-only permissions from the first byte (never chmod-after-write).
    let _ = std::fs::remove_file(path);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .map_err(|e| format!("couldn't write the mpv auth config: {e}"))?;
    f.write_all(content.as_bytes())
        .map_err(|e| format!("couldn't write the mpv auth config: {e}"))?;
    Ok(())
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

/// What to play, as opposed to the supervision plumbing `play` also takes:
/// the stream URL, the human title for mpv's window/OSD, the auth headers the
/// stream needs (see [`write_header_include`]), and the resume offset.
pub struct PlaySpec {
    pub url: String,
    pub title: String,
    pub http_headers: Vec<(String, String)>,
    pub start_seconds: f64,
    /// Absolute path to the bundled mpv `autocrop.lua`, resolved by the caller
    /// via Tauri's resource resolver (the caller has the `AppHandle`; `play`
    /// does not). `None` when it could not be resolved — autocrop is then
    /// skipped even if enabled. Whether it's actually injected depends on the
    /// `mpv_autocrop` config mode, read in `play`.
    pub autocrop_script: Option<String>,
    /// Absolute path to Vela's `vela-autocrop.lua` trigger shim (same resolver
    /// and existence check). Auto mode loads the stock script with its own
    /// auto trigger DISABLED and lets the shim fire detection after a settle
    /// delay — the stock trigger skips its delay on `--start` resumes and
    /// races hwdec init (see the shim header + `.agents/plans/autocrop-resume.md`).
    pub autocrop_shim: Option<String>,
}

/// mpv launch args for the autocrop feature, given the config `mode`
/// (`"off"|"manual"|"auto"`) and the resolved script paths (already existence-
/// checked by the caller; `None` when unresolved/missing). Pure so the injection
/// is unit-testable without spawning mpv:
/// - `off` / unknown, or no stock script → no args.
/// - `manual` → load the stock script but disable its auto-crop, so it only
///   crops on an explicit in-player `Shift+C`.
/// - `auto` → stock script with auto disabled + the Vela shim owning the
///   trigger (settle delay covers fresh AND resumed plays).
/// - `auto` with the shim missing → degrade to the stock script's own auto
///   trigger (fresh plays keep cropping; resume stays broken) — the caller
///   logs the degradation.
fn autocrop_args(mode: &str, script: Option<&str>, shim: Option<&str>) -> Vec<String> {
    let Some(path) = script else {
        return Vec::new();
    };
    match (mode, shim) {
        ("manual", _) => vec![
            format!("--script={path}"),
            "--script-opts-append=autocrop-auto=no".to_string(),
        ],
        ("auto", Some(shim_path)) => vec![
            format!("--script={path}"),
            "--script-opts-append=autocrop-auto=no".to_string(),
            format!("--script={shim_path}"),
        ],
        ("auto", None) => vec![format!("--script={path}")],
        _ => Vec::new(),
    }
}

/// Fired exactly once when a playback session ends — after the final server
/// check-in for tracked sessions, or at mpv exit for untracked ones — with
/// the last observed playback position in ms (0 when none was read). The
/// caller wires this to recents recording + UI notification (a
/// `playback-ended` Tauri event); this module stays UI-framework-free.
pub type EndNotify = Arc<dyn Fn(u64) + Send + Sync>;

/// Spawn mpv for `spec.url`, optionally seeking to `spec.start_seconds`, and
/// start background progress reporting that runs until mpv exits. Publishes the
/// child into `child_slot` the instant it's launched (so an app-exit that races
/// tracker setup can still find and kill it), and returns the stop flag so a
/// later play can cancel this one. The caller must have already cleared/killed
/// any previous child, since this overwrites the slot.
pub fn play(
    spec: &PlaySpec,
    progress: ProgressTarget,
    child_slot: &Arc<Mutex<Option<std::process::Child>>>,
    shutting_down: &Arc<AtomicBool>,
    advance: &Arc<tokio::sync::Notify>,
    on_end: Option<EndNotify>,
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

    // Bundled mpv autocrop script (Settings → Advanced). Placed with the user
    // extra args (before the re-asserted IPC/title/URL block) so it can't clobber
    // the socket. Only injected when enabled AND the bundled script actually
    // exists — a missing `--script=` would make mpv refuse to launch, so skip and
    // log instead. `autocrop-auto=no` in Manual mode keeps the known live
    // `video-crop` hang off the every-play path (see config `mpv_autocrop`).
    let autocrop_mode = cfg.mpv_autocrop.as_deref().unwrap_or("off");
    if autocrop_mode != "off" {
        let script = spec
            .autocrop_script
            .as_deref()
            .filter(|p| std::path::Path::new(p).is_file());
        if script.is_none() {
            eprintln!(
                "vela: autocrop is set to '{autocrop_mode}' but the bundled script \
                 did not resolve; skipping (playback continues uncropped)"
            );
        }
        let shim = spec
            .autocrop_shim
            .as_deref()
            .filter(|p| std::path::Path::new(p).is_file());
        if autocrop_mode == "auto" && script.is_some() && shim.is_none() {
            eprintln!(
                "vela: autocrop trigger shim (vela-autocrop.lua) did not resolve; \
                 falling back to the stock auto trigger (resumed plays won't crop)"
            );
        }
        for arg in autocrop_args(autocrop_mode, script, shim) {
            cmd.arg(arg);
        }
    }

    cmd.arg(format!("--input-ipc-server={}", ipc_path));
    // Drive mpv's window title and OSD media-title from the human title, NOT the
    // URL. Plex direct-stream URLs carry `?X-Plex-Token=…`, and mpv's default
    // title template derives from the URL — so without this the auth token leaks
    // into the title bar (and the on-screen media name). Asserted here, after the
    // user's extra args, so it can't be clobbered back into a leak. Fall back to a
    // neutral label rather than letting mpv reach for the URL when title is empty.
    let display_title = if spec.title.trim().is_empty() {
        "Vela"
    } else {
        spec.title.as_str()
    };
    cmd.arg(format!("--force-media-title={}", display_title));
    cmd.arg(format!("--title={}", display_title));
    // Stream auth (e.g. Plex's X-Plex-Token) rides in an owner-only include
    // file — see write_header_include for why neither argv nor the URL may
    // carry it. Asserted after the user's extra args so a user-set
    // --http-header-fields can't silently drop the auth the stream needs.
    if !spec.http_headers.is_empty() {
        cmd.arg(format!(
            "--include={}",
            write_header_include(&spec.http_headers)?
        ));
    }
    if spec.start_seconds > 0.0 {
        cmd.arg(format!("--start={}", spec.start_seconds));
    }
    // `--` terminates option parsing so a URL/path that begins with `-` (e.g. a
    // hostile filename from a local folder or server) can't be read as an mpv
    // option. In practice our URLs are absolute paths / http(s) / edl://, but
    // this closes the option-injection vector regardless.
    cmd.arg("--").arg(&spec.url);

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
    let start_ms = (spec.start_seconds.max(0.0) * 1000.0) as u64;
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Watch mpv's IPC for a clean EOF — that's the signal for whichever
    // playback context owns the sequence. We do NOT advance on user-close (mpv
    // reports reason=quit/stop), only on reason=eof. Always-on regardless of
    // progress target. Failure to spawn the watcher is non-fatal — playback
    // still works, just without sequence advancement.
    if let Err(e) = spawn_eof_watcher(ipc_path.clone(), stop_flag.clone(), advance.clone()) {
        eprintln!("vela: couldn't spawn mpv EOF watcher: {e}");
    }

    // Route the end-of-session notifier: tracked sessions fire it from their
    // tracker tail (after the final server write, so a refresh triggered by it
    // sees the new state); untracked sessions fire it when mpv exits. The match
    // guards keep degenerate track targets (empty server base) on the untracked
    // path so the notifier still fires exactly once per session.
    let tracking = match progress {
        ProgressTarget::Plex(info) if !info.server_base.is_empty() => {
            start_tracking_plex(ipc_path, info, start_ms, stop_flag.clone(), on_end)
        }
        ProgressTarget::Jellyfin(track) if !track.base_url.is_empty() => {
            start_tracking_jellyfin(ipc_path, track, start_ms, stop_flag.clone(), on_end)
        }
        _ => spawn_end_watcher(ipc_path, stop_flag.clone(), on_end),
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

/// Tiny IPC listener that signals the playback dispatcher when mpv finishes a
/// file CLEANLY (mpv emits `event=end-file, reason=eof`). User-closed (`quit`),
/// errored (`error`), or otherwise-stopped exits never fire the notifier — so
/// closing the player stops playback. Separate from the progress reader so it
/// runs for every source even when no progress tracker is active.
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
/// when mpv exits (this is what makes resume work). The caller guarantees a
/// non-empty `info.server_base` (see the match guards in [`play`]).
fn start_tracking_plex(
    socket_path: String,
    info: TrackInfo,
    start_ms: u64,
    stop_flag: Arc<AtomicBool>,
    on_end: Option<EndNotify>,
) -> std::io::Result<()> {
    let (last_t_ms, done) = spawn_position_reader(socket_path, start_ms, stop_flag.clone())?;

    std::thread::Builder::new()
        .name("mpv-tracker-plex".into())
        .spawn(move || {
            // Every exit funnels past the notifier below, so it fires exactly
            // once per session even on early bails inside this closure.
            (|| {
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
            })();
            if let Some(on_end) = on_end {
                on_end(last_t_ms.load(Ordering::Relaxed));
            }
        })?;
    Ok(())
}

/// Jellyfin/Emby: POST to the `/Sessions/Playing*` endpoints — Start on the first
/// tick, Progress on each tick, Stopped (authoritative) when mpv exits. The
/// caller guarantees a non-empty `track.base_url` (see the match guards in
/// [`play`]).
fn start_tracking_jellyfin(
    socket_path: String,
    track: JellyfinTrack,
    start_ms: u64,
    stop_flag: Arc<AtomicBool>,
    on_end: Option<EndNotify>,
) -> std::io::Result<()> {
    let (last_t_ms, done) = spawn_position_reader(socket_path, start_ms, stop_flag.clone())?;

    std::thread::Builder::new()
        .name("mpv-tracker-jellyfin".into())
        .spawn(move || {
            // Every exit funnels past the notifier below, so it fires exactly
            // once per session even on early bails inside this closure.
            (|| {
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
            })();
            if let Some(on_end) = on_end {
                on_end(last_t_ms.load(Ordering::Relaxed));
            }
        })?;
    Ok(())
}

/// For sessions with no progress tracker (local/SMB files, or a degenerate
/// track target): observe mpv's position over IPC and fire the end notifier
/// with the last seen position when mpv exits (or the session is replaced).
/// Tracked sessions instead notify from their tracker tail, after the final
/// server write. No-op without a notifier.
fn spawn_end_watcher(
    socket_path: String,
    stop_flag: Arc<AtomicBool>,
    on_end: Option<EndNotify>,
) -> std::io::Result<()> {
    let Some(on_end) = on_end else {
        return Ok(());
    };
    // Reuse the shared position reader: its `done` flag doubles as the EOF
    // signal (set when mpv exits / the socket drops).
    let (last_t_ms, done) = spawn_position_reader(socket_path, 0, stop_flag.clone())?;
    std::thread::Builder::new()
        .name("mpv-end-watcher".into())
        .spawn(move || {
            while !done.load(Ordering::Relaxed) && !stop_flag.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(100));
            }
            // Fire even when the session was replaced or mpv exited before a
            // position arrived (position 0): the session is over either way,
            // and a spurious refresh is harmless.
            on_end(last_t_ms.load(Ordering::Relaxed));
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("vela-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn autocrop_off_injects_nothing() {
        assert!(
            autocrop_args("off", Some("/x/autocrop.lua"), Some("/x/vela-autocrop.lua")).is_empty()
        );
        // Unknown/garbage mode is treated as off.
        assert!(
            autocrop_args("wat", Some("/x/autocrop.lua"), Some("/x/vela-autocrop.lua")).is_empty()
        );
    }

    #[test]
    fn autocrop_manual_loads_script_with_auto_disabled() {
        // Manual ignores the shim even when resolved: Shift+C is the trigger.
        let args = autocrop_args(
            "manual",
            Some("/x/autocrop.lua"),
            Some("/x/vela-autocrop.lua"),
        );
        assert_eq!(
            args,
            vec![
                "--script=/x/autocrop.lua".to_string(),
                "--script-opts-append=autocrop-auto=no".to_string(),
            ],
            "manual must load the stock script AND disable its auto-crop, shim excluded"
        );
    }

    #[test]
    fn autocrop_auto_disables_stock_trigger_and_loads_shim() {
        // The stock auto trigger skips its settle delay on --start resumes and
        // races hwdec init (plan autocrop-resume); auto mode therefore hands
        // the trigger to the Vela shim.
        let args = autocrop_args(
            "auto",
            Some("/x/autocrop.lua"),
            Some("/x/vela-autocrop.lua"),
        );
        assert_eq!(
            args,
            vec![
                "--script=/x/autocrop.lua".to_string(),
                "--script-opts-append=autocrop-auto=no".to_string(),
                "--script=/x/vela-autocrop.lua".to_string(),
            ],
            "auto must load the stock script with auto OFF and the shim as trigger"
        );
    }

    #[test]
    fn autocrop_auto_without_shim_degrades_to_stock_trigger() {
        // Missing shim (e.g. a package that only installed autocrop.lua):
        // keep the stock auto behavior — fresh plays crop, resume stays broken —
        // instead of losing autocrop entirely.
        let args = autocrop_args("auto", Some("/x/autocrop.lua"), None);
        assert_eq!(args, vec!["--script=/x/autocrop.lua".to_string()]);
        assert!(
            !args.iter().any(|a| a.contains("autocrop-auto=no")),
            "degraded auto must keep the stock crop-on-start trigger"
        );
    }

    #[test]
    fn autocrop_without_resolved_script_injects_nothing() {
        // Even when enabled, an unresolved stock script yields no args (play()
        // also existence-checks; here the path is simply absent). A shim alone
        // is useless — it triggers a script that isn't loaded.
        assert!(autocrop_args("manual", None, None).is_empty());
        assert!(autocrop_args("auto", None, Some("/x/vela-autocrop.lua")).is_empty());
    }

    #[test]
    fn header_include_writes_quoted_fields_owner_only() {
        let path = tmp("headers-ok.conf");
        let headers = vec![("X-Plex-Token".to_string(), "abc123".to_string())];
        write_header_include_at(&path, &headers).expect("write include");
        let content = std::fs::read_to_string(&path).expect("read back");
        assert_eq!(content, "http-header-fields=\"X-Plex-Token: abc123\"\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path)
                .expect("stat include")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "auth include must be owner-only");
        }
        // A later launch replaces the previous launch's file in place.
        let headers = vec![("X-Plex-Token".to_string(), "second".to_string())];
        write_header_include_at(&path, &headers).expect("overwrite include");
        let content = std::fs::read_to_string(&path).expect("read back after overwrite");
        assert_eq!(content, "http-header-fields=\"X-Plex-Token: second\"\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn header_include_fails_closed_on_conf_escaping_input() {
        let path = tmp("headers-bad.conf");
        for bad in [
            "with\"quote",
            "with,comma",
            "with#hash",
            "with\nnewline",
            "with\rreturn",
        ] {
            let headers = vec![("X-Plex-Token".to_string(), bad.to_string())];
            let err = write_header_include_at(&path, &headers).expect_err("must fail closed");
            assert!(
                !err.contains("with"),
                "error text must not echo the header value"
            );
        }
        for bad_name in ["", "X Token", "X:Token", "Tok\nen"] {
            let headers = vec![(bad_name.to_string(), "value".to_string())];
            write_header_include_at(&path, &headers).expect_err("bad name must fail");
        }
        assert!(
            !path.exists(),
            "nothing may be written when validation fails"
        );
    }
}
