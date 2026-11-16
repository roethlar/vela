//! Which player runs a play.
//!
//! The user's saved choice, whether the embedded player can run right now, and
//! what happens when it cannot (decisions D1 and D3 of
//! `.agents/plans/embedded-player.md`): the external player is the default on
//! every desktop OS; an embedded choice that cannot be honoured plays in the
//! external player and says why in one line; no play is ever refused over it.

/// A player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayerKind {
    /// mpv as a separate program in its own window.
    External,
    /// mpv's library drawing inside Vela's window.
    Embedded,
}

impl PlayerKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::External => "external",
            Self::Embedded => "embedded",
        }
    }

    /// The player the user's saved `player` setting asks for. Missing means the
    /// external player. Settings validation has already rejected every value
    /// but the two known ones, so anything else here is treated as missing.
    pub(crate) fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("embedded") => PlayerKind::Embedded,
            _ => PlayerKind::External,
        }
    }
}

/// Why the embedded player cannot run a play right now.
///
/// Platform-specific reasons exist only where that platform can report them.
/// Tests keep all reasons to check their notices and fallback rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmbeddedUnavailable {
    /// Vela has no embedded player for this operating system yet. (On
    /// Linux and Windows, which have one, nothing reports this.)
    #[cfg(any(test, not(any(target_os = "linux", target_os = "windows"))))]
    NotOnThisSystem,
    /// It was chosen after this run of Vela started. How the window is
    /// created is decided at start-up, so the choice takes effect next time.
    #[cfg(any(test, target_os = "linux", target_os = "windows"))]
    NeedsRestart,
    /// The previous start in embedded window mode did not get as far as
    /// showing the page, so this one started the ordinary way.
    #[cfg(any(test, target_os = "linux", target_os = "windows"))]
    FailedLastStart,
    /// The session is X11, or GTK was told to use X11.
    #[cfg(any(test, target_os = "linux"))]
    NotWayland,
    /// The NVIDIA proprietary driver is in use; untested there.
    #[cfg(any(test, target_os = "linux"))]
    NvidiaDriver,
    /// The user's environment turns off the WebKit renderer it depends on.
    #[cfg(any(test, target_os = "linux"))]
    RendererOff,
    /// The mpv library Vela ships could not be loaded.
    #[cfg(any(test, target_os = "linux", target_os = "windows"))]
    LibraryMissing,
    /// The compositor lacks a protocol the video surface needs.
    #[cfg(any(test, target_os = "linux"))]
    CompositorLacks,
    /// The GPU offers no OpenGL window format the video can use.
    #[cfg(any(test, target_os = "linux"))]
    NoGpuFormat,
    /// Vela's window has no native parent to attach the video to.
    #[cfg(any(test, target_os = "linux", target_os = "windows"))]
    WindowNotReady,
    /// An embedded play failed to start earlier in this run.
    StartFailed,
}

impl EmbeddedUnavailable {
    /// One line for the player's on-screen display. Short on purpose: mpv's
    /// OSD is large, and this explains a fallback rather than announcing one.
    /// The detail behind a reason (a loader error, a missing protocol's name)
    /// goes to Vela's log, not here.
    pub(crate) fn notice(&self) -> String {
        let why = match self {
            #[cfg(any(test, not(any(target_os = "linux", target_os = "windows"))))]
            EmbeddedUnavailable::NotOnThisSystem => "not on this system yet",
            #[cfg(any(test, target_os = "linux", target_os = "windows"))]
            EmbeddedUnavailable::NeedsRestart => "restart Vela to use it",
            #[cfg(any(test, target_os = "linux", target_os = "windows"))]
            EmbeddedUnavailable::FailedLastStart => "off after a failed start",
            #[cfg(any(test, target_os = "linux"))]
            EmbeddedUnavailable::NotWayland => "needs a Wayland session",
            #[cfg(any(test, target_os = "linux"))]
            EmbeddedUnavailable::NvidiaDriver => "not on NVIDIA drivers yet",
            #[cfg(any(test, target_os = "linux"))]
            EmbeddedUnavailable::RendererOff => "WebKit GPU drawing is off",
            #[cfg(any(test, target_os = "linux", target_os = "windows"))]
            EmbeddedUnavailable::LibraryMissing => "its mpv library did not load",
            #[cfg(any(test, target_os = "linux"))]
            EmbeddedUnavailable::CompositorLacks => "compositor lacks a protocol",
            #[cfg(any(test, target_os = "linux"))]
            EmbeddedUnavailable::NoGpuFormat => "no usable OpenGL format",
            #[cfg(any(test, target_os = "linux", target_os = "windows"))]
            EmbeddedUnavailable::WindowNotReady => "window not ready",
            EmbeddedUnavailable::StartFailed => "could not start",
        };
        format!("Embedded player: {why}")
    }

