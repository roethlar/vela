//! Players.
//!
//! One description of a launch ([`launch::LaunchPlan`]) and, per player,
//! something that starts it and a handle to what it started. There are two
//! players: mpv as a separate program ([`external`]), on every system and
//! the default, and mpv's library drawing inside Vela's own window
//! (`embedded`), on Linux and experimental. The trackers in
//! `crate::playback` reach whichever player is running through mpv's JSON
//! IPC socket and do not care which one it is. [`select`] decides which
//! player a play uses.

pub(crate) mod control;
#[cfg(target_os = "linux")]
pub(crate) mod embedded;
pub(crate) mod external;
pub(crate) mod launch;
pub(crate) mod report;
mod select;

pub(crate) use select::{choose, EmbeddedUnavailable, FallbackNotices, PlayerChoice, PlayerKind};

/// A started player, as the rest of the app holds it: the current player, and
/// replaced players waiting to be reaped.
pub(crate) enum PlayerHandle {
    /// mpv running as a child process.
    External(external::ManagedChild),
    /// mpv's library running inside Vela, on a thread of its own.
    #[cfg(target_os = "linux")]
    Embedded(embedded::Session),
}

/// A number for a new player, never handed out before in this run of Vela
/// (`PlayerHandle::serial`).
pub(crate) fn new_serial() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The player view the page has been told is open, kept with the telling
/// (review finding pc-2). `player-opened` and `player-closed` are sent only
/// through [`ViewRecord::announce`], which changes the record and sends the
/// event in one step under the record's lock, by the rule the page applies
/// to those events (`nextPlayerSession` in `src/lib/playerView.ts`): an
/// opening shows its session; a closing hides its session only if that is
/// the one showing. A report that has to name the view the page is showing
/// is sent through [`ViewRecord::report`], under the same lock, so no opening
/// or closing can come between what it reads and what it sends.
#[derive(Default)]
pub(crate) struct ViewRecord {
    showing: std::sync::Mutex<Option<String>>,
}

impl ViewRecord {
    /// Record that the view of `session_id` opened (`opened`) or closed, and
    /// send the event with `emit`, as one step. `emit` must only send. Only
    /// the embedded player has a view, and it runs only on Linux.
    #[cfg(target_os = "linux")]
    pub(crate) fn announce(&self, session_id: &str, opened: bool, emit: impl FnOnce()) {
        let mut showing = self.showing.lock().unwrap_or_else(|e| e.into_inner());
        if opened {
            *showing = Some(session_id.to_string());
        } else if showing.as_deref() == Some(session_id) {
            *showing = None;
        }
        emit();
    }

    /// Send, with `emit`, a report naming the session whose view the page is
    /// showing, or `own` when no view is showing. `emit` must only send.
    pub(crate) fn report(&self, own: &str, emit: impl FnOnce(&str)) {
        let showing = self.showing.lock().unwrap_or_else(|e| e.into_inner());
        emit(showing.as_deref().unwrap_or(own));
    }

    /// Whether the record's lock is held right now, by any thread (this one
    /// included). For tests that check a send happens under it.
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn is_locked(&self) -> bool {
        matches!(
            self.showing.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        )
    }
}

/// The embedded player's side of the app, for the life of the process.
#[cfg(target_os = "linux")]
pub(crate) use embedded::Host as EmbeddedHost;

/// A system with no embedded player has nothing to keep.
#[cfg(not(target_os = "linux"))]
pub(crate) struct EmbeddedHost;

#[cfg(not(target_os = "linux"))]
impl EmbeddedHost {
    pub(crate) fn new() -> Self {
        EmbeddedHost
    }
}

/// Whether an embedded play can be started now.
pub(crate) async fn embedded_availability(
    host: &std::sync::Arc<EmbeddedHost>,
    app: Option<&tauri::AppHandle>,
) -> Result<(), EmbeddedUnavailable> {
    #[cfg(target_os = "linux")]
    {
        let Some(app) = app.cloned() else {
            return Err(EmbeddedUnavailable::WindowNotReady);
        };
        let host = host.clone();
        // The answer comes from the window's own thread, which this must
        // wait for, so it is asked from a thread that may block.
        tauri::async_runtime::spawn_blocking(move || host.availability(&app))
            .await
            .unwrap_or(Err(EmbeddedUnavailable::WindowNotReady))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (host, app);
        Err(EmbeddedUnavailable::NotOnThisSystem)
    }
}

