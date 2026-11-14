//! The player view's commands to a running embedded play.
//!
//! The page names one of ten commands (`src/lib/playerCommands.json`, the one
//! list both sides read) and gives its arguments as plain JSON. Everything it
//! sends is checked here against a closed list, with exact arity, types and
//! ranges, and turned into one fixed mpv command built in Rust: the page can
//! never make mpv run anything else. Every refusal is one of five fixed texts,
//! never anything from mpv's reply, a path or a URL.
//!
//! Whose play it is, is asked of the player in the slot under the slot's lock,
//! in the same hold that either stops the play or copies its socket's
//! address (review finding ec-5). The socket is used only after that lock has
//! been let go of. An external player is never driven this way: it has a
//! window and keys of its own.
//!
//! The in-process mpv handle is not used for commands. It belongs to the
//! play's drawing thread (review finding hd-3); a command goes over mpv's IPC
//! socket like everything else the rest of Vela says to a player.

use std::sync::Mutex;

use serde_json::Value;

use super::PlayerHandle;

/// The page named something that is not one of the ten commands, or gave
/// one the wrong arguments.
pub(crate) const NOT_A_COMMAND: &str = "Not a player command.";
/// There is no such play, or its player has gone.
pub(crate) const NOT_RUNNING: &str = "The player is not running.";
// The other three come only from a player's socket, which only Linux has.
/// Another command to the same play has not been answered yet.
#[cfg(target_os = "linux")]
pub(crate) const BUSY: &str = "The player is busy.";
/// The player did not answer in time, or has not answered a command lately.
#[cfg(target_os = "linux")]
pub(crate) const NO_ANSWER: &str = "The player did not answer.";
/// The player answered with anything but success.
#[cfg(target_os = "linux")]
pub(crate) const REFUSED: &str = "The player refused.";

/// The farthest a relative seek may go, in seconds, either way.
const MAX_RELATIVE_SEEK: f64 = 600.0;
/// The farthest an absolute seek may go, in seconds.
const MAX_ABSOLUTE_SEEK: f64 = 10_000_000.0;

/// The ten commands, by name. Nothing else reaches a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Name {
    TogglePause,
    SeekRelative,
    SeekAbsolute,
    SetVolume,
    ToggleMute,
    SetAudioTrack,
    SetSubtitleTrack,
    Stop,
    SkipActiveMarker,
    ToggleAutocrop,
}

impl Name {
    pub(crate) const ALL: [Name; 10] = [
        Name::TogglePause,
        Name::SeekRelative,
        Name::SeekAbsolute,
        Name::SetVolume,
        Name::ToggleMute,
        Name::SetAudioTrack,
        Name::SetSubtitleTrack,
        Name::Stop,
        Name::SkipActiveMarker,
        Name::ToggleAutocrop,
    ];

    /// The name the page uses.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Name::TogglePause => "toggle-pause",
            Name::SeekRelative => "seek-relative",
            Name::SeekAbsolute => "seek-absolute",
            Name::SetVolume => "set-volume",
            Name::ToggleMute => "toggle-mute",
            Name::SetAudioTrack => "set-audio-track",
            Name::SetSubtitleTrack => "set-subtitle-track",
            Name::Stop => "stop",
            Name::SkipActiveMarker => "skip-active-marker",
            Name::ToggleAutocrop => "toggle-autocrop",
        }
    }

    /// The command a name stands for, when it is one of the ten.
    pub(crate) fn from_name(name: &str) -> Option<Name> {
        Name::ALL.into_iter().find(|known| known.as_str() == name)
    }
}

/// A command the page may give, with its arguments checked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PlayerCommand {
    TogglePause,
    SeekRelative(f64),
    SeekAbsolute(f64),
    SetVolume(f64),
    ToggleMute,
    SetAudioTrack(i32),
    /// `None` turns subtitles off.
    SetSubtitleTrack(Option<i32>),
    Stop,
    SkipActiveMarker,
    ToggleAutocrop,
}

/// A JSON number that is finite and within `low..=high`. JSON has no NaN or
/// infinity (the page's `NaN` arrives as null), so a type check refuses those.
fn number_within(value: &Value, low: f64, high: f64) -> Option<f64> {
    let number = value.as_f64()?;
    (number.is_finite() && (low..=high).contains(&number)).then_some(number)
}

/// A track id: a JSON integer from 1 up. `2.0` or `"2"` is not one.
fn track_id(value: &Value) -> Option<i32> {
    let id = value.as_i64()?;
    i32::try_from(id).ok().filter(|id| *id >= 1)
}

/// Check a command the page sent. The arguments must be a JSON array of
/// exactly the length the command takes.
pub(crate) fn parse(name: &Value, args: &Value) -> Result<PlayerCommand, &'static str> {
    let name = name
        .as_str()
        .and_then(Name::from_name)
        .ok_or(NOT_A_COMMAND)?;
    let args = args.as_array().ok_or(NOT_A_COMMAND)?;
    let command = match (name, args.as_slice()) {
        (Name::TogglePause, []) => Some(PlayerCommand::TogglePause),
        (Name::SeekRelative, [seconds]) => {
            number_within(seconds, -MAX_RELATIVE_SEEK, MAX_RELATIVE_SEEK)
                .map(PlayerCommand::SeekRelative)
        }
        (Name::SeekAbsolute, [seconds]) => {
            number_within(seconds, 0.0, MAX_ABSOLUTE_SEEK).map(PlayerCommand::SeekAbsolute)
        }
        (Name::SetVolume, [volume]) => {
            number_within(volume, 0.0, 100.0).map(PlayerCommand::SetVolume)
        }
        (Name::ToggleMute, []) => Some(PlayerCommand::ToggleMute),
        (Name::SetAudioTrack, [id]) => track_id(id).map(PlayerCommand::SetAudioTrack),
        (Name::SetSubtitleTrack, [id]) => match id.as_str() {
            Some("no") => Some(PlayerCommand::SetSubtitleTrack(None)),
            Some(_) => None,
            None => track_id(id).map(|id| PlayerCommand::SetSubtitleTrack(Some(id))),
        },
        (Name::Stop, []) => Some(PlayerCommand::Stop),
        (Name::SkipActiveMarker, []) => Some(PlayerCommand::SkipActiveMarker),
        (Name::ToggleAutocrop, []) => Some(PlayerCommand::ToggleAutocrop),
        // The right name with the wrong number of arguments.
        (
            Name::TogglePause
            | Name::SeekRelative
            | Name::SeekAbsolute
            | Name::SetVolume
            | Name::ToggleMute
            | Name::SetAudioTrack
            | Name::SetSubtitleTrack
            | Name::Stop
            | Name::SkipActiveMarker
            | Name::ToggleAutocrop,
            _,
        ) => None,
    };
    command.ok_or(NOT_A_COMMAND)
}

