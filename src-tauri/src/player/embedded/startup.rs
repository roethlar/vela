//! How Vela's window is created, decided once when the process starts.
//!
//! The embedded player shows video on a surface UNDER Vela's window, so the
//! window has to be transparent wherever the video shows through. Slice 0 of
//! `.agents/plans/embedded-player.md` (A2) measured what that takes: a
//! transparent window repaints correctly only with WebKitGTK's GPU
//! ("DMABUF") renderer, which Vela has turned off for everyone since a crash
//! on NVIDIA's driver. Both are fixed when the process starts and cannot be
//! changed afterwards.
//!
//! Windows and macOS also need a transparent main window. Its WebView2 and mpv child
//! need none of those Linux renderer/driver restrictions; only the recovery
//! marker determines its window mode.
//!
//! So there are two ways to start. **Embedded window mode**: a transparent
//! window with the renderer on, ready for Vela's window. Every start is this,
//! whichever player is chosen (owner, 2026-11-17), so that changing the
//! choice needs no restart: it applies to the next video. mpv's own window
//! plays the same from it. **Today's**: an opaque window with that renderer
//! off, as before this player existed, only for a start that cannot be ready
//! (below); it says why.
//!
//! Because embedded window mode changes how the whole window is drawn, a
//! failure there could leave the user unable to reach Settings to turn it
//! off. Before entering it Vela leaves a marker file; the page removes it
//! once it has loaded. A marker found at the next start means the last one
//! did not get that far: that start uses today's mode, and removes the
//! marker so the one after tries again.

use crate::player::EmbeddedUnavailable;

/// How the window is created for this run of Vela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowMode {
    /// Opaque window; on Linux, WebKit's GPU renderer is off.
    Today,
    /// Transparent window; on Linux, WebKit's GPU renderer is on.
    Embedded,
}

/// What is known when the process starts, before any window exists.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Facts {
    /// A marker left by a previous start in embedded window mode.
    pub(crate) marker_left_behind: bool,
    /// The session is Wayland and GTK is not being forced onto X11.
    pub(crate) wayland_session: bool,
    /// The user's own environment turns WebKit's GPU renderer off.
    pub(crate) renderer_turned_off_by_user: bool,
}

/// The decision, and when this start cannot be ready for Vela's window, why
/// (whichever player is chosen: the choice can change during the run).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decision {
    pub(crate) mode: WindowMode,
    pub(crate) refused: Option<EmbeddedUnavailable>,
}

/// Decide how this run starts: ready for Vela's window unless something
/// known rules it out. The saved player choice plays no part.
#[cfg(target_os = "linux")]
pub(crate) fn decide(facts: Facts) -> Decision {
    let today = |reason| Decision {
        mode: WindowMode::Today,
        refused: Some(reason),
    };
    if facts.marker_left_behind {
        return today(EmbeddedUnavailable::FailedLastStart);
    }
    if !facts.wayland_session {
        return today(EmbeddedUnavailable::NotWayland);
    }
    // NVIDIA's proprietary driver is not ruled out (owner, 2026-11-17; Vela's
    // window played on the owner's RTX 5090 that day); the marker above
    // catches a start that fails there.
    if facts.renderer_turned_off_by_user {
        return today(EmbeddedUnavailable::RendererOff);
    }
    Decision {
        mode: WindowMode::Embedded,
        refused: None,
    }
}

/// Windows has no Wayland or WebKitGTK restriction. The library is
/// checked when Settings/playing asks for availability, as on Linux.
#[cfg(any(target_os = "windows", target_os = "macos", test))]
fn decide_windows(marker_left_behind: bool) -> Decision {
    if marker_left_behind {
        return Decision {
            mode: WindowMode::Today,
            refused: Some(EmbeddedUnavailable::FailedLastStart),
        };
    }
    Decision {
        mode: WindowMode::Embedded,
        refused: None,
    }
}

/// macOS uses only the one-start recovery, just like Windows.
#[cfg(any(target_os = "macos", test))]
fn decide_macos(marker_left_behind: bool) -> Decision {
    decide_windows(marker_left_behind)
}

#[cfg(target_os = "macos")]
#[path = "macos_startup.rs"]
mod macos;
#[cfg(target_os = "macos")]
use macos::{regular_file_at, text_of};

