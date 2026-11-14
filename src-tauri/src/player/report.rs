//! What the player view is told about a running embedded play.
//!
//! One thread per embedded play, on a connection of its own to mpv's IPC
//! socket, observes the handful of properties the view's controls show and
//! sends them to the page as the `player-state` event: at most four times a
//! second, and at least once a second so a paused play still shows it is
//! alive.
//!
//! The payload is built field by field from values whose type has been
//! checked. Two of them come from `user-data`, which any script or IPC client
//! can write: they go through closed sets, so no other text from mpv reaches
//! the page. Track titles and languages are cut short. No URL, path or file
//! name is ever part of it (a track's `external-filename` is not read, and a
//! track from a file of its own goes without a title, which mpv may have made
//! from that file's name).
//!
//! Only Linux has an embedded play, so everything that reads from mpv is
//! built on Linux alone; the payload and the notifier's type are built
//! everywhere, for `commands.rs` and `crate::playback`.

#[cfg(target_os = "linux")]
use std::io::{BufRead, ErrorKind, Read};
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use serde::Serialize;
#[cfg(target_os = "linux")]
use serde_json::Value;

/// Hands a state to the page. Built in `commands.rs`, where the app is.
pub(crate) type StateNotify = Arc<dyn Fn(&PlayerState) + Send + Sync>;

/// The event the page listens for.
pub(crate) const EVENT: &str = "player-state";

/// The most characters of a track's title or language that are sent.
#[cfg(target_os = "linux")]
const TEXT_LIMIT: usize = 200;

/// The properties observed, in the order they are asked for.
#[cfg(target_os = "linux")]
const OBSERVED: [&str; 8] = [
    "time-pos",
    "duration",
    "pause",
    "volume",
    "mute",
    "track-list",
    "user-data/vela-markers/active",
    "user-data/vela-crop/state",
];

/// One audio or subtitle track as the page sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Track {
    pub(crate) id: i64,
    pub(crate) title: Option<String>,
    pub(crate) lang: Option<String>,
    pub(crate) selected: bool,
}

/// The `player-state` payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayerState {
    pub(crate) session_id: String,
    /// Seconds.
    pub(crate) position: Option<f64>,
    /// Seconds: mpv's, or the item's own when mpv reports none.
    pub(crate) duration: Option<f64>,
    pub(crate) paused: Option<bool>,
    /// 0 to 100, as mpv reports it.
    pub(crate) volume: Option<f64>,
    pub(crate) muted: Option<bool>,
    pub(crate) audio_tracks: Vec<Track>,
    pub(crate) subtitle_tracks: Vec<Track>,
    /// The marker that can be skipped now: `intro`, `credits` or
    /// `commercial`.
    pub(crate) skip: Option<&'static str>,
    /// The crop script's state: `off`, `measuring` or `on`. None when no
    /// crop script is loaded.
    pub(crate) crop: Option<&'static str>,
}

/// What mpv has said about the observed properties, each checked for type.
#[cfg(target_os = "linux")]
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Seen {
    position: Option<f64>,
    duration: Option<f64>,
    paused: Option<bool>,
    volume: Option<f64>,
    muted: Option<bool>,
    audio_tracks: Vec<Track>,
    subtitle_tracks: Vec<Track>,
    skip: Option<&'static str>,
    crop: Option<&'static str>,
}

#[cfg(target_os = "linux")]
fn finite(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
}

/// A text cut to [`TEXT_LIMIT`] characters.
#[cfg(target_os = "linux")]
fn short_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(|text| text.chars().take(TEXT_LIMIT).collect())
}

