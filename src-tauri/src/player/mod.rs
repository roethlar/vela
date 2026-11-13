//! Players.
//!
//! One description of a launch ([`launch::LaunchPlan`]) and, per player,
//! something that starts it and a handle to what it started. Today there is
//! one player: mpv as a separate program ([`external`]). The trackers in
//! `crate::playback` reach whichever player is running through mpv's JSON
//! IPC socket and do not care which one it is. [`select`] decides which
//! player a play uses.

pub(crate) mod external;
pub(crate) mod launch;
mod select;

pub(crate) use select::{choose, embedded_availability, FallbackNotices, PlayerKind};

/// A started player, as the rest of the app holds it: the current player, and
/// replaced players waiting to be reaped.
pub(crate) enum PlayerHandle {
    /// mpv running as a child process.
    External(external::ManagedChild),
}

impl PlayerHandle {
    /// Ask the player to stop now. Non-blocking.
    pub(crate) fn kill(&mut self) -> std::io::Result<()> {
        match self {
            PlayerHandle::External(child) => child.kill(),
        }
    }

    /// Non-blocking: has the player ended? `Ok(true)` only once its end is
    /// confirmed; a player that is still running is `Ok(false)`.
    pub(crate) fn has_ended(&mut self) -> std::io::Result<bool> {
        match self {
            PlayerHandle::External(child) => child.try_wait().map(|status| status.is_some()),
        }
    }

    /// A player that has been running has already consumed its startup
    /// configuration. Remove the credential include it was given before a
    /// replacement launch writes another one.
    pub(crate) fn remove_consumed_header_include(&mut self) -> Result<(), String> {
        match self {
            PlayerHandle::External(child) => child.remove_consumed_header_include(),
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