#[cfg(target_os = "windows")]
#[path = "windows_startup.rs"]
mod windows;
#[cfg(target_os = "windows")]
use windows::{regular_file_at, text_of};

static DECISION: std::sync::OnceLock<Decision> = std::sync::OnceLock::new();
// Keep the environment as the user supplied it: main may subsequently turn
// WebKit's renderer off for today's window, which is not a user refusal.
#[cfg(target_os = "linux")]
static SYSTEM_FACTS: std::sync::OnceLock<Facts> = std::sync::OnceLock::new();

/// What would rule out choosing embedded mode on this system, independent
/// of the saved choice and the one-start recovery marker. Reuses `decide`.
pub(crate) fn system_refusal() -> Option<EmbeddedUnavailable> {
    #[cfg(target_os = "linux")]
    {
        SYSTEM_FACTS.get().and_then(|facts| decide(*facts).refused)
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        None
    }
}

/// This run's decision. Today's mode when nothing decided (tests, or a
/// platform entry point that never asked).
pub(crate) fn decision() -> Decision {
    DECISION.get().cloned().unwrap_or(Decision {
        mode: WindowMode::Today,
        refused: None,
    })
}

fn marker_path() -> Option<std::path::PathBuf> {
    crate::storage::config_dir_file("embedded-window-starting").ok()
}

/// The page has loaded in embedded window mode: this start got far enough
/// for the user to reach Settings.
pub(crate) fn window_came_up() {
    if let Some(path) = marker_path() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(target_os = "linux")]
fn wayland_session(
    wayland_display: Option<&std::ffi::OsStr>,
    gdk_backend: Option<&std::ffi::OsStr>,
) -> bool {
    let has_wayland = wayland_display.is_some_and(|value| !value.is_empty());
    // `GDK_BACKEND` is a preference list; GTK takes the first that works.
    let first_backend = gdk_backend
        .and_then(|value| value.to_str())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .unwrap_or("");
    has_wayland && matches!(first_backend, "" | "wayland" | "*")
}

/// The `player` setting in the text of a settings file.
///
/// Read on its own rather than through the settings loader. This runs before
/// the app has checked, migrated or repaired its settings, and a file from
/// before the connections were split out of it does not load until that has
/// happened; all that is wanted here is one string. Anything but a JSON
/// object with that field as a string is "not chosen", and nothing of the
/// file is kept or reported.
fn player_setting_in(settings_file: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(settings_file).ok()?;
    value.as_object()?.get("player")?.as_str().map(str::to_string)
}

/// The most of a settings file that is read here. Vela's own are a few
/// kilobytes; nothing this large is one.
const SETTINGS_READ_LIMIT: u64 = 8 * 1024 * 1024;

/// The text of the settings file, when it is an ordinary file of ordinary
/// size. Anything else is "no text". The window mode does not depend on it;
/// it only decides whether a start that cannot be ready for Vela's window
/// says so (to someone who chose that window).
///
/// This read happens on EVERY start on Linux, before the window exists and
/// before the settings loader has had its say, so it must not be able to
/// stop a start or to touch anything. The loader refuses a path that is not
/// a regular file and shows the recovery screen; a plain read of such a path
/// could instead wait forever (a pipe nobody writes to) or never end (a
/// device), with no window to show anything in (review finding ec-3).
///
/// Nor may the path be opened for reading just to find out what it is:
/// opening is itself an act on some things. A terminal opened by a process
/// that has none becomes its controlling terminal, and hangs the process up
/// when it goes; a writer waiting on a pipe is woken, and then finds nobody
/// reading (the first repair did this, and was reopened for it). So:
///
/// 1. Take a handle on whatever is at the path WITHOUT opening it
///    (`O_PATH`), and without following a link.
/// 2. Ask the handle what it is. Only a regular file goes further.
/// 3. Open that same file, through the handle rather than through the path,
///    so that whatever is put at the path in the meantime is not what gets
///    read. Opening a regular file does nothing to anyone.
/// 4. Read no more than the limit.
fn settings_text(path: &std::path::Path) -> Option<String> {
    text_of(&regular_file_at(path)?)
}

