//! The external player: mpv as a separate program in its own window.
//!
//! mpv owns its own Vulkan/Wayland window, which is the reliable way to get
//! true HDR passthrough (10-bit PQ/BT.2020 negotiated with the compositor).
//! This module finds the mpv program, turns a [`LaunchPlan`] into its command
//! line, starts it, and owns the resulting child process.

use std::process::Command;

use super::launch::{HeaderInclude, LaunchPlan};
use super::PlayerHandle;

/// A child process and the private credential file it consumed. The reaper
/// drops this wrapper only after `try_wait` reports exit. Query failures retain
/// the wrapper so a transient OS error cannot orphan the credential include.
pub(crate) struct ManagedChild {
    child: std::process::Child,
    _header_include: Option<HeaderInclude>,
}

impl ManagedChild {
    /// A child with no credential include, for tests of the handle around it.
    #[cfg(test)]
    pub(crate) fn for_test(child: std::process::Child) -> Self {
        Self::new(child, None)
    }

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

/// The mpv command for a plan: every option in the plan's order, the plan's
/// environment, then the URL. Nothing is added, dropped or reordered here.
pub(crate) fn command(mpv_bin: &str, plan: &LaunchPlan) -> Command {
    let mut cmd = Command::new(mpv_bin);
    for arg in &plan.args {
        cmd.arg(arg.to_cli());
    }
    for (name, value) in &plan.env {
        cmd.env(name, value);
    }
    // `--` terminates option parsing so a URL/path that begins with `-` (e.g. a
    // hostile filename from a local folder or server) can't be read as an mpv
    // option. In practice our URLs are absolute paths / http(s) / edl://, but
    // this closes the option-injection vector regardless.
    cmd.arg("--").arg(&plan.url);
    cmd
}

/// Start mpv for a plan. The returned handle owns the child and the plan's
/// credential include. A plan that cannot be started leaves nothing behind.
pub(crate) fn launch(mpv_bin: &str, plan: LaunchPlan) -> Result<PlayerHandle, String> {
    let child = command(mpv_bin, &plan).spawn().map_err(|e| {
        plan.discard_marker_payload();
        if e.kind() == std::io::ErrorKind::NotFound {
            "mpv was not found. Install mpv to play video.".to_string()
        } else {
            format!("failed to launch mpv: {}", e)
        }
    })?;
    Ok(PlayerHandle::External(ManagedChild::new(
        child,
        plan.header_include,
    )))
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

#[cfg(test)]
mod tests {
    use super::super::launch::{build_launch, payload_test_lock, write_header_include_at};
    use super::*;
    use crate::playback::{PlaySpec, PlaybackWindowState, WindowStateObservation};

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("vela-test-{}-{name}", std::process::id()))
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

    // ---- The recorded external command line ------------------------------
    //
    // These two tests pin the COMPLETE mpv command Vela builds: every argument,
    // in order, plus the child environment. They were recorded from the code
    // as it stood before the player seam was introduced
    // (.agents/plans/embedded-player.md, Slice 1), so that refactor, and any
    // later change to how a launch is described, cannot move, drop, add or
    // reorder an argument of the external player without failing here. Change
    // an expected list only for a deliberate change to the external player.

    /// The parts of the command that differ per platform, exactly as
    /// `launch::base_args` documents them.
    fn recorded_platform_defaults(own_config: bool) -> Vec<String> {
        let gpu_api = if cfg!(target_os = "windows") {
            "--gpu-api=d3d11"
        } else {
            "--gpu-api=vulkan"
        };
        let mut args = vec![gpu_api.to_string()];
        if cfg!(target_os = "macos") && !own_config {
            args.push("--ao=avfoundation,coreaudio".to_string());
        }
        args
    }

    /// The child environment every launch gets, per platform.
    fn recorded_base_env() -> Vec<(String, Option<String>)> {
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            Vec::new()
        } else {
            vec![("ENABLE_HDR_WSI".to_string(), Some("1".to_string()))]
        }
    }