/// What starts embedded plays on this system, when anything does.
pub(crate) fn embedded_launcher(
    host: &std::sync::Arc<EmbeddedHost>,
    app: Option<&tauri::AppHandle>,
) -> Option<std::sync::Arc<dyn EmbeddedLauncher>> {
    #[cfg(target_os = "linux")]
    {
        app.map(|app| {
            std::sync::Arc::new(embedded::Launcher::new(host.clone(), app.clone()))
                as std::sync::Arc<dyn EmbeddedLauncher>
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (host, app);
        None
    }
}

/// Starts embedded plays for `crate::playback::play`. A system with no
/// embedded player has none of these to give it.
pub(crate) trait EmbeddedLauncher: Send + Sync {
    /// Do everything that needs Vela's window, and hand back the step that
    /// starts the player. In two parts because `play` starts a player while
    /// holding the lock on the player slot, and the window's own thread
    /// takes that lock too: asking the window anything under it would
    /// deadlock.
    fn prepare(&self, session_id: &str) -> Result<EmbeddedStart, String>;
}

/// The second half of [`EmbeddedLauncher::prepare`]: start the described
/// launch. Arguments: the launch, whether the user's own mpv configuration
/// is wanted, and the IPC path the launch names.
pub(crate) type EmbeddedStart =
    Box<dyn FnOnce(launch::LaunchPlan, bool, &str) -> Result<PlayerHandle, String> + Send>;

impl PlayerHandle {
    /// Ask the player to stop now. Non-blocking.
    pub(crate) fn kill(&mut self) -> std::io::Result<()> {
        match self {
            PlayerHandle::External(child) => child.kill(),
            #[cfg(target_os = "linux")]
            PlayerHandle::Embedded(session) => {
                session.stop();
                Ok(())
            }
        }
    }

    /// The player view's Close: stop this player if it is the EMBEDDED play
    /// of `session_id`, and say whether it was. The identity compared is the
    /// player's own, so the caller needs nothing but the lock that holds the
    /// player, and a Close left over from an earlier play cannot stop the
    /// play that replaced it however the two are timed (review finding
    /// ec-5). An external player is never stopped this way: it has a window
    /// of its own to close.
    pub(crate) fn close_embedded_play(&mut self, session_id: &str) -> bool {
        match self {
            PlayerHandle::External(_) => {
                let _ = session_id;
                false
            }
            #[cfg(target_os = "linux")]
            PlayerHandle::Embedded(session) => {
                if !session.is_play(session_id) {
                    return false;
                }
                session.stop();
                true
            }
        }
    }

    /// The way to send the player view's commands to this player, if it is
    /// the EMBEDDED play of `session_id`. Asked under the slot's lock, like
    /// [`Self::close_embedded_play`]. An external player has none: the page
    /// never drives mpv's own window.
    pub(crate) fn command_channel(
        &self,
        session_id: &str,
    ) -> Option<std::sync::Arc<control::Channel>> {
        match self {
            PlayerHandle::External(_) => {
                let _ = session_id;
                None
            }
            #[cfg(target_os = "linux")]
            PlayerHandle::Embedded(session) => session
                .is_play(session_id)
                .then(|| session.channel()),
        }
    }

    /// Whether this is the EMBEDDED play of `session_id`, still running: not
    /// told to stop, and not ended. Asked under the slot's lock, for the
    /// player view's fullscreen: a play that has been stopped (replaced, or
    /// stopped by a launch that then failed) may not put the window in
    /// fullscreen any more (review finding pc-2).
    pub(crate) fn is_running_embedded_play(&mut self, session_id: &str) -> bool {
        match self {
            PlayerHandle::External(_) => {
                let _ = session_id;
                false
            }
            #[cfg(target_os = "linux")]
            PlayerHandle::Embedded(session) => {
                session.is_play(session_id) && !session.stop_requested() && !session.has_ended()
            }
        }
    }

    /// Which player this is: a number no other player started in this run
    /// of Vela has. Tells the player a launch found in the slot from any
    /// player put there later.
    pub(crate) fn serial(&self) -> u64 {
        match self {
            PlayerHandle::External(child) => child.serial(),
            #[cfg(target_os = "linux")]
            PlayerHandle::Embedded(session) => session.serial(),
        }
    }

    /// Non-blocking: has the player ended? `Ok(true)` only once its end is
    /// confirmed; a player that is still running is `Ok(false)`.
    pub(crate) fn has_ended(&mut self) -> std::io::Result<bool> {
        match self {
            PlayerHandle::External(child) => child.try_wait().map(|status| status.is_some()),
            #[cfg(target_os = "linux")]
            PlayerHandle::Embedded(session) => Ok(session.has_ended()),
        }
    }

    /// A player that has been running has already consumed its startup
    /// configuration. Remove the credential include it was given before a
    /// replacement launch writes another one.
    pub(crate) fn remove_consumed_header_include(&mut self) -> Result<(), String> {
        match self {
            PlayerHandle::External(child) => child.remove_consumed_header_include(),
            #[cfg(target_os = "linux")]
            PlayerHandle::Embedded(session) => session.remove_consumed_header_include(),
        }
    }
}

/// Whether the reaper keeps a player after asking if it has ended. Only a
/// confirmed end may drop the player, and with it the credential include it
/// owns: a player that is still running stays, and so does one whose state
/// could not be read, so a transient OS error cannot orphan the include.
pub(crate) fn retain_after_poll(ended: &std::io::Result<bool>) -> bool {
    !matches!(ended, Ok(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reaper_drops_a_player_only_on_a_confirmed_end() {
        assert!(retain_after_poll(&Ok(false)));
        assert!(retain_after_poll(&Err(std::io::Error::other(
            "synthetic process-query error"
        ))));
        assert!(!retain_after_poll(&Ok(true)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_close_stops_the_embedded_play_it_names_and_no_other() {
        let stop_requested = |player: &PlayerHandle| match player {
            PlayerHandle::Embedded(session) => session.stop_requested(),
            PlayerHandle::External(_) => unreachable!(),
        };
        // Play B has replaced play A; A's Close arrives late.
        let mut player = PlayerHandle::Embedded(embedded::Session::for_test("play-b"));
        assert!(!player.close_embedded_play("play-a"));
        assert!(!stop_requested(&player), "the successor plays on");
        assert!(!player.close_embedded_play(""));
        assert!(!player.close_embedded_play("play-b "));
        assert!(!stop_requested(&player));
        // B's own Close stops it.
        assert!(player.close_embedded_play("play-b"));
        assert!(stop_requested(&player));
    }

    #[cfg(unix)]
    #[test]
    fn a_close_never_stops_an_external_player() {
        let child = std::process::Command::new("sleep")
            .arg("30")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("sleep is available");
        let mut player = PlayerHandle::External(external::ManagedChild::for_test(child));
        assert!(!player.close_embedded_play("any-session"));
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            matches!(player.has_ended(), Ok(false)),
            "the external player is still running"
        );
        player.kill().expect("the player can be stopped");
    }

    // A real child process that stays alive until it is told to stop.
    #[cfg(unix)]
    #[test]
    fn an_external_player_reports_its_end_only_after_the_process_exits() {
        let child = std::process::Command::new("sleep")
            .arg("30")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("sleep is available");
        let mut player = PlayerHandle::External(external::ManagedChild::for_test(child));
        assert!(
            matches!(player.has_ended(), Ok(false)),
            "a running player has not ended"
        );
        assert!(
            retain_after_poll(&player.has_ended()),
            "a running player is kept"
        );

        player.kill().expect("the player can be stopped");
        let mut ended = false;
        for _ in 0..500 {
            if matches!(player.has_ended(), Ok(true)) {
                ended = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(ended, "a stopped player is reported as ended");
        assert!(
            !retain_after_poll(&player.has_ended()),
            "an ended player is not kept"
        );
    }
}
