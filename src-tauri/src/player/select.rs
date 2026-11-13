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
/// Every reason but the first can only arise on Linux, the one system that
/// has an embedded player so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) enum EmbeddedUnavailable {
    /// Vela has no embedded player for this operating system yet. (On
    /// Linux, which has one, nothing reports this.)
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    NotOnThisSystem,
    /// It was chosen after this run of Vela started. How the window is
    /// created is decided at start-up, so the choice takes effect next time.
    NeedsRestart,
    /// The previous start in embedded window mode did not get as far as
    /// showing the page, so this one started the ordinary way.
    FailedLastStart,
    /// The session is X11, or GTK was told to use X11.
    NotWayland,
    /// The NVIDIA proprietary driver is in use; untested there.
    NvidiaDriver,
    /// The user's environment turns off the WebKit renderer it depends on.
    RendererOff,
    /// The mpv library Vela ships could not be loaded.
    LibraryMissing,
    /// The compositor lacks a protocol the video surface needs.
    CompositorLacks,
    /// The GPU offers no OpenGL window format the video can use.
    NoGpuFormat,
    /// Vela's window has no Wayland surface to attach the video to.
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
            EmbeddedUnavailable::NotOnThisSystem => "Linux only for now",
            EmbeddedUnavailable::NeedsRestart => "restart Vela to use it",
            EmbeddedUnavailable::FailedLastStart => "off after a failed start",
            EmbeddedUnavailable::NotWayland => "needs a Wayland session",
            EmbeddedUnavailable::NvidiaDriver => "not on NVIDIA drivers yet",
            EmbeddedUnavailable::RendererOff => "WebKit GPU drawing is off",
            EmbeddedUnavailable::LibraryMissing => "its mpv library did not load",
            EmbeddedUnavailable::CompositorLacks => "compositor lacks a protocol",
            EmbeddedUnavailable::NoGpuFormat => "no usable OpenGL format",
            EmbeddedUnavailable::WindowNotReady => "window not ready",
            EmbeddedUnavailable::StartFailed => "could not start",
        };
        format!("Embedded player: {why}")
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