    fn recorded_spec(url: &str) -> PlaySpec {
        PlaySpec {
            player: crate::player::PlayerKind::External,
            url: url.to_string(),
            title: String::new(),
            http_headers: Vec::new(),
            start_seconds: 0.0,
            autocrop_script: None,
            autocrop_shim: None,
            inherited_window_state: PlaybackWindowState::default(),
            screen_name: None,
            window_observation: WindowStateObservation::default(),
            markers_script: None,
            markers: Vec::new(),
            intro_policy: crate::config::SkipPolicy::Off,
            credits_policy: crate::config::SkipPolicy::Off,
            commercial_policy: crate::config::SkipPolicy::Off,
            step_down: None,
            duration: None,
            osd_notice: None,
            on_notice_shown: None,
            steps_taken: 0,
        }
    }

    fn command_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn command_envs(cmd: &Command) -> Vec<(String, Option<String>)> {
        cmd.get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect()
    }

    /// The environment a started command really gets. A child inherits all of
    /// Vela's own environment (`own`, as it stands when the child is started)
    /// unless the command says otherwise, so what the command adds
    /// (`command_envs`) is only part of it.
    fn effective_env(
        cmd: &Command,
        own: &[(std::ffi::OsString, std::ffi::OsString)],
    ) -> Vec<(String, String)> {
        let mut env: std::collections::BTreeMap<std::ffi::OsString, std::ffi::OsString> =
            own.iter().cloned().collect();
        for (key, value) in cmd.get_envs() {
            match value {
                Some(value) => env.insert(key.to_owned(), value.to_owned()),
                None => env.remove(key),
            };
        }
        env.into_iter()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect()
    }

    // The auth include and the marker payload live in the process-private
    // runtime directory, which on Windows is the user's real config directory.
    #[cfg(not(windows))]
    #[test]
    fn the_full_external_command_line_is_exactly_the_recorded_one() {
        use crate::config::SkipPolicy;
        use crate::source::{MarkerKind, MediaMarker};

        let dir = tmp("recorded-command");
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
            // Two ordinary options and two that try to take over what Vela
            // protects; all four must appear, verbatim, before Vela's tail.
            mpv_extra_args: Some(
                "# a comment\n--volume=70\n\n--sub-font=Noto Sans\n--idle=yes\n--input-ipc-server=/tmp/not-velas"
                    .to_string(),
            ),
            mpv_autocrop: Some("auto".to_string()),
            ..Default::default()
        };
        let spec = PlaySpec {
            title: "Recorded: Title".to_string(),
            http_headers: vec![("X-Plex-Token".to_string(), "abc123".to_string())],
            start_seconds: 123.5,
            autocrop_script: Some(autocrop.clone()),
            autocrop_shim: Some(shim.clone()),
            inherited_window_state: PlaybackWindowState {
                fullscreen: Some(true),
                maximized: Some(false),
            },
            screen_name: Some("DP-3".to_string()),
            markers_script: Some(markers.clone()),
            markers: vec![MediaMarker {
                kind: MarkerKind::Intro,
                start_ms: 1_000,
                end_ms: 2_000,
            }],
            intro_policy: SkipPolicy::Button,
            credits_policy: SkipPolicy::Autoskip,
            commercial_policy: SkipPolicy::Off,
            ..recorded_spec("https://media.invalid/video/stream.mkv?x=1")
        };

        // Held to the end of the test: the payload this launch writes is read
        // back below, and another test's write would remove it.
        let _payloads = payload_test_lock::hold();
        let plan = build_launch(&spec, &cfg, "/tmp/vela-recorded/mpv.sock")
            .expect("the launch is described");
        let cmd = command("mpv", &plan);
        assert_eq!(cmd.get_program().to_string_lossy(), "mpv");

        // The two per-launch paths are unpredictable; check what they point at,
        // then compare them by a fixed stand-in.
        let include_path = plan
            .header_include
            .as_ref()
            .expect("stream headers produce an include")
            .path()
            .to_path_buf();
        assert_eq!(
            std::fs::read_to_string(&include_path).expect("include is readable"),
            "http-header-fields=\"X-Plex-Token: abc123\"\n"
        );
        let payload_path = plan
            .marker_payload
            .clone()
            .expect("enabled markers produce a payload");
        assert_eq!(
            std::fs::read_to_string(&payload_path).expect("payload is readable"),
            "{\"markers\":[{\"kind\":\"intro\",\"start_ms\":1000,\"end_ms\":2000}]}"
        );
        let include_arg = format!("--include={}", include_path.to_string_lossy());
        let args: Vec<String> = command_args(&cmd)
            .into_iter()
            .map(|arg| {
                if arg == include_arg {
                    "--include=<AUTH INCLUDE>".to_string()
                } else {
                    arg
                }
            })
            .collect();