/// One entry of mpv's `track-list`, when it is an audio or subtitle track.
#[cfg(target_os = "linux")]
fn track(entry: &Value) -> Option<(bool, Track)> {
    let audio = match entry.get("type")?.as_str()? {
        "audio" => true,
        "sub" => false,
        _ => return None,
    };
    // A track from a file of its own (`--sub-file`, `sub-add`, ...) that has no
    // title of its own is given one by mpv: the last part of its file name or
    // URL, query and all, which can carry a server's token. So such a track
    // shows no title at all.
    let external = entry.get("external").and_then(Value::as_bool) != Some(false);
    Some((
        audio,
        Track {
            id: entry.get("id")?.as_i64()?,
            title: if external {
                None
            } else {
                short_text(entry.get("title"))
            },
            lang: short_text(entry.get("lang")),
            selected: entry
                .get("selected")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
    ))
}

#[cfg(target_os = "linux")]
impl Seen {
    /// Take in one line from mpv. Returns whether anything changed. Only a
    /// property-change event for an observed name is read.
    pub(crate) fn apply(&mut self, line: &Value) -> bool {
        if line.get("event").and_then(Value::as_str) != Some("property-change") {
            return false;
        }
        let Some(name) = line.get("name").and_then(Value::as_str) else {
            return false;
        };
        // A property mpv does not have (yet) comes without data.
        let data = line.get("data");
        let before = self.clone();
        match name {
            "time-pos" => self.position = finite(data).map(|seconds| seconds.max(0.0)),
            "duration" => self.duration = finite(data).filter(|seconds| *seconds > 0.0),
            "pause" => self.paused = data.and_then(Value::as_bool),
            "volume" => self.volume = finite(data),
            "mute" => self.muted = data.and_then(Value::as_bool),
            "track-list" => {
                let (audio, subtitles): (Vec<_>, Vec<_>) = data
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(track)
                    .partition(|(audio, _)| *audio);
                self.audio_tracks = audio.into_iter().map(|(_, track)| track).collect();
                self.subtitle_tracks = subtitles.into_iter().map(|(_, track)| track).collect();
            }
            "user-data/vela-markers/active" => {
                self.skip = match data.and_then(Value::as_str) {
                    Some("intro") => Some("intro"),
                    Some("credits") => Some("credits"),
                    Some("commercial") => Some("commercial"),
                    // Nothing active (""), or anything else written there.
                    _ => None,
                }
            }
            "user-data/vela-crop/state" => {
                self.crop = match data.and_then(Value::as_str) {
                    Some("off") => Some("off"),
                    Some("measuring") => Some("measuring"),
                    Some("on") => Some("on"),
                    _ => None,
                }
            }
            _ => return false,
        }
        *self != before
    }

    /// The payload, field by field.
    pub(crate) fn payload(&self, session_id: &str, item_duration: Option<Duration>) -> PlayerState {
        PlayerState {
            session_id: session_id.to_string(),
            position: self.position,
            duration: self.duration.or_else(|| {
                item_duration
                    .map(|duration| duration.as_secs_f64())
                    .filter(|seconds| *seconds > 0.0)
            }),
            paused: self.paused,
            volume: self.volume,
            muted: self.muted,
            audio_tracks: self.audio_tracks.clone(),
            subtitle_tracks: self.subtitle_tracks.clone(),
            skip: self.skip,
            crop: self.crop,
        }
    }
}

/// The shortest time between two states sent: at most four a second.
#[cfg(target_os = "linux")]
pub(crate) const MIN_GAP: Duration = Duration::from_millis(250);
/// The longest time without one: at least one a second.
#[cfg(target_os = "linux")]
pub(crate) const MAX_GAP: Duration = Duration::from_secs(1);

/// When to send. A change is sent at once when [`MIN_GAP`] has passed since
/// the last state sent, otherwise when it has; with no change, a state goes
/// every [`MAX_GAP`]. Nothing is sent before mpv has said anything.
#[cfg(target_os = "linux")]
#[derive(Debug, Default)]
pub(crate) struct Throttle {
    last_sent: Option<Instant>,
    changed: bool,
}

#[cfg(target_os = "linux")]
impl Throttle {
    pub(crate) fn note_change(&mut self) {
        self.changed = true;
    }

    /// Whether a state should be sent at `now`.
    pub(crate) fn due(&self, now: Instant) -> bool {
        match self.last_sent {
            None => self.changed,
            Some(last) => {
                let since = now.saturating_duration_since(last);
                (self.changed && since >= MIN_GAP) || since >= MAX_GAP
            }
        }
    }

    pub(crate) fn sent(&mut self, now: Instant) {
        self.last_sent = Some(now);
        self.changed = false;
    }

    /// How long a read may wait at `now` without making a state late.
    pub(crate) fn patience(&self, now: Instant) -> Duration {
        let Some(last) = self.last_sent else {
            return MIN_GAP;
        };
        let next = last + if self.changed { MIN_GAP } else { MAX_GAP };
        next.saturating_duration_since(now)
            .clamp(Duration::from_millis(10), MIN_GAP)
    }
}

/// The most one line may hold. A longer one is passed over: no observed
/// property comes near it, but any script can make mpv send every client a
/// message of any size.
#[cfg(target_os = "linux")]
const LINE_CAP: usize = 1 << 20;

/// What one read gave.
#[cfg(target_os = "linux")]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Read1 {
    /// A whole line, without its newline.
    Line(Vec<u8>),
    /// The read limit passed. Whatever part of a line had arrived is kept.
    Waited,
    /// The connection ended or failed.
    End,
}

