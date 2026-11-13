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
//!   when the user's profile contains an `include`. That include, too, takes
//!   effect after the rest of the file the profile was selected in. Measured:
//!   with the profile and Vela's `idle=once` in one file, the user's included
//!   `idle=yes` won; with the profile in a file of its own, applied first,
//!   Vela's won (review finding ec-4). Which options can do this is mpv's
//!   business and may grow, so the rule here does not name them: every line
//!   of the user's is a step of its own. Whatever it brings in has been
//!   applied before the next option in the list is, as on a command line.

use crate::player::launch::LaunchArg;

/// One thing to hand to mpv, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// Options to apply together: the text of one configuration file.
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

/// Turn a launch's ordered options into the ordered steps that apply them.
///
/// Vela's own options next to each other share a step. An include is a step
/// of its own, and so is every line of the user's (the module's notes say
/// why): nothing of the user's is ever in the same options file as an option
/// of Vela's, so nothing the user's line brings in can be applied after the
/// options Vela puts behind it.
pub(crate) fn steps(args: &[LaunchArg]) -> Vec<Step> {
    let mut steps = Vec::new();
    // Vela's options collected since the last step.
    let mut ours = String::new();
    fn flush(steps: &mut Vec<Step>, ours: &mut String) {
        if !ours.is_empty() {
            steps.push(Step::Options(std::mem::take(ours)));
        }
    }
    for arg in args {
        let (name, value, users) = match arg {
            LaunchArg::Option { name, value } => (*name, value.as_deref(), false),
            LaunchArg::User(user) => match parse_user_line(user) {
                Some(option) if !is_config_switch(option.name) => {
                    (option.name, option.value, true)
                }
                _ => continue,
            },
        };
        match (name, value) {
            ("include", Some(path)) => {
                flush(&mut steps, &mut ours);
                steps.push(Step::Include(path.to_string()));
            }
            _ if users => {
                flush(&mut steps, &mut ours);
                steps.push(Step::Options(line(name, value)));
            }
            _ => ours.push_str(&line(name, value)),
        }
    }
    flush(&mut steps, &mut ours);
    steps
}

/// The user's own mpv configuration files, in the order mpv reads them: the
/// system-wide file, then the user's. Only `mpv.conf` is honoured in the
/// embedded player. The user's scripts folder, per-script option files and
/// key bindings are not loaded: the first two would run before Vela's options
/// could be asserted over them, and no key press reaches an embedded mpv.
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
                Step::Options("no-ytdl\nhwdec=%4%auto\n".to_string()),
                Step::Options("volume=%2%70\n".to_string()),
                Step::Include("/home/u/extra.conf".to_string()),
                Step::Options("vo=%8%gpu-next\n".to_string()),
                Step::Options("idle=%4%once\nvo=%6%libmpv\n".to_string()),
                Step::Include("/tmp/vela-x/mpv-headers-1-n.conf".to_string()),
                Step::Options("start=%5%123.5\n".to_string()),
            ]
        );
    }

    // A profile of the user's may contain an include, and mpv applies that
    // include after the rest of the options file the profile was selected in
    // (measured; review finding ec-4). So no line of the user's shares a file
    // with anything, least of all with the options Vela puts after it.
    #[test]
    fn a_line_of_the_users_is_a_step_of_its_own() {
        let args = [
            flag("no-ytdl"),
            option("osc", "no"),
            user("--profile=example"),
            user("--volume=50"),
            user("--fs"),
            option("idle", "once"),
            option("vo", "libmpv"),
        ];
        assert_eq!(
            steps(&args),
            [
                Step::Options("no-ytdl\nosc=%2%no\n".to_string()),
                Step::Options("profile=%7%example\n".to_string()),
                Step::Options("volume=%2%50\n".to_string()),
                Step::Options("fs\n".to_string()),
                Step::Options("idle=%4%once\nvo=%6%libmpv\n".to_string()),
            ]
        );
    }

    #[test]
    fn no_step_holds_an_option_of_the_users_together_with_one_of_velas() {
        // Whatever the user writes and wherever it stands in the list.
        let velas = ["no-ytdl", "idle", "vo", "osc", "input-ipc-server"];
        let users = ["--profile=x", "--volume=1", "--fs", "-sub-font=A B", "--idle=yes"];
        for first_user in 0..=velas.len() {
            let mut args: Vec<LaunchArg> = Vec::new();
            for (at, name) in velas.iter().enumerate() {
                if at == first_user {
                    args.extend(users.iter().map(|text| user(text)));
                }
                args.push(option(name, "v"));
            }
            if first_user == velas.len() {
                args.extend(users.iter().map(|text| user(text)));
            }
            let produced = steps(&args);
            let mut seen_users = 0;
            for step in &produced {
                let Step::Options(text) = step else {
                    panic!("no include in this list");
                };
                let lines: Vec<&str> = text.lines().collect();
                let of_velas = |line: &&str| velas.iter().any(|name| line.starts_with(&format!("{name}=%1%v")));
                if lines.iter().all(of_velas) {
                    continue;
                }
                assert_eq!(lines.len(), 1, "a step with a line of the user's has only that line: {text:?}");
                seen_users += 1;
            }
            assert_eq!(seen_users, users.len(), "every line of the user's is applied");
            // And the order of the list is the order of the steps.
            let flat: String = produced
                .iter()
                .map(|step| match step {
                    Step::Options(text) => text.as_str(),
                    Step::Include(_) => "",
                })
                .collect();
            let position = |needle: &str| flat.find(needle).expect(needle);
            if first_user < velas.len() {
                assert!(position("profile=%1%x") < position(&format!("{}=%1%v", velas[first_user])));
            }
            if first_user > 0 {
                assert!(position(&format!("{}=%1%v", velas[first_user - 1])) < position("profile=%1%x"));
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
