//! HDR for the embedded player: what to tell mpv and the compositor.
//!
//! The external player gets HDR from mpv's own Vulkan window, which
//! negotiates with the compositor by itself. Embedded, mpv draws into a
//! surface Vela owns, and nothing is negotiated unless Vela does it. Two
//! things have to agree, or the picture is wrong:
//!
//! - **mpv** is told what it is drawing for: PQ, BT.2020, and the brightest
//!   level the display reaches (`target-trc`, `target-prim`, `target-peak`);
//! - **the compositor** is told what the surface's pixels mean, with an image
//!   description saying the same thing (the `wp_color_manager_v1` protocol).
//!
//! The numbers come from the compositor: for the output the surface is on it
//! reports a reference white and the output's own luminance range. Its range
//! is expressed relative to ITS reference white, which the user's brightness
//! setting moves; mpv, like PQ video, works with a fixed reference of 203
//! nits. So every luminance is rescaled by `203 / reference`. This is what
//! Mesa's Vulkan window code sends for the external player (the spike's log,
//! `.agents/plans/embedded-libmpv-hdr-spike.md`).
//!
//! This file is the arithmetic, the decisions, and the one place where a
//! decision is carried out on both sides (`carry_out`), all of it against
//! two small traits so that it can be tested without a compositor or an mpv.
//! `surface.rs` speaks the protocol and `session.rs` passes on what the
//! compositor says.

/// The reference white PQ video is mastered against, in nits.
const VIDEO_REFERENCE_WHITE: u64 = 203;

/// The brightest level PQ can express, in nits. Neither mpv nor the
/// compositor takes a peak above it.
const PQ_MAXIMUM: u32 = 10_000;

/// What the compositor says about the output the video surface is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OutputDescription {
    /// The output's reference white, in nits.
    pub(crate) reference: u32,
    /// The darkest level the output shows, in ten-thousandths of a nit.
    pub(crate) min_e4: u32,
    /// The brightest level the output shows, in nits.
    pub(crate) max: u32,
    /// The output's own primaries and white point as CIE xy pairs, each
    /// multiplied by a million, when the compositor gives them.
    pub(crate) primaries: Option<[i32; 8]>,
}

/// What Vela can do about HDR on this machine, apart from the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Capabilities {
    /// The compositor can be told what a surface's pixels mean: as PQ and
    /// BT.2020, built from parameters, with the mastering display's range
    /// and primaries, shown with perceptual intent. All of it or none: a
    /// request the compositor did not offer ends the connection, and the
    /// connection is the whole app's.
    pub(crate) can_describe: bool,
    /// The window has ten bits per channel. PQ in eight bands visibly.
    pub(crate) ten_bit_window: bool,
}

impl Capabilities {
    /// Whether HDR can be presented here at all, whatever the output.
    pub(crate) fn hdr_possible(self) -> bool {
        self.can_describe && self.ten_bit_window
    }
}

/// An HDR presentation: the same facts for mpv and for the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HdrTarget {
    /// The brightest level to draw, in nits: mpv's `target-peak`, and the
    /// description's mastering maximum and content light level.
    pub(crate) peak: u32,
    /// The darkest level, in ten-thousandths of a nit.
    pub(crate) min_e4: u32,
    /// The mastering display's primaries, when the output reports its own.
    pub(crate) primaries: Option<[i32; 8]>,
}

/// How the video is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Presentation {
    /// Standard range: mpv's target left to itself, the surface untagged.
    Sdr,
    Hdr(HdrTarget),
}

