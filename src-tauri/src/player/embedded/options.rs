//! How a launch's options reach an embedded mpv.
//!
//! The external player gets its options as command-line arguments. mpv's
//! library takes options through its API instead, and through the API it
//! rejects every spelling that exists only on the command line: `script` is
//! one, and so are the `-append` forms Vela's list already uses. A
//! configuration file, however, is read with command-line meaning, and the
//! library accepts `include` (Slice 0 of `.agents/plans/embedded-player.md`,
//! A1). So the launch's options are written as configuration text and
//! included, which also means the user's Advanced options mean exactly what
//! they mean for the external player.
//!
//! Three things measured against the shipped library (mpv 0.42) shape this
//! file:
//!
//! - A line mpv cannot parse is skipped, and `include` still reports success.
//!   Nothing here may therefore produce a line by accident: see [`line`].
//! - An `include` written INSIDE a configuration file takes effect after the
//!   rest of that file, not where it stands. An option that includes a file
//!   is therefore never written into the text; it becomes a step of its own
//!   ([`Step::Include`]) so the order of the list is the order mpv applies.
//! - An option can bring in an include without being one: `profile=NAME`,
//!   when the profile contains an `include`. That include, too, takes effect
//!   after the rest of the file the profile was selected in. Measured: with
//!   a profile and Vela's `idle=once` in one file, an `idle=yes` the profile
//!   included won; with the profile in a file of its own, applied first,
//!   Vela's won (review finding ec-4). It does not matter whose line selects
//!   the profile: Vela's own `profile=gpu-hq` selects the user's profile of
//!   that name if their configuration defines one (the first repair, which
//!   separated only the user's lines, was reopened for that). And which
//!   options can do this is mpv's business and may grow.
//!
//! So there is one rule and it names no option: EVERY option is a step of
//! its own, applied by its own call into mpv, in the order of the list.
//! Whatever an option brings in has been applied before the next option is,
//! as on a command line. A launch has a few dozen options; a few dozen small
//! files, each written, read once and removed, cost nothing a play notices.

use crate::player::launch::LaunchArg;

/// One thing to hand to mpv, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// One option, as the text of a configuration file of its own.
    Options(String),
    /// A file for mpv to include at this point.
    Include(String),
}

/// One option of the user's Advanced list, taken apart.
#[derive(Debug, PartialEq, Eq)]
struct UserOption<'a> {
    name: &'a str,
    /// `None` for a bare flag such as `--fs`.
    value: Option<&'a str>,
}

/// Read one line of the user's Advanced options the way mpv's command line
/// would: `--name=value`, `--name`, or the same with a single dash. Anything
/// else is not an option. On a command line mpv would take it for a file to
/// play; there is no such thing to do with it here, so it is left out.
fn parse_user_line(text: &str) -> Option<UserOption<'_>> {
    let text = text.trim();
    let rest = text
        .strip_prefix("--")
        .or_else(|| text.strip_prefix('-'))?;
    let (name, value) = match rest.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (rest, None),
    };
    let well_formed = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    well_formed.then_some(UserOption { name, value })
}

/// Whether mpv loads the user's configuration files is decided by the
/// embedded launcher (it applies them itself, before Vela's options, so that
/// the protected options still win). Left to mpv's own `config` option, the
/// files would be read last and override everything.
fn is_config_switch(name: &str) -> bool {
    matches!(name, "config" | "no-config")
}

/// One option as a line of mpv configuration.
///
/// A value is written in mpv's counted form, `name=%<bytes>%<value>`, which
/// needs no quoting or escaping whatever the value contains. The count is in
/// BYTES. A configuration file is read line by line, so a line break cannot be
/// part of a value in any form: each one becomes a space, as does a NUL, which
/// no C string can carry.
fn line(name: &str, value: Option<&str>) -> String {
    match value {
        None => format!("{name}\n"),
        Some(value) => {
            let value: String = value
                .chars()
                .map(|c| if matches!(c, '\n' | '\r' | '\0') { ' ' } else { c })
                .collect();
            format!("{name}=%{}%{value}\n", value.len())
        }
    }
}

