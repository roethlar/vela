#![allow(dead_code)] // Provider version candidates consume this in Slice 2.

use crate::display::HdrState;
use crate::locality::EndpointLocality;
use serde::Serialize;
use std::cmp::Ordering;

/// Persisted duplicate-copy selection policy. Config keeps the raw value as a
/// tolerant string; this closed type is the only value the rest of the app sees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PlaybackSourcePolicy {
    #[default]
    Best,
    Compatible,
    Fastest,
    Ask,
}

impl PlaybackSourcePolicy {
    pub(crate) fn normalize(value: Option<&str>) -> Self {
        match value {
            Some("compatible") => Self::Compatible,
            Some("fastest") => Self::Fastest,
            Some("ask") => Self::Ask,
            _ => Self::Best,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Compatible => "compatible",
            Self::Fastest => "fastest",
            Self::Ask => "ask",
        }
    }
}

/// Provider-neutral facts needed to rank one exact media version. Providers
/// populate this in Slice 2; keeping the selector pure makes the settled order
/// independently guardable now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaybackCandidate {
    pub source_id: String,
    pub version_id: String,
    pub width: u32,
    pub height: u32,
    pub hdr: bool,
    pub bitrate: u64,
    /// Lower is better. Direct play/direct stream eligibility is decided before
    /// quality so an unsupported version can never win on pixels alone.
    pub direct_play_rank: u8,
    pub locality: EndpointLocality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompatibilityTarget {
    pub width: u32,
    pub height: u32,
    pub hdr: HdrState,
}

fn stable_tie(left: &PlaybackCandidate, right: &PlaybackCandidate) -> Ordering {
    left.source_id
        .cmp(&right.source_id)
        .then_with(|| left.version_id.cmp(&right.version_id))
}

fn best_quality(left: &PlaybackCandidate, right: &PlaybackCandidate) -> Ordering {
    right
        .height
        .cmp(&left.height)
        .then_with(|| right.width.cmp(&left.width))
        .then_with(|| right.hdr.cmp(&left.hdr))
        .then_with(|| right.bitrate.cmp(&left.bitrate))
        .then_with(|| stable_tie(left, right))
}

fn hdr_matches(candidate: &PlaybackCandidate, target: CompatibilityTarget) -> bool {
    match target.hdr {
        HdrState::Enabled => candidate.hdr,
        // Unknown deliberately follows the SDR-safe path.
        HdrState::Disabled | HdrState::Unknown => !candidate.hdr,
    }
}

fn fits(candidate: &PlaybackCandidate, target: CompatibilityTarget) -> bool {
    candidate.width <= target.width && candidate.height <= target.height
}

fn compatible_quality(
    left: &PlaybackCandidate,
    right: &PlaybackCandidate,
    target: CompatibilityTarget,
) -> Ordering {
    let left_fits = fits(left, target);
    let right_fits = fits(right, target);
    right_fits.cmp(&left_fits).then_with(|| {
        if left_fits && right_fits {
            // Within the playable display envelope, match HDR first and then
            // take the highest available resolution.
            hdr_matches(right, target)
                .cmp(&hdr_matches(left, target))
                .then_with(|| right.height.cmp(&left.height))
                .then_with(|| right.width.cmp(&left.width))
                .then_with(|| right.bitrate.cmp(&left.bitrate))
                .then_with(|| stable_tie(left, right))
        } else if !left_fits && !right_fits {
            // Nothing fits: nearest larger resolution wins; HDR and bitrate
            // break ties inside that resolution tier.
            left.height
                .cmp(&right.height)
                .then_with(|| left.width.cmp(&right.width))
                .then_with(|| hdr_matches(right, target).cmp(&hdr_matches(left, target)))
                .then_with(|| right.bitrate.cmp(&left.bitrate))
                .then_with(|| stable_tie(left, right))
        } else {
            Ordering::Equal
        }
    })
}

