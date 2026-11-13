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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EmbeddedUnavailable {
    /// This build of Vela does not contain the embedded player.
    NotBuilt,
}

impl EmbeddedUnavailable {
    /// One line for the player's on-screen display. Short on purpose: mpv's
    /// OSD is large, and this explains a fallback rather than announcing one.
    pub(crate) fn notice(&self) -> String {
        match self {
            EmbeddedUnavailable::NotBuilt => "Embedded player: not in this build".to_string(),
        }
    }
}

/// Whether the embedded player can run a play right now.
///
/// It cannot yet: the launcher arrives with Slice 3 of the plan. Until then
/// every play goes to the external player, whatever the setting says.
pub(crate) fn embedded_availability() -> Result<(), EmbeddedUnavailable> {
    Err(EmbeddedUnavailable::NotBuilt)
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
            fallback: Some(reason.clone()),
        },
    }
}

/// Which fallback the user has already been told about in this run of Vela,
/// so the same line is not put on screen at the start of every play.
#[derive(Debug, Default)]
pub(crate) struct FallbackNotices {
    told: Option<EmbeddedUnavailable>,
}

impl FallbackNotices {
    /// The line to show for a play that fell back, or `None` when there is no
    /// fallback or the user has already been told this reason. Changes
    /// nothing: call [`FallbackNotices::record`] once the play has started.
    pub(crate) fn due(&self, fallback: Option<&EmbeddedUnavailable>) -> Option<String> {
        match fallback {
            Some(reason) if self.told.as_ref() != Some(reason) => Some(reason.notice()),
            _ => None,
        }
    }

    /// Remember what a started play was (or was not) falling back from. A
    /// reason is announced again only after it changed or went away.
    pub(crate) fn record(&mut self, fallback: Option<&EmbeddedUnavailable>) {
        self.told = fallback.cloned();
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
        let unavailable = Err(EmbeddedUnavailable::NotBuilt);
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
                fallback: Some(EmbeddedUnavailable::NotBuilt)
            },
            "an embedded choice that cannot run still plays, externally, and says why"
        );
    }

    #[test]
    fn this_build_cannot_run_the_embedded_player() {
        // `playback::play` has no embedded launcher yet and refuses the kind
        // outright. That refusal is unreachable only while this stays true.
        assert_eq!(embedded_availability(), Err(EmbeddedUnavailable::NotBuilt));
    }

    /// Every reason there is. A new variant must be added here, which is what
    /// puts it under the notice test below.
    fn every_reason() -> Vec<EmbeddedUnavailable> {
        vec![EmbeddedUnavailable::NotBuilt]
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
    }

    #[test]
    fn a_fallback_is_announced_once_until_it_changes_or_goes_away() {
        let reason = EmbeddedUnavailable::NotBuilt;
        let mut notices = FallbackNotices::default();
        assert_eq!(notices.due(None), None, "no fallback, nothing to say");
        assert_eq!(notices.due(Some(&reason)), Some(reason.notice()));
        // Asking does not count as telling: a play that failed to start has
        // shown the user nothing.
        assert_eq!(notices.due(Some(&reason)), Some(reason.notice()));

        notices.record(Some(&reason));
        assert_eq!(
            notices.due(Some(&reason)),
            None,
            "the same reason is not repeated on the next play"
        );

        // A play with no fallback clears it, so a later fallback is explained
        // again rather than silently resumed.
        notices.record(None);
        assert_eq!(notices.due(Some(&reason)), Some(reason.notice()));
    }
}