fn rescaled(value: u64, reference: u64) -> u32 {
    // Rounded to nearest; the inputs are far too small to overflow.
    let scaled = (value * VIDEO_REFERENCE_WHITE + reference / 2) / reference;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

/// Decide how to present video on an output.
///
/// HDR only when everything it needs is there: an output brighter than its
/// own reference white, a compositor that can be told, and a ten-bit window.
/// Anything less is SDR, which is always correct, if less than the display
/// could do.
pub(crate) fn presentation(output: Option<OutputDescription>, can: Capabilities) -> Presentation {
    let Some(output) = output else {
        return Presentation::Sdr;
    };
    if !can.hdr_possible() {
        return Presentation::Sdr;
    }
    if output.reference == 0 || output.max <= output.reference {
        return Presentation::Sdr;
    }
    let reference = u64::from(output.reference);
    let peak = rescaled(u64::from(output.max), reference).min(PQ_MAXIMUM);
    let min_e4 = rescaled(u64::from(output.min_e4), reference);
    // A range the protocol would refuse: the minimum must lie below the
    // maximum (compared in the same unit).
    if u64::from(min_e4) >= u64::from(peak) * 10_000 {
        return Presentation::Sdr;
    }
    Presentation::Hdr(HdrTarget {
        peak,
        min_e4,
        primaries: output.primaries,
    })
}

/// What has to be done next to keep mpv and the surface in agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Nothing,
    /// Ask the compositor for an image description of this target. Its
    /// answer comes back later, as ready or failed.
    Describe(HdrTarget),
    /// The description is ready: put it on the surface and give mpv the
    /// same target, together, and draw a frame with both in effect.
    ShowHdr(HdrTarget),
    /// Take the description off the surface and let mpv choose its own
    /// target again, together, and draw a frame.
    ShowSdr,
}

/// The compositor's side of the agreement: the video surface.
pub(crate) trait Describable {
    /// Ask for an image description of `target`. The answer comes later.
    fn describe(&mut self, target: &HdrTarget);
    /// Put the description that was made on the surface. `false` when it
    /// could not be.
    fn tag(&mut self) -> bool;
    /// Take any description off the surface.
    fn untag(&mut self);
}

/// mpv's side of the agreement.
pub(crate) trait Targetable {
    /// Set one of the options that say what mpv draws for. `false` when mpv
    /// refuses it.
    fn set_target(&self, option: &str, value: &str) -> bool;
}

/// The three options that tell mpv what it is drawing for, with the values
/// that hand the choice back to mpv.
const TARGET_OPTIONS: [(&str, &str); 3] = [
    ("target-trc", "auto"),
    ("target-prim", "auto"),
    ("target-peak", "auto"),
];

/// What carrying out a step changed about the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    /// Nothing that a frame would show.
    Nothing,
    /// HDR on both sides, with this peak in nits.
    Hdr(u32),
    /// Standard range on both sides.
    Sdr,
    /// HDR was to be shown and one side would not take it: standard range
    /// on both sides.
    HdrRefused,
}

impl Change {
    /// Whether a frame has to be drawn for the change to be seen.
    pub(crate) fn needs_a_frame(self) -> bool {
        self != Change::Nothing
    }
}

fn standard_range(mpv: &impl Targetable, surface: &mut impl Describable) {
    for (option, value) in TARGET_OPTIONS {
        mpv.set_target(option, value);
    }
    surface.untag();
}

/// Carry out one step of keeping mpv and the surface in agreement.
///
/// mpv is given an HDR target in exactly one place, here, in the same breath
/// as the surface is given its description, and loses it in the same breath
/// as the surface does. Both changes take effect with the next frame drawn.
/// PQ pixels on a surface the compositor takes for sRGB, or the reverse, is
/// the one outcome that must not happen: when either side refuses, both go
/// to standard range.
pub(crate) fn carry_out(
    step: Step,
    tagging: &mut Tagging,
    mpv: &impl Targetable,
    surface: &mut impl Describable,
) -> Change {
    match step {
        Step::Nothing => Change::Nothing,
        Step::Describe(target) => {
            surface.describe(&target);
            Change::Nothing
        }
        Step::ShowHdr(target) => {
            let peak = target.peak.to_string();
            // The surface first: if it cannot be tagged, mpv is never asked.
            let both = surface.tag()
                && [
                    ("target-trc", "pq"),
                    ("target-prim", "bt.2020"),
                    ("target-peak", peak.as_str()),
                ]
                .iter()
                .all(|(option, value)| mpv.set_target(option, value));
            if both {
                Change::Hdr(target.peak)
            } else {
                standard_range(mpv, surface);
                tagging.could_not_show();
                Change::HdrRefused
            }
        }
        Step::ShowSdr => {
            standard_range(mpv, surface);
            Change::Sdr
        }
    }
}