#[cfg(target_os = "linux")]
impl PlayerCommand {
    /// The one mpv command each stands for, as the array of mpv's JSON IPC.
    /// `stop` has none: it never reaches mpv (see [`command`]).
    pub(crate) fn mpv_command(self) -> Option<Vec<Value>> {
        let text = |words: &[&str]| words.iter().map(|word| Value::from(*word)).collect();
        // Numbers are given as mpv's own command syntax writes them: Rust
        // prints an f64 without an exponent.
        Some(match self {
            PlayerCommand::TogglePause => text(&["cycle", "pause"]),
            PlayerCommand::SeekRelative(seconds) => {
                text(&["seek", &seconds.to_string(), "relative"])
            }
            PlayerCommand::SeekAbsolute(seconds) => {
                text(&["seek", &seconds.to_string(), "absolute"])
            }
            PlayerCommand::SetVolume(volume) => text(&["set", "volume", &volume.to_string()]),
            PlayerCommand::ToggleMute => text(&["cycle", "mute"]),
            PlayerCommand::SetAudioTrack(id) => text(&["set", "aid", &id.to_string()]),
            PlayerCommand::SetSubtitleTrack(None) => text(&["set", "sid", "no"]),
            PlayerCommand::SetSubtitleTrack(Some(id)) => text(&["set", "sid", &id.to_string()]),
            PlayerCommand::Stop => return None,
            PlayerCommand::SkipActiveMarker => {
                text(&["script-binding", "vela_markers/activate-skip"])
            }
            PlayerCommand::ToggleAutocrop => text(&["script-binding", "vela_crop/toggle"]),
        })
    }
}

/// Carry out one command from the player view of `session_id`. The whole of
/// the `player_command` Tauri command, run on the blocking pool: it takes the
/// slot's lock and may wait on a socket.
pub(crate) fn command(
    slot: &Mutex<Option<PlayerHandle>>,
    session_id: &Value,
    name: &Value,
    args: &Value,
) -> Result<(), String> {
    let command = parse(name, args)?;
    let Some(session_id) = session_id.as_str() else {
        return Err(NOT_RUNNING.to_string());
    };
    // One hold of the lock decides whose play it is and acts on it: stops it,
    // or copies the way to its socket.
    let channel = {
        let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
        match (command, slot.as_mut()) {
            (PlayerCommand::Stop, Some(player)) => {
                player.close_embedded_play(session_id);
                return Ok(());
            }
            (PlayerCommand::Stop, None) => return Ok(()),
            (_, Some(player)) => player.command_channel(session_id),
            (_, None) => None,
        }
    };
    let Some(channel) = channel else {
        return Err(NOT_RUNNING.to_string());
    };
    channel.send(command).map_err(str::to_string)
}

/// Whether `session_id` is the running embedded play, asked under the slot's
/// lock. For the player view's fullscreen: only that play's view may put the
/// window in fullscreen.
pub(crate) fn is_running_embedded_play(
    slot: &Mutex<Option<PlayerHandle>>,
    session_id: &Value,
) -> bool {
    let Some(session_id) = session_id.as_str() else {
        return false;
    };
    let slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    slot.as_ref()
        .is_some_and(|player| player.command_channel(session_id).is_some())
}

#[cfg(target_os = "linux")]
pub(crate) use channel::Channel;

/// No system but Linux has an embedded play, and so nothing to send to.
#[cfg(not(target_os = "linux"))]
pub(crate) enum Channel {}

#[cfg(not(target_os = "linux"))]
impl Channel {
    fn send(&self, _command: PlayerCommand) -> Result<(), &'static str> {
        match *self {}
    }
}

#[cfg(target_os = "linux")]
mod channel {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use serde_json::Value;

    use super::{PlayerCommand, BUSY, NOT_RUNNING, NO_ANSWER, REFUSED};
    use crate::player::report::{self, Lines, Read1};

    /// How long a command waits for mpv to take it and to answer it.
    pub(crate) const ANSWER_LIMIT: Duration = Duration::from_secs(2);

    /// How long every further command is refused at once after one that got
    /// no answer. A player that has stopped answering then costs one waiting
    /// thread per this long, not one per key press.
    pub(crate) const QUIET_AFTER_NO_ANSWER: Duration = Duration::from_secs(5);

    /// Ids for the commands' answers, unique in this run of Vela.
    static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

    /// The way to one play's player, kept with the play: its IPC socket, and
    /// the gate that lets at most one command at a time through to it.
    pub(crate) struct Channel {
        path: String,
        gate: Gate,
        /// How long to wait for an answer. [`ANSWER_LIMIT`] outside tests.
        answer_limit: Duration,
    }