/// Turn a launch's ordered options into the ordered steps that apply them:
/// one step for each option, in order, whoever's it is (the module's notes
/// say why). An option that includes a file is that file's inclusion; a line
/// of the user's that is not an option, or that would have mpv load
/// configuration files by itself, is left out.
pub(crate) fn steps(args: &[LaunchArg]) -> Vec<Step> {
    args.iter()
        .filter_map(|arg| {
            let (name, value) = match arg {
                LaunchArg::Option { name, value } => (*name, value.as_deref()),
                LaunchArg::User(user) => {
                    let option =
                        parse_user_line(user).filter(|option| !is_config_switch(option.name))?;
                    (option.name, option.value)
                }
            };
            Some(match (name, value) {
                ("include", Some(path)) => Step::Include(path.to_string()),
                _ => Step::Options(line(name, value)),
            })
        })
        .collect()
}

/// The user's own mpv configuration files, in the order mpv reads them: the
/// system-wide file, then the user's. Only `mpv.conf` is honoured in the
/// embedded player. The user's scripts folder, per-script option files and
/// key bindings are not loaded: the first two would run before Vela's options
/// could be asserted over them, and no key press reaches an embedded mpv.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn user_config_files(
    mpv_home: Option<&std::ffi::OsStr>,
    xdg_config_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let set = |value: Option<&std::ffi::OsStr>| value.filter(|v| !v.is_empty()).map(PathBuf::from);
    let user_dir = set(mpv_home)
        .or_else(|| set(xdg_config_home).map(|dir| dir.join("mpv")))
        .or_else(|| set(home).map(|dir| dir.join(".config").join("mpv")));
    let mut files = vec![PathBuf::from("/etc/mpv/mpv.conf")];
    files.extend(user_dir.map(|dir| dir.join("mpv.conf")));
    files
}

/// Windows mpv.conf files, lowest priority first, matching mpv 0.42's
/// options/path.c and osdep/path-win.c. Portable mode replaces home and
/// suppresses global, but exe_dir still applies. MPV_HOME replaces them all.
#[cfg(any(target_os = "windows", test))]
fn windows_user_config_files(
    exe_dir: Option<&std::path::Path>,
    portable_present: bool,
    appdata_dir: Option<&std::path::Path>,
    mpv_home: Option<&std::ffi::OsStr>,
) -> Vec<std::path::PathBuf> {
    if let Some(home) = mpv_home.filter(|value| !value.is_empty()) {
        return vec![std::path::PathBuf::from(home).join("mpv.conf")];
    }
    let mut files = Vec::new();
    if let Some(dir) = exe_dir {
        if !portable_present {
            files.push(dir.join("mpv").join("mpv.conf"));
        }
        files.push(dir.join("mpv.conf"));
    }
    let home = if portable_present {
        exe_dir.map(|dir| dir.join("portable_config"))
    } else {
        appdata_dir.map(|dir| dir.join("mpv"))
    };
    files.extend(home.map(|dir| dir.join("mpv.conf")));
    files
}

/// Decimal HWND, as mpv's WinID option and the W0 toplevel probe require.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn windows_window_options(hwnd: usize) -> Vec<LaunchArg> {
    let mut args = crate::player::launch::windows_embedded_required_args();
    args.push(LaunchArg::Option {
        name: "wid",
        value: Some(hwnd.to_string()),
    });
    args
}