/// Keeps track of what is in effect and what has been asked for, across the
/// compositor's changes of mind: a monitor swapped, HDR switched off, the
/// brightness slider moved (which changes the reference white, and with it
/// the peak; seen in Slice 0: 83 nits one evening, 265 another).
#[derive(Debug)]
pub(crate) struct Tagging {
    in_effect: Presentation,
    /// A description that has been asked for and not answered.
    asked: Option<HdrTarget>,
    /// The compositor has described the output at least once.
    heard: bool,
}

impl Tagging {
    pub(crate) fn new() -> Self {
        Tagging {
            in_effect: Presentation::Sdr,
            asked: None,
            heard: false,
        }
    }

    /// Whether there is nothing left to wait for: the output has been
    /// described and no request is unanswered.
    pub(crate) fn settled(&self) -> bool {
        self.heard && self.asked.is_none()
    }

    /// The HDR presentation that was just to be shown could not be: what is
    /// in effect is standard range. The same output described again will be
    /// tried again.
    fn could_not_show(&mut self) {
        self.in_effect = Presentation::Sdr;
    }

    #[cfg(test)]
    pub(crate) fn in_effect(&self) -> Presentation {
        self.in_effect
    }

    /// The compositor has described the output, for the first time or again.
    pub(crate) fn output_described(&mut self, wanted: Presentation) -> Step {
        self.heard = true;
        match wanted {
            Presentation::Sdr => {
                // Whatever was asked for is no longer wanted.
                self.asked = None;
                if self.in_effect == Presentation::Sdr {
                    Step::Nothing
                } else {
                    self.in_effect = Presentation::Sdr;
                    Step::ShowSdr
                }
            }
            Presentation::Hdr(target) => {
                if self.asked == Some(target) {
                    // Already asked for exactly this.
                    Step::Nothing
                } else if self.asked.is_none() && self.in_effect == wanted {
                    Step::Nothing
                } else {
                    self.asked = Some(target);
                    Step::Describe(target)
                }
            }
        }
    }

    /// The compositor made the description that was last asked for.
    pub(crate) fn description_ready(&mut self) -> Step {
        match self.asked.take() {
            Some(target) => {
                self.in_effect = Presentation::Hdr(target);
                Step::ShowHdr(target)
            }
            // An answer to a request that was since withdrawn.
            None => Step::Nothing,
        }
    }