    /// A full sentence for Settings, using the names shown in its select.
    /// No diagnostic detail from the machine belongs in this text.
    pub(crate) fn settings_note(&self) -> &'static str {
        match self {
            #[cfg(any(test, not(any(target_os = "linux", target_os = "windows"))))]
            Self::NotOnThisSystem => {
                #[cfg(target_os = "linux")]
                { "Videos play in mpv's own window because video in Vela's window is only available on Linux." }
                #[cfg(not(target_os = "linux"))]
                { "Videos play in mpv's own window because video in Vela's window is not available on this system yet." }
            },
            #[cfg(any(test, target_os = "linux", target_os = "windows"))]
            Self::NeedsRestart => "Videos play in mpv's own window until you restart Vela to use Vela's window.",
            #[cfg(any(test, target_os = "linux", target_os = "windows"))]
            Self::FailedLastStart => "Videos play in mpv's own window because Vela did not finish starting with video in its window last time; this run started without it, and the next start tries again.",
            #[cfg(any(test, target_os = "linux"))]
            Self::NotWayland => "Videos play in mpv's own window because video in Vela's window requires a Wayland session.",
            #[cfg(any(test, target_os = "linux"))]
            Self::NvidiaDriver => "Videos play in mpv's own window because video in Vela's window does not support the NVIDIA proprietary driver yet.",
            #[cfg(any(test, target_os = "linux"))]
            Self::RendererOff => "Videos play in mpv's own window because your environment disables the renderer needed for video in Vela's window.",
            #[cfg(any(test, target_os = "linux", target_os = "windows"))]
            Self::LibraryMissing => "Videos play in mpv's own window because the library needed for video in Vela's window could not be loaded.",
            #[cfg(any(test, target_os = "linux"))]
            Self::CompositorLacks => "Videos play in mpv's own window because this desktop cannot provide the video surface Vela's window needs.",
            #[cfg(any(test, target_os = "linux"))]
            Self::NoGpuFormat => "Videos play in mpv's own window because this graphics driver cannot draw video in Vela's window.",
            #[cfg(any(test, target_os = "linux", target_os = "windows"))]
            Self::WindowNotReady => "Videos play in mpv's own window because Vela's window is not ready to show video.",
            Self::StartFailed => "Videos play in mpv's own window because a video failed to start in Vela's window earlier in this run; restarting Vela tries again.",
        }
    }
}

/// Settings explains the saved choice, this run, and what a restart could
/// fix. A library that is absent can be explained more precisely than one
/// that exists but failed to load; an already loaded library stays usable.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn settings_note(
    saved: PlayerKind,
    started: &super::embedded::startup::Decision,
    available: Result<(), EmbeddedUnavailable>,
    known: Result<(), EmbeddedUnavailable>,
) -> Option<&'static str> {
    use super::embedded::startup::WindowMode;
    let reason = if saved == PlayerKind::External {
        known.err()?
    } else if started.mode == WindowMode::Embedded {
        match available {
            Ok(()) => return Some("Videos play in Vela's window."),
            Err(reason) => reason,
        }
    } else {
        started.refused.or(known.err()).unwrap_or(EmbeddedUnavailable::NeedsRestart)
    };
    if reason == EmbeddedUnavailable::LibraryMissing && known == Err(reason) {
        Some("Videos play in mpv's own window because this copy of Vela does not include the library needed for video in Vela's window.")
    } else {
        Some(reason.settings_note())
    }
}

/// The player one play will use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlayerChoice {
    pub(crate) kind: PlayerKind,
    /// Set when the embedded player was wanted and cannot be used: the play
    /// goes to the external player, and this is why.
    pub(crate) fallback: Option<EmbeddedUnavailable>,
}

