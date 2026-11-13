//! One description of a player launch.
//!
//! [`build_launch`] turns "what to play" ([`PlaySpec`]) and the user's
//! settings into a [`LaunchPlan`]: the ordered mpv options, the environment
//! the player needs, the URL, and the two private files the options refer to.
//! Whichever player starts the plan (see `super::external`) adds nothing and
//! reorders nothing, so the order built here is the order mpv sees.

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::playback::{PlaySpec, PlaybackWindowState};
use crate::player::PlayerKind;

/// One mpv option of a launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LaunchArg {
    /// An option Vela sets. `name` has no leading dashes; `value` is `None`
    /// for a bare flag.
    Option {
        name: &'static str,
        value: Option<String>,
    },
    /// One line of the user's Advanced mpv options, exactly as written.
    User(String),
}

impl LaunchArg {
    fn flag(name: &'static str) -> Self {
        LaunchArg::Option { name, value: None }
    }

    fn set(name: &'static str, value: impl Into<String>) -> Self {
        LaunchArg::Option {
            name,
            value: Some(value.into()),
        }
    }

    /// The command-line spelling: `--name=value`, `--name`, or the user's own
    /// text unchanged.
    pub(crate) fn to_cli(&self) -> String {
        match self {
            LaunchArg::Option {
                name,
                value: Some(value),
            } => format!("--{name}={value}"),
            LaunchArg::Option { name, value: None } => format!("--{name}"),
            LaunchArg::User(text) => text.clone(),
        }
    }
}