/// Steps 1 and 2 of [`settings_text`]: a handle on the regular file at
/// `path`, which has not been opened for reading. Nothing when what is there
/// is anything else, a link included.
#[cfg(target_os = "linux")]
fn regular_file_at(path: &std::path::Path) -> Option<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let held = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_PATH | libc::O_NOFOLLOW)
        .open(path)
        .ok()?;
    held.metadata().ok()?.file_type().is_file().then_some(held)
}

/// Steps 3 and 4 of [`settings_text`]: the text of the file `held` is a
/// handle on. It is given no path, so it cannot read anything else.
#[cfg(target_os = "linux")]
fn text_of(held: &std::fs::File) -> Option<String> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(format!("/proc/self/fd/{}", held.as_raw_fd()))
        .ok()?;
    if !file.metadata().ok()?.file_type().is_file() {
        return None;
    }
    bounded_text(file)
}

/// The text `source` holds when that is no more than the limit; nothing when
/// there is more. Never takes more than one byte past the limit from it,
/// however much it has.
fn bounded_text(source: impl std::io::Read) -> Option<String> {
    use std::io::Read;
    let mut text = String::new();
    source
        .take(SETTINGS_READ_LIMIT + 1)
        .read_to_string(&mut text)
        .ok()?;
    (text.len() as u64 <= SETTINGS_READ_LIMIT).then_some(text)
}

fn saved_player_setting() -> Option<String> {
    let path = crate::config::config_path().ok()?;
    player_setting_in(&settings_text(&path)?)
}