/// Deterministically rank exact versions for an automatic policy. Ask uses the
/// Best order inside each source once the user has selected that source.
pub(crate) fn rank_candidates(
    candidates: &mut [PlaybackCandidate],
    policy: PlaybackSourcePolicy,
    target: Option<CompatibilityTarget>,
) {
    candidates.sort_by(|left, right| {
        left.direct_play_rank
            .cmp(&right.direct_play_rank)
            .then_with(|| match policy {
                PlaybackSourcePolicy::Compatible => target
                    .map(|target| compatible_quality(left, right, target))
                    .unwrap_or_else(|| best_quality(left, right)),
                PlaybackSourcePolicy::Fastest => left
                    .locality
                    .cmp(&right.locality)
                    .then_with(|| best_quality(left, right)),
                PlaybackSourcePolicy::Best | PlaybackSourcePolicy::Ask => best_quality(left, right),
            })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        source: &str,
        version: &str,
        width: u32,
        height: u32,
        hdr: bool,
        bitrate: u64,
        locality: EndpointLocality,
    ) -> PlaybackCandidate {
        PlaybackCandidate {
            source_id: source.to_string(),
            version_id: version.to_string(),
            width,
            height,
            hdr,
            bitrate,
            direct_play_rank: 0,
            locality,
        }
    }

    #[test]
    fn policy_normalization_is_closed_and_best_is_fail_safe() {
        assert_eq!(
            PlaybackSourcePolicy::normalize(None),
            PlaybackSourcePolicy::Best
        );
        assert_eq!(
            PlaybackSourcePolicy::normalize(Some("future-mode")),
            PlaybackSourcePolicy::Best
        );
        assert_eq!(
            PlaybackSourcePolicy::normalize(Some("compatible")),
            PlaybackSourcePolicy::Compatible
        );
        assert_eq!(
            PlaybackSourcePolicy::normalize(Some("fastest")),
            PlaybackSourcePolicy::Fastest
        );
        assert_eq!(
            PlaybackSourcePolicy::normalize(Some("ask")),
            PlaybackSourcePolicy::Ask
        );
    }

    #[test]
    fn best_orders_resolution_before_hdr_then_bitrate() {
        let mut values = vec![
            candidate(
                "a",
                "1080-hdr",
                1920,
                1080,
                true,
                40,
                EndpointLocality::Internet,
            ),
            candidate(
                "a",
                "4k-sdr",
                3840,
                2160,
                false,
                20,
                EndpointLocality::Internet,
            ),
            candidate(
                "a",
                "4k-hdr-low",
                3840,
                2160,
                true,
                20,
                EndpointLocality::Internet,
            ),
            candidate(
                "a",
                "4k-hdr-high",
                3840,
                2160,
                true,
                30,
                EndpointLocality::Internet,
            ),
        ];
        rank_candidates(&mut values, PlaybackSourcePolicy::Best, None);
        assert_eq!(
            values
                .iter()
                .map(|item| item.version_id.as_str())
                .collect::<Vec<_>>(),
            ["4k-hdr-high", "4k-hdr-low", "4k-sdr", "1080-hdr"]
        );
    }

    #[test]
    fn compatible_stays_within_resolution_and_matches_hdr_state() {
        let target = CompatibilityTarget {
            width: 1920,
            height: 1080,
            hdr: HdrState::Disabled,
        };
        let mut values = vec![
            candidate(
                "a",
                "4k-sdr",
                3840,
                2160,
                false,
                80,
                EndpointLocality::Internet,
            ),
            candidate(
                "a",
                "1080-hdr",
                1920,
                1080,
                true,
                40,
                EndpointLocality::Internet,
            ),
            candidate(
                "a",
                "1080-sdr",
                1920,
                1080,
                false,
                20,
                EndpointLocality::Internet,
            ),
        ];
        rank_candidates(&mut values, PlaybackSourcePolicy::Compatible, Some(target));
        assert_eq!(values[0].version_id, "1080-sdr");
        assert_eq!(values[1].version_id, "1080-hdr");
        assert_eq!(values[2].version_id, "4k-sdr");
    }

    #[test]
    fn compatible_uses_nearest_larger_version_when_nothing_fits() {
        let target = CompatibilityTarget {
            width: 1280,
            height: 720,
            hdr: HdrState::Unknown,
        };
        let mut values = vec![
            candidate("a", "4k", 3840, 2160, false, 80, EndpointLocality::Internet),
            candidate(
                "a",
                "1080",
                1920,
                1080,
                false,
                20,
                EndpointLocality::Internet,
            ),
        ];
        rank_candidates(&mut values, PlaybackSourcePolicy::Compatible, Some(target));
        assert_eq!(values[0].version_id, "1080");
    }

    #[test]
    fn fastest_enforces_host_then_lan_then_internet_before_quality() {
        let mut values = vec![
            candidate(
                "remote",
                "8k",
                7680,
                4320,
                true,
                100,
                EndpointLocality::Internet,
            ),
            candidate("lan", "4k", 3840, 2160, true, 50, EndpointLocality::Lan),
            candidate(
                "host",
                "720",
                1280,
                720,
                false,
                5,
                EndpointLocality::SameMachine,
            ),
        ];
        rank_candidates(&mut values, PlaybackSourcePolicy::Fastest, None);
        assert_eq!(
            values
                .iter()
                .map(|item| item.source_id.as_str())
                .collect::<Vec<_>>(),
            ["host", "lan", "remote"]
        );
    }

    #[test]
    fn exact_ties_use_source_then_version_ids() {
        let mut values = vec![
            candidate("b", "1", 1920, 1080, false, 20, EndpointLocality::Internet),
            candidate("a", "2", 1920, 1080, false, 20, EndpointLocality::Internet),
            candidate("a", "1", 1920, 1080, false, 20, EndpointLocality::Internet),
        ];
        rank_candidates(&mut values, PlaybackSourcePolicy::Best, None);
        assert_eq!(
            values
                .iter()
                .map(|item| (item.source_id.as_str(), item.version_id.as_str()))
                .collect::<Vec<_>>(),
            [("a", "1"), ("a", "2"), ("b", "1")]
        );
    }
}
