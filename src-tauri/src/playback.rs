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

/// Where the per-launch mpv auth include lives. Unix uses the same
/// process-private 0700 runtime dir as the IPC socket. Windows uses Vela's
/// protected per-user config directory so its verified DACL can be established
/// before credential bytes are written.
fn header_conf_path() -> Result<std::path::PathBuf, String> {
    let nonce = uuid::Uuid::new_v4().simple();
    #[cfg(not(windows))]
    {
        Ok(private_runtime_dir()?.join(format!("mpv-headers-{}-{nonce}.conf", std::process::id())))
    }
    #[cfg(windows)]
    {
        crate::storage::config_dir_file(&format!("mpv-headers-{}-{nonce}.conf", std::process::id()))
            .map_err(|_| "couldn't prepare the mpv auth config".to_string())
    }
}

/// Deletes one launch's credential include when its owning child is reaped.
/// The path is unique per launch, so a delayed old-child cleanup can never
/// remove a newer player's credentials.
#[derive(Debug)]
struct HeaderInclude {
    path: std::path::PathBuf,
}

impl HeaderInclude {
    fn remove_now(&mut self) -> std::io::Result<()> {
        // A transient sharing violation must not be the only cleanup attempt.
        // Never print the path: it is credential-adjacent runtime state.
        for attempt in 0..3 {
            match std::fs::remove_file(&self.path) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(_) if attempt < 2 => std::thread::yield_now(),
                Err(error) => return Err(error),
            }
        }
        unreachable!("the bounded removal loop always returns")
    }
}

impl Drop for HeaderInclude {
    fn drop(&mut self) {
        if self.remove_now().is_err() {
            eprintln!("vela: could not remove an mpv authentication include");
        }
    }
}

/// A child process and the private credential file it consumed. The reaper
/// drops this wrapper only after `try_wait` reports exit. Query failures retain
/// the wrapper so a transient OS error cannot orphan the credential include.
pub(crate) struct ManagedChild {
    child: std::process::Child,
    _header_include: Option<HeaderInclude>,
}

impl ManagedChild {
    fn new(child: std::process::Child, header_include: Option<HeaderInclude>) -> Self {
        Self {
            child,
            _header_include: header_include,
        }
    }

    pub(crate) fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }

    pub(crate) fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    /// A player that has been running has already consumed its startup config.
    /// Remove that include before a replacement launch writes another one. A
    /// failed removal leaves ownership attached to this child and fails the
    /// replacement closed.
    pub(crate) fn remove_consumed_header_include(&mut self) -> Result<(), String> {
        let Some(mut include) = self._header_include.take() else {
            return Ok(());
        };
        if include.remove_now().is_ok() {
            Ok(())
        } else {
            self._header_include = Some(include);
            Err("could not remove the prior mpv authentication include".to_string())
        }
    }
}

pub(crate) fn retain_child_after_try_wait(
    result: &std::io::Result<Option<std::process::ExitStatus>>,
) -> bool {
    !matches!(result, Ok(Some(_)))
}

/// Write the mpv include file that carries stream auth headers
/// (`http-header-fields`). An include file — not a command-line option —
/// because argv is world-readable (`/proc/<pid>/cmdline`) while this file is
/// owner-only; and not the URL, because mpv renders `${path}` in its title,
/// stats overlay (Shift+I), and playlist. mpv honors `--include` even under
/// `--no-config`, and an include asserted after the user's extra args
/// overrides any `--http-header-fields` they set — both verified against
/// mpv 0.41 (the header reaches the wire exactly once). Each launch gets a
/// unique file, held by [`ManagedChild`] until that exact child is reaped.
fn write_header_include(headers: &[(String, String)]) -> Result<HeaderInclude, String> {
    let path = header_conf_path()?;
    write_header_include_at(&path, headers)
}