    /// The compositor refused the description that was last asked for. What
    /// is in effect stays: it was accepted once and mpv still matches it.
    pub(crate) fn description_failed(&mut self) -> Step {
        self.asked = None;
        Step::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Capabilities = Capabilities {
        can_describe: true,
        ten_bit_window: true,
    };

    /// BT.2020 primaries and D65, as the protocol carries them.
    const PRIMARIES: [i32; 8] = [708_000, 292_000, 170_000, 797_000, 131_000, 46_000, 312_700, 329_000];

    fn output(reference: u32, min_e4: u32, max: u32) -> Option<OutputDescription> {
        Some(OutputDescription {
            reference,
            min_e4,
            max,
            primaries: Some(PRIMARIES),
        })
    }

    fn hdr(peak: u32, min_e4: u32) -> Presentation {
        Presentation::Hdr(HdrTarget {
            peak,
            min_e4,
            primaries: Some(PRIMARIES),
        })
    }

    // The two readings taken on the development display, with the desktop
    // brightness at two different settings (the plan, "Colour", and Slice 0).
    #[test]
    fn the_peak_is_the_outputs_maximum_rescaled_to_the_video_reference_white() {
        // 465 nits at a reference white of 265: 465 * 203 / 265 = 356.2.
        assert_eq!(presentation(output(265, 0, 465), ALL), hdr(356, 0));
        // The same panel with the reference white at 83: 1137.3.
        assert_eq!(presentation(output(83, 0, 465), ALL), hdr(1137, 0));
        // A reference white of exactly 203 changes nothing.
        assert_eq!(presentation(output(203, 50, 1000), ALL), hdr(1000, 50));
        // Rounded to nearest, not down: 600 * 203 / 240 = 507.5.
        assert_eq!(presentation(output(240, 0, 600), ALL), hdr(508, 0));
    }

    #[test]
    fn the_minimum_is_rescaled_the_same_way() {
        // 0.05 nits (500 ten-thousandths) at reference 265: 383.0.
        assert_eq!(presentation(output(265, 500, 465), ALL), hdr(356, 383));
    }

    #[test]
    fn an_output_no_brighter_than_its_reference_white_is_sdr() {
        assert_eq!(presentation(output(203, 2000, 203), ALL), Presentation::Sdr);
        assert_eq!(presentation(output(250, 2000, 200), ALL), Presentation::Sdr);
        // One nit above is HDR; the line is "brighter than reference white".
        assert_ne!(presentation(output(203, 0, 204), ALL), Presentation::Sdr);
    }

    #[test]
    fn nothing_to_go_on_is_sdr() {
        assert_eq!(presentation(None, ALL), Presentation::Sdr);
        // A compositor that reports no reference white cannot be rescaled to.
        assert_eq!(presentation(output(0, 0, 1000), ALL), Presentation::Sdr);
        // A range with no room in it: 600 nits "minimum" under a 500 nit peak.
        assert_eq!(presentation(output(203, 6_000_000, 500), ALL), Presentation::Sdr);
    }

    #[test]
    fn hdr_needs_a_compositor_that_can_be_told_and_a_ten_bit_window() {
        let bright = output(265, 0, 465);
        assert_eq!(
            presentation(
                bright,
                Capabilities {
                    can_describe: false,
                    ..ALL
                }
            ),
            Presentation::Sdr,
            "PQ pixels on a surface the compositor takes for sRGB would be wrong"
        );
        assert_eq!(
            presentation(
                bright,
                Capabilities {
                    ten_bit_window: false,
                    ..ALL
                }
            ),
            Presentation::Sdr,
            "PQ in eight bits bands"
        );
    }

    #[test]
    fn the_peak_never_exceeds_what_pq_can_express() {
        // 4000 nits at a reference white of 80 would be 10150.
        assert_eq!(presentation(output(80, 0, 4000), ALL), hdr(10_000, 0));
        // And the range is judged against the peak that is sent: a minimum
        // of 10000 nits has no room under it.
        assert_eq!(
            presentation(output(80, 39_408_867, 4000), ALL),
            Presentation::Sdr
        );
    }

    #[test]
    fn an_output_that_reports_no_primaries_has_none_to_send() {
        let unreported = Some(OutputDescription {
            reference: 265,
            min_e4: 0,
            max: 465,
            primaries: None,
        });
        assert_eq!(
            presentation(unreported, ALL),
            Presentation::Hdr(HdrTarget {
                peak: 356,
                min_e4: 0,
                primaries: None
            })
        );
    }

    fn target(peak: u32) -> HdrTarget {
        HdrTarget {
            peak,
            min_e4: 0,
            primaries: Some(PRIMARIES),
        }
    }

    #[test]
    fn hdr_takes_effect_only_once_the_compositor_has_the_description() {
        let mut tagging = Tagging::new();
        assert_eq!(tagging.in_effect(), Presentation::Sdr);
        assert_eq!(
            tagging.output_described(Presentation::Hdr(target(356))),
            Step::Describe(target(356))
        );
        // Asked for, not yet in effect: mpv must not be switched to PQ while
        // the surface still says sRGB.
        assert_eq!(tagging.in_effect(), Presentation::Sdr);
        assert_eq!(tagging.description_ready(), Step::ShowHdr(target(356)));
        assert_eq!(tagging.in_effect(), Presentation::Hdr(target(356)));
    }

    #[test]
    fn the_same_description_again_changes_nothing() {
        let mut tagging = Tagging::new();
        tagging.output_described(Presentation::Hdr(target(356)));
        // The compositor repeats itself before answering.
        assert_eq!(
            tagging.output_described(Presentation::Hdr(target(356))),
            Step::Nothing
        );
        tagging.description_ready();
        // And after.
        assert_eq!(
            tagging.output_described(Presentation::Hdr(target(356))),
            Step::Nothing
        );
        assert_eq!(tagging.output_described(Presentation::Sdr), Step::ShowSdr);
        assert_eq!(tagging.output_described(Presentation::Sdr), Step::Nothing);
    }

    #[test]
    fn a_moved_brightness_slider_retags_with_the_new_peak() {
        let mut tagging = Tagging::new();
        tagging.output_described(Presentation::Hdr(target(356)));
        tagging.description_ready();
        // The reference white moved; so did the peak.
        assert_eq!(
            tagging.output_described(Presentation::Hdr(target(1137))),
            Step::Describe(target(1137))
        );
        // Until the new description exists the old one stays in effect.
        assert_eq!(tagging.in_effect(), Presentation::Hdr(target(356)));
        assert_eq!(tagging.description_ready(), Step::ShowHdr(target(1137)));
    }

    #[test]
    fn a_change_of_mind_before_the_answer_asks_again_and_the_last_one_wins() {
        let mut tagging = Tagging::new();
        assert_eq!(
            tagging.output_described(Presentation::Hdr(target(356))),
            Step::Describe(target(356))
        );
        assert_eq!(
            tagging.output_described(Presentation::Hdr(target(1137))),
            Step::Describe(target(1137))
        );
        // One answer arrives: it is for the last request (the earlier
        // description was dropped when the new one was asked for).
        assert_eq!(tagging.description_ready(), Step::ShowHdr(target(1137)));
        assert_eq!(tagging.description_ready(), Step::Nothing);
    }

    #[test]
    fn hdr_switched_off_untags_even_with_a_request_outstanding() {
        let mut tagging = Tagging::new();
        tagging.output_described(Presentation::Hdr(target(356)));
        tagging.description_ready();
        tagging.output_described(Presentation::Hdr(target(1137)));
        assert_eq!(tagging.output_described(Presentation::Sdr), Step::ShowSdr);
        // The answer to the withdrawn request must not bring HDR back.
        assert_eq!(tagging.description_ready(), Step::Nothing);
        assert_eq!(tagging.in_effect(), Presentation::Sdr);
    }

    #[test]
    fn a_refused_description_leaves_what_was_in_effect() {
        let mut tagging = Tagging::new();
        // Refused at the start: stays SDR, and may be asked again later.
        tagging.output_described(Presentation::Hdr(target(356)));
        assert_eq!(tagging.description_failed(), Step::Nothing);
        assert_eq!(tagging.in_effect(), Presentation::Sdr);
        assert_eq!(
            tagging.output_described(Presentation::Hdr(target(356))),
            Step::Describe(target(356))
        );
        tagging.description_ready();
        // Refused on a re-tag: the accepted one stays, with mpv matching it.
        tagging.output_described(Presentation::Hdr(target(1137)));
        assert_eq!(tagging.description_failed(), Step::Nothing);
        assert_eq!(tagging.in_effect(), Presentation::Hdr(target(356)));
    }

    /// Both sides of the agreement, remembering what they were last told.
    #[derive(Default)]
    struct Fake {
        /// The description asked for and not yet put on the surface.
        described: Option<HdrTarget>,
        /// What the surface is tagged with.
        tagged: Option<HdrTarget>,
        /// mpv's options, in the order they were set.
        set: std::cell::RefCell<Vec<(String, String)>>,
        cannot_tag: bool,
        /// An option mpv refuses when it is not being handed back.
        refuses: Option<&'static str>,
    }

    impl Describable for Fake {
        fn describe(&mut self, target: &HdrTarget) {
            self.described = Some(*target);
        }
        fn tag(&mut self) -> bool {
            if self.cannot_tag {
                return false;
            }
            self.tagged = self.described;
            self.tagged.is_some()
        }
        fn untag(&mut self) {
            self.tagged = None;
        }
    }

    impl Fake {
        fn mpv(&self, option: &str) -> Option<String> {
            let set = self.set.borrow();
            set.iter()
                .rev()
                .find(|(name, _)| name == option)
                .map(|(_, value)| value.clone())
        }

        /// What the two sides say, when they say the same: the peak in HDR,
        /// `None` in standard range. Panics when they disagree.
        fn agreed(&self) -> Option<u32> {
            let mpv = (
                self.mpv("target-trc").unwrap_or("auto".into()),
                self.mpv("target-prim").unwrap_or("auto".into()),
                self.mpv("target-peak").unwrap_or("auto".into()),
            );
            match self.tagged {
                Some(target) => {
                    assert_eq!(
                        mpv,
                        ("pq".into(), "bt.2020".into(), target.peak.to_string()),
                        "the surface is tagged for {} nits",
                        target.peak
                    );
                    Some(target.peak)
                }
                None => {
                    assert_eq!(
                        mpv,
                        ("auto".into(), "auto".into(), "auto".into()),
                        "the surface is untagged"
                    );
                    None
                }
            }
        }

        fn was_ever_given_pq(&self) -> bool {
            self.set.borrow().iter().any(|(_, value)| value == "pq")
        }
    }

    /// The mpv side of a [`Fake`], borrowed apart from its surface side the
    /// way the real player and the real surface are two objects.
    struct FakeMpv<'a>(&'a std::cell::RefCell<Vec<(String, String)>>, Option<&'static str>);

    impl Targetable for FakeMpv<'_> {
        fn set_target(&self, option: &str, value: &str) -> bool {
            if value != "auto" && self.1 == Some(option) {
                return false;
            }
            self.0.borrow_mut().push((option.to_string(), value.to_string()));
            true
        }
    }