pub(crate) fn current_user_config_files() -> Vec<std::path::PathBuf> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        user_config_files(
            std::env::var_os("MPV_HOME").as_deref(),
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        )
    }
    #[cfg(target_os = "windows")]
    {
        let executable = std::env::current_exe().ok();
        let directory = executable.as_deref().and_then(std::path::Path::parent);
        let portable_present = directory.is_some_and(|dir| dir.join("portable_config").is_dir());
        let dirs = directories::BaseDirs::new();
        windows_user_config_files(
            directory,
            portable_present,
            dirs.as_ref().map(|dirs| dirs.config_dir()),
            std::env::var_os("MPV_HOME").as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(name: &'static str, value: &str) -> LaunchArg {
        LaunchArg::Option {
            name,
            value: Some(value.to_string()),
        }
    }

    fn flag(name: &'static str) -> LaunchArg {
        LaunchArg::Option { name, value: None }
    }

    fn user(text: &str) -> LaunchArg {
        LaunchArg::User(text.to_string())
    }

    #[test]
    fn tls_verification_is_last_in_the_embedded_options_in_both_states() {
        for verify in [None, Some(true), Some(false)] {
            for headers in [false, true] {
                let expected = if verify.unwrap_or(true) { "yes" } else { "no" };
                let conflicting = if verify.unwrap_or(true) { "no" } else { "yes" };
                let cfg = crate::config::AppConfig {
                    mpv_tls_verify: verify,
                    mpv_use_own_config: Some(true),
                    mpv_extra_args: Some(format!(
                        "--tls-verify={conflicting}\n--include=/test/user.conf\n--tls-ca-file=/test/private-ca.pem"
                    )),
                    ..Default::default()
                };
                let mut spec = crate::playback::embedded_spec();
                if headers {
                    spec.http_headers.push(("X-Test".into(), "fixture".into()));
                }
                let plan =
                    crate::player::launch::build_launch(&spec, &cfg, "/tmp/vela-tls-test/mpv.sock")
                        .unwrap();
                let applied = steps(&plan.args);
                assert!(applied.contains(&Step::Options(format!(
                    "tls-verify=%{}%{conflicting}\n",
                    conflicting.len()
                ))));
                assert!(applied.contains(&Step::Include("/test/user.conf".into())));
                assert!(applied.contains(&Step::Options(
                    "tls-ca-file=%20%/test/private-ca.pem\n".into()
                )));
                assert_eq!(
                    applied.last(),
                    Some(&Step::Options(format!(
                        "tls-verify=%{}%{expected}\n",
                        expected.len()
                    ))),
                    "the setting is applied after every user option and include"
                );
            }
        }
    }

    #[test]
    fn a_value_is_written_with_its_length_in_bytes() {
        assert_eq!(line("vo", Some("libmpv")), "vo=%6%libmpv\n");
        assert_eq!(line("no-ytdl", None), "no-ytdl\n");
        assert_eq!(line("sub-font", Some("Noto Sans")), "sub-font=%9%Noto Sans\n");
        // An empty value is still a value.
        assert_eq!(line("start", Some("")), "start=%0%\n");
        // Bytes, not characters: mpv counts what it reads from the file.
        assert_eq!(
            line("force-media-title", Some("Amélie — 日本")),
            format!("force-media-title=%{}%Amélie — 日本\n", "Amélie — 日本".len())
        );
        assert_eq!("Amélie — 日本".len(), 18, "13 characters, 18 bytes");
        // Nothing mpv treats specially in an unquoted value matters here.
        assert_eq!(
            line("force-media-title", Some("  #1: \"A\" = 'B' %5% ")),
            "force-media-title=%20%  #1: \"A\" = 'B' %5% \n"
        );
    }

    #[test]
    fn a_line_break_in_a_value_cannot_start_a_second_option() {
        let written = line("force-media-title", Some("Title\nvo=gpu\r\nidle=yes\0"));
        assert_eq!(written, "force-media-title=%23%Title vo=gpu  idle=yes \n");
        assert_eq!(written.matches('\n').count(), 1, "one option, one line");
    }

    #[test]
    fn a_user_line_is_read_as_the_command_line_would_read_it() {
        let parsed = |text| parse_user_line(text).map(|o| (o.name, o.value));
        assert_eq!(parsed("--volume=70"), Some(("volume", Some("70"))));
        assert_eq!(parsed("-volume=70"), Some(("volume", Some("70"))));
        assert_eq!(parsed("  --fs  "), Some(("fs", None)));
        assert_eq!(parsed("--no-border"), Some(("no-border", None)));
        assert_eq!(
            parsed("--sub-font=Noto Sans"),
            Some(("sub-font", Some("Noto Sans")))
        );
        // Only the first `=` separates; the rest belongs to the value.
        assert_eq!(
            parsed("--script-opts=a=b,c=d"),
            Some(("script-opts", Some("a=b,c=d")))
        );
        assert_eq!(parsed("--start="), Some(("start", Some(""))));
        // Not options: mpv's command line would try to play these.
        assert_eq!(parsed("movie.mkv"), None);
        assert_eq!(parsed("volume=70"), None);
        assert_eq!(parsed("--"), None);
        assert_eq!(parsed("--=70"), None);
        assert_eq!(parsed("--vol ume=70"), None);
        assert_eq!(parsed("--{"), None);
    }

    #[test]
    fn the_options_keep_their_order_and_an_include_is_a_step_of_its_own() {
        let args = [
            flag("no-ytdl"),
            option("hwdec", "auto"),
            user("--volume=70"),
            user("--include=/home/u/extra.conf"),
            user("--vo=gpu-next"),
            option("idle", "once"),
            option("vo", "libmpv"),
            option("include", "/tmp/vela-x/mpv-headers-1-n.conf"),
            option("start", "123.5"),
        ];
        assert_eq!(
            steps(&args),
            [
                Step::Options("no-ytdl\n".to_string()),
                Step::Options("hwdec=%4%auto\n".to_string()),
                Step::Options("volume=%2%70\n".to_string()),
                Step::Include("/home/u/extra.conf".to_string()),
                Step::Options("vo=%8%gpu-next\n".to_string()),
                Step::Options("idle=%4%once\n".to_string()),
                Step::Options("vo=%6%libmpv\n".to_string()),
                Step::Include("/tmp/vela-x/mpv-headers-1-n.conf".to_string()),
                Step::Options("start=%5%123.5\n".to_string()),
            ]
        );
    }

    // A profile may contain an include, and mpv applies that include after
    // the rest of the options file the profile was selected in (measured;
    // review finding ec-4). The profile may be the user's even when the line
    // selecting it is Vela's: `gpu-hq` is whatever the user's configuration
    // says it is. So no option shares a file with the one after it.
    #[test]
    fn a_profile_is_never_in_one_file_with_the_options_after_it() {
        // Vela's own profile line, straight before Vela's protected options,
        // with nothing of the user's in the list at all.
        let velas = [
            flag("no-ytdl"),
            option("profile", "gpu-hq"),
            option("hwdec", "auto"),
            option("idle", "once"),
            option("vo", "libmpv"),
        ];
        assert_eq!(
            steps(&velas),
            [
                Step::Options("no-ytdl\n".to_string()),
                Step::Options("profile=%6%gpu-hq\n".to_string()),
                Step::Options("hwdec=%4%auto\n".to_string()),
                Step::Options("idle=%4%once\n".to_string()),
                Step::Options("vo=%6%libmpv\n".to_string()),
            ]
        );
        // And the user's.
        let users = [user("--profile=example"), user("--volume=50"), option("idle", "once")];
        assert_eq!(
            steps(&users),
            [
                Step::Options("profile=%7%example\n".to_string()),
                Step::Options("volume=%2%50\n".to_string()),
                Step::Options("idle=%4%once\n".to_string()),
            ]
        );
    }

    #[test]
    fn no_step_holds_more_than_one_option_and_none_is_lost_or_moved() {
        // Whatever is in the list and in whatever order.
        let pool = [
            flag("no-ytdl"),
            option("profile", "gpu-hq"),
            option("idle", "once"),
            option("vo", "libmpv"),
            option("input-ipc-server", "/tmp/s.sock"),
            user("--profile=x"),
            user("--volume=1"),
            user("--fs"),
            user("-sub-font=A B"),
            user("--idle=yes"),
        ];
        for rotate in 0..pool.len() {
            let mut args = pool.to_vec();
            args.rotate_left(rotate);
            let produced = steps(&args);
            assert_eq!(produced.len(), args.len(), "one step for each option");
            for (step, arg) in produced.iter().zip(&args) {
                let Step::Options(text) = step else {
                    panic!("no include in this list");
                };
                assert_eq!(text.lines().count(), 1, "one option: {text:?}");
                let name = match arg {
                    LaunchArg::Option { name, .. } => name.to_string(),
                    LaunchArg::User(user) => parse_user_line(user).unwrap().name.to_string(),
                };
                assert!(
                    text.trim_end() == name || text.starts_with(&format!("{name}=%")),
                    "in the list's order: {text:?} for {name}"
                );
            }
        }
    }

    #[test]
    fn an_include_is_never_written_into_the_options_text() {
        // Inside a configuration file mpv applies an include after the rest
        // of the file, which would put the user's file after Vela's protected
        // options. Every spelling must come out as a step.
        for include in [
            user("--include=/a.conf"),
            user("-include=/a.conf"),
            option("include", "/a.conf"),
        ] {
            let produced = steps(&[option("vo", "libmpv"), include, option("idle", "once")]);
            assert_eq!(
                produced,
                [
                    Step::Options("vo=%6%libmpv\n".to_string()),
                    Step::Include("/a.conf".to_string()),
                    Step::Options("idle=%4%once\n".to_string()),
                ]
            );
            for step in &produced {
                if let Step::Options(text) = step {
                    assert!(!text.contains("include"), "text: {text}");
                }
            }
        }
    }

    #[test]
    fn the_user_cannot_make_mpv_load_configuration_files_by_itself() {
        // mpv would read them after everything here and override it.
        let args = [
            user("--config=yes"),
            user("--no-config"),
            user("-config"),
            user("--volume=70"),
            // A different option that merely starts the same way stays.
            user("--config-dir=/home/u/mpv"),
        ];
        assert_eq!(
            steps(&args),
            [
                Step::Options("volume=%2%70\n".to_string()),
                Step::Options("config-dir=%11%/home/u/mpv\n".to_string()),
            ]
        );
    }

    #[test]
    fn what_is_not_an_option_is_left_out_and_nothing_else_is() {
        let args = [user("movie.mkv"), user("--fs"), user("garbage here")];
        assert_eq!(steps(&args), [Step::Options("fs\n".to_string())]);
        assert_eq!(steps(&[]), []);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_users_own_configuration_is_found_where_mpv_looks_for_it() {
        use std::ffi::OsStr;
        use std::path::PathBuf;
        let files = |mpv_home: Option<&str>, xdg: Option<&str>, home: Option<&str>| {
            user_config_files(
                mpv_home.map(OsStr::new),
                xdg.map(OsStr::new),
                home.map(OsStr::new),
            )
        };
        let system = PathBuf::from("/etc/mpv/mpv.conf");
        assert_eq!(
            files(None, None, Some("/home/u")),
            [system.clone(), PathBuf::from("/home/u/.config/mpv/mpv.conf")]
        );
        assert_eq!(
            files(None, Some("/home/u/.cfg"), Some("/home/u")),
            [system.clone(), PathBuf::from("/home/u/.cfg/mpv/mpv.conf")]
        );
        assert_eq!(
            files(Some("/opt/mpvhome"), Some("/home/u/.cfg"), Some("/home/u")),
            [system.clone(), PathBuf::from("/opt/mpvhome/mpv.conf")]
        );
        // An empty variable counts as unset, as it does for mpv.
        assert_eq!(
            files(Some(""), Some(""), Some("/home/u")),
            [system.clone(), PathBuf::from("/home/u/.config/mpv/mpv.conf")]
        );
        assert_eq!(files(None, None, None), [system]);
    }
}

#[cfg(test)]
mod windows_tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::Path;

    #[test]
    fn windows_nonportable_configs_load_global_exe_then_appdata() {
        let exe = Path::new("install");
        let appdata = Path::new("roaming");
        for mpv_home in [None, Some(OsStr::new(""))] {
            assert_eq!(
                windows_user_config_files(Some(exe), false, Some(appdata), mpv_home),
                [
                    exe.join("mpv").join("mpv.conf"),
                    exe.join("mpv.conf"),
                    appdata.join("mpv").join("mpv.conf"),
                ]
            );
        }
    }

    #[test]
    fn windows_portable_configs_load_exe_then_portable() {
        let exe = Path::new("install");
        for mpv_home in [None, Some(OsStr::new(""))] {
            assert_eq!(
                windows_user_config_files(Some(exe), true, Some(Path::new("roaming")), mpv_home),
                [
                    exe.join("mpv.conf"),
                    exe.join("portable_config").join("mpv.conf"),
                ]
            );
        }
    }

    #[test]
    fn windows_mpv_home_excludes_all_other_config_dirs() {
        let home = Path::new("custom");
        for portable_present in [false, true] {
            assert_eq!(
                windows_user_config_files(
                    Some(Path::new("install")),
                    portable_present,
                    Some(Path::new("roaming")),
                    Some(home.as_os_str()),
                ),
                [home.join("mpv.conf")]
            );
        }
    }

    #[test]
    fn windows_play_uses_a_decimal_parent_and_the_native_video_output() {
        let args = windows_window_options(123456);
        let steps = steps(&args);
        assert!(steps.contains(&Step::Options("wid=%6%123456\n".into())));
        assert!(steps.contains(&Step::Options("vo=%8%gpu-next\n".into())));
        assert!(steps.contains(&Step::Options("gpu-api=%5%d3d11\n".into())));
        assert!(steps.contains(&Step::Options("gpu-context=%5%d3d11\n".into())));
        assert!(!steps.contains(&Step::Options("vo=%6%libmpv\n".into())));
        assert!(steps.contains(&Step::Options("target-colorspace-hint=%3%yes\n".into())));
    }

    #[test]
    fn windows_protected_gpu_options_follow_user_options() {
        let mut args = vec![
            LaunchArg::User("--vo=gpu".into()),
            LaunchArg::User("--gpu-api=opengl".into()),
            LaunchArg::User("--gpu-context=win".into()),
        ];
        args.extend(windows_window_options(123456));
        let steps = steps(&args);
        for (name, user, required) in [
            ("vo", "gpu", "gpu-next"),
            ("gpu-api", "opengl", "d3d11"),
            ("gpu-context", "win", "d3d11"),
        ] {
            let applied: Vec<_> = steps
                .iter()
                .filter(|step| {
                    matches!(step, Step::Options(text) if text.starts_with(&format!("{name}=")))
                })
                .cloned()
                .collect();
            assert_eq!(
                applied,
                [
                    Step::Options(line(name, Some(user))),
                    Step::Options(line(name, Some(required))),
                ],
                "{name}: the protected value must override the user's option"
            );
        }
    }

    #[test]
    fn windows_play_leaves_every_input_source_with_the_page() {
        let args = windows_window_options(123456);
        let steps = steps(&args);
        for option in [
            "osc",
            "input-default-bindings",
            "input-builtin-bindings",
            "input-vo-keyboard",
            "input-cursor",
            "input-media-keys",
            "input-terminal",
            "input-builtin-dragging",
            "input-builtin-drag-and-drop",
        ] {
            assert!(
                steps.contains(&Step::Options(format!("{option}=%2%no\n"))),
                "{option}"
            );
        }
    }
}