/// The testable core of [`write_header_include`]: validates and writes to an
/// explicit path. Header names/values are restricted to characters that cannot
/// escape mpv's quoted conf value or smuggle extra list entries or config
/// lines — anything else fails closed, and the error text never echoes the
/// offending value (it may be a credential).
fn write_header_include_at(
    path: &std::path::Path,
    headers: &[(String, String)],
) -> Result<HeaderInclude, String> {
    write_header_include_at_with(path, headers, |file, content| file.write_all(content))
}

fn write_header_include_at_with(
    path: &std::path::Path,
    headers: &[(String, String)],
    write_content: impl FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
) -> Result<HeaderInclude, String> {
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
    // Replace only this explicit target (used by tests), then create exclusively
    // with owner-only permissions from the first byte (never chmod-after-write).
    // Production targets are unique per launch.
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
    let guard = HeaderInclude {
        path: path.to_path_buf(),
    };
    // On Windows this installs and verifies the protected current-user/SYSTEM/
    // Administrators DACL. It must happen after empty-file creation but before
    // the first credential byte. Unix re-verifies the already-0600 file.
    if crate::storage::harden_existing_regular(path).is_err() {
        drop(f);
        drop(guard);
        return Err("couldn't protect the mpv auth config".to_string());
    }
    if let Err(error) = write_content(&mut f, content.as_bytes()) {
        drop(f);
        drop(guard);
        return Err(format!("couldn't write the mpv auth config: {error}"));
    }
    if let Err(error) = f.sync_all() {
        drop(f);
        drop(guard);
        return Err(format!("couldn't write the mpv auth config: {error}"));
    }
    Ok(guard)
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
pub fn resolve_mpv() -> Result<Option<String>, String> {
    // 1. Explicit override the user set in Settings → mpv player. Honored only
    //    if it actually resolves to a runnable file (validated on save too).
    if let Some(p) = crate::config::load_config()
        .map_err(|_| "could not read Vela settings".to_string())?
        .mpv_path
    {
        let p = p.trim().to_string();
        if !p.is_empty() && mpv_usable(&p) {
            return Ok(Some(p));
        }
    }
    // 2. A bundled copy next to the app executable (zero-config "just works").
    if let Some(p) = bundled_mpv().filter(|p| mpv_usable(p)) {
        return Ok(Some(p));
    }
    // 3. Bare `mpv` on PATH.
    if mpv_runs("mpv") {
        return Ok(Some("mpv".to_string()));
    }
    // 4. Known install locations.
    Ok(mpv_candidates().into_iter().find(|cand| mpv_usable(cand)))
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

/// Runtime mpv window state that may be carried to an exact automatic
/// successor. `None` means mpv has not reported the property, which is distinct
/// from an explicit `false` and therefore leaves configured launch options
/// authoritative.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlaybackWindowState {
    pub fullscreen: Option<bool>,
    pub maximized: Option<bool>,
}

/// Per-launch observation handle. Each IPC reader owns a fresh handle so a
/// delayed event from an older mpv process cannot overwrite a newer session's
/// published state.
#[derive(Debug, Clone, Default)]
pub struct WindowStateObservation {
    state: Arc<Mutex<PlaybackWindowState>>,
    display: Arc<Mutex<crate::display::ObservedDisplay>>,
}

impl WindowStateObservation {
    pub fn snapshot(&self) -> PlaybackWindowState {
        *self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn display_snapshot(&self) -> crate::display::ObservedDisplay {
        self.display
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(crate) fn apply_ipc_event(&self, value: &serde_json::Value) {
        if let Some((property, enabled)) = window_property_change(value) {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            match property {
                WindowProperty::Fullscreen => state.fullscreen = Some(enabled),
                WindowProperty::Maximized => state.maximized = Some(enabled),
            }
        }
        if let Some(change) = display_property_change(value) {
            let mut display = self
                .display
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            match change {
                DisplayPropertyChange::Names(names) => display.names = names,
                DisplayPropertyChange::Width(width) => display.width_px = Some(width),
                DisplayPropertyChange::Height(height) => display.height_px = Some(height),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowProperty {
    Fullscreen,
    Maximized,
}

/// Accept only owned boolean property-change events. Observe-property command
/// replies, nulls, numeric/string coercions, and unrelated properties must not
/// invent state.
fn window_property_change(value: &serde_json::Value) -> Option<(WindowProperty, bool)> {
    if value.get("event")?.as_str()? != "property-change" {
        return None;
    }
    let property = match value.get("name")?.as_str()? {
        "fullscreen" => WindowProperty::Fullscreen,
        "window-maximized" => WindowProperty::Maximized,
        _ => return None,
    };
    Some((property, value.get("data")?.as_bool()?))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DisplayPropertyChange {
    Names(Vec<String>),
    Width(u32),
    Height(u32),
}

/// mpv reports actual output identity and pixel dimensions as property-change
/// events. Accept only the exact JSON types: malformed replies must not invent
/// compatibility evidence.
fn display_property_change(value: &serde_json::Value) -> Option<DisplayPropertyChange> {
    if value.get("event")?.as_str()? != "property-change" {
        return None;
    }
    match value.get("name")?.as_str()? {
        "display-names" => Some(DisplayPropertyChange::Names(
            value
                .get("data")?
                .as_array()?
                .iter()
                .map(|name| name.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()?,
        )),
        "display-width" => value
            .get("data")?
            .as_u64()
            .and_then(|width| u32::try_from(width).ok())
            .filter(|width| *width > 0)
            .map(DisplayPropertyChange::Width),
        "display-height" => value
            .get("data")?
            .as_u64()
            .and_then(|height| u32::try_from(height).ok())
            .filter(|height| *height > 0)
            .map(DisplayPropertyChange::Height),
        _ => None,
    }
}

/// Explicit inherited flags follow user/autocrop options so observed runtime
/// state wins under mpv's last-value-wins option handling. Unknown properties
/// are omitted, preserving normal configuration behavior.
fn window_state_args(state: PlaybackWindowState) -> Vec<String> {
    let mut args = Vec::with_capacity(2);
    if let Some(maximized) = state.maximized {
        args.push(format!(
            "--window-maximized={}",
            if maximized { "yes" } else { "no" }
        ));
    }
    if let Some(fullscreen) = state.fullscreen {
        args.push(format!(
            "--fullscreen={}",
            if fullscreen { "yes" } else { "no" }
        ));
    }
    args
}

fn screen_name_arg(name: Option<&str>) -> Option<String> {
    name.map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| format!("--screen-name={name}"))
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
    /// Actual state sampled from the exact automatic predecessor. Manual plays
    /// and unknown properties use the default (both `None`).
    pub inherited_window_state: PlaybackWindowState,
    /// Native output containing Vela's window for a manual play, or the exact
    /// observed predecessor output for an automatic successor.
    pub screen_name: Option<String>,
    /// Fresh handle populated by this launch's IPC reader and published by the
    /// command layer only after the full launch succeeds.
    pub window_observation: WindowStateObservation,
    /// Absolute path to Vela's bundled `vela-markers.lua`, resolved by the
    /// caller through the same resource resolver as autocrop. `None` when it
    /// could not be resolved — skipping is then simply absent, never an error.
    pub markers_script: Option<String>,
    /// Marker ranges for this exact launch, already normalized AND already
    /// filtered to the kinds whose policy is enabled. Empty means nothing is
    /// injected at all.
    pub markers: Vec<crate::source::MediaMarker>,
    /// Resolved per-kind policies. `play` passes these to the script; it never
    /// reads config to derive them and never interprets a policy string.
    pub intro_policy: crate::config::SkipPolicy,
    pub credits_policy: crate::config::SkipPolicy,
    pub commercial_policy: crate::config::SkipPolicy,
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

/// mpv launch args for marker skipping. Pure, so the injection is testable
/// without spawning mpv. Every "don't act" condition collapses to no args:
/// no resolved script, every policy Off, or no payload successfully written —
/// the script is useless without its ranges, and injecting a `--script=` whose
/// file is missing would make mpv refuse to start outright.
///
/// The payload PATH is deliberately absent here: it travels on the child's
/// environment, because mpv's script-opts list is comma-split and would mangle
/// Windows and user paths.
fn markers_args(
    script: Option<&str>,
    intro: crate::config::SkipPolicy,
    credits: crate::config::SkipPolicy,
    commercial: crate::config::SkipPolicy,
    has_payload: bool,
) -> Vec<String> {
    let Some(path) = script else {
        return Vec::new();
    };
    if !has_payload || (intro.is_off() && credits.is_off() && commercial.is_off()) {
        return Vec::new();
    }
    vec![
        format!("--script={path}"),
        format!(
            "--script-opts-append=vela-markers-intro-policy={}",
            intro.as_option_value()
        ),
        format!(
            "--script-opts-append=vela-markers-credits-policy={}",
            credits.as_option_value()
        ),
        format!(
            "--script-opts-append=vela-markers-commercial-policy={}",
            commercial.as_option_value()
        ),
    ]
}

/// Unique per-launch payload name in this process's private directory. Mirrors
/// [`header_conf_path`]: the caller never reuses a path, so a stale cleanup can
/// never delete a newer launch's file.
fn marker_payload_path() -> Result<std::path::PathBuf, String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let name = format!(
        "vela-markers-{}-{}.json",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    #[cfg(not(windows))]
    {
        Ok(private_runtime_dir()?.join(name))
    }
    #[cfg(windows)]
    {
        crate::storage::config_dir_file(&name)
            .map_err(|_| "couldn't prepare the marker payload".to_string())
    }
}

/// Best-effort sweep of payloads this process left behind — normally the Lua
/// script unlinks its own, so anything here is the residue of a crash. Only
/// Vela's own prefix, only inside the directory we created ourselves.
fn prune_marker_payloads(dir: &std::path::Path) {
    let prefix = format!("vela-markers-{}-", std::process::id());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with(&prefix) && name.ends_with(".json") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Write the marker payload for one launch. Non-fatal by construction: every
/// failure returns `None` and playback proceeds without skipping. Never `?` this
/// into the play path.
fn try_write_marker_payload(markers: &[crate::source::MediaMarker]) -> Option<std::path::PathBuf> {
    if markers.is_empty() {
        return None;
    }
    #[derive(serde::Serialize)]
    struct Payload<'a> {
        markers: &'a [crate::source::MediaMarker],
    }
    let body = serde_json::to_vec(&Payload { markers }).ok()?;
    let path = marker_payload_path().ok()?;
    if let Some(dir) = path.parent() {
        prune_marker_payloads(dir);
    }
    // Exclusive creation, owner-only from the first byte — same rule as the
    // auth include, even though marker timings are not credentials.
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(&path).ok()?;
    if file.write_all(&body).is_err() {
        // A partial file must not be handed to the script.
        let _ = std::fs::remove_file(&path);
        return None;
    }
    Some(path)
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
    child_slot: &Arc<Mutex<Option<ManagedChild>>>,
    shutting_down: &Arc<AtomicBool>,
    advance: &Arc<crate::commands::PlaybackAdvance>,
    session_id: String,
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

    let mpv_bin = resolve_mpv()?
        .ok_or_else(|| "mpv was not found. Install mpv to play video.".to_string())?;

    // Advanced/user mpv config (Settings → Advanced mpv options), loaded once here.
    let cfg = crate::config::load_config()
        .map_err(|_| "could not read Vela settings".to_string())?;
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

    // Marker skipping. Like autocrop this lands after the user's own options so
    // user configuration cannot replace Vela's policies, and every failure here
    // degrades to a normal play rather than refusing one.
    let marker_payload = {
        let script = spec
            .markers_script
            .as_deref()
            .filter(|path| std::path::Path::new(path).is_file());
        let wanted = !(spec.intro_policy.is_off()
            && spec.credits_policy.is_off()
            && spec.commercial_policy.is_off());
        if script.is_none() && wanted && !spec.markers.is_empty() {
            eprintln!(
                "vela: marker script (vela-markers.lua) did not resolve; \
                 playing without skip controls"
            );
        }
        let payload = if script.is_some() && wanted {
            try_write_marker_payload(&spec.markers)
        } else {
            None
        };
        for arg in markers_args(
            script,
            spec.intro_policy,
            spec.credits_policy,
            spec.commercial_policy,
            payload.is_some(),
        ) {
            cmd.arg(arg);
        }
        if let Some(path) = &payload {
            // Child-only, and never on the command line: the payload path must
            // not appear in the process argument list.
            cmd.env("VELA_MARKERS_PAYLOAD", path);
        }
        payload
    };

    for arg in window_state_args(spec.inherited_window_state) {
        cmd.arg(arg);
    }
    if let Some(screen_arg) = screen_name_arg(spec.screen_name.as_deref()) {
        // Reassert after user options. On Wayland this is a placement request,
        // not a guarantee; the compositor's actual choice is observed over IPC.
        cmd.arg(screen_arg);
    }

    cmd.arg(format!("--input-ipc-server={}", ipc_path));
    // Drive mpv's window title and OSD media-title from the human title, NOT the
    // URL. Authenticated media URLs have historically carried tokens, and mpv's
    // default title template derives from the URL — so without this a regression
    // could leak one into the title bar or on-screen media name. Asserted after the
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
    let header_include = if spec.http_headers.is_empty() {
        None
    } else {
        let include = write_header_include(&spec.http_headers)?;
        cmd.arg(format!("--include={}", include.path.to_string_lossy()));
        Some(include)
    };
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
            // Nothing will read the payload now; the Lua unlink never happens.
            if let Some(path) = &marker_payload {
                let _ = std::fs::remove_file(path);
            }
            return Err("Vela is shutting down.".to_string());
        }
        let child = cmd.spawn().map_err(|e| {
            if let Some(path) = &marker_payload {
                let _ = std::fs::remove_file(path);
            }
            if e.kind() == std::io::ErrorKind::NotFound {
                "mpv was not found. Install mpv to play video.".to_string()
            } else {
                format!("failed to launch mpv: {}", e)
            }
        })?;
        *slot = Some(ManagedChild::new(child, header_include));
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
    if let Err(e) = spawn_eof_watcher(
        ipc_path.clone(),
        stop_flag.clone(),
        advance.clone(),
        session_id,
    ) {
        eprintln!("vela: couldn't spawn mpv EOF watcher: {e}");
    }

    // Route the end-of-session notifier: tracked sessions fire it from their
    // tracker tail (after the final server write, so a refresh triggered by it
    // sees the new state); untracked sessions fire it when mpv exits. The match
    // guards keep degenerate track targets (empty server base) on the untracked
    // path so the notifier still fires exactly once per session.
    let tracking = match progress {
        ProgressTarget::Plex(info) if !info.server_base.is_empty() => start_tracking_plex(
            ipc_path,
            info,
            start_ms,
            stop_flag.clone(),
            spec.window_observation.clone(),
            on_end,
        ),
        ProgressTarget::Jellyfin(track) if !track.base_url.is_empty() => start_tracking_jellyfin(
            ipc_path,
            track,
            start_ms,
            stop_flag.clone(),
            spec.window_observation.clone(),
            on_end,
        ),
        _ => spawn_end_watcher(
            ipc_path,
            stop_flag.clone(),
            spec.window_observation.clone(),
            on_end,
        ),
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
    advance: Arc<crate::commands::PlaybackAdvance>,
    session_id: String,
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
                    advance.mark_eof(session_id);
                    return;
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
    window_observation: WindowStateObservation,
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

            // Ask mpv to push position and window-state updates; then drain every
            // line. Distinct ids keep command replies unambiguous during IPC
            // diagnostics, while parsing below trusts only named property events.
            let _ = stream.write_all(
                b"{\"command\":[\"observe_property\",1,\"time-pos\"]}\n\
                  {\"command\":[\"observe_property\",2,\"fullscreen\"]}\n\
                  {\"command\":[\"observe_property\",3,\"window-maximized\"]}\n\
                  {\"command\":[\"observe_property\",4,\"display-names\"]}\n\
                  {\"command\":[\"observe_property\",5,\"display-width\"]}\n\
                  {\"command\":[\"observe_property\",6,\"display-height\"]}\n",
            );

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
                    window_observation.apply_ipc_event(&v);
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
    window_observation: WindowStateObservation,
    on_end: Option<EndNotify>,
) -> std::io::Result<()> {
    let (last_t_ms, done) =
        spawn_position_reader(socket_path, start_ms, stop_flag.clone(), window_observation)?;

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
    window_observation: WindowStateObservation,
    on_end: Option<EndNotify>,
) -> std::io::Result<()> {
    let (last_t_ms, done) =
        spawn_position_reader(socket_path, start_ms, stop_flag.clone(), window_observation)?;

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
    window_observation: WindowStateObservation,
    on_end: Option<EndNotify>,
) -> std::io::Result<()> {
    // Reuse the shared position reader: its `done` flag doubles as the EOF
    // signal (set when mpv exits / the socket drops).
    let (last_t_ms, done) =
        spawn_position_reader(socket_path, 0, stop_flag.clone(), window_observation)?;
    let Some(on_end) = on_end else {
        return Ok(());
    };
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
    fn window_property_parser_accepts_only_named_boolean_change_events() {
        assert_eq!(
            window_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "fullscreen",
                "data": true
            })),
            Some((WindowProperty::Fullscreen, true))
        );
        assert_eq!(
            window_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "window-maximized",
                "data": false
            })),
            Some((WindowProperty::Maximized, false))
        );

        for rejected in [
            serde_json::json!({ "request_id": 2, "data": true }),
            serde_json::json!({
                "event": "property-change",
                "name": "fullscreen",
                "data": null
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "fullscreen",
                "data": "yes"
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "window-minimized",
                "data": true
            }),
            serde_json::json!({
                "event": "end-file",
                "name": "fullscreen",
                "data": true
            }),
        ] {
            assert_eq!(window_property_change(&rejected), None);
        }
    }

    #[test]
    fn window_observation_keeps_true_false_and_unknown_independent() {
        let observation = WindowStateObservation::default();
        assert_eq!(observation.snapshot(), PlaybackWindowState::default());

        observation.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "fullscreen",
            "data": true
        }));
        assert_eq!(
            observation.snapshot(),
            PlaybackWindowState {
                fullscreen: Some(true),
                maximized: None,
            }
        );

        observation.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "fullscreen",
            "data": false
        }));
        observation.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "window-maximized",
            "data": true
        }));
        assert_eq!(
            observation.snapshot(),
            PlaybackWindowState {
                fullscreen: Some(false),
                maximized: Some(true),
            }
        );
    }

    #[test]
    fn display_property_parser_accepts_only_owned_typed_change_events() {
        assert_eq!(
            display_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "display-names",
                "data": ["DP-1", "HDMI-A-1"]
            })),
            Some(DisplayPropertyChange::Names(vec![
                "DP-1".to_string(),
                "HDMI-A-1".to_string()
            ]))
        );
        assert_eq!(
            display_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "display-width",
                "data": 3840
            })),
            Some(DisplayPropertyChange::Width(3840))
        );
        assert_eq!(
            display_property_change(&serde_json::json!({
                "event": "property-change",
                "name": "display-height",
                "data": 2160
            })),
            Some(DisplayPropertyChange::Height(2160))
        );
        for rejected in [
            serde_json::json!({ "request_id": 4, "data": ["DP-1"] }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-names",
                "data": ["DP-1", 2]
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-width",
                "data": "3840"
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-height",
                "data": 0
            }),
        ] {
            assert_eq!(display_property_change(&rejected), None);
        }
    }

    #[test]
    fn display_observation_is_complete_and_isolated_per_launch() {
        let old = WindowStateObservation::default();
        let successor = WindowStateObservation::default();
        for event in [
            serde_json::json!({
                "event": "property-change",
                "name": "display-names",
                "data": ["DP-1"]
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-width",
                "data": 2560
            }),
            serde_json::json!({
                "event": "property-change",
                "name": "display-height",
                "data": 1440
            }),
        ] {
            old.apply_ipc_event(&event);
        }
        assert_eq!(
            old.display_snapshot(),
            crate::display::ObservedDisplay {
                names: vec!["DP-1".to_string()],
                width_px: Some(2560),
                height_px: Some(1440),
            }
        );
        assert_eq!(
            successor.display_snapshot(),
            crate::display::ObservedDisplay::default(),
            "an older IPC reader cannot publish into a successor's handle"
        );
    }

    #[test]
    fn inherited_window_args_emit_known_values_in_override_order() {
        assert!(window_state_args(PlaybackWindowState::default()).is_empty());
        assert_eq!(
            window_state_args(PlaybackWindowState {
                fullscreen: Some(true),
                maximized: None,
            }),
            vec!["--fullscreen=yes"]
        );
        assert_eq!(
            window_state_args(PlaybackWindowState {
                fullscreen: Some(false),
                maximized: Some(true),
            }),
            vec!["--window-maximized=yes", "--fullscreen=no"]
        );
    }

    #[test]
    fn screen_name_uses_the_backend_name_option_and_ignores_empty_values() {
        assert_eq!(
            screen_name_arg(Some("XG27UQDMS (16843009)")),
            Some("--screen-name=XG27UQDMS (16843009)".to_string())
        );
        assert_eq!(screen_name_arg(Some("  ")), None);
        assert_eq!(screen_name_arg(None), None);
    }

    #[test]
    fn playback_window_observation_handles_are_isolated_per_launch() {
        let old = WindowStateObservation::default();
        let successor = WindowStateObservation::default();
        old.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "fullscreen",
            "data": true
        }));
        successor.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "fullscreen",
            "data": false
        }));

        assert_eq!(old.snapshot().fullscreen, Some(true));
        assert_eq!(successor.snapshot().fullscreen, Some(false));

        old.apply_ipc_event(&serde_json::json!({
            "event": "property-change",
            "name": "window-maximized",
            "data": true
        }));
        assert_eq!(successor.snapshot().maximized, None);
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

    fn marker(kind: crate::source::MarkerKind) -> crate::source::MediaMarker {
        crate::source::MediaMarker {
            kind,
            start_ms: 1_000,
            end_ms: 2_000,
        }
    }

    // Injection polarity. Each of these alone must produce nothing: injecting a
    // `--script=` for a missing file makes mpv refuse to start, and a script
    // with no payload or no enabled policy has nothing to act on.
    #[test]
    fn marker_args_inject_nothing_without_script_payload_or_an_enabled_policy() {
        use crate::config::SkipPolicy::{Autoskip, Button, Off};
        assert!(
            markers_args(None, Button, Button, Button, true).is_empty(),
            "no resolved script"
        );
        assert!(
            markers_args(Some("/x/vela-markers.lua"), Button, Autoskip, Button, false).is_empty(),
            "no payload was written"
        );
        assert!(
            markers_args(Some("/x/vela-markers.lua"), Off, Off, Off, true).is_empty(),
            "every policy is off"
        );
    }

    // The script reads these exact dashed option names; mpv would otherwise
    // derive an underscored prefix and silently ignore them.
    #[test]
    fn marker_args_carry_the_script_and_all_three_policies() {
        use crate::config::SkipPolicy::{Autoskip, Button, Off};
        let args = markers_args(Some("/x/vela-markers.lua"), Button, Autoskip, Off, true);
        assert_eq!(
            args,
            vec![
                "--script=/x/vela-markers.lua".to_string(),
                "--script-opts-append=vela-markers-intro-policy=button".to_string(),
                "--script-opts-append=vela-markers-credits-policy=autoskip".to_string(),
                "--script-opts-append=vela-markers-commercial-policy=off".to_string(),
            ]
        );
        assert!(
            !args.iter().any(|arg| arg.contains("PAYLOAD") || arg.contains(".json")),
            "the payload path must never reach the argument list"
        );
    }

    // A payload is written owner-only, and its absence is a normal outcome the
    // caller degrades on rather than an error it propagates.
    #[test]
    fn marker_payload_is_written_owner_only_and_absent_for_no_markers() {
        use crate::source::MarkerKind;
        assert!(
            try_write_marker_payload(&[]).is_none(),
            "no markers means no payload at all"
        );
        let path = try_write_marker_payload(&[marker(MarkerKind::Intro)])
            .expect("a payload is written for real markers");
        let body = std::fs::read_to_string(&path).expect("payload is readable");
        assert!(body.contains("\"kind\":\"intro\""), "body: {body}");
        assert!(body.contains("\"start_ms\":1000"), "body: {body}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "payload must be owner-only from creation");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn header_include_writes_quoted_fields_owner_only() {
        let path = tmp("headers-ok.conf");
        let headers = vec![("X-Plex-Token".to_string(), "abc123".to_string())];
        let first_guard = write_header_include_at(&path, &headers).expect("write include");
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
        drop(first_guard);
        assert!(
            !path.exists(),
            "the first include is removed with its guard"
        );
        // The explicit-path test helper can reuse a path after its prior guard
        // has been dropped. Production paths are always unique per launch.
        let headers = vec![("X-Plex-Token".to_string(), "second".to_string())];
        let second_guard = write_header_include_at(&path, &headers).expect("rewrite include");
        let content = std::fs::read_to_string(&path).expect("read back after overwrite");
        assert_eq!(content, "http-header-fields=\"X-Plex-Token: second\"\n");
        drop(second_guard);
        assert!(
            !path.exists(),
            "the second include is removed with its guard"
        );
    }

    #[test]
    fn header_include_guard_removes_only_its_exact_file() {
        let first = tmp("headers-cleanup-first.conf");
        let second = tmp("headers-cleanup-second.conf");
        let headers = vec![("X-Plex-Token".to_string(), "synthetic".to_string())];
        let first_guard = write_header_include_at(&first, &headers).unwrap();
        let second_guard = write_header_include_at(&second, &headers).unwrap();
        drop(first_guard);
        assert!(!first.exists(), "reaped launch include must be removed");
        assert!(
            second.exists(),
            "one launch must not remove another's include"
        );
        drop(second_guard);
    }

    #[test]
    fn partial_header_include_write_removes_the_credential_file() {
        let path = tmp("headers-partial.conf");
        let token = "synthetic-partial-token";
        let headers = vec![("X-Plex-Token".to_string(), token.to_string())];
        let error = write_header_include_at_with(&path, &headers, |file, content| {
            file.write_all(&content[..content.len() / 2])?;
            Err(std::io::Error::other("synthetic interrupted write"))
        })
        .expect_err("partial writes must fail");
        assert!(
            !error.contains(token),
            "the error must not expose the token"
        );
        assert!(
            !path.exists(),
            "a failed partial write must not leave credential bytes behind"
        );
    }

    #[test]
    fn consumed_include_is_removed_before_a_replacement_launch() {
        let path = tmp("headers-replaced.conf");
        let headers = vec![("X-Plex-Token".to_string(), "synthetic".to_string())];
        let include = write_header_include_at(&path, &headers).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut managed = ManagedChild::new(child, Some(include));

        managed.remove_consumed_header_include().unwrap();
        assert!(
            !path.exists(),
            "the old include must be gone before replacement playback"
        );
        let _ = managed.kill();
        let _ = managed.child.wait();
    }

    #[test]
    fn process_query_result_reaps_only_a_confirmed_exit() {
        assert!(retain_child_after_try_wait(&Ok(None)));
        assert!(retain_child_after_try_wait(&Err(std::io::Error::other(
            "synthetic process-query error"
        ))));
        #[cfg(unix)]
        let exited = {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(0)
        };
        #[cfg(windows)]
        let exited = {
            use std::os::windows::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(0)
        };
        assert!(!retain_child_after_try_wait(&Ok(Some(exited))));
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