        let mut expected: Vec<String> = [
            "--no-config",
            "--no-ytdl",
            "--vo=gpu-next,gpu",
            "--profile=gpu-hq",
            "--hwdec=auto",
            "--hwdec-codecs=all",
            "--target-colorspace-hint=yes",
            "--hdr-compute-peak=yes",
        ]
        .map(str::to_string)
        .to_vec();
        expected.extend(recorded_platform_defaults(false));
        expected.extend(
            [
                "--volume=70".to_string(),
                "--sub-font=Noto Sans".to_string(),
                "--idle=yes".to_string(),
                "--input-ipc-server=/tmp/not-velas".to_string(),
                format!("--script={autocrop}"),
                "--script-opts-append=autocrop-auto=no".to_string(),
                format!("--script={shim}"),
                format!("--script={markers}"),
                "--script-opts-append=vela-markers-intro-policy=button".to_string(),
                "--script-opts-append=vela-markers-credits-policy=autoskip".to_string(),
                "--script-opts-append=vela-markers-commercial-policy=off".to_string(),
                "--window-maximized=no".to_string(),
                "--fullscreen=yes".to_string(),
                "--screen-name=DP-3".to_string(),
                "--idle=no".to_string(),
                "--input-ipc-server=/tmp/vela-recorded/mpv.sock".to_string(),
                "--force-media-title=Recorded: Title".to_string(),
                "--title=Recorded: Title".to_string(),
                "--include=<AUTH INCLUDE>".to_string(),
                "--start=123.5".to_string(),
                "--".to_string(),
                "https://media.invalid/video/stream.mkv?x=1".to_string(),
            ],
        );
        assert_eq!(args, expected);

        let payload = payload_path.to_string_lossy().into_owned();
        let mut expected_env = recorded_base_env();
        expected_env.push(("VELA_MARKERS_PAYLOAD".to_string(), Some(payload)));
        assert_eq!(command_envs(&cmd), expected_env);