/// Lines from mpv over a connection with a read limit. A line that arrives
/// in pieces across a timeout is kept until it is whole: the buffer is
/// emptied only after a complete line has been taken out of it. A line
/// longer than [`LINE_CAP`] is dropped as it arrives, so it never takes more
/// memory than that, and the next line is read as usual.
#[cfg(target_os = "linux")]
pub(crate) struct Lines<R> {
    reader: std::io::BufReader<R>,
    partial: Vec<u8>,
    /// The line being read has passed the cap: its bytes are dropped up to
    /// its newline.
    dropping: bool,
}

#[cfg(target_os = "linux")]
impl<R: Read> Lines<R> {
    pub(crate) fn new(reader: R) -> Self {
        Lines {
            reader: std::io::BufReader::new(reader),
            partial: Vec::new(),
            dropping: false,
        }
    }

    pub(crate) fn next(&mut self) -> Read1 {
        loop {
            let available = match self.reader.fill_buf() {
                // The connection ended, in the middle of a line or not.
                Ok([]) => return Read1::End,
                Ok(available) => available,
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    return Read1::Waited
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Read1::End,
            };
            let (piece, newline) = match available.iter().position(|byte| *byte == b'\n') {
                Some(at) => (&available[..at], true),
                None => (available, false),
            };
            if !self.dropping && self.partial.len() + piece.len() > LINE_CAP {
                self.dropping = true;
                self.partial = Vec::new();
            }
            if !self.dropping {
                self.partial.extend_from_slice(piece);
            }
            let used = piece.len() + usize::from(newline);
            self.reader.consume(used);
            if newline {
                if std::mem::replace(&mut self.dropping, false) {
                    continue;
                }
                return Read1::Line(std::mem::take(&mut self.partial));
            }
        }
    }
}

