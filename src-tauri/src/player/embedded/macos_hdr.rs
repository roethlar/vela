//! macOS colour policy, independent of AppKit, CGL and an mpv instance.
//! The layer technique is described in the approved embedded-player plan;
//! this implementation does not contain IINA code.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Range {
    Sdr,
    Pq2020,
    PqP3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Decision {
    pub(crate) range: Range,
    pub(crate) reason: &'static str,
}

/// EDR iff the video is PQ or HLG with BT.2020 or Display P3 primaries,
/// the screen's finite potential EDR headroom exceeds 1.0, and CGL is float.
/// Current headroom is deliberately not an eligibility test: Apple says it
/// can be 1.0 until an onscreen layer requests EDR. Neither ratio is nits.
pub(crate) fn decide(
    transfer: Option<&str>,
    primaries: Option<&str>,
    potential: f64,
    float: bool,
) -> Decision {
    let sdr = |reason| Decision {
        range: Range::Sdr,
        reason,
    };
    if !matches!(transfer, Some("pq" | "hlg")) {
        return sdr("video transfer is SDR or unknown");
    }
    let range = match primaries {
        Some("bt.2020") => Range::Pq2020,
        Some("display-p3") => Range::PqP3,
        _ => return sdr("video primaries are unsupported or unknown"),
    };
    if !float {
        return sdr("no float surface");
    }
    if !potential.is_finite() || potential <= 1.0 {
        return sdr("screen has no potential EDR headroom");
    }
    Decision {
        range,
        reason: "HDR video, float surface and screen EDR headroom",
    }
}

pub(crate) const TARGET_NAMES: [&str; 5] = [
    "target-trc",
    "target-prim",
    "target-peak",
    "icc-profile-auto",
    "icc-profile",
];