        drop(plan);
        assert!(
            !include_path.exists(),
            "the include is removed with the launch that owns it"
        );
        let _ = std::fs::remove_file(&payload_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Stream authentication reaches mpv through the owner-only include and
    // nowhere else. Another program can read a process's argument list, so no
    // argument may carry a header's name or value, under any option name; the
    // only trace of the headers in what the player is started with is the path
    // of the include.
    //
    // The player's environment is part of that, and a started program inherits
    // Vela's own. A test process has one environment for all its tests, which
    // run side by side: a header leaked into it by this launch could be
    // overwritten by another test's launch before it is looked for here. So
    // the check runs in a process of its own, where it is the only test.
    #[cfg(not(windows))]
    #[test]
    fn stream_auth_reaches_the_player_only_through_the_private_include() {
        const ALONE: &str = "VELA_TEST_RUNS_ALONE";
        if std::env::var_os(ALONE).is_none() {
            let name = format!(
                "{}::stream_auth_reaches_the_player_only_through_the_private_include",
                module_path!()
                    .split_once("::")
                    .expect("a module inside the crate")
                    .1
            );
            let run = Command::new(std::env::current_exe().expect("the test program"))
                .args(["--exact", &name, "--test-threads=1"])
                .env(ALONE, "1")
                .output()
                .expect("the test program can be started again");
            let report = String::from_utf8_lossy(&run.stdout);
            assert!(
                run.status.success() && report.contains("test result: ok. 1 passed"),
                "run alone, the check must run once and pass:\n{report}{}",
                String::from_utf8_lossy(&run.stderr)
            );
            return;
        }
        let secrets = [
            ("X-Synthetic-Auth", "synthetic-credential-one"),
            ("X-Synthetic-Session", "synthetic-credential-two"),
        ];
        let spec = PlaySpec {
            title: "A Title".to_string(),
            http_headers: secrets
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            ..recorded_spec("https://media.invalid/video/stream.mkv")
        };
        let cfg = crate::config::AppConfig::default();
        let plan = build_launch(&spec, &cfg, "/tmp/vela-recorded/auth.sock")
            .expect("the launch is described");
        let cmd = command("mpv", &plan);

        let include_path = plan
            .header_include
            .as_ref()
            .expect("stream headers produce an include")
            .path()
            .to_path_buf();
        let include = std::fs::read_to_string(&include_path).expect("include is readable");
        for (name, value) in secrets {
            assert!(
                include.contains(&format!("{name}: {value}")),
                "the include must carry every header"
            );
        }

        // Everything the player is started with: its arguments, and its whole
        // environment. That is Vela's own environment as it stands after the
        // launch was described, plus what the command adds: describing a
        // launch must not leave a header behind in either.
        let own: Vec<_> = std::env::vars_os().collect();
        let mut started_with = command_args(&cmd);
        for (key, value) in effective_env(&cmd, &own) {
            started_with.push(key);
            started_with.push(value);
        }
        // The check below is only as good as what it looks at.
        for (key, _) in command_envs(&cmd) {
            assert!(
                started_with.contains(&key),
                "what the command adds must be part of what is checked"
            );
        }
        assert!(!own.is_empty(), "a test run has an environment");
        for (key, _) in &own {
            assert!(
                started_with.contains(&key.to_string_lossy().into_owned()),
                "Vela's own environment must be part of what is checked"
            );
        }
        for text in &started_with {
            for (name, value) in secrets {
                assert!(
                    !text.contains(name) && !text.contains(value),
                    "a stream header reached the player outside the include"
                );
            }
            assert!(
                !text.to_ascii_lowercase().contains("http-header-fields"),
                "the header option must be set by the include, never by an argument"
            );
        }
        let include_arg = format!("--include={}", include_path.to_string_lossy());
        assert_eq!(
            started_with
                .iter()
                .filter(|text| text.contains(&*include_path.to_string_lossy()))
                .collect::<Vec<_>>(),
            vec![&include_arg],
            "the include is named once, by the option that points mpv at it"
        );

        // Without headers there is no include and nothing pointing at one.
        let bare = recorded_spec("https://media.invalid/video/stream.mkv");
        let bare_plan =
            build_launch(&bare, &cfg, "/tmp/vela-recorded/auth.sock").expect("described");
        assert!(bare_plan.header_include.is_none());
        assert!(!command_args(&command("mpv", &bare_plan))
            .iter()
            .any(|arg| arg.starts_with("--include")));
    }

    #[test]
    fn the_minimal_external_command_line_is_exactly_the_recorded_one() {
        // Nothing optional: the expert's own mpv config, no Advanced options,
        // no scripts, no inherited window, no title, no headers, no resume.
        let cfg = crate::config::AppConfig {
            mpv_use_own_config: Some(true),
            ..Default::default()
        };
        let spec = recorded_spec("edl://not-a-url-with-a-dash");
        let plan = build_launch(&spec, &cfg, "/tmp/vela-recorded/min.sock")
            .expect("the launch is described");
        let cmd = command("/opt/mpv/bin/mpv", &plan);
        assert_eq!(cmd.get_program().to_string_lossy(), "/opt/mpv/bin/mpv");
        assert!(plan.header_include.is_none());
        assert!(plan.marker_payload.is_none());

        let mut expected: Vec<String> = [
            "--no-ytdl",
            "--vo=gpu-next,gpu",
            "--profile=gpu-hq",
            "--hwdec=auto",
            "--hwdec-codecs=all",
            "--target-colorspace-hint=yes",
            "--hdr-compute-peak=yes",
        ]
        .map(str::to_string)
        .to_vec();
        expected.extend(recorded_platform_defaults(true));
        expected.extend(
            [
                "--idle=no",
                "--input-ipc-server=/tmp/vela-recorded/min.sock",
                "--force-media-title=Vela",
                "--title=Vela",
                "--",
                "edl://not-a-url-with-a-dash",
            ]
            .map(str::to_string),
        );
        assert_eq!(command_args(&cmd), expected);

        assert_eq!(command_envs(&cmd), recorded_base_env());
    }
}