/// Connect to a player's IPC socket, waiting at most `limit`. With no such
/// file, or nobody listening any more, the connect fails at once. But while
/// a listener's queue of connections not yet taken is full, Linux holds a
/// connect until there is room, for as long as the socket's send timeout
/// allows: std's `connect` would wait for ever, so the socket is made, given
/// its timeout, then connected. A wait that ran out (or that a signal ended)
/// is an error of `EAGAIN` (or `EINTR`). The stream keeps `limit` as its
/// write timeout.
#[cfg(target_os = "linux")]
pub(crate) fn connect(
    path: &str,
    limit: Duration,
) -> std::io::Result<std::os::unix::net::UnixStream> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::net::UnixStream;

    let bytes = std::path::Path::new(path).as_os_str().as_bytes();
    // SAFETY: an all-zero sockaddr_un is a valid, empty address.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    // The path and the zero after it must fit.
    if bytes.is_empty() || bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(ErrorKind::InvalidInput.into());
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in address.sun_path.iter_mut().zip(bytes) {
        *slot = *byte as libc::c_char;
    }
    let length = std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1;

    // SAFETY: plain arguments; the result is checked before use.
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a new socket that nothing else owns.
    let stream = UnixStream::from(unsafe { OwnedFd::from_raw_fd(fd) });
    // The send timeout (SO_SNDTIMEO) is also how long a connect may wait.
    stream.set_write_timeout(Some(limit))?;
    // SAFETY: a live socket, and an address of the length given.
    let connected = unsafe {
        libc::connect(
            stream.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            length as libc::socklen_t,
        )
    };
    if connected == 0 {
        Ok(stream)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// How long one of the reporter's connects may wait for room in a full
/// queue before it looks at the play's stop flag again.
#[cfg(target_os = "linux")]
const CONNECT_LIMIT: Duration = Duration::from_millis(100);

/// How long the reporter's requests to observe may take to go out.
#[cfg(target_os = "linux")]
const OBSERVE_WRITE_LIMIT: Duration = Duration::from_secs(1);

/// One line from mpv as JSON. mpv passes file names and tags on as it found
/// them, so a line can hold bytes that are not UTF-8 (its IPC documentation
/// tells clients to replace them before parsing); refusing the whole line
/// would lose every track with it.
#[cfg(target_os = "linux")]
fn parse_line(line: &[u8]) -> Option<Value> {
    serde_json::from_str(&String::from_utf8_lossy(line)).ok()
}

/// Start the reporter of one embedded play. It ends when mpv closes the
/// connection, when the play's stop flag is set, or when it could not
/// connect within the retries the other per-play threads allow.
#[cfg(target_os = "linux")]
pub(crate) fn spawn_reporter(
    socket_path: String,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    session_id: String,
    item_duration: Option<Duration>,
    notify: StateNotify,
) -> std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::Ordering;
    std::thread::Builder::new()
        .name("mpv-state-reporter".into())
        .spawn(move || {
            let mut stream = None;
            for _ in 0..50 {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                // Bounded, so a player that takes no connections cannot keep
                // the thread from seeing the stop flag.
                match connect(&socket_path, CONNECT_LIMIT) {
                    Ok(connected) => {
                        stream = Some(connected);
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            let Some(mut stream) = stream else { return };
            if stream
                .set_write_timeout(Some(OBSERVE_WRITE_LIMIT))
                .is_err()
            {
                return;
            }
            let Ok(read_half) = stream.try_clone() else {
                return;
            };
            let mut observe = String::new();
            for (id, name) in OBSERVED.iter().enumerate() {
                let line = serde_json::json!({ "command": ["observe_property", id + 1, name] });
                observe.push_str(&line.to_string());
                observe.push('\n');
            }
            if stream.write_all(observe.as_bytes()).is_err() {
                return;
            }
            let mut lines = Lines::new(read_half);
            let mut seen = Seen::default();
            let mut throttle = Throttle::default();
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                if stream
                    .set_read_timeout(Some(throttle.patience(Instant::now())))
                    .is_err()
                {
                    return;
                }
                match lines.next() {
                    Read1::Line(line) => {
                        if let Some(value) = parse_line(&line) {
                            if seen.apply(&value) {
                                throttle.note_change();
                            }
                        }
                    }
                    Read1::Waited => {}
                    Read1::End => return,
                }
                let now = Instant::now();
                if throttle.due(now) && !stop_flag.load(Ordering::Relaxed) {
                    notify(&seen.payload(&session_id, item_duration));
                    throttle.sent(now);
                }
            }
        })?;
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use serde_json::json;

    fn change(name: &str, data: Value) -> Value {
        json!({ "event": "property-change", "id": 1, "name": name, "data": data })
    }

    fn seen_after(lines: &[Value]) -> Seen {
        let mut seen = Seen::default();
        for line in lines {
            seen.apply(line);
        }
        seen
    }

    #[test]
    fn the_payload_carries_the_observed_values_under_the_pages_names() {
        let seen = seen_after(&[
            change("time-pos", json!(12.5)),
            change("duration", json!(600.0)),
            change("pause", json!(true)),
            change("volume", json!(80.0)),
            change("mute", json!(false)),
            change(
                "track-list",
                json!([
                    { "id": 1, "type": "video", "selected": true, "title": "Main" },
                    { "id": 1, "type": "audio", "selected": true, "external": false, "lang": "eng", "title": "Stereo" },
                    { "id": 2, "type": "audio", "selected": false },
                    { "id": 1, "type": "sub", "selected": false, "lang": "fre" },
                ]),
            ),
            change("user-data/vela-markers/active", json!("credits")),
            change("user-data/vela-crop/state", json!("measuring")),
        ]);
        let payload = serde_json::to_value(seen.payload("play-1", None)).unwrap();
        assert_eq!(
            payload,
            json!({
                "sessionId": "play-1",
                "position": 12.5,
                "duration": 600.0,
                "paused": true,
                "volume": 80.0,
                "muted": false,
                "audioTracks": [
                    { "id": 1, "title": "Stereo", "lang": "eng", "selected": true },
                    { "id": 2, "title": null, "lang": null, "selected": false },
                ],
                "subtitleTracks": [
                    { "id": 1, "title": null, "lang": "fre", "selected": false },
                ],
                "skip": "credits",
                "crop": "measuring",
            })
        );
    }

    #[test]
    fn nothing_seen_is_all_null_and_the_items_own_duration_stands_in() {
        let payload =
            serde_json::to_value(Seen::default().payload("p", Some(Duration::from_secs(90))))
                .unwrap();
        assert_eq!(
            payload,
            json!({
                "sessionId": "p",
                "position": null,
                "duration": 90.0,
                "paused": null,
                "volume": null,
                "muted": null,
                "audioTracks": [],
                "subtitleTracks": [],
                "skip": null,
                "crop": null,
            })
        );
        // mpv's own duration wins once it has one.
        let seen = seen_after(&[change("duration", json!(95.5))]);
        assert_eq!(
            seen.payload("p", Some(Duration::from_secs(90))).duration,
            Some(95.5)
        );
        // A zero duration is no duration.
        let seen = seen_after(&[change("duration", json!(0.0))]);
        assert_eq!(seen.payload("p", None).duration, None);
        assert_eq!(seen.payload("p", Some(Duration::ZERO)).duration, None);
    }

    #[test]
    fn skip_and_crop_are_closed_sets() {
        for (written, skip) in [
            (json!("intro"), Some("intro")),
            (json!("credits"), Some("credits")),
            (json!("commercial"), Some("commercial")),
            (json!(""), None),
            (json!("Intro"), None),
            (json!("https://example.invalid/?token=secret"), None),
            (json!(1), None),
            (json!(null), None),
        ] {
            let seen = seen_after(&[change("user-data/vela-markers/active", written.clone())]);
            assert_eq!(seen.payload("p", None).skip, skip, "{written}");
        }
        for (written, crop) in [
            (json!("off"), Some("off")),
            (json!("measuring"), Some("measuring")),
            (json!("on"), Some("on")),
            (json!(""), None),
            (json!("ON"), None),
            (json!("/home/someone/video.mkv"), None),
            (json!(true), None),
        ] {
            let seen = seen_after(&[change("user-data/vela-crop/state", written.clone())]);
            assert_eq!(seen.payload("p", None).crop, crop, "{written}");
        }
        // Set, then cleared.
        let seen = seen_after(&[
            change("user-data/vela-markers/active", json!("intro")),
            change("user-data/vela-markers/active", json!("")),
        ]);
        assert_eq!(seen.payload("p", None).skip, None);
    }

    #[test]
    fn no_url_or_other_text_from_mpv_reaches_the_page() {
        let secret = "https://media.invalid/stream?api_key=SECRET-TOKEN";
        let seen = seen_after(&[
            change("user-data/vela-markers/active", json!(secret)),
            change("user-data/vela-crop/state", json!(secret)),
            change("filename", json!(secret)),
            change("path", json!(secret)),
            change("user-data/anything", json!(secret)),
            change(
                "track-list",
                json!([
                    { "id": 3, "type": "sub", "selected": true, "external": true,
                      "external-filename": secret, "codec": secret, "decoder-desc": secret },
                    { "id": 2, "type": "audio", "selected": true, "unknown": secret, "metadata": { "url": secret } },
                    // mpv titles an untitled file of its own after the last
                    // part of its URL, query included.
                    { "id": 4, "type": "sub", "selected": false, "external": true,
                      "title": "Stream.srt?api_key=SECRET-TOKEN", "lang": "en" },
                    { "id": 5, "type": "audio", "selected": false, "external": true,
                      "title": "dub.mka?api_key=SECRET-TOKEN" },
                    // A track that does not say where it is from is taken as
                    // one from a file of its own.
                    { "id": 6, "type": "sub", "selected": false,
                      "title": "?api_key=SECRET-TOKEN" },
                ]),
            ),
            json!({ "event": "end-file", "reason": secret }),
            json!({ "request_id": 1, "error": "success", "data": secret }),
        ]);
        let text = serde_json::to_string(&seen.payload("p", None)).unwrap();
        assert!(!text.contains("SECRET"), "{text}");
        assert!(!text.contains("media.invalid"), "{text}");
        assert!(
            text.contains(
                r#""subtitleTracks":[{"id":3,"title":null,"lang":null,"selected":true},{"id":4,"title":null,"lang":"en","selected":false},{"id":6,"title":null,"lang":null,"selected":false}]"#
            ),
            "{text}"
        );
        assert!(
            text.contains(r#""audioTracks":[{"id":2,"title":null,"lang":null,"selected":true},{"id":5,"title":null,"lang":null,"selected":false}]"#),
            "{text}"
        );
    }

    #[test]
    fn every_property_observed_is_one_the_state_reads() {
        // A value of the right type for each, in the order they are observed.
        let values = [
            json!(3.0),
            json!(60.0),
            json!(true),
            json!(50.0),
            json!(true),
            json!([{ "id": 1, "type": "audio", "external": false, "selected": true }]),
            json!("intro"),
            json!("on"),
        ];
        assert_eq!(OBSERVED.len(), values.len());
        for (name, value) in OBSERVED.iter().zip(values) {
            let mut seen = Seen::default();
            assert!(seen.apply(&change(name, value)), "{name} is observed but never read");
        }
    }

    #[test]
    fn track_text_is_cut_to_two_hundred_characters() {
        let long = "é".repeat(500);
        let seen = seen_after(&[change(
            "track-list",
            json!([{ "id": 1, "type": "audio", "selected": true, "external": false, "title": long, "lang": long }]),
        )]);
        let track = &seen.payload("p", None).audio_tracks[0];
        assert_eq!(track.title.as_ref().unwrap().chars().count(), 200);
        assert_eq!(track.lang.as_ref().unwrap().chars().count(), 200);
    }

    #[test]
    fn only_typed_property_changes_count() {
        let mut seen = Seen::default();
        assert!(!seen.apply(&json!({ "event": "seek" })));
        assert!(!seen.apply(&json!({ "request_id": 1, "error": "success", "data": 5.0 })));
        assert!(!seen.apply(&change("display-width", json!(3840))));
        assert!(!seen.apply(&change("time-pos", json!("5"))));
        assert!(!seen.apply(&change("pause", json!(1))));
        assert!(!seen.apply(&change("volume", json!(null))));
        assert_eq!(seen, Seen::default());
        assert!(seen.apply(&change("time-pos", json!(5.0))));
        assert!(
            !seen.apply(&change("time-pos", json!(5.0))),
            "the same value is no change"
        );
        assert!(seen.apply(&change("time-pos", json!(-1.0))));
        assert_eq!(
            seen.payload("p", None).position,
            Some(0.0),
            "a negative position reads as the start"
        );
        // A property that goes away goes back to unknown.
        assert!(seen.apply(&json!({ "event": "property-change", "name": "time-pos" })));
        assert_eq!(seen.payload("p", None).position, None);
    }

    #[test]
    fn the_throttle_sends_at_most_four_a_second_and_at_least_one() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut throttle = Throttle::default();
        assert!(!throttle.due(at(0)), "nothing before mpv has said anything");
        assert!(!throttle.due(at(5000)));
        throttle.note_change();
        assert!(throttle.due(at(5000)), "the first values go at once");
        throttle.sent(at(5000));
        // A change soon after waits for the boundary.
        throttle.note_change();
        assert!(!throttle.due(at(5100)));
        assert!(!throttle.due(at(5249)));
        assert!(throttle.due(at(5250)));
        throttle.sent(at(5250));
        // A change long after goes at once.
        assert!(!throttle.due(at(5900)));
        throttle.note_change();
        assert!(throttle.due(at(5900)));
        throttle.sent(at(5900));
        // No change: one a second.
        assert!(!throttle.due(at(6899)));
        assert!(throttle.due(at(6900)));
        throttle.sent(at(6900));
        // Changes every 10 ms for ten seconds: one every quarter second.
        let mut sent = vec![6900];
        for ms in (6910..16910).step_by(10) {
            throttle.note_change();
            if throttle.due(at(ms)) {
                throttle.sent(at(ms));
                sent.push(ms);
            }
        }
        assert_eq!(sent.len(), 41, "{sent:?}");
        assert!(
            sent.windows(2).all(|pair| pair[1] - pair[0] == 250),
            "{sent:?}"
        );
    }

    #[test]
    fn a_read_never_waits_past_the_next_state() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut throttle = Throttle::default();
        assert_eq!(throttle.patience(at(0)), MIN_GAP);
        throttle.note_change();
        throttle.sent(at(0));
        assert_eq!(
            throttle.patience(at(100)),
            MIN_GAP,
            "a quarter second at most"
        );
        assert_eq!(
            throttle.patience(at(900)),
            Duration::from_millis(100),
            "the heartbeat is due then"
        );
        throttle.note_change();
        assert_eq!(
            throttle.patience(at(200)),
            Duration::from_millis(50),
            "the change is due then"
        );
        assert_eq!(
            throttle.patience(at(400)),
            Duration::from_millis(10),
            "never zero, which would block"
        );
    }

    /// Hands out its pieces one read at a time, at most as much as is asked
    /// for; `None` is a read whose limit passed.
    struct Scripted(std::collections::VecDeque<Option<Vec<u8>>>);

    impl Read for Scripted {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.pop_front() {
                None => Ok(0),
                Some(None) => Err(ErrorKind::WouldBlock.into()),
                Some(Some(mut piece)) => {
                    let n = piece.len().min(buf.len());
                    let rest = piece.split_off(n);
                    buf[..n].copy_from_slice(&piece);
                    if !rest.is_empty() {
                        self.0.push_front(Some(rest));
                    }
                    Ok(n)
                }
            }
        }
    }

    fn scripted(pieces: Vec<Option<Vec<u8>>>) -> Lines<Scripted> {
        Lines::new(Scripted(pieces.into()))
    }

    #[test]
    fn lines_are_whole_across_waits_and_end_with_the_connection() {
        let mut lines = scripted(vec![
            Some(b"{\"a\":".to_vec()),
            None,
            Some(b"1}\n{\"b\":2}\n{\"c\"".to_vec()),
        ]);
        assert_eq!(lines.next(), Read1::Waited);
        assert_eq!(lines.next(), Read1::Line(b"{\"a\":1}".to_vec()));
        assert_eq!(lines.next(), Read1::Line(b"{\"b\":2}".to_vec()));
        assert_eq!(lines.next(), Read1::End, "the connection ended inside a line");
    }

    #[test]
    fn a_line_past_the_cap_is_dropped_as_it_arrives_and_the_next_one_is_read() {
        let chunk = vec![b'x'; 64 * 1024];
        let mut pieces: Vec<Option<Vec<u8>>> = (0..48).map(|_| Some(chunk.clone())).collect();
        // Three MiB in, with no newline yet.
        pieces.push(None);
        pieces.push(Some(b"tail\n{\"after\":true}\n".to_vec()));
        let mut lines = scripted(pieces);
        assert_eq!(lines.next(), Read1::Waited);
        // A mebibyte, written out: the bound must not move with the constant.
        assert!(
            lines.partial.capacity() <= 1 << 20,
            "the long line was not kept: {} bytes held",
            lines.partial.capacity()
        );
        assert_eq!(
            lines.next(),
            Read1::Line(b"{\"after\":true}".to_vec()),
            "the long line is passed over and the next is read"
        );
        assert_eq!(lines.next(), Read1::End);
        // A line of exactly the cap is still a line.
        let mut whole = vec![b'y'; LINE_CAP];
        whole.push(b'\n');
        let mut lines = scripted(vec![Some(whole)]);
        assert!(matches!(lines.next(), Read1::Line(line) if line.len() == LINE_CAP));
    }

    mod with_a_player {
        use super::super::*;
        use serde_json::Value;
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::{UnixListener, UnixStream};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};

        fn listen(name: &str) -> (UnixListener, String) {
            let dir = crate::player::launch::private_runtime_dir().expect("the private directory");
            let path = dir.join(format!("report-{name}-{}.sock", std::process::id()));
            let _ = std::fs::remove_file(&path);
            let listener = UnixListener::bind(&path).expect("a socket to listen on");
            (listener, path.to_string_lossy().into_owned())
        }

        fn recording() -> (Arc<Mutex<Vec<PlayerState>>>, StateNotify) {
            let states = Arc::new(Mutex::new(Vec::new()));
            let notify: StateNotify = {
                let states = states.clone();
                Arc::new(move |state: &PlayerState| states.lock().unwrap().push(state.clone()))
            };
            (states, notify)
        }

        fn wait_for(what: impl Fn() -> bool) -> bool {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !what() {
                if Instant::now() >= deadline {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            true
        }

        /// Accept the reporter and read its eight requests.
        fn accept(listener: &UnixListener) -> (UnixStream, Vec<Value>) {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut asked = Vec::new();
            for _ in 0..8 {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                asked.push(serde_json::from_str(&line).unwrap());
            }
            (stream, asked)
        }

        #[test]
        fn the_reporter_observes_the_views_properties_and_reports_them() {
            let (listener, path) = listen("observes");
            let (states, notify) = recording();
            let kept = notify.clone();
            let stop = Arc::new(AtomicBool::new(false));
            spawn_reporter(path.clone(), stop.clone(), "play-7".into(), None, notify).unwrap();
            let (mut stream, asked) = accept(&listener);
            let names: Vec<&str> = asked
                .iter()
                .map(|line| line["command"][2].as_str().unwrap())
                .collect();
            // mpv's own names, written out: a name mpv does not have would
            // never be reported.
            assert_eq!(
                names,
                [
                    "time-pos",
                    "duration",
                    "pause",
                    "volume",
                    "mute",
                    "track-list",
                    "user-data/vela-markers/active",
                    "user-data/vela-crop/state",
                ]
            );
            assert!(asked
                .iter()
                .all(|line| line["command"][0] == "observe_property"));
            stream
                .write_all(
                    b"{\"event\":\"property-change\",\"id\":3,\"name\":\"pause\",\"data\":true}\n",
                )
                .unwrap();
            assert!(wait_for(|| !states.lock().unwrap().is_empty()));
            let first = states.lock().unwrap()[0].clone();
            assert_eq!(first.session_id, "play-7");
            assert_eq!(first.paused, Some(true));
            // Paused, nothing changes: still one a second.
            std::thread::sleep(Duration::from_millis(2300));
            let count = states.lock().unwrap().len();
            assert!(
                (2..=4).contains(&count),
                "{count} states in 2.3 s of silence"
            );
            // The play stops while mpv still holds the connection open: the
            // reporter ends by itself.
            stop.store(true, Ordering::SeqCst);
            assert!(
                wait_for(|| Arc::strong_count(&kept) == 1),
                "a stopped play's reporter ends and lets go of its notifier"
            );
            let ended = states.lock().unwrap().len();
            std::thread::sleep(Duration::from_millis(1500));
            assert_eq!(states.lock().unwrap().len(), ended, "and sends nothing more");
            drop(stream);
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn a_line_that_arrives_in_two_pieces_across_a_wait_is_read_whole() {
            let (listener, path) = listen("pieces");
            let (states, notify) = recording();
            let stop = Arc::new(AtomicBool::new(false));
            spawn_reporter(path.clone(), stop.clone(), "play".into(), None, notify).unwrap();
            let (mut stream, _) = accept(&listener);
            stream
                .write_all(
                    b"{\"event\":\"property-change\",\"id\":8,\"name\":\"user-data/vela-crop/st",
                )
                .unwrap();
            // Longer than any read waits.
            std::thread::sleep(Duration::from_millis(700));
            stream.write_all(b"ate\",\"data\":\"on\"}\n").unwrap();
            assert!(wait_for(|| states
                .lock()
                .unwrap()
                .iter()
                .any(|state| state.crop == Some("on"))));
            stop.store(true, Ordering::SeqCst);
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn a_line_with_bytes_that_are_not_utf8_keeps_its_tracks() {
            // mpv sends a file name as it found it: here a Latin-1 é (0xE9).
            let (listener, path) = listen("latin1");
            let (states, notify) = recording();
            let stop = Arc::new(AtomicBool::new(false));
            spawn_reporter(path.clone(), stop.clone(), "play".into(), None, notify).unwrap();
            let (mut stream, _) = accept(&listener);
            let mut line = br#"{"event":"property-change","id":6,"name":"track-list","data":["#.to_vec();
            line.extend_from_slice(br#"{"id":1,"type":"audio","external":false,"selected":true,"title":"One"},"#);
            line.extend_from_slice(br#"{"id":2,"type":"audio","external":false,"selected":false,"title":"Caf"#);
            line.push(0xE9);
            line.extend_from_slice(br#""},"#);
            line.extend_from_slice(br#"{"id":1,"type":"sub","external":true,"selected":true,"title":"subs-"#);
            line.push(0xE9);
            line.extend_from_slice(b".srt\"}]}\n");
            stream.write_all(&line).unwrap();
            assert!(
                wait_for(|| states
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|state| state.audio_tracks.len() == 2)),
                "the tracks survive a byte that is not UTF-8"
            );
            let last = states.lock().unwrap().last().unwrap().clone();
            assert_eq!(last.audio_tracks[1].title.as_deref(), Some("Caf\u{FFFD}"));
            assert_eq!(last.subtitle_tracks.len(), 1);
            stop.store(true, Ordering::SeqCst);
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn a_message_past_the_cap_does_not_end_the_reporter() {
            // Any script can make mpv send every client a message of any size.
            let (listener, path) = listen("huge");
            let (states, notify) = recording();
            let stop = Arc::new(AtomicBool::new(false));
            spawn_reporter(path.clone(), stop.clone(), "play".into(), None, notify).unwrap();
            let (mut stream, _) = accept(&listener);
            let mut huge = br#"{"event":"client-message","args":["x",""#.to_vec();
            huge.extend(std::iter::repeat_n(b'a', 1_500_000));
            huge.extend_from_slice(b"\"]}\n");
            stream.write_all(&huge).unwrap();
            stream
                .write_all(b"{\"event\":\"property-change\",\"id\":1,\"name\":\"time-pos\",\"data\":7.5}\n")
                .unwrap();
            assert!(
                wait_for(|| states
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|state| state.position == Some(7.5))),
                "the reporter reads on after the long line"
            );
            stop.store(true, Ordering::SeqCst);
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn a_stopped_play_ends_a_reporter_waiting_on_a_full_queue_of_connections() {
            use std::os::fd::AsRawFd;
            // A player that takes no connection, with room for one in its
            // queue (listening again sets the queue's length): one connect
            // fills it, and the reporter's has to wait for room.
            let (listener, path) = listen("full-queue");
            // SAFETY: the listener's own socket, still open.
            assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
            let _queued = UnixStream::connect(&path).expect("the one place in the queue");
            let (states, notify) = recording();
            let stop = Arc::new(AtomicBool::new(false));
            spawn_reporter(
                path.clone(),
                stop.clone(),
                "play".into(),
                None,
                notify.clone(),
            )
            .unwrap();
            std::thread::sleep(Duration::from_millis(500));
            stop.store(true, Ordering::SeqCst);
            assert!(
                wait_for(|| Arc::strong_count(&notify) == 1),
                "the reporter heard the stop and let go of its notifier"
            );
            assert!(states.lock().unwrap().is_empty());
            drop(listener);
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn the_reporter_ends_when_the_player_does() {
            let (listener, path) = listen("ends");
            let (_states, notify) = recording();
            let stop = Arc::new(AtomicBool::new(false));
            spawn_reporter(
                path.clone(),
                stop.clone(),
                "play".into(),
                None,
                notify.clone(),
            )
            .unwrap();
            let (stream, _) = accept(&listener);
            drop(stream);
            assert!(
                wait_for(|| Arc::strong_count(&notify) == 1),
                "the thread let go of its notifier"
            );
            let _ = std::fs::remove_file(&path);
        }
    }
}