    /// Everything the session does with what the compositor says, against a
    /// fake of both sides.
    struct Play {
        tagging: Tagging,
        fake: Fake,
    }

    impl Play {
        fn new(fake: Fake) -> Self {
            Play {
                tagging: Tagging::new(),
                fake,
            }
        }

        fn carry_out(&mut self, step: Step) -> Change {
            let set = std::mem::take(&mut self.fake.set);
            let change = carry_out(
                step,
                &mut self.tagging,
                &FakeMpv(&set, self.fake.refuses),
                &mut self.fake,
            );
            self.fake.set = set;
            change
        }

        fn output(&mut self, wanted: Presentation) -> Change {
            let step = self.tagging.output_described(wanted);
            self.carry_out(step)
        }

        fn answer(&mut self, made: bool) -> Change {
            let step = if made {
                self.tagging.description_ready()
            } else {
                self.tagging.description_failed()
            };
            self.carry_out(step)
        }
    }

    #[test]
    fn mpv_and_the_surface_change_together_through_a_whole_evening() {
        let mut play = Play::new(Fake::default());
        assert_eq!(play.fake.agreed(), None);

        // An HDR output: asked for, nothing shown until the answer.
        assert_eq!(play.output(Presentation::Hdr(target(356))), Change::Nothing);
        assert_eq!(play.fake.agreed(), None);
        assert!(!play.tagging.settled());
        assert_eq!(play.answer(true), Change::Hdr(356));
        assert_eq!(play.fake.agreed(), Some(356));
        assert!(play.tagging.settled());

        // The brightness slider moves.
        assert_eq!(play.output(Presentation::Hdr(target(1137))), Change::Nothing);
        assert_eq!(play.fake.agreed(), Some(356), "the old one until the new one is made");
        assert_eq!(play.answer(true), Change::Hdr(1137));
        assert_eq!(play.fake.agreed(), Some(1137));

        // A new description is refused: the accepted one stays on both sides.
        assert_eq!(play.output(Presentation::Hdr(target(600))), Change::Nothing);
        assert_eq!(play.answer(false), Change::Nothing);
        assert_eq!(play.fake.agreed(), Some(1137));

        // HDR is switched off, then on again.
        assert_eq!(play.output(Presentation::Sdr), Change::Sdr);
        assert_eq!(play.fake.agreed(), None);
        assert_eq!(play.output(Presentation::Hdr(target(1137))), Change::Nothing);
        assert_eq!(play.answer(true), Change::Hdr(1137));
        assert_eq!(play.fake.agreed(), Some(1137));
    }