/// Choose the player for one play from what was wanted and whether the
/// embedded player can run. The external player is always usable from here:
/// whether mpv itself is installed is found out when it is started.
pub(crate) fn choose(
    wanted: PlayerKind,
    embedded: &Result<(), EmbeddedUnavailable>,
) -> PlayerChoice {
    match (wanted, embedded) {
        (PlayerKind::External, _) => PlayerChoice {
            kind: PlayerKind::External,
            fallback: None,
        },
        (PlayerKind::Embedded, Ok(())) => PlayerChoice {
            kind: PlayerKind::Embedded,
            fallback: None,
        },
        (PlayerKind::Embedded, Err(reason)) => PlayerChoice {
            kind: PlayerKind::External,
            fallback: Some(*reason),
        },
    }
}

/// Which fallback the user has already been told about in this run of Vela,
/// so the same line is not put on screen at the start of every play.
#[derive(Debug, Default)]
pub(crate) struct FallbackNotices {
    told: Option<EmbeddedUnavailable>,
}

/// What one play does about the explanation of a fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoticePlan {
    /// The reason to explain on this play's display, when there is one and
    /// the user has not been told it yet.
    pub(crate) explain: Option<EmbeddedUnavailable>,
    /// This play has nothing to fall back from. Once it has started, what was
    /// explained before is forgotten, so that a later fallback is explained
    /// again rather than silently resumed.
    pub(crate) forget_when_started: bool,
}

impl FallbackNotices {
    /// Decide what one play does about the explanation. Changes nothing.
    ///
    /// `chooses_player` is true for a play that picks its player from the
    /// user's setting, which is every play the user starts. It is false for
    /// an automatic relaunch, which continues a running play in the player it
    /// already has: that play was explained, or not, when it started, and its
    /// relaunch neither explains anything nor changes what the user has been
    /// told. (A relaunch is handed its player, so it has no fallback of its
    /// own to report; taking that for "nothing to fall back from" would
    /// forget the explanation and repeat it on the next play.)
    pub(crate) fn plan(
        &self,
        chooses_player: bool,
        fallback: Option<&EmbeddedUnavailable>,
    ) -> NoticePlan {
        if !chooses_player {
            return NoticePlan {
                explain: None,
                forget_when_started: false,
            };
        }
        match fallback {
            None => NoticePlan {
                explain: None,
                forget_when_started: true,
            },
            Some(reason) => NoticePlan {
                explain: (self.told.as_ref() != Some(reason)).then_some(*reason),
                forget_when_started: false,
            },
        }
    }

    /// The user has been told this reason.
    pub(crate) fn explained(&mut self, reason: &EmbeddedUnavailable) {
        self.told = Some(*reason);
    }