/// Decide how this run starts and do what the decision needs done before
/// GTK exists. Call once, first thing in `main`. Returns the mode.
pub(crate) fn prepare() -> WindowMode {
    let player_setting = saved_player_setting();
    let marker = marker_path();
    let marker_left_behind = marker.as_ref().is_some_and(|path| path.exists());
    #[cfg(target_os = "linux")]
    let facts = Facts {
        marker_left_behind,
        wayland_session: wayland_session(
            std::env::var_os("WAYLAND_DISPLAY").as_deref(),
            std::env::var_os("GDK_BACKEND").as_deref(),
        ),
        renderer_turned_off_by_user: std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER")
            .is_some_and(|value| value != "0" && !value.is_empty()),
    };
    #[cfg(target_os = "linux")]
    let _ = SYSTEM_FACTS.set(Facts {
        marker_left_behind: false,
        ..facts
    });
    #[cfg(target_os = "linux")]
    let mut decision = decide(facts);
    #[cfg(target_os = "windows")]
    let mut decision = decide_windows(marker_left_behind);
    #[cfg(target_os = "macos")]
    let mut decision = decide_macos(marker_left_behind);
    if marker_left_behind {
        // Whatever this start is, the next one may try again.
        if let Some(path) = &marker {
            let _ = std::fs::remove_file(path);
        }
    }
    if decision.mode == WindowMode::Embedded {
        // No marker, no embedded window mode: without it a start that fails
        // would simply be repeated.
        let left = marker
            .as_ref()
            .is_some_and(|path| std::fs::write(path, b"").is_ok());
        if left {
            // Each embedded play starts threads, and the C library keeps
            // freed memory in an arena per thread, up to eight per CPU core.
            // Over many plays that looked like a leak of a few megabytes a
            // play; capped, memory stayed flat (Slice 0, A8). It has to be
            // set before any other thread exists.
            // SAFETY: a documented tunable and a positive value.
            #[cfg(target_os = "linux")]
            unsafe {
                libc::mallopt(libc::M_ARENA_MAX, 4)
            };
        } else {
            decision = Decision {
                mode: WindowMode::Today,
                refused: Some(EmbeddedUnavailable::FailedLastStart),
            };
        }
    }
    if let Some(reason) = decision
        .refused
        .as_ref()
        .filter(|_| player_setting.as_deref() == Some("embedded"))
    {
        // Said once, here, because nothing later can say it: a play in this
        // run will only be told the embedded player is not in use. Only to
        // someone who chose Vela's window; mpv's window is not affected.
        eprintln!(
            "vela: the embedded player is selected, but this start cannot use it: {}",
            reason.notice()
        );
    }
    let mode = decision.mode;
    let _ = DECISION.set(decision);
    mode
}

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::fs::OpenOptionsExt;

    /// Everything in favour of embedded window mode.
    const WILLING: Facts = Facts {
        marker_left_behind: false,
        wayland_session: true,
        renderer_turned_off_by_user: false,
    };

    fn today(refused: Option<EmbeddedUnavailable>) -> Decision {
        Decision {
            mode: WindowMode::Today,
            refused,
        }
    }

    // Every start that can be is ready for Vela's window, so that choosing
    // either player needs no restart; one that cannot always says why.
    #[test]
    fn a_start_is_embedded_window_mode_unless_something_rules_it_out_and_then_says_why() {
        for marker_left_behind in [false, true] {
            for wayland_session in [false, true] {
                for renderer_turned_off_by_user in [false, true] {
                    let facts = Facts {
                        marker_left_behind,
                        wayland_session,
                        renderer_turned_off_by_user,
                    };
                    let decided = decide(facts);
                    if facts == WILLING {
                        assert_eq!(decided.mode, WindowMode::Embedded, "{facts:?}");
                        assert_eq!(decided.refused, None, "{facts:?}");
                    } else {
                        assert_eq!(decided.mode, WindowMode::Today, "{facts:?}");
                        assert!(decided.refused.is_some(), "{facts:?} must say why");
                    }
                }
            }
        }
    }

    #[test]
    fn nothing_against_it_starts_embedded_window_mode() {
        assert_eq!(
            decide(WILLING),
            Decision {
                mode: WindowMode::Embedded,
                refused: None
            }
        );
    }

    #[test]
    fn a_marker_from_the_last_start_forces_todays_mode() {
        assert_eq!(
            decide(Facts {
                marker_left_behind: true,
                ..WILLING
            }),
            today(Some(EmbeddedUnavailable::FailedLastStart))
        );
    }

    #[test]
    fn each_thing_that_rules_embedded_window_mode_out_does_so_alone_and_says_why() {
        assert_eq!(
            decide(Facts {
                wayland_session: false,
                ..WILLING
            }),
            today(Some(EmbeddedUnavailable::NotWayland))
        );
        assert_eq!(
            decide(Facts {
                renderer_turned_off_by_user: true,
                ..WILLING
            }),
            today(Some(EmbeddedUnavailable::RendererOff))
        );
    }

    #[test]
    fn a_wayland_session_is_one_gtk_will_actually_use() {
        let session = |display: Option<&str>, backend: Option<&str>| {
            wayland_session(display.map(OsStr::new), backend.map(OsStr::new))
        };
        assert!(session(Some("wayland-0"), None));
        assert!(session(Some("wayland-0"), Some("wayland")));
        assert!(session(Some("wayland-0"), Some("wayland,x11")));
        assert!(session(Some("wayland-0"), Some("*")));
        assert!(!session(None, None), "an X11 session");
        assert!(!session(Some(""), None), "an empty variable is not a display");
        assert!(!session(Some("wayland-0"), Some("x11")), "GTK forced onto X11");
        assert!(!session(Some("wayland-0"), Some("x11,wayland")), "X11 preferred");
        assert!(!session(None, Some("wayland")), "asked for, but there is none");
    }

    #[test]
    fn the_player_setting_is_read_from_any_settings_file_that_has_it() {
        assert_eq!(
            player_setting_in(r#"{"player":"embedded"}"#).as_deref(),
            Some("embedded")
        );
        // A settings file from before connections were split out of it, which
        // the settings loader refuses until it has been migrated. Fields this
        // code has never heard of are no obstacle either.
        assert_eq!(
            player_setting_in(
                r#"{"sources":[{"kind":"jellyfin","token":"synthetic"}],"from_the_future":1,"player":"embedded"}"#
            )
            .as_deref(),
            Some("embedded")
        );
        assert_eq!(
            player_setting_in(r#"{"player":"external"}"#).as_deref(),
            Some("external")
        );
        // Not chosen: absent, not a string, not an object, not JSON at all.
        assert_eq!(player_setting_in(r#"{"mpv_path":"/usr/bin/mpv"}"#), None);
        assert_eq!(player_setting_in(r#"{"player":true}"#), None);
        assert_eq!(player_setting_in(r#"{"player":null}"#), None);
        assert_eq!(player_setting_in(r#"["player","embedded"]"#), None);
        assert_eq!(player_setting_in(r#""embedded""#), None);
        assert_eq!(player_setting_in("{\"player\":\"embedded\""), None);
        assert_eq!(player_setting_in(""), None);
    }

    #[test]
    fn a_run_that_never_decided_is_in_todays_mode() {
        // Unit tests never call `prepare`.
        assert_eq!(decision(), today(None));
    }

    /// A directory of its own for one test, removed when the guard drops.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "vela-test-{}-startup-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }

        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// `settings_text`, given two seconds. A read that waits on its file
    /// would hang the test run; this turns that into a failure.
    fn read_within_two_seconds(path: &std::path::Path) -> Option<String> {
        let (answer, answered) = std::sync::mpsc::channel();
        let path = path.to_path_buf();
        std::thread::spawn(move || {
            let _ = answer.send(settings_text(&path));
        });
        answered
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("the start-up settings read must never wait on its file")
    }

    #[test]
    fn an_ordinary_settings_file_is_read() {
        let scratch = Scratch::new("ordinary");
        let file = scratch.path("config.json");
        std::fs::write(&file, r#"{"player":"embedded"}"#).unwrap();
        let text = read_within_two_seconds(&file).expect("a regular file is read");
        assert_eq!(player_setting_in(&text).as_deref(), Some("embedded"));
        assert_eq!(read_within_two_seconds(&scratch.path("absent.json")), None);
    }

    // The settings loader refuses anything that is not a regular file and
    // shows the recovery screen. This read runs before the window exists: it
    // must give up on such a path at once, so that the loader gets its turn.
    #[test]
    fn a_settings_path_that_is_not_a_regular_file_is_not_read_and_not_waited_for() {
        let scratch = Scratch::new("irregular");

        // A pipe nobody writes to: an ordinary open would wait forever.
        let pipe = scratch.path("pipe.json");
        let c_pipe = std::ffi::CString::new(pipe.to_str().unwrap()).unwrap();
        // SAFETY: a valid path string and an ordinary mode.
        assert_eq!(unsafe { libc::mkfifo(c_pipe.as_ptr(), 0o600) }, 0);
        assert_eq!(read_within_two_seconds(&pipe), None, "a pipe");

        // A link, even to a perfectly good settings file: the loader does
        // not follow one either.
        let real = scratch.path("real.json");
        std::fs::write(&real, r#"{"player":"embedded"}"#).unwrap();
        let link = scratch.path("link.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(read_within_two_seconds(&link), None, "a link to a file");

        // A link to something that never ends.
        let endless = scratch.path("endless.json");
        std::os::unix::fs::symlink("/dev/zero", &endless).unwrap();
        assert_eq!(read_within_two_seconds(&endless), None, "a link to a device");

        // The device itself, and a directory.
        assert_eq!(
            read_within_two_seconds(std::path::Path::new("/dev/zero")),
            None,
            "a device"
        );
        assert_eq!(read_within_two_seconds(&scratch.0), None, "a directory");
    }

    // Finding out that a path is not a settings file must not touch what is
    // there. A writer waiting on a pipe is woken by anyone who opens the pipe
    // for reading, and then fails for want of a reader; the same open would
    // adopt a terminal as the process's own. Nothing is opened for reading
    // unless it is a regular file, and this shows it on the pipe: the writer
    // is still waiting afterwards.
    #[test]
    fn looking_at_a_settings_path_does_not_wake_a_writer_waiting_on_a_pipe() {
        let scratch = Scratch::new("pipe-writer");
        let pipe = scratch.path("pipe.json");
        let c_pipe = std::ffi::CString::new(pipe.to_str().unwrap()).unwrap();
        // SAFETY: a valid path string and an ordinary mode.
        assert_eq!(unsafe { libc::mkfifo(c_pipe.as_ptr(), 0o600) }, 0);
        let (opened, was_opened) = std::sync::mpsc::channel();
        let writer_path = pipe.clone();
        let writer = std::thread::spawn(move || {
            // Waits here until something opens the pipe for reading.
            let end = std::fs::OpenOptions::new().write(true).open(&writer_path);
            let _ = opened.send(());
            drop(end);
        });
        // Let the writer reach its wait.
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(read_within_two_seconds(&pipe), None);
        assert!(
            was_opened
                .recv_timeout(std::time::Duration::from_millis(300))
                .is_err(),
            "the writer was woken: the pipe was opened for reading"
        );
        // Let the writer go: now a reader does appear.
        let reader = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&pipe)
            .unwrap();
        was_opened
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("the writer wakes for a real reader");
        writer.join().unwrap();
        drop(reader);
    }

    // What is read is the file that was looked at, not whatever is at the
    // path by the time of the read.
    #[test]
    fn the_file_that_was_checked_is_the_file_that_is_read() {
        let scratch = Scratch::new("same-file");
        let file = scratch.path("config.json");
        std::fs::write(&file, r#"{"player":"embedded"}"#).unwrap();
        // The file is looked at, and then the path is given to something
        // else before the read: first another file, then a pipe.
        let held = regular_file_at(&file).expect("a regular file");
        std::fs::rename(&file, scratch.path("moved.json")).unwrap();
        std::fs::write(&file, r#"{"player":"external"}"#).unwrap();
        assert_eq!(
            player_setting_in(&text_of(&held).unwrap()).as_deref(),
            Some("embedded"),
            "what was looked at is what is read"
        );
        std::fs::remove_file(&file).unwrap();
        let c_path = std::ffi::CString::new(file.to_str().unwrap()).unwrap();
        // SAFETY: a valid path string and an ordinary mode.
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        assert_eq!(
            player_setting_in(&text_of(&held).unwrap()).as_deref(),
            Some("embedded"),
            "a pipe put at the path afterwards is never opened"
        );
        // A handle on something that is not a regular file is never made.
        assert!(regular_file_at(&file).is_none(), "the pipe");
        assert!(regular_file_at(&scratch.0).is_none(), "a directory");
    }

    // A huge regular file passes every check on the path. Without a bound
    // the start would read all of it into memory before throwing it away;
    // the answer would be the same, so only the amount taken shows it.
    #[test]
    fn no_more_than_one_byte_past_the_limit_is_ever_taken() {
        /// A source that never ends, and counts what was taken from it.
        struct Endless {
            taken: u64,
        }
        impl std::io::Read for Endless {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                // A reader that has taken this much has lost its bound.
                // Stop it here rather than let a failing test fill the
                // machine's memory.
                if self.taken > SETTINGS_READ_LIMIT * 2 {
                    return Err(std::io::Error::other("read far past the limit"));
                }
                buffer.fill(b' ');
                self.taken += buffer.len() as u64;
                Ok(buffer.len())
            }
        }
        let mut source = Endless { taken: 0 };
        assert_eq!(bounded_text(&mut source), None);
        assert_eq!(source.taken, SETTINGS_READ_LIMIT + 1);
    }

    #[test]
    fn a_settings_file_is_read_up_to_the_limit_and_not_beyond() {
        let scratch = Scratch::new("limit");
        let limit = usize::try_from(SETTINGS_READ_LIMIT).unwrap();
        let at = scratch.path("at.json");
        std::fs::write(&at, vec![b' '; limit]).unwrap();
        assert_eq!(
            read_within_two_seconds(&at).map(|text| text.len()),
            Some(limit)
        );
        let over = scratch.path("over.json");
        std::fs::write(&over, vec![b' '; limit + 1]).unwrap();
        assert_eq!(read_within_two_seconds(&over), None);
    }
}

#[cfg(test)]
mod windows_tests {
    use super::*;

    #[test]
    fn windows_start_is_embedded_window_mode_whatever_the_choice() {
        assert_eq!(
            decide_windows(false),
            Decision {
                mode: WindowMode::Embedded,
                refused: None,
            }
        );
    }

    #[test]
    fn windows_failed_start_recovers_to_an_opaque_window() {
        assert_eq!(
            decide_windows(true),
            Decision {
                mode: WindowMode::Today,
                refused: Some(EmbeddedUnavailable::FailedLastStart),
            }
        );
    }
}


#[cfg(test)]
mod macos_tests {
    use super::*;

    #[test]
    fn macos_start_is_embedded_window_mode_and_recovers_for_one_start() {
        assert_eq!(decide_macos(false), Decision {
            mode: WindowMode::Embedded, refused: None,
        });
        assert_eq!(decide_macos(true), Decision {
            mode: WindowMode::Today, refused: Some(EmbeddedUnavailable::FailedLastStart),
        });
    }
}