    #[test]
    fn a_change_is_drawn_and_nothing_is_not() {
        assert!(!Change::Nothing.needs_a_frame());
        for change in [Change::Hdr(356), Change::Sdr, Change::HdrRefused] {
            assert!(change.needs_a_frame(), "{change:?}");
        }
    }

    #[test]
    fn an_mpv_that_refuses_the_target_leaves_both_sides_in_standard_range() {
        for refused in ["target-trc", "target-prim", "target-peak"] {
            let mut play = Play::new(Fake {
                refuses: Some(refused),
                ..Fake::default()
            });
            play.output(Presentation::Hdr(target(356)));
            assert_eq!(play.answer(true), Change::HdrRefused, "{refused}");
            assert_eq!(play.fake.agreed(), None, "{refused}");
            // And the record says so: the same output described again is
            // tried again rather than taken for shown.
            assert_eq!(play.tagging.in_effect(), Presentation::Sdr);
            assert_eq!(
                play.tagging.output_described(Presentation::Hdr(target(356))),
                Step::Describe(target(356))
            );
        }
    }

    #[test]
    fn a_surface_that_cannot_be_tagged_never_gives_mpv_an_hdr_target() {
        let mut play = Play::new(Fake {
            cannot_tag: true,
            ..Fake::default()
        });
        play.output(Presentation::Hdr(target(356)));
        assert_eq!(play.answer(true), Change::HdrRefused);
        assert_eq!(play.fake.agreed(), None);
        assert!(!play.fake.was_ever_given_pq(), "not even for a moment");
    }

    #[test]
    fn a_refusal_while_hdr_is_showing_takes_both_sides_out_of_it() {
        let mut play = Play::new(Fake::default());
        play.output(Presentation::Hdr(target(356)));
        play.answer(true);
        // The next target is one mpv will not take.
        play.fake.refuses = Some("target-peak");
        play.output(Presentation::Hdr(target(1137)));
        assert_eq!(play.answer(true), Change::HdrRefused);
        assert_eq!(play.fake.agreed(), None);
    }

    #[test]
    fn a_play_has_settled_once_the_output_is_known_and_nothing_is_unanswered() {
        let mut tagging = Tagging::new();
        assert!(!tagging.settled(), "nothing heard yet");
        tagging.output_described(Presentation::Sdr);
        assert!(tagging.settled(), "a standard-range output needs no answer");

        let mut tagging = Tagging::new();
        tagging.output_described(Presentation::Hdr(target(356)));
        assert!(!tagging.settled());
        tagging.description_failed();
        assert!(tagging.settled(), "a refusal is an answer too");
    }
}