    impl Channel {
        pub(crate) fn new(path: String) -> Self {
            Channel {
                path,
                gate: Gate::new(QUIET_AFTER_NO_ANSWER),
                answer_limit: ANSWER_LIMIT,
            }
        }

        /// The same, with shorter waits, for tests.
        #[cfg(test)]
        pub(crate) fn with_limits(path: String, answer_limit: Duration, quiet: Duration) -> Self {
            Channel {
                path,
                gate: Gate::new(quiet),
                answer_limit,
            }
        }

        /// Send one command and wait for mpv's answer to it.
        pub(crate) fn send(&self, command: PlayerCommand) -> Result<(), &'static str> {
            // `stop` never comes here; it does not use the socket.
            let mpv = command.mpv_command().ok_or(NOT_RUNNING)?;
            let _pass = self.gate.enter(Instant::now())?;
            let answer = exchange(&self.path, mpv, self.answer_limit);
            if answer == Err(NO_ANSWER) {
                self.gate.unanswered(Instant::now());
            }
            answer
        }
    }

    /// At most one command in flight, and a quiet spell after one that got
    /// no answer.
    pub(crate) struct Gate {
        in_flight: AtomicBool,
        quiet_until: Mutex<Option<Instant>>,
        quiet_for: Duration,
    }

    /// A command's turn. The next one may go when this is dropped.
    pub(crate) struct Pass<'a>(&'a Gate);

    impl Drop for Pass<'_> {
        fn drop(&mut self) {
            self.0.in_flight.store(false, Ordering::SeqCst);
        }
    }

    impl Gate {
        pub(crate) fn new(quiet_for: Duration) -> Self {
            Gate {
                in_flight: AtomicBool::new(false),
                quiet_until: Mutex::new(None),
                quiet_for,
            }
        }

        /// Let a command through at `now`, or say at once why not.
        pub(crate) fn enter(&self, now: Instant) -> Result<Pass<'_>, &'static str> {
            let quiet_until = *self.quiet_until.lock().unwrap_or_else(|e| e.into_inner());
            if quiet_until.is_some_and(|until| now < until) {
                return Err(NO_ANSWER);
            }
            if self.in_flight.swap(true, Ordering::SeqCst) {
                return Err(BUSY);
            }
            Ok(Pass(self))
        }

        /// A command sent at about `now` got no answer.
        pub(crate) fn unanswered(&self, now: Instant) {
            *self.quiet_until.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(now + self.quiet_for);
        }
    }

    fn is_timeout(error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        )
    }

    /// Connect to the player's socket, waiting at most `limit` (see
    /// [`report::connect`]). With no such file, or nobody listening any more,
    /// the player has gone.
    fn connect(path: &str, limit: Duration) -> Result<UnixStream, &'static str> {
        report::connect(path, limit).map_err(|error| match error.raw_os_error() {
            // The wait ran out with the queue still full (or a signal ended
            // it): the player is not taking connections.
            Some(libc::EAGAIN) | Some(libc::EINTR) => NO_ANSWER,
            _ => NOT_RUNNING,
        })
    }

    /// One short connection: send the command with an id and read until the
    /// answer carrying that id arrives, all within `limit`.
    fn exchange(path: &str, mpv: Vec<Value>, limit: Duration) -> Result<(), &'static str> {
        let deadline = Instant::now() + limit;
        let stream = connect(path, limit)?;
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(NO_ANSWER);
        }
        stream
            .set_write_timeout(Some(left))
            .map_err(|_| NOT_RUNNING)?;
        let request_id = NEXT_REQUEST.fetch_add(1, Ordering::Relaxed);
        let mut line = serde_json::json!({ "command": mpv, "request_id": request_id }).to_string();
        line.push('\n');
        (&stream).write_all(line.as_bytes()).map_err(|error| {
            if is_timeout(&error) {
                NO_ANSWER
            } else {
                NOT_RUNNING
            }
        })?;
        // A line past the reader's cap (a script's broadcast, say) is passed
        // over, so it costs neither the answer nor more than the cap.
        let mut lines = Lines::new(&stream);
        loop {
            // Events can keep arriving; the limit is for the whole wait.
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(NO_ANSWER);
            }
            stream
                .set_read_timeout(Some(left))
                .map_err(|_| NOT_RUNNING)?;
            let answer = match lines.next() {
                Read1::Line(line) => line,
                Read1::Waited => return Err(NO_ANSWER),
                Read1::End => return Err(NOT_RUNNING),
            };
            let Ok(Value::Object(reply)) = serde_json::from_slice::<Value>(&answer) else {
                continue;
            };
            // mpv's events go to every client, before and after the answer.
            if reply.contains_key("event")
                || reply.get("request_id").and_then(Value::as_u64) != Some(request_id)
            {
                continue;
            }
            return match reply.get("error").and_then(Value::as_str) {
                Some("success") => Ok(()),
                _ => Err(REFUSED),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The one list of names both sides read.
    const NAMES: &str = include_str!("../../../src/lib/playerCommands.json");
    /// The texts a refusal may have, as the page's notice line is tested
    /// against them.
    #[cfg(target_os = "linux")]
    const REFUSALS: &str = include_str!("../../../src/lib/playerRefusals.json");

    #[cfg(target_os = "linux")]
    #[test]
    fn the_five_refusals_are_the_list_the_page_is_tested_with() {
        let listed: Vec<String> = serde_json::from_str(REFUSALS).expect("a JSON list of texts");
        assert_eq!(
            listed,
            [NOT_A_COMMAND, NOT_RUNNING, BUSY, NO_ANSWER, REFUSED],
            "src/lib/playerRefusals.json"
        );
    }

    #[test]
    fn the_ten_names_are_the_list_both_sides_read() {
        let names: Vec<String> = serde_json::from_str(NAMES).expect("a JSON list of names");
        assert_eq!(names.len(), Name::ALL.len());
        for name in &names {
            let known =
                Name::from_name(name).unwrap_or_else(|| panic!("{name} is not a command here"));
            assert_eq!(known.as_str(), name);
        }
        for known in Name::ALL {
            assert!(
                names.iter().any(|name| name == known.as_str()),
                "{} is not in the list",
                known.as_str()
            );
            assert_eq!(Name::from_name(known.as_str()), Some(known));
        }
    }

    fn parsed(name: &str, args: Value) -> Result<PlayerCommand, &'static str> {
        parse(&json!(name), &args)
    }

    #[test]
    fn each_command_takes_exactly_its_arguments() {
        assert_eq!(
            parsed("toggle-pause", json!([])),
            Ok(PlayerCommand::TogglePause)
        );
        assert_eq!(
            parsed("seek-relative", json!([-5])),
            Ok(PlayerCommand::SeekRelative(-5.0))
        );
        assert_eq!(
            parsed("seek-relative", json!([600])),
            Ok(PlayerCommand::SeekRelative(600.0))
        );
        assert_eq!(
            parsed("seek-relative", json!([-600])),
            Ok(PlayerCommand::SeekRelative(-600.0))
        );
        assert_eq!(
            parsed("seek-absolute", json!([0])),
            Ok(PlayerCommand::SeekAbsolute(0.0))
        );
        assert_eq!(
            parsed("seek-absolute", json!([12.5])),
            Ok(PlayerCommand::SeekAbsolute(12.5))
        );
        assert_eq!(
            parsed("seek-absolute", json!([10_000_000])),
            Ok(PlayerCommand::SeekAbsolute(10_000_000.0))
        );
        assert_eq!(
            parsed("set-volume", json!([0])),
            Ok(PlayerCommand::SetVolume(0.0))
        );
        assert_eq!(
            parsed("set-volume", json!([42.5])),
            Ok(PlayerCommand::SetVolume(42.5))
        );
        assert_eq!(
            parsed("set-volume", json!([100])),
            Ok(PlayerCommand::SetVolume(100.0))
        );
        assert_eq!(
            parsed("toggle-mute", json!([])),
            Ok(PlayerCommand::ToggleMute)
        );
        assert_eq!(
            parsed("set-audio-track", json!([2])),
            Ok(PlayerCommand::SetAudioTrack(2))
        );
        assert_eq!(
            parsed("set-audio-track", json!([i32::MAX])),
            Ok(PlayerCommand::SetAudioTrack(i32::MAX))
        );
        assert_eq!(
            parsed("set-subtitle-track", json!([3])),
            Ok(PlayerCommand::SetSubtitleTrack(Some(3)))
        );
        assert_eq!(
            parsed("set-subtitle-track", json!(["no"])),
            Ok(PlayerCommand::SetSubtitleTrack(None))
        );
        assert_eq!(parsed("stop", json!([])), Ok(PlayerCommand::Stop));
        assert_eq!(
            parsed("skip-active-marker", json!([])),
            Ok(PlayerCommand::SkipActiveMarker)
        );
        assert_eq!(
            parsed("toggle-autocrop", json!([])),
            Ok(PlayerCommand::ToggleAutocrop)
        );
    }

    #[test]
    fn anything_else_is_not_a_player_command() {
        let refused = [
            // Names that are not among the ten, mpv's own included.
            (json!("loadfile"), json!(["https://example.invalid/x"])),
            (json!("quit"), json!([])),
            (json!("cycle"), json!(["pause"])),
            (json!("Toggle-Pause"), json!([])),
            (json!("toggle-pause "), json!([])),
            (json!(""), json!([])),
            (json!(null), json!([])),
            (json!(7), json!([])),
            (json!(["toggle-pause"]), json!([])),
            // Arguments that are not a list.
            (json!("toggle-pause"), json!(null)),
            (json!("toggle-pause"), json!({})),
            (json!("seek-relative"), json!(5)),
            (json!("seek-relative"), json!("5")),
            // The wrong number of arguments, for every command.
            (json!("toggle-pause"), json!([1])),
            (json!("seek-relative"), json!([])),
            (json!("seek-relative"), json!([5, "relative"])),
            (json!("seek-absolute"), json!([])),
            (json!("seek-absolute"), json!([5, 6])),
            (json!("set-volume"), json!([])),
            (json!("set-volume"), json!([5, 6])),
            (json!("toggle-mute"), json!([true])),
            (json!("set-audio-track"), json!([])),
            (json!("set-audio-track"), json!([1, 2])),
            (json!("set-subtitle-track"), json!([])),
            (json!("set-subtitle-track"), json!(["no", 1])),
            (json!("stop"), json!([0])),
            (json!("skip-active-marker"), json!(["intro"])),
            (json!("toggle-autocrop"), json!([1])),
            // The wrong types.
            (json!("seek-relative"), json!(["5"])),
            (json!("seek-relative"), json!([null])),
            (json!("seek-relative"), json!([true])),
            (json!("seek-absolute"), json!(["10"])),
            (json!("set-volume"), json!(["50"])),
            (json!("set-volume"), json!([null])),
            (json!("set-audio-track"), json!(["2"])),
            (json!("set-audio-track"), json!([2.0])),
            (json!("set-audio-track"), json!([2.5])),
            (json!("set-audio-track"), json!(["no"])),
            (json!("set-subtitle-track"), json!(["off"])),
            (json!("set-subtitle-track"), json!(["2"])),
            (json!("set-subtitle-track"), json!([1.5])),
            (json!("set-subtitle-track"), json!([null])),
            // Out of range.
            (json!("seek-relative"), json!([600.001])),
            (json!("seek-relative"), json!([-601])),
            (json!("seek-relative"), json!([1e308])),
            (json!("seek-absolute"), json!([-1])),
            (json!("seek-absolute"), json!([10_000_000.5])),
            (json!("seek-absolute"), json!([1e308])),
            (json!("set-volume"), json!([-0.1])),
            (json!("set-volume"), json!([100.5])),
            (json!("set-volume"), json!([130])),
            (json!("set-audio-track"), json!([0])),
            (json!("set-audio-track"), json!([-1])),
            (json!("set-audio-track"), json!([i64::from(i32::MAX) + 1])),
            (json!("set-audio-track"), json!([u64::MAX])),
            // 2^32 + 5: cut to 32 bits it would be track 5.
            (json!("set-audio-track"), json!([4_294_967_301_i64])),
            (json!("set-subtitle-track"), json!([0])),
            (
                json!("set-subtitle-track"),
                json!([i64::from(i32::MAX) + 1]),
            ),
            (json!("set-subtitle-track"), json!([4_294_967_301_i64])),
        ];
        for (name, args) in refused {
            assert_eq!(
                parse(&name, &args),
                Err(NOT_A_COMMAND),
                "{name} {args} must be refused"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn each_command_but_stop_is_one_fixed_mpv_command() {
        let mpv = |command: PlayerCommand| command.mpv_command().map(Value::Array);
        assert_eq!(
            mpv(PlayerCommand::TogglePause),
            Some(json!(["cycle", "pause"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SeekRelative(-5.0)),
            Some(json!(["seek", "-5", "relative"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SeekRelative(60.0)),
            Some(json!(["seek", "60", "relative"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SeekRelative(-2.5)),
            Some(json!(["seek", "-2.5", "relative"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SeekAbsolute(12.25)),
            Some(json!(["seek", "12.25", "absolute"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SeekAbsolute(10_000_000.0)),
            Some(json!(["seek", "10000000", "absolute"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SeekAbsolute(1e-7)),
            Some(json!(["seek", "0.0000001", "absolute"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SetVolume(42.5)),
            Some(json!(["set", "volume", "42.5"]))
        );
        assert_eq!(
            mpv(PlayerCommand::ToggleMute),
            Some(json!(["cycle", "mute"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SetAudioTrack(2)),
            Some(json!(["set", "aid", "2"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SetSubtitleTrack(Some(3))),
            Some(json!(["set", "sid", "3"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SetSubtitleTrack(None)),
            Some(json!(["set", "sid", "no"]))
        );
        assert_eq!(
            mpv(PlayerCommand::SkipActiveMarker),
            Some(json!(["script-binding", "vela_markers/activate-skip"]))
        );
        assert_eq!(
            mpv(PlayerCommand::ToggleAutocrop),
            Some(json!(["script-binding", "vela_crop/toggle"]))
        );
        assert_eq!(mpv(PlayerCommand::Stop), None);
    }

    #[cfg(target_os = "linux")]
    mod with_a_player {
        use super::super::channel::{Gate, ANSWER_LIMIT, QUIET_AFTER_NO_ANSWER};
        use super::super::*;
        use crate::player::embedded::Session;
        use serde_json::json;
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::{UnixListener, UnixStream};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};

        /// A socket a test listens on, in Vela's private directory.
        fn listen(name: &str) -> (UnixListener, String) {
            let dir = crate::player::launch::private_runtime_dir().expect("the private directory");
            let path = dir.join(format!("control-{name}-{}.sock", std::process::id()));
            let _ = std::fs::remove_file(&path);
            let listener = UnixListener::bind(&path).expect("a socket to listen on");
            (listener, path.to_string_lossy().into_owned())
        }

        /// A slot holding the embedded play of `session_id` on `path`.
        fn slot_with(session_id: &str, path: &str) -> Mutex<Option<PlayerHandle>> {
            Mutex::new(Some(PlayerHandle::Embedded(Session::for_test_on(
                session_id, path,
            ))))
        }

        fn stop_requested(slot: &Mutex<Option<PlayerHandle>>) -> bool {
            match slot.lock().unwrap().as_ref() {
                Some(PlayerHandle::Embedded(session)) => session.stop_requested(),
                _ => unreachable!(),
            }
        }

        /// Answers each command with `answer(request_id)`, after `before`
        /// lines of its own (in which `{id}` stands for the command's
        /// request_id), and records what it was sent.
        fn serve(
            listener: UnixListener,
            before: Vec<String>,
            answer: impl Fn(u64) -> String + Send + 'static,
        ) -> std::thread::JoinHandle<Vec<Value>> {
            std::thread::spawn(move || {
                let mut received = Vec::new();
                listener.set_nonblocking(false).unwrap();
                // Each command is one connection; the test ends the server by
                // connecting with nothing to say.
                loop {
                    let (stream, _) = listener.accept().unwrap();
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return received;
                    }
                    let sent: Value = serde_json::from_str(&line).unwrap();
                    let id = sent["request_id"].as_u64().unwrap();
                    received.push(sent);
                    let mut writer = &stream;
                    for extra in &before {
                        let extra = extra.replace("{id}", &id.to_string());
                        writer.write_all(extra.as_bytes()).unwrap();
                        writer.write_all(b"\n").unwrap();
                    }
                    writer.write_all(answer(id).as_bytes()).unwrap();
                    writer.write_all(b"\n").unwrap();
                }
            })
        }

        fn finish(path: &str, server: std::thread::JoinHandle<Vec<Value>>) -> Vec<Value> {
            drop(UnixStream::connect(path).unwrap());
            let received = server.join().unwrap();
            let _ = std::fs::remove_file(path);
            received
        }

        fn send(
            slot: &Mutex<Option<PlayerHandle>>,
            session: &str,
            name: &str,
            args: Value,
        ) -> Result<(), String> {
            command(slot, &json!(session), &json!(name), &args)
        }

        #[test]
        fn each_command_reaches_the_player_as_its_fixed_mpv_command() {
            let (listener, path) = listen("bytes");
            let server = serve(listener, Vec::new(), |id| {
                format!(r#"{{"request_id":{id},"error":"success","data":null}}"#)
            });
            let slot = slot_with("play", &path);
            let sent = [
                ("toggle-pause", json!([])),
                ("seek-relative", json!([-5])),
                ("seek-absolute", json!([90.5])),
                ("set-volume", json!([40])),
                ("toggle-mute", json!([])),
                ("set-audio-track", json!([2])),
                ("set-subtitle-track", json!(["no"])),
                ("set-subtitle-track", json!([3])),
                ("skip-active-marker", json!([])),
                ("toggle-autocrop", json!([])),
            ];
            for (name, args) in &sent {
                assert_eq!(send(&slot, "play", name, args.clone()), Ok(()), "{name}");
            }
            let received = finish(&path, server);
            let commands: Vec<Value> = received
                .iter()
                .map(|line| line["command"].clone())
                .collect();
            assert_eq!(
                commands,
                [
                    json!(["cycle", "pause"]),
                    json!(["seek", "-5", "relative"]),
                    json!(["seek", "90.5", "absolute"]),
                    json!(["set", "volume", "40"]),
                    json!(["cycle", "mute"]),
                    json!(["set", "aid", "2"]),
                    json!(["set", "sid", "no"]),
                    json!(["set", "sid", "3"]),
                    json!(["script-binding", "vela_markers/activate-skip"]),
                    json!(["script-binding", "vela_crop/toggle"]),
                ]
            );
            // Nothing but the command and its id goes to the player.
            for line in &received {
                let keys: Vec<&String> = line.as_object().unwrap().keys().collect();
                assert_eq!(keys, ["command", "request_id"]);
            }
            assert!(!stop_requested(&slot));
        }

        #[test]
        fn events_before_the_answer_are_not_the_answer() {
            // mpv sends a new client its events too, and one may come first.
            let (listener, path) = listen("events");
            // Each line before the answer would refuse the command if it were
            // taken for the answer; the answer itself says yes.
            let server = serve(
                listener,
                vec![
                    "not json".to_string(),
                    r#"{"event":"seek"}"#.to_string(),
                    // An event that even carries this command's id is still
                    // an event.
                    r#"{"event":"property-change","request_id":{id},"error":"property unavailable"}"#
                        .to_string(),
                ],
                |id| format!(r#"{{"request_id":{id},"error":"success"}}"#),
            );
            let slot = slot_with("play", &path);
            assert_eq!(
                send(&slot, "play", "toggle-pause", json!([])),
                Ok(()),
                "the answer is the line with the command's id and no event"
            );
            finish(&path, server);
        }

        #[test]
        fn a_message_past_the_cap_before_the_answer_is_passed_over() {
            // Any script can make mpv send every client a message of any size.
            let (listener, path) = listen("huge");
            let huge = format!(
                r#"{{"event":"client-message","args":["x","{}"]}}"#,
                "a".repeat(1_500_000)
            );
            let server = serve(listener, vec![huge], |id| {
                format!(r#"{{"request_id":{id},"error":"success"}}"#)
            });
            let slot = slot_with("play", &path);
            assert_eq!(send(&slot, "play", "toggle-pause", json!([])), Ok(()));
            finish(&path, server);
        }

        #[test]
        fn events_that_keep_coming_do_not_stretch_the_limit() {
            // A player that sends events without end and never answers: the
            // limit is for the whole wait, not for each line.
            let (listener, path) = listen("event-stream");
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                let _ = BufReader::new(&stream).read_line(&mut line);
                let began = Instant::now();
                while began.elapsed() < Duration::from_secs(3) {
                    if (&stream).write_all(b"{\"event\":\"seek\"}\n").is_err() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            });
            let slot = Mutex::new(Some(PlayerHandle::Embedded(
                Session::for_test_with_channel(
                    "play",
                    crate::player::control::Channel::with_limits(
                        path.clone(),
                        Duration::from_millis(300),
                        Duration::from_secs(60),
                    ),
                ),
            )));
            let began = Instant::now();
            assert_eq!(
                send(&slot, "play", "toggle-pause", json!([])),
                Err(NO_ANSWER.to_string())
            );
            assert!(
                began.elapsed() < Duration::from_millis(1000),
                "given up at the limit: {:?}",
                began.elapsed()
            );
            server.join().unwrap();
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn an_answer_to_another_request_is_not_this_ones() {
            let (listener, path) = listen("other-id");
            let server = serve(
                listener,
                vec![r#"{"request_id":999999999,"error":"success"}"#.to_string()],
                |id| format!(r#"{{"request_id":{id},"error":"invalid parameter"}}"#),
            );
            let slot = slot_with("play", &path);
            assert_eq!(
                send(&slot, "play", "toggle-mute", json!([])),
                Err(REFUSED.to_string())
            );
            finish(&path, server);
        }

        #[test]
        fn another_sessions_view_and_a_bad_command_reach_nothing() {
            let (listener, path) = listen("identity");
            let server = serve(listener, Vec::new(), |id| {
                format!(r#"{{"request_id":{id},"error":"success"}}"#)
            });
            let slot = slot_with("play-b", &path);
            assert_eq!(
                send(&slot, "play-a", "toggle-pause", json!([])),
                Err(NOT_RUNNING.to_string())
            );
            assert_eq!(
                send(&slot, "", "toggle-pause", json!([])),
                Err(NOT_RUNNING.to_string())
            );
            assert_eq!(
                send(&slot, "play-b ", "toggle-pause", json!([])),
                Err(NOT_RUNNING.to_string())
            );
            assert_eq!(
                command(&slot, &json!(7), &json!("toggle-pause"), &json!([])),
                Err(NOT_RUNNING.to_string())
            );
            assert_eq!(
                send(
                    &slot,
                    "play-b",
                    "loadfile",
                    json!(["https://example.invalid/x"])
                ),
                Err(NOT_A_COMMAND.to_string())
            );
            // Another session's stop stops nothing.
            assert_eq!(send(&slot, "play-a", "stop", json!([])), Ok(()));
            assert!(!stop_requested(&slot), "the successor plays on");
            assert!(
                finish(&path, server).is_empty(),
                "nothing reached the player"
            );
        }

        #[test]
        fn stop_stops_its_own_play_without_touching_the_socket() {
            let (listener, path) = listen("stop");
            listener.set_nonblocking(true).unwrap();
            let slot = slot_with("play", &path);
            assert_eq!(send(&slot, "play", "stop", json!([])), Ok(()));
            assert!(stop_requested(&slot));
            assert!(
                matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
                "nobody connected"
            );
            let _ = std::fs::remove_file(&path);
            // Nothing playing: nothing to stop, and nothing to say about it.
            let empty = Mutex::new(None);
            assert_eq!(send(&empty, "play", "stop", json!([])), Ok(()));
            assert_eq!(
                send(&empty, "play", "toggle-pause", json!([])),
                Err(NOT_RUNNING.to_string())
            );
        }

        #[test]
        fn stop_works_while_another_command_waits_for_its_answer() {
            let (listener, path) = listen("stop-busy");
            let slot = Arc::new(slot_with("play", &path));
            let waiting = std::thread::spawn({
                let slot = slot.clone();
                move || send(&slot, "play", "toggle-pause", json!([]))
            });
            let (held, _) = listener.accept().unwrap();
            assert_eq!(send(&slot, "play", "stop", json!([])), Ok(()));
            assert!(stop_requested(&slot), "stop is never held up by the gate");
            drop(held);
            let _ = waiting.join();
            let _ = std::fs::remove_file(&path);
        }

        #[cfg(unix)]
        #[test]
        fn an_external_player_is_never_driven_or_stopped_from_the_view() {
            let child = std::process::Command::new("sleep")
                .arg("30")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("sleep is available");
            let slot = Mutex::new(Some(PlayerHandle::External(
                crate::player::external::ManagedChild::for_test(child),
            )));
            assert_eq!(
                send(&slot, "any", "toggle-pause", json!([])),
                Err(NOT_RUNNING.to_string())
            );
            assert_eq!(send(&slot, "any", "stop", json!([])), Ok(()));
            assert!(!is_running_embedded_play(&slot, &json!("any")));
            std::thread::sleep(Duration::from_millis(50));
            let mut player = slot.lock().unwrap().take().unwrap();
            assert!(
                matches!(player.has_ended(), Ok(false)),
                "the external player is still running"
            );
            player.kill().expect("the player can be stopped");
        }

        #[test]
        fn only_the_running_embedded_play_counts_as_running() {
            let slot = slot_with("play-b", "");
            assert!(is_running_embedded_play(&slot, &json!("play-b")));
            assert!(!is_running_embedded_play(&slot, &json!("play-a")));
            assert!(!is_running_embedded_play(&slot, &json!(null)));
            assert!(!is_running_embedded_play(
                &Mutex::new(None),
                &json!("play-b")
            ));
        }

        #[test]
        fn a_player_that_has_gone_is_not_running() {
            // No socket file at all.
            let slot = slot_with("play", "/nonexistent/vela-test/gone.sock");
            assert_eq!(
                send(&slot, "play", "toggle-pause", json!([])),
                Err(NOT_RUNNING.to_string())
            );
            // A socket file left behind with nobody listening.
            let (listener, path) = listen("left-behind");
            drop(listener);
            let slot = slot_with("play", &path);
            assert_eq!(
                send(&slot, "play", "toggle-pause", json!([])),
                Err(NOT_RUNNING.to_string())
            );
            let _ = std::fs::remove_file(&path);
            // A player that hangs up without answering.
            let (listener, path) = listen("hangs-up");
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                let _ = BufReader::new(&stream).read_line(&mut line);
            });
            let slot = slot_with("play", &path);
            assert_eq!(
                send(&slot, "play", "toggle-pause", json!([])),
                Err(NOT_RUNNING.to_string())
            );
            server.join().unwrap();
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn a_player_whose_queue_of_connections_is_full_does_not_hold_the_gate() {
            use std::os::fd::AsRawFd;
            // A listener that takes no connection and has room for one in
            // its queue (listening again sets the queue's length): the first
            // connect fills it, the next has to wait.
            let (listener, path) = listen("full-queue");
            // SAFETY: the listener's own socket, still open.
            assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
            let _queued = UnixStream::connect(&path).expect("the one place in the queue");
            let slot = Arc::new(Mutex::new(Some(PlayerHandle::Embedded(
                Session::for_test_with_channel(
                    "play",
                    crate::player::control::Channel::with_limits(
                        path.clone(),
                        Duration::from_millis(300),
                        Duration::from_secs(60),
                    ),
                ),
            ))));
            let (done, answer) = std::sync::mpsc::channel();
            std::thread::spawn({
                let slot = slot.clone();
                move || {
                    let _ = done.send(send(&slot, "play", "toggle-pause", json!([])));
                }
            });
            assert_eq!(
                answer.recv_timeout(Duration::from_secs(3)),
                Ok(Err(NO_ANSWER.to_string())),
                "the connect gives up within the limit"
            );
            // The command let go of its turn; the quiet spell follows.
            assert_eq!(
                send(&slot, "play", "toggle-pause", json!([])),
                Err(NO_ANSWER.to_string())
            );
            drop(listener);
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn the_waits_are_two_and_five_seconds() {
            assert_eq!(ANSWER_LIMIT, Duration::from_secs(2));
            assert_eq!(QUIET_AFTER_NO_ANSWER, Duration::from_secs(5));
        }

        #[test]
        fn twenty_commands_to_a_player_that_never_answers_make_one_connection() {
            let (listener, path) = listen("silent");
            listener.set_nonblocking(true).unwrap();
            let connections = Arc::new(AtomicUsize::new(0));
            let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
            // Takes every connection and never answers.
            let server = std::thread::spawn({
                let (connections, done) = (connections.clone(), done.clone());
                move || {
                    let mut held = Vec::new();
                    while !done.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((stream, _)) => {
                                connections.fetch_add(1, Ordering::SeqCst);
                                held.push(stream);
                            }
                            Err(_) => std::thread::sleep(Duration::from_millis(5)),
                        }
                    }
                }
            });
            let slot = Arc::new(Mutex::new(Some(PlayerHandle::Embedded(
                Session::for_test_with_channel(
                    "play",
                    crate::player::control::Channel::with_limits(
                        path.clone(),
                        Duration::from_millis(400),
                        Duration::from_secs(60),
                    ),
                ),
            ))));
            let first = std::thread::spawn({
                let slot = slot.clone();
                move || send(&slot, "play", "toggle-pause", json!([]))
            });
            let deadline = Instant::now() + Duration::from_secs(5);
            while connections.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            let mut answers = Vec::new();
            // Nine while the first waits: refused at once as busy.
            for _ in 0..9 {
                let began = Instant::now();
                answers.push(send(&slot, "play", "seek-relative", json!([5])));
                assert!(
                    began.elapsed() < Duration::from_millis(100),
                    "refused at once"
                );
            }
            assert_eq!(first.join().unwrap(), Err(NO_ANSWER.to_string()));
            // Ten after it went unanswered: refused at once.
            for _ in 0..10 {
                let began = Instant::now();
                answers.push(send(&slot, "play", "seek-relative", json!([5])));
                assert!(
                    began.elapsed() < Duration::from_millis(100),
                    "refused at once"
                );
            }
            assert_eq!(&answers[..9], vec![Err(BUSY.to_string()); 9].as_slice());
            assert_eq!(
                &answers[9..],
                vec![Err(NO_ANSWER.to_string()); 10].as_slice()
            );
            std::thread::sleep(Duration::from_millis(100));
            assert_eq!(
                connections.load(Ordering::SeqCst),
                1,
                "one connection for twenty commands"
            );
            // Stop is never refused.
            assert_eq!(send(&slot, "play", "stop", json!([])), Ok(()));
            assert!(stop_requested(&slot));
            done.store(true, Ordering::SeqCst);
            server.join().unwrap();
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn after_an_unanswered_command_the_player_is_left_alone_for_the_quiet_spell() {
            let quiet = Duration::from_secs(5);
            let gate = Gate::new(quiet);
            let start = Instant::now();
            let pass = gate.enter(start).expect("the first command goes");
            assert_eq!(gate.enter(start).err(), Some(BUSY), "one at a time");
            gate.unanswered(start + Duration::from_secs(2));
            drop(pass);
            let unanswered = start + Duration::from_secs(2);
            assert_eq!(gate.enter(unanswered).err(), Some(NO_ANSWER));
            assert_eq!(
                gate.enter(unanswered + quiet - Duration::from_millis(1))
                    .err(),
                Some(NO_ANSWER),
                "still quiet just before the spell ends"
            );
            let pass = gate
                .enter(unanswered + quiet)
                .expect("and goes again after it");
            drop(pass);
            assert!(
                gate.enter(unanswered + quiet).is_ok(),
                "an answered command leaves no spell"
            );
        }

        #[test]
        fn the_quiet_spell_is_kept_by_the_play_and_ends() {
            // The same through the channel: a short spell, then commands go
            // again.
            let (listener, path) = listen("quiet-ends");
            let answering = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let server = std::thread::spawn({
                let answering = answering.clone();
                move || {
                    for stream in listener.incoming() {
                        let stream = stream.unwrap();
                        let mut line = String::new();
                        if BufReader::new(&stream).read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        if answering.load(Ordering::SeqCst) {
                            let id = serde_json::from_str::<Value>(&line).unwrap()["request_id"]
                                .as_u64()
                                .unwrap();
                            let _ = (&stream).write_all(
                                format!("{{\"request_id\":{id},\"error\":\"success\"}}\n")
                                    .as_bytes(),
                            );
                        } else {
                            std::thread::sleep(Duration::from_millis(400));
                        }
                    }
                }
            });
            let slot = Mutex::new(Some(PlayerHandle::Embedded(
                Session::for_test_with_channel(
                    "play",
                    crate::player::control::Channel::with_limits(
                        path.clone(),
                        Duration::from_millis(150),
                        Duration::from_millis(600),
                    ),
                ),
            )));
            assert_eq!(
                send(&slot, "play", "toggle-pause", json!([])),
                Err(NO_ANSWER.to_string())
            );
            answering.store(true, Ordering::SeqCst);
            assert_eq!(
                send(&slot, "play", "toggle-pause", json!([])),
                Err(NO_ANSWER.to_string())
            );
            std::thread::sleep(Duration::from_millis(700));
            assert_eq!(send(&slot, "play", "toggle-pause", json!([])), Ok(()));
            drop(UnixStream::connect(&path).unwrap());
            server.join().unwrap();
            let _ = std::fs::remove_file(&path);
        }
    }
}