/// Environment a player's process needs: variable name and value.
pub(crate) type LaunchEnv = Vec<(&'static str, std::ffi::OsString)>;

/// Everything needed to start one play, in the form a player consumes.
pub(crate) struct LaunchPlan {
    /// The options, in the order mpv must see them. A later option wins over
    /// an earlier one, which is how Vela's protected tail overrides the user.
    pub(crate) args: Vec<LaunchArg>,
    /// Environment the player's process needs.
    pub(crate) env: LaunchEnv,
    pub(crate) url: String,
    /// The owner-only file carrying stream authentication. Owned by the plan,
    /// then by the player that consumed it, and deleted when that player is
    /// reaped.
    pub(crate) header_include: Option<HeaderInclude>,
    /// The marker ranges the script reads once and then deletes.
    pub(crate) marker_payload: Option<std::path::PathBuf>,
}

impl LaunchPlan {
    /// For a plan that will not be started: nothing will read the marker
    /// payload now, so the script's own unlink never happens.
    pub(crate) fn discard_marker_payload(&self) {
        if let Some(path) = &self.marker_payload {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Process-private, owner-only (0700) runtime directory with an unpredictable
/// name, so a local user can't pre-create (symlink) the files we put here the
/// way they could in world-writable `/tmp`. Holds mpv's IPC socket and the
/// auth-header include file. The base stays under `/tmp` so socket paths keep
/// well under the unix socket-path limit (notably macOS's 104 bytes), unlike
/// the long per-user config dir.
#[cfg(not(windows))]
pub(crate) fn private_runtime_dir() -> Result<std::path::PathBuf, String> {
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

/// Where this launch's player opens its JSON IPC endpoint: a socket in the
/// process-private runtime directory, or a named pipe on Windows.
pub(crate) fn new_ipc_path() -> Result<String, String> {
    // mpv emulates the IPC socket with a named pipe under \\.\pipe\ on Windows.
    #[cfg(windows)]
    {
        let pid = std::process::id();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        Ok(format!(r"\\.\pipe\mpv-plex-{}-{}", pid, ts))
    }
    #[cfg(not(windows))]
    {
        ipc_socket_path()
    }
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
pub(crate) struct HeaderInclude {
    path: std::path::PathBuf,
}

impl HeaderInclude {
    pub(super) fn remove_now(&mut self) -> std::io::Result<()> {
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

    /// Remove an include its player has finished reading, before a
    /// replacement launch writes another. `owner` gives the include up only
    /// once the file is gone. When the removal fails the include goes back to
    /// its owner, so that whatever holds that owner (a player waiting to be
    /// reaped, the exit sweep) tries again later, and the replacement fails
    /// closed. An include that is dropped after a failed removal gets one
    /// more attempt and is then a credential file nobody knows about; both
    /// players use this one function so that neither can do that (review
    /// finding ec-7).
    pub(super) fn remove_consumed(owner: &mut Option<HeaderInclude>) -> Result<(), String> {
        let Some(mut include) = owner.take() else {
            return Ok(());
        };
        if include.remove_now().is_ok() {
            return Ok(());
        }
        *owner = Some(include);
        Err("could not remove the prior mpv authentication include".to_string())
    }
}

impl Drop for HeaderInclude {
    fn drop(&mut self) {
        if self.remove_now().is_err() {
            eprintln!("vela: could not remove an mpv authentication include");
        }
    }
}

#[cfg(test)]
impl HeaderInclude {
    /// Where the include was written. Tests only: production code names the
    /// path once, in the option that points mpv at it.
    pub(super) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Write the mpv include file that carries stream auth headers
/// (`http-header-fields`). An include file — not a command-line option —
/// because argv is world-readable (`/proc/<pid>/cmdline`) while this file is
/// owner-only; and not the URL, because mpv renders `${path}` in its title,
/// stats overlay (Shift+I), and playlist. mpv honors `--include` even under
/// `--no-config`, and an include asserted after the user's extra args
/// overrides any `--http-header-fields` they set — both verified against
/// mpv 0.41 (the header reaches the wire exactly once). Each launch gets a
/// unique file, held by the player that consumed it (`external::ManagedChild`)
/// until that exact player is reaped.
fn write_header_include(headers: &[(String, String)]) -> Result<HeaderInclude, String> {
    let path = header_conf_path()?;
    write_header_include_at(&path, headers)
}

/// The testable core of [`write_header_include`]: validates and writes to an
/// explicit path. Header names/values are restricted to characters that cannot
/// escape mpv's quoted conf value or smuggle extra list entries or config
/// lines — anything else fails closed, and the error text never echoes the
/// offending value (it may be a credential).
pub(super) fn write_header_include_at(
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

/// The deterministic, non-media-specific head of every launch: Vela's render
/// defaults, then the user's Advanced options. Per-play resources
/// (credentials, IPC, title, resume point, scripts, and URL) are added by
/// [`build_launch`]; keeping the defaults here makes their ordering directly
/// testable.
fn base_args(cfg: &crate::config::AppConfig, player: PlayerKind) -> (Vec<LaunchArg>, LaunchEnv) {
    if player == PlayerKind::Embedded {
        let mut args = embedded_default_args();
        args.extend(user_args(cfg));
        return (args, Vec::new());
    }
    let use_own_config = cfg.mpv_use_own_config.unwrap_or(false);
    let mut args = Vec::new();
    if !use_own_config {
        // Keep Vela's playback deterministic unless the expert explicitly asks
        // mpv to load their own configuration.
        args.push(LaunchArg::flag("no-config"));
    }
    args.extend([
        LaunchArg::flag("no-ytdl"),
        LaunchArg::set("vo", "gpu-next,gpu"), // gpu-next is the libplacebo HDR path
        LaunchArg::set("profile", "gpu-hq"),
        LaunchArg::set("hwdec", "auto"),
        LaunchArg::set("hwdec-codecs", "all"),
        // Preserve source HDR metadata (PQ/BT.2020) for the compositor or OS.
        LaunchArg::set("target-colorspace-hint", "yes"),
        LaunchArg::set("hdr-compute-peak", "yes"),
    ]);

    // GPU API/context selection is platform-specific.
    // * Linux: Vulkan via Wayland/X11 WSI is the reliable HDR path.
    // * macOS: mpv only exposes Vulkan over Metal (macvk), so let it auto-pick.
    // * Windows: D3D11 negotiates HDR through the DXGI swapchain.
    #[cfg(target_os = "macos")]
    {
        args.push(LaunchArg::set("gpu-api", "vulkan")); // macvk (Metal-backed) only context on macOS
    }
    #[cfg(target_os = "windows")]
    {
        args.push(LaunchArg::set("gpu-api", "d3d11")); // HDR via DXGI swapchain on Windows 10/11
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Force Vulkan (the HDR-capable backend, always built into modern mpv)
        // but let mpv auto-pick a context. Naming specific backends (e.g.
        // x11vk) risks listing one a given build lacks — mpv validates option
        // values up front and would exit after spawn() succeeded (silent failure).
        args.push(LaunchArg::set("gpu-api", "vulkan"));
    }
    let env = if cfg!(any(target_os = "macos", target_os = "windows")) {
        Vec::new()
    } else {
        // Enables the experimental Vulkan HDR WSI layer where present (KDE/NVIDIA).
        vec![("ENABLE_HDR_WSI", std::ffi::OsString::from("1"))]
    };

    #[cfg(target_os = "macos")]
    if !use_own_config {
        // AVFoundation owns AirPlay presentation timing and follows system
        // route changes. CoreAudio remains initialization fallback.
        args.push(LaunchArg::set("ao", "avfoundation,coreaudio"));
    }

    // User-supplied advanced options remain last among defaults, so repeated
    // mpv options retain their documented last-option-wins behavior.
    args.extend(user_args(cfg));

    (args, env)
}

/// The user's Advanced mpv options, one per line, exactly as written.
fn user_args(cfg: &crate::config::AppConfig) -> Vec<LaunchArg> {
    cfg.mpv_extra_args
        .as_deref()
        .map(parse_extra_mpv_args)
        .unwrap_or_default()
        .into_iter()
        .map(LaunchArg::User)
        .collect()
}

/// Vela's defaults for the embedded player, before the user's options.
///
/// What the external player's list has and this one leaves out, and why
/// (`.agents/plans/embedded-player.md`, "The seam"):
/// - `no-config`: mpv's library never loads the user's configuration on its
///   own. The embedded launcher applies the user's `mpv.conf` itself when
///   asked to, before these options.
/// - `vo`: asserted in the protected tail instead, because the embedded
///   player cannot work with any other value.
/// - `gpu-api`, `ENABLE_HDR_WSI`, `target-colorspace-hint`: these make mpv's
///   own window negotiate HDR with the compositor. Embedded, Vela owns the
///   surface and tags it itself.
/// - the macOS audio default: the embedded player exists on Linux only.
fn embedded_default_args() -> Vec<LaunchArg> {
    vec![
        LaunchArg::flag("no-ytdl"),
        LaunchArg::set("profile", "gpu-hq"),
        LaunchArg::set("hwdec", "auto"),
        LaunchArg::set("hwdec-codecs", "all"),
        LaunchArg::set("hdr-compute-peak", "yes"),
        // mpv's own on-screen controller answers the pointer, and no pointer
        // reaches an embedded mpv. Vela's page draws the controls.
        LaunchArg::set("osc", "no"),
    ]
}

/// Explicit inherited flags follow user/autocrop options so observed runtime
/// state wins under mpv's last-value-wins option handling. Unknown properties
/// are omitted, preserving normal configuration behavior.
fn window_state_args(state: PlaybackWindowState) -> Vec<LaunchArg> {
    let mut args = Vec::with_capacity(2);
    if let Some(maximized) = state.maximized {
        args.push(LaunchArg::set(
            "window-maximized",
            if maximized { "yes" } else { "no" },
        ));
    }
    if let Some(fullscreen) = state.fullscreen {
        args.push(LaunchArg::set(
            "fullscreen",
            if fullscreen { "yes" } else { "no" },
        ));
    }
    args
}

/// Options Vela's session lifecycle depends on. Asserted after every
/// user-controlled option so neither Advanced arguments nor the expert's own
/// mpv.conf can leave mpv running after its file ends.
///
/// Windows mpv started without a console or redirected standard handles (which
/// is how a GUI app starts it) turns idle mode on and then never exits at end
/// of file. The trackers' `on_end` runs only once mpv's IPC connection drops,
/// so an idling mpv means no recents stamp, no `playback-ended`, and no
/// continuation, ever.
///
/// The embedded player is created first and given its file afterwards, once
/// its surface exists. With `no` mpv's library shuts down in that gap, before
/// the file arrives; `once` waits for the first file and exits at its end,
/// which is the same lifecycle (plan review ep-1, proved in Slice 0, A9).
fn lifecycle_args(player: PlayerKind) -> [LaunchArg; 1] {
    match player {
        PlayerKind::External => [LaunchArg::set("idle", "no")],
        PlayerKind::Embedded => [LaunchArg::set("idle", "once")],
    }
}

/// What the embedded player cannot work without, asserted after every option
/// the user gives: mpv draws through the library's render interface into
/// the surface Vela gives it, and nowhere else.
///
/// And `force-window` off, so that no option opens that video output before
/// a file: with `force-window` mpv opens it as it starts or goes idle, and
/// then the player's own thread, which hands mpv its file with a call that
/// waits (`Live::load`), could be waiting for mpv while mpv waits for that
/// thread to draw (review finding hd-3). `force-window=immediate` also made
/// the library fail to start at all. Vela's window is there either way.
/// This holds for options only: a conditional profile in the user's mpv.conf,
/// applied once mpv runs, or a command on its IPC socket, can still turn
/// `force-window` on (see `Live::load`).
fn embedded_required_args() -> [LaunchArg; 2] {
    [
        LaunchArg::set("vo", "libmpv"),
        LaunchArg::set("force-window", "no"),
    ]
}

fn screen_name_arg(name: Option<&str>) -> Option<LaunchArg> {
    name.map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| LaunchArg::set("screen-name", name))
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
fn autocrop_args(mode: &str, script: Option<&str>, shim: Option<&str>) -> Vec<LaunchArg> {
    let Some(path) = script else {
        return Vec::new();
    };
    match (mode, shim) {
        ("manual", _) => vec![
            LaunchArg::set("script", path),
            LaunchArg::set("script-opts-append", "autocrop-auto=no"),
        ],
        ("auto", Some(shim_path)) => vec![
            LaunchArg::set("script", path),
            LaunchArg::set("script-opts-append", "autocrop-auto=no"),
            LaunchArg::set("script", shim_path),
        ],
        ("auto", None) => vec![LaunchArg::set("script", path)],
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
) -> Vec<LaunchArg> {
    let Some(path) = script else {
        return Vec::new();
    };
    if !has_payload || (intro.is_off() && credits.is_off() && commercial.is_off()) {
        return Vec::new();
    }
    vec![
        LaunchArg::set("script", path),
        LaunchArg::set(
            "script-opts-append",
            format!("vela-markers-intro-policy={}", intro.as_option_value()),
        ),
        LaunchArg::set(
            "script-opts-append",
            format!("vela-markers-credits-policy={}", credits.as_option_value()),
        ),
        LaunchArg::set(
            "script-opts-append",
            format!(
                "vela-markers-commercial-policy={}",
                commercial.as_option_value()
            ),
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

/// One at a time, for tests that write a marker payload.
///
/// Every test runs in the same process, and writing a payload first removes
/// every payload that process wrote before ([`prune_marker_payloads`]). In the
/// app that is right: plays are started one at a time. Between two tests
/// running side by side it deletes the file the other is about to read. A test
/// holds this from before its payload is written until it has finished with the
/// file, and [`try_write_marker_payload`] refuses to write for a test that does
/// not, so forgetting it fails every run instead of one run in thirty.
#[cfg(test)]
pub(crate) mod payload_test_lock {
    use std::cell::Cell;
    use std::sync::{Mutex, MutexGuard};

    static LOCK: Mutex<()> = Mutex::new(());
    thread_local! {
        static HELD: Cell<bool> = const { Cell::new(false) };
    }

    /// The lock, held until dropped.
    pub(crate) struct Held {
        _guard: MutexGuard<'static, ()>,
    }

    /// Wait for the other payload-writing tests to finish, then keep them out.
    pub(crate) fn hold() -> Held {
        // A test that failed while holding it has already reported; the lock
        // guards files, not data, so a poisoned one is still usable.
        let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        HELD.with(|held| held.set(true));
        Held { _guard: guard }
    }

    impl Drop for Held {
        fn drop(&mut self) {
            HELD.with(|held| held.set(false));
        }
    }

    /// Whether the calling test holds the lock right now.
    pub(crate) fn is_held() -> bool {
        HELD.with(Cell::get)
    }
}

/// Write the marker payload for one launch. Non-fatal by construction: every
/// failure returns `None` and playback proceeds without skipping. Never `?` this
/// into the play path.
fn try_write_marker_payload(markers: &[crate::source::MediaMarker]) -> Option<std::path::PathBuf> {
    if markers.is_empty() {
        return None;
    }
    #[cfg(test)]
    assert!(
        payload_test_lock::is_held(),
        "a test that writes a marker payload must hold payload_test_lock::hold() \
         until it is done with the file: the next write removes it"
    );
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

/// Describe the complete launch for one play: Vela's defaults and the user's
/// options first, then the protected tail that no user option may override.
/// Writes the auth include and the marker payload as a side effect; starts
/// nothing.
pub(crate) fn build_launch(
    spec: &PlaySpec,
    cfg: &crate::config::AppConfig,
    ipc_path: &str,
) -> Result<LaunchPlan, String> {
    build_launch_for(spec, spec.player, cfg, ipc_path)
}

/// The launch of `spec` in `player`, which is the player the play was given
/// except in one case: a play given the embedded player that it could not
/// start is launched in the external one instead (`crate::playback::play`).
pub(crate) fn build_launch_for(
    spec: &PlaySpec,
    player: PlayerKind,
    cfg: &crate::config::AppConfig,
    ipc_path: &str,
) -> Result<LaunchPlan, String> {
    // Which player this launch is for decides a handful of options, here and
    // in `base_args`. The list is never edited afterwards, by either player.
    let embedded = player == PlayerKind::Embedded;
    let (mut args, mut env) = base_args(cfg, player);

    // Bundled mpv autocrop script (Settings → Advanced). Placed with the user
    // extra args (before the re-asserted IPC/title/URL block) so it can't clobber
    // the socket. Only injected when enabled AND the bundled script actually
    // exists — a missing `--script=` would make mpv refuse to launch, so skip and
    // log instead. `autocrop-auto=no` in Manual mode keeps the known live
    // `video-crop` hang off the every-play path (see config `mpv_autocrop`).
    //
    // External only. The stock script measures black bars with an FFmpeg
    // filter the library Vela ships does not contain, and it is GPL code that
    // must never be loaded into Vela's own process; the embedded player gets a
    // crop script of its own (plan, "Autocrop in the embedded player").
    let autocrop_mode = cfg.mpv_autocrop.as_deref().unwrap_or("off");
    if autocrop_mode != "off" && !embedded {
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
        args.extend(autocrop_args(autocrop_mode, script, shim));
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
        args.extend(markers_args(
            script,
            spec.intro_policy,
            spec.credits_policy,
            spec.commercial_policy,
            payload.is_some(),
        ));
        if let Some(path) = &payload {
            if embedded {
                // An embedded mpv has no environment of its own, and setting
                // the variable on Vela's process would leak it to every later
                // child. The script reads this option first. It names a file
                // in Vela's private directory and reaches mpv through an
                // owner-only options file, never an argument list.
                args.push(LaunchArg::set(
                    "script-opts-append",
                    format!("vela-markers-payload={}", path.to_string_lossy()),
                ));
            } else {
                // The player's environment only, and never an option: the
                // payload path must not appear in the process argument list.
                env.push(("VELA_MARKERS_PAYLOAD", path.as_os_str().to_owned()));
            }
        }
        payload
    };

    if !embedded {
        // The embedded player has no window of its own to place or size.
        args.extend(window_state_args(spec.inherited_window_state));
        if let Some(screen_arg) = screen_name_arg(spec.screen_name.as_deref()) {
            // Reassert after user options. On Wayland this is a placement request,
            // not a guarantee; the compositor's actual choice is observed over IPC.
            args.push(screen_arg);
        }
    }

    args.extend(lifecycle_args(player));
    args.push(LaunchArg::set("input-ipc-server", ipc_path));
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
    args.push(LaunchArg::set("force-media-title", display_title));
    if embedded {
        args.extend(embedded_required_args());
    } else {
        // The window title. An embedded mpv has no window.
        args.push(LaunchArg::set("title", display_title));
    }
    // Stream auth (e.g. Plex's X-Plex-Token) rides in an owner-only include
    // file — see write_header_include for why neither argv nor the URL may
    // carry it. Asserted after the user's extra args so a user-set
    // --http-header-fields can't silently drop the auth the stream needs.
    let header_include = if spec.http_headers.is_empty() {
        None
    } else {
        let include = write_header_include(&spec.http_headers)?;
        args.push(LaunchArg::set("include", include.path.to_string_lossy()));
        Some(include)
    };
    if spec.start_seconds > 0.0 {
        args.push(LaunchArg::set("start", spec.start_seconds.to_string()));
    }

    Ok(LaunchPlan {
        args,
        env,
        url: spec.url.clone(),
        header_include,
        marker_payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A play with nothing optional, for the tests below to build on.
    fn plain_spec(player: PlayerKind, url: &str) -> PlaySpec {
        PlaySpec {
            player,
            url: url.to_string(),
            title: String::new(),
            http_headers: Vec::new(),
            start_seconds: 0.0,
            autocrop_script: None,
            autocrop_shim: None,
            inherited_window_state: PlaybackWindowState::default(),
            screen_name: None,
            window_observation: crate::playback::WindowStateObservation::default(),
            markers_script: None,
            markers: Vec::new(),
            intro_policy: crate::config::SkipPolicy::Off,
            credits_policy: crate::config::SkipPolicy::Off,
            commercial_policy: crate::config::SkipPolicy::Off,
            step_down: None,
            duration: None,
            osd_notice: None,
            on_notice_shown: None,
            if_embedded_cannot_start: Default::default(),
            steps_taken: 0,
        }
    }

    // The assertions in this module were written against command-line
    // spellings, before a launch was described as data. These wrappers keep
    // them exactly as they were: each renders what the function of the same
    // name now returns.
    fn cli(args: impl IntoIterator<Item = LaunchArg>) -> Vec<String> {
        args.into_iter().map(|arg| arg.to_cli()).collect()
    }

    fn base_command_args(cfg: crate::config::AppConfig) -> Vec<String> {
        cli(base_args(&cfg, PlayerKind::External).0)
    }

    fn window_state_args(state: PlaybackWindowState) -> Vec<String> {
        cli(super::window_state_args(state))
    }

    fn lifecycle_args() -> Vec<String> {
        cli(super::lifecycle_args(PlayerKind::External))
    }

    fn screen_name_arg(name: Option<&str>) -> Option<String> {
        super::screen_name_arg(name).map(|arg| arg.to_cli())
    }

    fn autocrop_args(mode: &str, script: Option<&str>, shim: Option<&str>) -> Vec<String> {
        cli(super::autocrop_args(mode, script, shim))
    }

    fn markers_args(
        script: Option<&str>,
        intro: crate::config::SkipPolicy,
        credits: crate::config::SkipPolicy,
        commercial: crate::config::SkipPolicy,
        has_payload: bool,
    ) -> Vec<String> {
        cli(super::markers_args(
            script, intro, credits, commercial, has_payload,
        ))
    }

    #[test]
    fn a_launch_argument_renders_as_a_flag_an_assignment_or_the_users_own_text() {
        assert_eq!(LaunchArg::flag("no-config").to_cli(), "--no-config");
        assert_eq!(LaunchArg::set("vo", "gpu-next,gpu").to_cli(), "--vo=gpu-next,gpu");
        // A value is never quoted or escaped: each argument is one argv entry.
        assert_eq!(
            LaunchArg::set("title", "A: B = C").to_cli(),
            "--title=A: B = C"
        );
        assert_eq!(LaunchArg::User("-v".to_string()).to_cli(), "-v");
        assert_eq!(
            LaunchArg::User("--sub-font=Noto Sans".to_string()).to_cli(),
            "--sub-font=Noto Sans"
        );
    }

    #[test]
    fn base_mpv_command_preserves_render_defaults_without_managed_sync_overrides() {
        let args = base_command_args(crate::config::AppConfig::default());

        for expected in [
            "--no-config",
            "--no-ytdl",
            "--vo=gpu-next,gpu",
            "--profile=gpu-hq",
            "--hwdec=auto",
            "--hwdec-codecs=all",
            "--target-colorspace-hint=yes",
            "--hdr-compute-peak=yes",
        ] {
            assert!(args.iter().any(|arg| arg == expected), "missing {expected}");
        }
        #[cfg(target_os = "windows")]
        assert!(args.iter().any(|arg| arg == "--gpu-api=d3d11"));
        #[cfg(not(target_os = "windows"))]
        assert!(args.iter().any(|arg| arg == "--gpu-api=vulkan"));
        assert!(
            args.iter()
                .all(|arg| !arg.starts_with("--audio-delay") && !arg.starts_with("--audio-device")),
            "Vela must not guess AirPlay delay or pin an output device: {args:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_base_mpv_command_prefers_avfoundation_before_advanced_overrides() {
        let cfg = crate::config::AppConfig {
            mpv_extra_args: Some("# expert override\n--volume=80\n--ao=coreaudio".into()),
            ..Default::default()
        };
        let args = base_command_args(cfg);
        let preferred = args
            .iter()
            .position(|arg| arg == "--ao=avfoundation,coreaudio")
            .expect("macOS default must prefer AVFoundation with CoreAudio fallback");
        let override_position = args
            .iter()
            .position(|arg| arg == "--ao=coreaudio")
            .expect("Advanced AO override must be retained");

        assert_eq!(
            args.iter()
                .filter(|arg| *arg == "--ao=avfoundation,coreaudio")
                .count(),
            1,
            "Vela's AO preference must be appended exactly once"
        );
        assert!(
            preferred < override_position,
            "Advanced options must remain last-option-wins: {args:?}"
        );
    }

    #[test]
    fn own_mpv_config_owns_audio_output_selection() {
        let cfg = crate::config::AppConfig {
            mpv_use_own_config: Some(true),
            mpv_extra_args: Some("--ao=coreaudio".into()),
            ..Default::default()
        };
        let args = base_command_args(cfg);

        assert!(!args.iter().any(|arg| arg == "--no-config"));
        assert!(
            !args.iter().any(|arg| arg == "--ao=avfoundation,coreaudio"),
            "own-config mode must not override ao from mpv.conf"
        );
        assert!(
            args.iter().any(|arg| arg == "--ao=coreaudio"),
            "Advanced options remain authoritative in own-config mode"
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_base_command_does_not_set_an_audio_output() {
        let args = base_command_args(crate::config::AppConfig::default());
        assert!(
            args.iter().all(|arg| !arg.starts_with("--ao=")),
            "the AVFoundation policy is macOS-only: {args:?}"
        );
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("vela-test-{}-{name}", std::process::id()))
    }

    // Nothing can be removed from a directory that cannot be written to,
    // which is how these tests make a removal fail and then let it succeed.
    // (Not for root, who can remove it anyway; the test then proves nothing
    // and says so by returning early.)
    #[cfg(unix)]
    #[test]
    fn an_include_whose_removal_fails_stays_with_its_owner_until_it_is_removed() {
        use std::os::unix::fs::PermissionsExt;
        // SAFETY: takes nothing and cannot fail.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let dir = tmp("include-kept");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lock = |mode: u32| {
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap()
        };
        let path = dir.join("headers.conf");
        let headers = vec![("X-Plex-Token".to_string(), "synthetic".to_string())];
        let mut owner = Some(write_header_include_at(&path, &headers).unwrap());

        lock(0o500);
        assert!(HeaderInclude::remove_consumed(&mut owner).is_err());
        assert!(owner.is_some(), "a failed removal leaves the include owned");
        // Asked again while it still cannot be done: still owned.
        assert!(HeaderInclude::remove_consumed(&mut owner).is_err());
        assert!(owner.is_some());
        lock(0o700);
        assert!(path.exists(), "and the file is still there to be removed");

        // Whatever was in the way has cleared; the same owner finishes it.
        assert!(HeaderInclude::remove_consumed(&mut owner).is_ok());
        assert!(owner.is_none());
        assert!(!path.exists());
        // Nothing owned: nothing to do, and not an error.
        assert!(HeaderInclude::remove_consumed(&mut owner).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
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
    fn lifecycle_args_make_mpv_exit_when_its_file_ends() {
        assert_eq!(lifecycle_args(), ["--idle=no"]);
    }

    // The embedded player is created before it is given its file. `no` would
    // shut the library down in that gap and `yes` would never end the session;
    // `once` waits for the first file and exits at its end (plan review ep-1).
    #[test]
    fn the_embedded_player_waits_for_its_first_file_and_exits_at_its_end() {
        assert_eq!(
            cli(super::lifecycle_args(PlayerKind::Embedded)),
            ["--idle=once"]
        );
    }

    // Everything a launch for the embedded player is made of, in order, with
    // every optional part present and a user who tries to take over each thing
    // the player depends on. Rendered as command-line text only to be read;
    // the embedded player applies the same list through an options file.
    #[cfg(not(windows))]
    #[test]
    fn the_full_embedded_launch_is_exactly_the_recorded_one() {
        use crate::config::SkipPolicy;
        use crate::source::MarkerKind;

        let dir = tmp("embedded-launch");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory");
        let script = |name: &str| {
            let path = dir.join(name);
            std::fs::write(&path, "-- test stand-in\n").expect("script stand-in");
            path.to_string_lossy().into_owned()
        };
        let autocrop = script("autocrop.lua");
        let shim = script("vela-autocrop.lua");
        let markers = script("vela-markers.lua");

        let cfg = crate::config::AppConfig {
            mpv_extra_args: Some(
                "--volume=70\n--vo=gpu-next\n--force-window=yes\n--idle=yes\n--input-ipc-server=/tmp/not-velas"
                    .to_string(),
            ),
            // The stock crop script must not be named for an embedded play.
            mpv_autocrop: Some("auto".to_string()),
            // Loading the user's own configuration is the embedded launcher's
            // job; it changes nothing in the list.
            mpv_use_own_config: Some(true),
            ..Default::default()
        };
        let spec = PlaySpec {
            title: "Recorded: Title".to_string(),
            http_headers: vec![("X-Plex-Token".to_string(), "abc123".to_string())],
            start_seconds: 123.5,
            autocrop_script: Some(autocrop),
            autocrop_shim: Some(shim),
            inherited_window_state: PlaybackWindowState {
                fullscreen: Some(true),
                maximized: Some(false),
            },
            screen_name: Some("DP-3".to_string()),
            markers_script: Some(markers.clone()),
            markers: vec![marker(MarkerKind::Intro)],
            intro_policy: SkipPolicy::Button,
            credits_policy: SkipPolicy::Autoskip,
            commercial_policy: SkipPolicy::Off,
            ..plain_spec(
                PlayerKind::Embedded,
                "https://media.invalid/video/stream.mkv?x=1",
            )
        };

        let _payloads = payload_test_lock::hold();
        let plan = build_launch(&spec, &cfg, "/tmp/vela-recorded/mpv.sock")
            .expect("the launch is described");
        let include_path = plan
            .header_include
            .as_ref()
            .expect("stream headers produce an include")
            .path()
            .to_path_buf();
        let payload_path = plan
            .marker_payload
            .clone()
            .expect("enabled markers produce a payload");
        assert!(payload_path.is_file(), "the payload is written");

        let expected: Vec<String> = [
            "--no-ytdl".to_string(),
            "--profile=gpu-hq".to_string(),
            "--hwdec=auto".to_string(),
            "--hwdec-codecs=all".to_string(),
            "--hdr-compute-peak=yes".to_string(),
            "--osc=no".to_string(),
            "--volume=70".to_string(),
            "--vo=gpu-next".to_string(),
            "--force-window=yes".to_string(),
            "--idle=yes".to_string(),
            "--input-ipc-server=/tmp/not-velas".to_string(),
            format!("--script={markers}"),
            "--script-opts-append=vela-markers-intro-policy=button".to_string(),
            "--script-opts-append=vela-markers-credits-policy=autoskip".to_string(),
            "--script-opts-append=vela-markers-commercial-policy=off".to_string(),
            format!(
                "--script-opts-append=vela-markers-payload={}",
                payload_path.to_string_lossy()
            ),
            "--idle=once".to_string(),
            "--input-ipc-server=/tmp/vela-recorded/mpv.sock".to_string(),
            "--force-media-title=Recorded: Title".to_string(),
            "--vo=libmpv".to_string(),
            "--force-window=no".to_string(),
            format!("--include={}", include_path.to_string_lossy()),
            "--start=123.5".to_string(),
        ]
        .to_vec();
        assert_eq!(cli(plan.args.iter().cloned()), expected);
        assert!(
            plan.env.is_empty(),
            "an embedded mpv has no environment of its own to add to"
        );
        assert_eq!(plan.url, "https://media.invalid/video/stream.mkv?x=1");

        drop(plan);
        let _ = std::fs::remove_file(&payload_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_minimal_embedded_launch_is_exactly_the_recorded_one() {
        let cfg = crate::config::AppConfig::default();
        let spec = plain_spec(PlayerKind::Embedded, "edl://not-a-url-with-a-dash");
        let plan = build_launch(&spec, &cfg, "/tmp/vela-recorded/min.sock")
            .expect("the launch is described");
        assert_eq!(
            cli(plan.args.iter().cloned()),
            [
                "--no-ytdl",
                "--profile=gpu-hq",
                "--hwdec=auto",
                "--hwdec-codecs=all",
                "--hdr-compute-peak=yes",
                "--osc=no",
                "--idle=once",
                "--input-ipc-server=/tmp/vela-recorded/min.sock",
                "--force-media-title=Vela",
                "--vo=libmpv",
                "--force-window=no",
            ]
        );
        assert!(plan.env.is_empty());
        assert!(plan.header_include.is_none());
        assert!(plan.marker_payload.is_none());
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
        let _payloads = payload_test_lock::hold();
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

    // The two tests below keep the payload tests from deleting each other's
    // files (see `payload_test_lock`). Without the refusal, a test that forgot
    // the lock would fail only now and then, in whichever test lost the race.
    #[test]
    #[should_panic(expected = "must hold payload_test_lock::hold()")]
    fn a_test_that_writes_a_marker_payload_without_the_lock_is_refused() {
        use crate::source::MarkerKind;
        let _ = try_write_marker_payload(&[marker(MarkerKind::Intro)]);
    }

    #[test]
    fn the_payload_test_lock_is_held_only_while_its_guard_lives() {
        assert!(!payload_test_lock::is_held());
        let held = payload_test_lock::hold();
        assert!(payload_test_lock::is_held());
        drop(held);
        assert!(
            !payload_test_lock::is_held(),
            "a finished test must not leave the next one on its thread covered"
        );
    }

    #[test]
    fn the_payload_test_lock_keeps_a_second_test_out_until_the_first_is_done() {
        use std::time::Duration;
        let first = payload_test_lock::hold();
        let (got_in, second_got_in) = std::sync::mpsc::channel();
        let second = std::thread::spawn(move || {
            let _second = payload_test_lock::hold();
            let _ = got_in.send(());
        });
        assert!(
            second_got_in
                .recv_timeout(Duration::from_millis(300))
                .is_err(),
            "a second test got in while the first still held the lock"
        );
        drop(first);
        second_got_in
            .recv_timeout(Duration::from_secs(30))
            .expect("the second test gets in once the first is done");
        second.join().expect("the second holder finishes");
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