impl Range {
    /// mpv 0.42 options.rst: auto peak follows the selected TRC (PQ's
    /// nominal peak), not an invented conversion of Apple's EDR ratio to
    /// nits. ColorSync maps the PQ layer to the actual display. HLG is
    /// converted to PQ. Keep the user's tone-mapping algorithm: it does
    /// not change the encoding described by the layer, only the mapping
    /// into it. No ICC transform may supersede these explicit targets.
    pub(crate) fn values(self) -> [&'static str; 5] {
        match self {
            Self::Sdr => ["srgb", "bt.709", "203", "no", ""],
            Self::Pq2020 => ["pq", "bt.2020", "auto", "no", ""],
            Self::PqP3 => ["pq", "display-p3", "auto", "no", ""],
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Sdr => "SDR sRGB",
            Self::Pq2020 => "EDR PQ BT.2020",
            Self::PqP3 => "EDR PQ Display P3",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceFormat {
    Float,
    EightBit,
    Software,
}

pub(crate) const SURFACE_FORMATS: [SurfaceFormat; 3] = [
    SurfaceFormat::Float,
    SurfaceFormat::EightBit,
    SurfaceFormat::Software,
];

impl SurfaceFormat {
    pub(crate) fn depth(self) -> i32 {
        if self == Self::Float {
            16
        } else {
            8
        }
    }

    /// CGLTypes.h / objc2-open-gl 0.3.2. Plain values keep the exact lists
    /// testable on Linux; the CGL boundary wraps them in its attribute type.
    pub(crate) fn attributes(self) -> Vec<u32> {
        const PROFILE: u32 = 99;
        const CORE_3_2: u32 = 0x3200;
        const ACCELERATED: u32 = 73;
        const RENDERER_ID: u32 = 70;
        const GENERIC_FLOAT: u32 = 0x00020400;
        const COLOR_SIZE: u32 = 8;
        const COLOR_FLOAT: u32 = 58;
        const DOUBLE_BUFFER: u32 = 5;
        let mut attrs = vec![PROFILE, CORE_3_2];
        if self == Self::Software {
            attrs.extend([RENDERER_ID, GENERIC_FLOAT]);
        } else {
            attrs.push(ACCELERATED);
        }
        if self == Self::Float {
            attrs.extend([COLOR_SIZE, 64, COLOR_FLOAT]);
        }
        // The two fallbacks preserve the host's original attribute lists.
        attrs.extend([DOUBLE_BUFFER, 0]);
        attrs
    }
}

pub(crate) fn choose_surface<T>(
    mut create: impl FnMut(SurfaceFormat) -> Result<T, String>,
) -> Result<(SurfaceFormat, T), String> {
    let mut last = String::new();
    for format in SURFACE_FORMATS {
        match create(format) {
            Ok(surface) => return Ok((format, surface)),
            Err(error) => last = error,
        }
    }
    Err(last)
}

/// All three operations are nonblocking client API calls, never property().
pub(crate) trait TargetAccess {
    fn ask(&self, id: u64, name: &str, value: &str) -> bool;
    fn observe(&self, id: u64, name: &str) -> bool;
    fn unobserve(&self, id: u64);
}

/// Count all profile repairs, including overrides made after a completed
/// switch. Repeated rewrites must not cause endless black/video flashing.
pub(crate) fn allow_restoration(recent: &mut VecDeque<Instant>, now: Instant) -> bool {
    recent.retain(|time| now.duration_since(*time) < Duration::from_secs(10));
    if recent.len() >= 5 {
        return false;
    }
    recent.push_back(now);
    true
}

/// Whether a draw shows black rather than mpv's frame: a hold in force
/// when the draw ends, or one in force when it began. mpv reads its colour
/// targets when a render starts, so a frame begun under a hold can carry
/// the previous range's encoding even if the switch is released, and the
/// layer retagged, before the draw is flushed.
pub(crate) fn draws_black(held_at_start: u64, held_now: u64) -> bool {
    held_at_start != 0 || held_now != 0
}

/// Like Linux's HDR controller, acknowledgement alone is insufficient:
/// a conditional profile can have replaced the value in that same core
/// iteration. Reobserve after each reply, using a fresh registration, and
/// require all five current reports before changing the layer.
pub(crate) struct Targets {
    pub(crate) range: Range,
    epoch: u64,
    observing: [u64; 5],
    unanswered: [bool; 5],
    unreported: [bool; 5],
    mismatch: bool,
}

impl Targets {
    pub(crate) fn new() -> Self {
        Self {
            range: Range::Sdr,
            epoch: 0,
            observing: [0; 5],
            unanswered: [true; 5],
            unreported: [true; 5],
            mismatch: false,
        }
    }

    fn id(&self, index: usize) -> u64 {
        self.epoch * 8 + index as u64
    }

    pub(crate) fn begin(&mut self, range: Range, mpv: &impl TargetAccess) -> bool {
        self.epoch += 1;
        self.range = range;
        self.unanswered = [true; 5];
        self.unreported = [true; 5];
        self.mismatch = false;
        let mut sent = true;
        for (i, name) in TARGET_NAMES.iter().enumerate() {
            sent &= mpv.ask(self.id(i), name, range.values()[i]);
        }
        sent
    }

    pub(crate) fn answered(&mut self, id: u64, taken: bool, mpv: &impl TargetAccess) -> bool {
        let i = (id % 8) as usize;
        if id / 8 != self.epoch || i >= 5 || !self.unanswered[i] {
            return true;
        }
        if !taken || !mpv.observe(id, TARGET_NAMES[i]) {
            return false;
        }
        if self.observing[i] != 0 {
            mpv.unobserve(self.observing[i]);
        }
        self.observing[i] = id;
        self.unanswered[i] = false;
        true
    }

    pub(crate) fn reported(&mut self, id: u64, name: &str, value: Option<&str>) {
        let Some(i) = TARGET_NAMES.iter().position(|candidate| *candidate == name) else {
            return;
        };
        if id != self.id(i) || id != self.observing[i] || self.unanswered[i] {
            return;
        }
        self.unreported[i] = false;
        let expected = self.range.values()[i];
        let agrees = if i == 2 && expected == "203" {
            value.and_then(|v| v.parse::<f64>().ok()) == Some(203.0)
        } else {
            value == Some(expected)
        };
        self.mismatch |= !agrees;
    }

    pub(crate) fn settled(&self) -> bool {
        !self.unanswered.contains(&true) && !self.unreported.contains(&true)
    }
    pub(crate) fn ready(&self) -> bool {
        self.settled() && !self.mismatch
    }
    pub(crate) fn overridden(&self) -> bool {
        self.mismatch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn a_draw_begun_or_ended_under_a_hold_is_black() {
        assert!(!draws_black(0, 0));
        assert!(draws_black(3, 0), "released while mpv rendered");
        assert!(draws_black(0, 4), "held while mpv rendered");
        assert!(draws_black(3, 4), "released and held again");
    }

    #[test]
    fn pq_bt2020_uses_edr() {
        assert_eq!(
            decide(Some("pq"), Some("bt.2020"), 2.0, true).range,
            Range::Pq2020
        );
        assert_eq!(Range::Pq2020.label(), "EDR PQ BT.2020");
    }
    #[test]
    fn hlg_is_converted_to_pq() {
        assert_eq!(
            decide(Some("hlg"), Some("bt.2020"), 2.0, true).range,
            Range::Pq2020
        );
    }
    #[test]
    fn display_p3_preserves_its_primaries() {
        for trc in ["pq", "hlg"] {
            assert_eq!(
                decide(Some(trc), Some("display-p3"), 2.0, true).range,
                Range::PqP3
            );
        }
        assert_eq!(Range::PqP3.label(), "EDR PQ Display P3");
    }
    #[test]
    fn sdr_and_unknown_transfers_stay_sdr() {
        for trc in [None, Some("srgb"), Some("bt.1886"), Some("unknown")] {
            assert_eq!(decide(trc, Some("bt.2020"), 2.0, true).range, Range::Sdr);
        }
        assert_eq!(Range::Sdr.label(), "SDR sRGB");
    }
    #[test]
    fn unsupported_and_unknown_primaries_stay_sdr() {
        for prim in [None, Some("bt.709"), Some("dci-p3"), Some("unknown")] {
            assert_eq!(decide(Some("pq"), prim, 2.0, true).range, Range::Sdr);
        }
    }
    #[test]
    fn absent_or_invalid_headroom_stays_sdr() {
        for peak in [0.0, 1.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                decide(Some("pq"), Some("bt.2020"), peak, true).range,
                Range::Sdr
            );
        }
    }
    #[test]
    fn eight_bit_surface_stays_sdr() {
        let decision = decide(Some("pq"), Some("bt.2020"), 2.0, false);
        assert_eq!(decision.range, Range::Sdr);
        assert_eq!(decision.reason, "no float surface");
    }
    #[test]
    fn protected_options_describe_each_layer_encoding() {
        assert_eq!(
            TARGET_NAMES,
            [
                "target-trc",
                "target-prim",
                "target-peak",
                "icc-profile-auto",
                "icc-profile"
            ]
        );
        assert_eq!(Range::Sdr.values(), ["srgb", "bt.709", "203", "no", ""]);
        assert_eq!(Range::Pq2020.values(), ["pq", "bt.2020", "auto", "no", ""]);
        assert_eq!(Range::PqP3.values(), ["pq", "display-p3", "auto", "no", ""]);
    }
    #[test]
    fn pixel_formats_and_depth_match_the_float_and_legacy_surfaces() {
        assert_eq!(
            SurfaceFormat::Float.attributes(),
            [99, 0x3200, 73, 8, 64, 58, 5, 0]
        );
        assert_eq!(SurfaceFormat::EightBit.attributes(), [99, 0x3200, 73, 5, 0]);
        assert_eq!(
            SurfaceFormat::Software.attributes(),
            [99, 0x3200, 70, 0x20400, 5, 0]
        );
        assert_eq!(SURFACE_FORMATS.map(SurfaceFormat::depth), [16, 8, 8]);
    }
    #[test]
    fn falls_back_from_float_to_eight_bit_then_software_and_propagates_failure() {
        for accepted in [
            Some(SurfaceFormat::Float),
            Some(SurfaceFormat::EightBit),
            Some(SurfaceFormat::Software),
            None,
        ] {
            let mut attempts = Vec::new();
            let result = choose_surface(|format| {
                attempts.push(format);
                if Some(format) == accepted {
                    Ok(())
                } else {
                    Err("unavailable".into())
                }
            });
            let count = accepted.map_or(3, |format| {
                SURFACE_FORMATS.iter().position(|f| *f == format).unwrap() + 1
            });
            assert_eq!(attempts, SURFACE_FORMATS[..count]);
            assert_eq!(
                result,
                accepted
                    .map(|format| (format, ()))
                    .ok_or_else(|| "unavailable".into())
            );
        }
    }

    #[derive(Default)]
    struct Mpv {
        asks: RefCell<Vec<(u64, String, String)>>,
        observations: RefCell<Vec<u64>>,
        removed: RefCell<Vec<u64>>,
    }
    impl TargetAccess for Mpv {
        fn ask(&self, id: u64, name: &str, value: &str) -> bool {
            self.asks.borrow_mut().push((id, name.into(), value.into()));
            true
        }
        fn observe(&self, id: u64, _: &str) -> bool {
            self.observations.borrow_mut().push(id);
            true
        }
        fn unobserve(&self, id: u64) {
            self.removed.borrow_mut().push(id);
        }
    }
    fn confirm(targets: &mut Targets, mpv: &Mpv) {
        for (i, name) in TARGET_NAMES.iter().enumerate() {
            let id = targets.id(i);
            assert!(targets.answered(id, true, mpv));
            targets.reported(id, name, Some(targets.range.values()[i]));
        }
    }
    #[test]
    fn every_target_needs_an_answer_and_a_fresh_report() {
        let mpv = Mpv::default();
        let mut targets = Targets::new();
        assert!(targets.begin(Range::PqP3, &mpv));
        assert_eq!(mpv.asks.borrow().len(), 5);
        for (i, name) in TARGET_NAMES.iter().enumerate() {
            assert!(!targets.ready());
            targets.reported(targets.id(i), name, Some(targets.range.values()[i]));
            assert!(targets.answered(targets.id(i), true, &mpv));
            assert!(!targets.ready());
        }
        confirm(&mut targets, &mpv);
        assert!(targets.ready());
        assert_eq!(*mpv.observations.borrow(), [8, 9, 10, 11, 12]);
    }
    #[test]
    fn old_replies_and_reports_cannot_complete_a_replacement() {
        let mpv = Mpv::default();
        let mut targets = Targets::new();
        targets.begin(Range::Pq2020, &mpv);
        confirm(&mut targets, &mpv);
        targets.begin(Range::Sdr, &mpv);
        for (i, name) in TARGET_NAMES.iter().enumerate() {
            assert!(targets.answered(8 + i as u64, false, &mpv));
            targets.reported(8 + i as u64, name, Some("wrong"));
        }
        assert!(!targets.settled());
        confirm(&mut targets, &mpv);
        assert!(targets.ready());
        assert_eq!(*mpv.removed.borrow(), [8, 9, 10, 11, 12]);
    }
    #[test]
    fn profiles_cannot_override_targets_in_either_range() {
        for range in [Range::Sdr, Range::Pq2020, Range::PqP3] {
            for (i, name) in TARGET_NAMES.iter().enumerate() {
                let mpv = Mpv::default();
                let mut targets = Targets::new();
                targets.begin(range, &mpv);
                confirm(&mut targets, &mpv);
                targets.reported(targets.id(i), name, Some("wrong"));
                assert!(targets.overridden());
                assert!(!targets.ready());
                targets.begin(range, &mpv);
                confirm(&mut targets, &mpv);
                assert!(targets.ready());
                assert_eq!(mpv.asks.borrow().last().unwrap().2, "");
            }
        }
    }
    #[test]
    fn refused_target_never_becomes_ready() {
        let mpv = Mpv::default();
        let mut targets = Targets::new();
        targets.begin(Range::Pq2020, &mpv);
        assert!(!targets.answered(8, false, &mpv));
        assert!(!targets.ready());
    }

    #[test]
    fn repeated_profile_repairs_are_bounded_and_old_repairs_expire() {
        let mut recent = VecDeque::new();
        let now = Instant::now();
        for _ in 0..5 {
            assert!(allow_restoration(&mut recent, now));
        }
        assert!(!allow_restoration(
            &mut recent,
            now + Duration::from_secs(9)
        ));
        assert!(allow_restoration(
            &mut recent,
            now + Duration::from_secs(10)
        ));
        assert_eq!(recent.len(), 1);
    }
}