    /// See [`NoticePlan::forget_when_started`].
    pub(crate) fn forget(&mut self) {
        self.told = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn video_player_settings_note_explains_the_saved_choice_and_this_run() {
        use super::super::embedded::startup::{Decision, WindowMode};
        use EmbeddedUnavailable::*;
        use PlayerKind::*;
        let today = Decision { mode: WindowMode::Today, refused: None };
        let active = Decision { mode: WindowMode::Embedded, refused: None };
        let recovered = Decision { mode: WindowMode::Today, refused: Some(FailedLastStart) };
        let missing = "Videos play in mpv's own window because this copy of Vela does not include the library needed for video in Vela's window.";
        let restarting = "Videos play in mpv's own window until you restart Vela to use Vela's window.";
        let failed_start = "Videos play in mpv's own window because Vela did not finish starting with video in its window last time; this run started without it, and the next start tries again.";
        let mut cases = vec![
            (External, today.clone(), Err(NeedsRestart), Ok(()), None),
            (External, recovered.clone(), Err(FailedLastStart), Ok(()), None),
            (External, active.clone(), Err(StartFailed), Ok(()), None),
            (External, today.clone(), Err(NeedsRestart), Err(LibraryMissing), Some(missing)),
            (Embedded, active.clone(), Ok(()), Ok(()), Some("Videos play in Vela's window.")),
            // Removing the file cannot unload a library already in use.
            (Embedded, active.clone(), Ok(()), Err(LibraryMissing), Some("Videos play in Vela's window.")),
            (Embedded, today.clone(), Err(NeedsRestart), Ok(()), Some(restarting)),
            (Embedded, today.clone(), Err(NeedsRestart), Err(LibraryMissing), Some(missing)),
            (Embedded, active.clone(), Err(LibraryMissing), Err(LibraryMissing), Some(missing)),
            // Recovery skips this run only, even when another problem is known.
            (Embedded, recovered, Err(FailedLastStart), Err(LibraryMissing), Some(failed_start)),
        ];
        for (reason, expected) in [
            (NotOnThisSystem, "Videos play in mpv's own window because video in Vela's window is only available on Linux."),
            (NeedsRestart, restarting),
            (FailedLastStart, failed_start),
            (NotWayland, "Videos play in mpv's own window because video in Vela's window requires a Wayland session."),
            (NvidiaDriver, "Videos play in mpv's own window because video in Vela's window does not support the NVIDIA proprietary driver yet."),
            (RendererOff, "Videos play in mpv's own window because your environment disables the renderer needed for video in Vela's window."),
            (LibraryMissing, "Videos play in mpv's own window because the library needed for video in Vela's window could not be loaded."),
            (CompositorLacks, "Videos play in mpv's own window because this desktop cannot provide the video surface Vela's window needs."),
            (NoGpuFormat, "Videos play in mpv's own window because this graphics driver cannot draw video in Vela's window."),
            (WindowNotReady, "Videos play in mpv's own window because Vela's window is not ready to show video."),
            (StartFailed, "Videos play in mpv's own window because a video failed to start in Vela's window earlier in this run; restarting Vela tries again."),
        ] {
            cases.push((Embedded, active.clone(), Err(reason), Ok(()), Some(expected)));
            if matches!(reason, NotWayland | NvidiaDriver | RendererOff) {
                cases.push((External, today.clone(), Err(NeedsRestart), Err(reason), Some(expected)));
                cases.push((Embedded, today.clone(), Err(NeedsRestart), Err(reason), Some(expected)));
                cases.push((Embedded, Decision { mode: WindowMode::Today, refused: Some(reason) }, Err(reason), Ok(()), Some(expected)));
            }
        }
        for (saved, started, available, known, expected) in cases {
            assert_eq!(
                settings_note(saved, &started, available, known),
                expected,
                "{saved:?}, {started:?}, available={available:?}, known={known:?}"
            );
        }
    }

    #[test]
    fn a_missing_or_external_setting_asks_for_the_external_player() {
        assert_eq!(PlayerKind::from_setting(None), PlayerKind::External);
        assert_eq!(
            PlayerKind::from_setting(Some("external")),
            PlayerKind::External
        );
        assert_eq!(
            PlayerKind::from_setting(Some("embedded")),
            PlayerKind::Embedded
        );
        // Validation rejects these before a play can see them; if one arrived
        // anyway it must not select the experimental player.
        for other in ["", "Embedded", "embedded ", "internal"] {
            assert_eq!(PlayerKind::from_setting(Some(other)), PlayerKind::External);
        }
    }

    // Every combination of what was wanted and whether embedded can run.
    #[test]
    fn the_external_player_is_chosen_unless_embedded_was_wanted_and_can_run() {
        let unavailable = Err(EmbeddedUnavailable::NeedsRestart);
        assert_eq!(
            choose(PlayerKind::External, &Ok(())),
            PlayerChoice {
                kind: PlayerKind::External,
                fallback: None
            },
            "external stays external even where embedded could run"
        );
        assert_eq!(
            choose(PlayerKind::External, &unavailable),
            PlayerChoice {
                kind: PlayerKind::External,
                fallback: None
            },
            "an external play has nothing to fall back from, so nothing to explain"
        );
        assert_eq!(
            choose(PlayerKind::Embedded, &Ok(())),
            PlayerChoice {
                kind: PlayerKind::Embedded,
                fallback: None
            }
        );
        assert_eq!(
            choose(PlayerKind::Embedded, &unavailable),
            PlayerChoice {
                kind: PlayerKind::External,
                fallback: Some(EmbeddedUnavailable::NeedsRestart)
            },
            "an embedded choice that cannot run still plays, externally, and says why"
        );
    }

    /// Every reason there is. The `match` does not compile until a new
    /// variant is added to it, and that is the reminder to add it to the list,
    /// which is what puts it under the notice test below.
    fn every_reason() -> Vec<EmbeddedUnavailable> {
        use EmbeddedUnavailable::*;
        let all = vec![
            NotOnThisSystem,
            NeedsRestart,
            FailedLastStart,
            NotWayland,
            NvidiaDriver,
            RendererOff,
            LibraryMissing,
            CompositorLacks,
            NoGpuFormat,
            WindowNotReady,
            StartFailed,
        ];
        for reason in &all {
            match reason {
                NotOnThisSystem | NeedsRestart | FailedLastStart | NotWayland | NvidiaDriver
                | RendererOff | LibraryMissing | CompositorLacks | NoGpuFormat | WindowNotReady
                | StartFailed => {}
            }
        }
        all
    }

    #[test]
    fn the_fallback_notice_stays_short_and_says_which_player() {
        for reason in every_reason() {
            let notice = reason.notice();
            assert!(
                notice.chars().count() <= 48,
                "{notice:?} is too long for an OSD line"
            );
            assert!(
                notice.starts_with("Embedded player: "),
                "{notice:?} must say what it is about"
            );
            assert!(!notice.contains('\n'));
        }
        // Two reasons with one line would be told once between them.
        let mut lines: Vec<String> = every_reason().iter().map(|r| r.notice()).collect();
        lines.sort();
        lines.dedup();
        assert_eq!(lines.len(), every_reason().len(), "every reason has its own line");
    }

    #[test]
    fn a_different_reason_is_explained_even_after_another_was() {
        let mut notices = FallbackNotices::default();
        notices.explained(&EmbeddedUnavailable::NeedsRestart);
        assert_eq!(
            notices.plan(true, Some(&EmbeddedUnavailable::StartFailed)),
            explain(&EmbeddedUnavailable::StartFailed)
        );
    }

    fn explain(reason: &EmbeddedUnavailable) -> NoticePlan {
        NoticePlan {
            explain: Some(*reason),
            forget_when_started: false,
        }
    }

    const SAY_NOTHING: NoticePlan = NoticePlan {
        explain: None,
        forget_when_started: false,
    };

    #[test]
    fn a_fallback_is_explained_once_until_it_goes_away() {
        let reason = EmbeddedUnavailable::NeedsRestart;
        let mut notices = FallbackNotices::default();
        assert_eq!(notices.plan(true, Some(&reason)), explain(&reason));
        // Asking is not telling: a play that never showed the line leaves it
        // still to be shown.
        assert_eq!(notices.plan(true, Some(&reason)), explain(&reason));

        notices.explained(&reason);
        assert_eq!(
            notices.plan(true, Some(&reason)),
            SAY_NOTHING,
            "the same reason is not repeated on the next play"
        );

        // A play with nothing to fall back from says nothing and, once it has
        // started, clears what was told.
        let nothing_to_explain = notices.plan(true, None);
        assert_eq!(
            nothing_to_explain,
            NoticePlan {
                explain: None,
                forget_when_started: true
            }
        );
        assert_eq!(
            notices.plan(true, Some(&reason)),
            SAY_NOTHING,
            "deciding changes nothing; only a started play forgets"
        );
        notices.forget();
        assert_eq!(notices.plan(true, Some(&reason)), explain(&reason));
    }

    // Review finding sel-2: fallback play, Automatic step-down, next play.
    #[test]
    fn an_automatic_relaunch_neither_explains_nor_forgets() {
        let reason = EmbeddedUnavailable::NeedsRestart;
        let mut notices = FallbackNotices::default();
        notices.explained(&reason);

        // The relaunch was handed the external player, so its own choice
        // reports no fallback. That must not read as "nothing to fall back
        // from".
        assert_eq!(notices.plan(false, None), SAY_NOTHING);
        // A relaunch never carries the explanation either, told or not: it
        // carries the notice that says why it happened.
        assert_eq!(notices.plan(false, Some(&reason)), SAY_NOTHING);
        assert_eq!(
            FallbackNotices::default().plan(false, Some(&reason)),
            SAY_NOTHING
        );

        assert_eq!(
            notices.plan(true, Some(&reason)),
            SAY_NOTHING,
            "the play after a step-down is not told again"
        );
    }
}
