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
//! decision is carried out on both sides (`carry_out`; `answered` for
//! mpv's answers; `drained` once mpv's queue for Vela has been read empty),
//! all of it against two small traits so that it can be
//! tested without a compositor or an mpv. It also decides what becomes of
//! each frame while mpv's side changes (`Tagging::frame`). `surface.rs`
//! speaks the protocol and `session.rs` passes on what the compositor and
//! mpv say and does what `Tagging::frame` says.

use std::time::{Duration, Instant};

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

/// One piece of the compositor's description of an output, as it arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Piece {
    /// The reference white, in nits.
    Reference(u32),
    /// The darkest level in ten-thousandths of a nit, and the brightest in
    /// nits.
    Range { min_e4: u32, max: u32 },
    /// The output's own primaries and white point.
    Primaries([i32; 8]),
}

/// The compositor's descriptions of the output, put together from the
/// pieces they arrive in.
///
/// Vela asks; the compositor says it is ready; Vela asks for the contents;
/// they arrive as several events and then "done". Vela may ask again before
/// an earlier answer has finished arriving, because the display changed
/// again. The protocol does not promise that the rest of an earlier answer
/// arrives before a later question is acknowledged. So every piece is
/// handed in with the number of the question it answers, and only the
/// latest question's answer is put together and handed on. One shared slot
/// filled by whoever spoke let the tail of an old answer overwrite a new one
/// and lose it (review finding hd-1).
#[derive(Debug, Default)]
pub(crate) struct OutputAnswers {
    /// The number of the latest question. Zero before the first.
    asked: u32,
    /// The latest question's answer while its pieces arrive.
    arriving: Option<OutputDescription>,
    /// A complete answer to the latest question that nobody has collected.
    complete: Option<OutputDescription>,
}

impl OutputAnswers {
    /// A new question is being asked; this is its number. Whatever was
    /// known or arriving about earlier questions is about a display that has
    /// changed since, and is dropped.
    pub(crate) fn ask(&mut self) -> u32 {
        self.asked += 1;
        self.arriving = None;
        self.complete = None;
        self.asked
    }

    /// The compositor is ready to answer `question`. `true` when its
    /// contents are wanted, which is when it is the latest question.
    pub(crate) fn ready(&mut self, question: u32) -> bool {
        if question != self.asked {
            return false;
        }
        self.arriving = Some(OutputDescription {
            reference: 0,
            min_e4: 0,
            max: 0,
            primaries: None,
        });
        true
    }

    /// A piece of the answer to `question` has arrived.
    pub(crate) fn piece(&mut self, question: u32, piece: Piece) {
        if question != self.asked {
            return;
        }
        let Some(arriving) = self.arriving.as_mut() else {
            return;
        };
        match piece {
            Piece::Reference(reference) => arriving.reference = reference,
            Piece::Range { min_e4, max } => {
                arriving.min_e4 = min_e4;
                arriving.max = max;
            }
            Piece::Primaries(primaries) => arriving.primaries = Some(primaries),
        }
    }

    /// The answer to `question` has arrived in full.
    pub(crate) fn done(&mut self, question: u32) {
        if question == self.asked {
            if let Some(answer) = self.arriving.take() {
                self.complete = Some(answer);
            }
        }
    }

    /// A complete answer to the latest question, once.
    pub(crate) fn take(&mut self) -> Option<OutputDescription> {
        self.complete.take()
    }
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
    /// The description is ready: put it on the surface and ask mpv for the
    /// same target.
    ShowHdr(HdrTarget),
    /// Take the description off the surface and ask mpv to choose its own
    /// target again.
    ShowSdr,
}

/// The compositor's side of the agreement: the video surface. What `tag`
/// and `untag` change is pending: it takes effect when the next frame is
/// presented, and only then.
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
    /// Ask mpv, without waiting, to set one of the options that say what it
    /// draws for. Its answer arrives later as an event carrying `request`,
    /// which goes to [`answered`]. `false`: the ask could not be sent and no
    /// answer will come.
    fn ask(&self, request: u64, option: &str, value: &str) -> bool;
}

/// The three options that tell mpv what it is drawing for. `auto` for all
/// three hands the choice back to mpv.
const TARGET_OPTIONS: [&str; 3] = ["target-trc", "target-prim", "target-peak"];

/// The longest unbroken time presentation is held while mpv's targets
/// change. Drawing and reporting every frame it holds, mpv answers ONE
/// change within about one frame interval: at most 84 ms at 24 frames a
/// second, 51 ms at 2, under 1 ms paused. Changes that replace each other
/// are held as one stretch, and those chain: a brightness slider dragged
/// for ten seconds, one change per frame, held mpv for up to 667 ms at 24
/// frames a second, and fewer frames are presented while that lasts (all
/// measured with the shipped library for review finding hd-3). So this
/// leaves about 12 times the room a single change needs and 1.5 times what
/// a long drag measured; a drag that keeps mpv changing for longer than
/// this shows black. Past this limit something is wrong, and black is
/// presented instead of a picture that may be half changed.
pub(crate) const HOLD_LIMIT: Duration = Duration::from_millis(1000);

/// What a finished change of mpv's targets changed about the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    /// Nothing has finished.
    Nothing,
    /// HDR on both sides, with this peak in nits.
    Hdr(u32),
    /// Standard range on both sides.
    Sdr,
    /// HDR was to be shown and one side would not take it: standard range
    /// on both sides.
    HdrRefused,
}

/// What the play does with a frame (see [`Tagging::frame`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Frame {
    /// Nothing to draw.
    Nothing,
    /// Draw mpv's frame and report it shown, but do not present it. The
    /// report is what keeps mpv's video thread from waiting for this one
    /// (it waits up to 200 ms for it, and only answers an ask between
    /// frames); not presenting is what keeps a half-changed picture, and
    /// the surface's pending description, off the screen.
    Hold,
    /// Draw mpv's frame, present it, and report it shown.
    Show,
    /// Present black: mpv's targets have been changing for longer than
    /// [`HOLD_LIMIT`]. Black is the same picture under either description.
    Black,
    /// Both of the above: hold mpv's frame, then present black.
    HoldAndBlack,
}

/// Where a change of mpv's targets is going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Goal {
    Hdr(HdrTarget),
    /// Standard range; `refused` when it is because HDR could not be shown.
    Sdr { refused: bool },
}

/// A change of mpv's three targets that mpv has not finished answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Transition {
    number: u64,
    goal: Goal,
    /// Which of the three asks have not been answered.
    unanswered: [bool; 3],
    /// An ask was refused.
    refused: bool,
    /// An ask could not be sent: mpv's queue for Vela was full. This change
    /// is asked again once Vela has read that queue empty (see [`drained`]).
    unsent: bool,
}

/// Each ask carries the number of its transition and the index of its
/// option, so that an answer says which option of which transition it is.
fn request(transition: u64, option: usize) -> u64 {
    transition * 4 + option as u64
}

fn split(request: u64) -> (u64, usize) {
    (request / 4, (request % 4) as usize)
}

/// Carry out one step of keeping mpv and the surface in agreement.
///
/// mpv is asked for an HDR target in exactly one place, here, in the same
/// breath as the surface is given its description, and asked to give it up
/// in the same breath as the surface loses it. The surface's side is
/// pending and mpv's is answered later; no frame is presented between the
/// two (see [`Tagging::frame`]), so both take effect with the same one. PQ
/// pixels on a surface the compositor takes for sRGB, or the reverse, is
/// the one outcome that must not happen: when either side refuses, both go
/// to standard range. A change is reported when it has finished, which is
/// only ever in [`answered`].
pub(crate) fn carry_out(
    step: Step,
    tagging: &mut Tagging,
    mpv: &impl Targetable,
    surface: &mut impl Describable,
    now: Instant,
) {
    match step {
        Step::Nothing => {}
        Step::Describe(target) => surface.describe(&target),
        Step::ShowHdr(target) => {
            // The surface first: if it cannot be tagged, mpv is never asked.
            if surface.tag() {
                tagging.begin(Goal::Hdr(target), mpv, surface, now);
            } else {
                tagging.refused(mpv, surface, now);
            }
        }
        Step::ShowSdr => {
            surface.untag();
            tagging.begin(Goal::Sdr { refused: false }, mpv, surface, now);
        }
    }
}

/// mpv has answered the ask made with `request`: `taken`, or refused. The
/// only way a change of mpv's targets finishes. Answers to a change that has
/// since been replaced are ignored: mpv applies asks in the order they were
/// made, so the latest is the one that holds.
pub(crate) fn answered(
    request: u64,
    taken: bool,
    tagging: &mut Tagging,
    mpv: &impl Targetable,
    surface: &mut impl Describable,
    now: Instant,
) -> Change {
    let (number, option) = split(request);
    if option >= TARGET_OPTIONS.len() {
        return Change::Nothing;
    }
    let Some(pending) = tagging.pending.as_mut() else {
        return Change::Nothing;
    };
    if pending.number != number || !pending.unanswered[option] {
        return Change::Nothing;
    }
    pending.unanswered[option] = false;
    pending.refused |= !taken;
    if pending.unanswered.contains(&true) || pending.unsent {
        // An ask of this change could not be sent; it is asked again, whole,
        // once there is room (see [`drained`]).
        return Change::Nothing;
    }
    tagging.finish(mpv, surface, now)
}

/// Vela has read mpv's queue for it empty, so asks can be sent again. A
/// change some of whose asks could not be sent is asked again, whole, here
/// and not earlier: once that queue has filled, mpv takes no ask until it
/// has been read empty (client.c, `reserve_reply` and `choked`), and asks
/// made before it filled still land and change what mpv draws. Asked last,
/// this change is the one that holds (mpv applies asks in the order they
/// were made), so it finishes only when mpv draws what the surface says.
pub(crate) fn drained(
    tagging: &mut Tagging,
    mpv: &impl Targetable,
    surface: &mut impl Describable,
    now: Instant,
) {
    if let Some(pending) = tagging.pending {
        if pending.unsent {
            tagging.begin(pending.goal, mpv, surface, now);
        }
    }
}

/// Keeps track of what is in effect and what has been asked for, across the
/// compositor's changes of mind: a monitor swapped, HDR switched off, the
/// brightness slider moved (which changes the reference white, and with it
/// the peak; seen in Slice 0: 83 nits one evening, 265 another).
#[derive(Debug, Clone)]
pub(crate) struct Tagging {
    /// What is being shown, or is on its way to being shown: the goal from
    /// the moment a change begins. A finish confirms it, or after a refusal
    /// takes it back to standard range.
    in_effect: Presentation,
    /// A description that has been asked for and not answered.
    asked: Option<HdrTarget>,
    /// The compositor has described the output at least once.
    heard: bool,
    /// The change of mpv's targets in flight, if any.
    pending: Option<Transition>,
    /// The number of the last change begun.
    transitions: u64,
    /// When the unbroken stretch of changes in flight began.
    changing_since: Option<Instant>,
    /// A finished change, or a redraw, is waiting for a frame to show it.
    owed: bool,
    /// Black has been presented since the hold ran out.
    black_shown: bool,
}

impl Tagging {
    pub(crate) fn new() -> Self {
        Tagging {
            in_effect: Presentation::Sdr,
            asked: None,
            heard: false,
            pending: None,
            transitions: 0,
            changing_since: None,
            owed: false,
            black_shown: false,
        }
    }

    /// Whether a change of mpv's targets is in flight.
    pub(crate) fn changing(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether there is nothing left to wait for: the output has been
    /// described, no description is unanswered, and mpv has answered every
    /// ask.
    pub(crate) fn settled(&self) -> bool {
        self.heard && self.asked.is_none() && self.pending.is_none()
    }

    #[cfg(test)]
    pub(crate) fn in_effect(&self) -> Presentation {
        self.in_effect
    }

    /// Whether the change in flight is waiting for room in mpv's queue for
    /// Vela, to be asked again once that queue has been read empty.
    pub(crate) fn waiting_for_room(&self) -> bool {
        self.pending.is_some_and(|pending| pending.unsent)
    }

    /// Ask mpv for the three targets of `goal`. A new change replaces one in
    /// flight; the time it has been held does not start again.
    fn begin(
        &mut self,
        goal: Goal,
        mpv: &impl Targetable,
        surface: &mut impl Describable,
        now: Instant,
    ) {
        self.transitions += 1;
        let number = self.transitions;
        let peak = match goal {
            Goal::Hdr(target) => target.peak.to_string(),
            Goal::Sdr { .. } => String::new(),
        };
        let values = match goal {
            Goal::Hdr(_) => ["pq", "bt.2020", peak.as_str()],
            Goal::Sdr { .. } => ["auto", "auto", "auto"],
        };
        let mut unanswered = [false; 3];
        for (option, (name, value)) in TARGET_OPTIONS.iter().zip(values).enumerate() {
            unanswered[option] = mpv.ask(request(number, option), name, value);
        }
        let unsent = unanswered.contains(&false);
        self.changing_since.get_or_insert(now);
        self.pending = Some(Transition {
            number,
            goal,
            unanswered,
            refused: false,
            unsent,
        });
        if unsent && matches!(goal, Goal::Hdr(_)) {
            // An ask that cannot be sent counts as refused: standard range,
            // at once. The way back is asked after whichever of this
            // change's asks were sent, so it is the one that holds.
            self.refused(mpv, surface, now);
        }
        // A way to standard range with an ask that could not be sent waits,
        // held, to be asked again (see [`drained`]). Ended here, mpv could
        // go on drawing for HDR under an untagged surface: asks made before
        // the queue filled still land.
    }

    /// Every ask of the change in flight has been sent and answered.
    fn finish(
        &mut self,
        mpv: &impl Targetable,
        surface: &mut impl Describable,
        now: Instant,
    ) -> Change {
        let Some(done) = self.pending.take() else {
            return Change::Nothing;
        };
        let change = match done.goal {
            Goal::Hdr(target) if !done.refused => Change::Hdr(target.peak),
            Goal::Hdr(_) => {
                self.refused(mpv, surface, now);
                return Change::Nothing;
            }
            Goal::Sdr { refused } => {
                if done.refused {
                    // Nothing further is tried from here: this is already
                    // the way out.
                    eprintln!("vela: embedded player: mpv did not take back its own target");
                }
                if refused {
                    Change::HdrRefused
                } else {
                    Change::Sdr
                }
            }
        };
        self.changing_since = None;
        self.black_shown = false;
        self.owed = true;
        change
    }

    /// HDR could not be shown: standard range on both sides. The same output
    /// described again will be tried again.
    fn refused(
        &mut self,
        mpv: &impl Targetable,
        surface: &mut impl Describable,
        now: Instant,
    ) {
        self.in_effect = Presentation::Sdr;
        surface.untag();
        self.begin(Goal::Sdr { refused: true }, mpv, surface, now);
    }

    /// What to do with a frame: `new_frame` when mpv has one, `redraw` when
    /// the window has changed under the picture. Called after mpv's events
    /// have been read and before anything is drawn, so a frame is presented
    /// only if every answer had been seen when its drawing began. A frame mpv
    /// hands over is always drawn and reported; it is presented only when no
    /// change of mpv's targets is in flight. A finished change is presented
    /// even when mpv has no new frame, such as a paused picture.
    pub(crate) fn frame(&mut self, new_frame: bool, redraw: bool, now: Instant) -> Frame {
        let Some(since) = self.changing_since else {
            let owed = std::mem::take(&mut self.owed);
            return if new_frame || redraw || owed {
                Frame::Show
            } else {
                Frame::Nothing
            };
        };
        if now.saturating_duration_since(since) < HOLD_LIMIT {
            return if new_frame { Frame::Hold } else { Frame::Nothing };
        }
        let black = !self.black_shown || redraw;
        if !self.black_shown {
            eprintln!("vela: embedded player: mpv has not answered a change of target; showing black");
        }
        self.black_shown = true;
        match (new_frame, black) {
            (true, true) => Frame::HoldAndBlack,
            (true, false) => Frame::Hold,
            (false, true) => Frame::Black,
            (false, false) => Frame::Nothing,
        }
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

    // ---- the output's description, put together (review finding hd-1)

    fn described(reference: u32, max: u32) -> OutputDescription {
        OutputDescription {
            reference,
            min_e4: 0,
            max,
            primaries: Some(PRIMARIES),
        }
    }

    /// Everything the compositor says in answer to one question.
    fn tell(answers: &mut OutputAnswers, question: u32, reference: u32, max: u32) {
        answers.piece(question, Piece::Primaries(PRIMARIES));
        answers.piece(question, Piece::Reference(reference));
        answers.piece(question, Piece::Range { min_e4: 0, max });
        answers.done(question);
    }

    #[test]
    fn an_answer_is_handed_on_once_it_is_complete() {
        let mut answers = OutputAnswers::default();
        assert_eq!(answers.take(), None);
        let question = answers.ask();
        assert!(answers.ready(question));
        answers.piece(question, Piece::Reference(265));
        answers.piece(question, Piece::Range { min_e4: 0, max: 465 });
        assert_eq!(answers.take(), None, "not before the compositor says that is all");
        answers.piece(question, Piece::Primaries(PRIMARIES));
        answers.done(question);
        assert_eq!(answers.take(), Some(described(265, 465)));
        assert_eq!(answers.take(), None, "and once only");
    }

    // The order the reviewer produced: the display changes twice quickly;
    // the compositor acknowledges the second question and only then sends
    // the contents of the first answer.
    #[test]
    fn the_late_tail_of_an_old_answer_does_not_replace_the_new_one() {
        let mut answers = OutputAnswers::default();
        let first = answers.ask();
        assert!(answers.ready(first), "the first answer's contents are asked for");
        let second = answers.ask();
        assert!(answers.ready(second));
        // The first answer's contents, late.
        tell(&mut answers, first, 265, 465);
        assert_eq!(answers.take(), None, "that is about the display as it was");
        // The second answer's contents.
        tell(&mut answers, second, 203, 203);
        assert_eq!(
            answers.take(),
            Some(described(203, 203)),
            "the display as it is now is not lost"
        );
    }

    #[test]
    fn two_answers_arriving_in_turns_do_not_mix() {
        let mut answers = OutputAnswers::default();
        let first = answers.ask();
        answers.ready(first);
        let second = answers.ask();
        answers.ready(second);
        answers.piece(second, Piece::Reference(203));
        answers.piece(first, Piece::Reference(265));
        answers.piece(first, Piece::Range { min_e4: 9, max: 465 });
        answers.piece(second, Piece::Range { min_e4: 0, max: 203 });
        answers.piece(second, Piece::Primaries(PRIMARIES));
        answers.done(first);
        assert_eq!(answers.take(), None);
        answers.done(second);
        assert_eq!(answers.take(), Some(described(203, 203)));
    }

    #[test]
    fn only_the_latest_question_has_its_contents_asked_for() {
        let mut answers = OutputAnswers::default();
        let first = answers.ask();
        let second = answers.ask();
        assert!(!answers.ready(first), "an answer to a question since replaced");
        assert!(answers.ready(second));
        // Pieces before "ready", or with no question asked, go nowhere.
        let mut unasked = OutputAnswers::default();
        tell(&mut unasked, 0, 265, 465);
        tell(&mut unasked, 1, 265, 465);
        assert_eq!(unasked.take(), None);
    }

    #[test]
    fn asking_again_drops_an_answer_nobody_collected() {
        let mut answers = OutputAnswers::default();
        let first = answers.ask();
        answers.ready(first);
        tell(&mut answers, first, 265, 465);
        // Complete, but the display has changed again before it was used.
        let second = answers.ask();
        assert_eq!(answers.take(), None);
        answers.ready(second);
        tell(&mut answers, second, 83, 465);
        assert_eq!(answers.take(), Some(described(83, 465)));
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

    // ---- mpv's side answers later (review finding hd-3)
    //
    // The rig below stands in for both sides the way they really behave:
    // mpv APPLIES each ask at a moment the test chooses, one option at a
    // time, and a draw uses whatever it has applied by then; its answer is
    // read later still. The surface's description is pending until a frame
    // is presented. Every presented frame is checked against both.

    use std::cell::{Cell, RefCell};
    use std::collections::{HashSet, VecDeque};

    const AUTO: [&str; 3] = ["auto", "auto", "auto"];

    /// mpv, as far as Vela's asks go.
    #[derive(Debug, Clone)]
    struct FakeMpv {
        /// Asked and not yet applied: request, option, value.
        asked: RefCell<VecDeque<(u64, usize, String)>>,
        /// What a draw uses.
        applied: [String; 3],
        /// Applied and answered, not yet read by Vela: request, taken.
        answers: VecDeque<(u64, bool)>,
        /// An option mpv refuses when it is not being handed back.
        refuses: Option<&'static str>,
        /// How many more asks fit in mpv's queue for Vela; `None` while there
        /// is room. Like the real queue once it has filled, it takes nothing
        /// more until Vela has read it empty (`Rig::drains`): reading
        /// answers alone does not make room.
        room: Cell<Option<usize>>,
        asks: Cell<usize>,
        given_pq: Cell<bool>,
    }

    impl FakeMpv {
        fn new() -> Self {
            FakeMpv {
                asked: RefCell::new(VecDeque::new()),
                applied: AUTO.map(String::from),
                answers: VecDeque::new(),
                refuses: None,
                room: Cell::new(None),
                asks: Cell::new(0),
                given_pq: Cell::new(false),
            }
        }

        fn refusing(option: &'static str) -> Self {
            FakeMpv {
                refuses: Some(option),
                ..FakeMpv::new()
            }
        }

        /// mpv's queue for Vela fills after `asks` more asks.
        fn fills_after(&self, asks: usize) {
            self.room.set(Some(asks));
        }
    }

    impl Targetable for FakeMpv {
        fn ask(&self, request: u64, option: &str, value: &str) -> bool {
            let asks = self.asks.get() + 1;
            self.asks.set(asks);
            // A mistake that asks again and again fails here rather than
            // hanging the run.
            assert!(asks < 60, "mpv is asked without end");
            if let Some(room) = self.room.get() {
                if room == 0 {
                    return false;
                }
                self.room.set(Some(room - 1));
            }
            let index = TARGET_OPTIONS
                .iter()
                .position(|name| *name == option)
                .expect("one of the three target options");
            if value == "pq" {
                self.given_pq.set(true);
            }
            self.asked
                .borrow_mut()
                .push_back((request, index, value.to_string()));
            true
        }
    }

    /// The video surface. Like the real one, it forgets a description when
    /// a new one is asked for, cannot be tagged without one, and changes
    /// what it shows only when a frame is presented.
    #[derive(Debug, Clone, Default)]
    struct FakeSurface {
        described: Option<HdrTarget>,
        made: bool,
        /// Set or unset and not yet presented.
        pending: Option<Option<HdrTarget>>,
        /// What the compositor takes the presented pixels for.
        committed: Option<HdrTarget>,
        cannot_tag: bool,
    }

    impl Describable for FakeSurface {
        fn describe(&mut self, target: &HdrTarget) {
            self.described = Some(*target);
            self.made = false;
        }
        fn tag(&mut self) -> bool {
            if self.cannot_tag || !self.made {
                return false;
            }
            self.pending = Some(self.described);
            true
        }
        fn untag(&mut self) {
            self.pending = Some(None);
        }
    }

    /// What was last presented.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Presented {
        Black,
        /// mpv's picture, on a surface tagged for this peak or untagged.
        Video(Option<u32>),
    }

    /// One play: what the session does, against fakes of both sides.
    #[derive(Debug, Clone)]
    struct Rig {
        tagging: Tagging,
        mpv: FakeMpv,
        surface: FakeSurface,
        now: Instant,
        presented: Presented,
        refusals: usize,
        /// Whether mpv has a frame at every turn (playing) or none (paused).
        playing: bool,
        /// What happened, for the message when a check fails.
        trail: Vec<String>,
    }

    impl Rig {
        fn new(mpv: FakeMpv, surface: FakeSurface) -> Self {
            let mut rig = Rig {
                tagging: Tagging::new(),
                mpv,
                surface,
                now: Instant::now(),
                presented: Presented::Black,
                refusals: 0,
                playing: true,
                trail: Vec::new(),
            };
            // The play's first picture.
            rig.frame(true);
            assert_eq!(rig.presented, Presented::Video(None));
            rig
        }

        fn plain() -> Self {
            Rig::new(FakeMpv::new(), FakeSurface::default())
        }

        /// The play with HDR at `peak` already settled.
        fn showing(peak: u32) -> Self {
            let mut rig = Rig::plain();
            rig.describes(Presentation::Hdr(target(peak)));
            rig.answers(true);
            rig.settle();
            assert_eq!(rig.frame(false), Frame::Show);
            assert_eq!(rig.presented, Presented::Video(Some(peak)));
            rig
        }

        /// Everything but the trail, which differs between orders that end
        /// in the same state.
        fn key(&self) -> String {
            format!(
                "{:?}{:?}{:?}{:?}{}{}",
                self.tagging, self.mpv, self.surface, self.presented, self.refusals, self.playing
            )
        }

        fn record(&mut self, change: Change) {
            if change == Change::HdrRefused {
                self.refusals += 1;
            }
        }

        fn carry_out(&mut self, step: Step) {
            carry_out(step, &mut self.tagging, &self.mpv, &mut self.surface, self.now);
        }

        /// The compositor describes the output.
        fn describes(&mut self, wanted: Presentation) -> Step {
            self.trail.push(format!("compositor describes {wanted:?}"));
            let step = self.tagging.output_described(wanted);
            self.carry_out(step);
            step
        }

        /// The compositor answers the last request for a description.
        fn answers(&mut self, made: bool) -> Step {
            self.trail.push(format!("compositor answers {made}"));
            let step = if made {
                self.surface.made = true;
                self.tagging.description_ready()
            } else {
                self.tagging.description_failed()
            };
            self.carry_out(step);
            step
        }

        /// mpv applies the oldest ask and queues its answer.
        fn applies(&mut self) -> bool {
            let Some((request, option, value)) = self.mpv.asked.borrow_mut().pop_front() else {
                return false;
            };
            self.trail.push(format!("mpv applies {}={value}", TARGET_OPTIONS[option]));
            let refused = value != "auto" && self.mpv.refuses == Some(TARGET_OPTIONS[option]);
            if !refused {
                self.mpv.applied[option] = value;
            }
            self.mpv.answers.push_back((request, !refused));
            true
        }

        /// Vela reads mpv's queue empty, as every turn of the play's loop
        /// does: every answer waiting, and then nothing, which makes room.
        fn drains(&mut self) {
            self.reads();
            self.trail.push("vela has read mpv's queue empty".to_string());
            self.mpv.room.set(None);
            drained(&mut self.tagging, &self.mpv, &mut self.surface, self.now);
        }

        /// Vela reads every answer that is waiting, with other events still
        /// behind them.
        fn reads(&mut self) {
            self.trail.push("vela reads".to_string());
            while let Some((request, taken)) = self.mpv.answers.pop_front() {
                let change = answered(
                    request,
                    taken,
                    &mut self.tagging,
                    &self.mpv,
                    &mut self.surface,
                    self.now,
                );
                self.record(change);
            }
        }

        /// mpv applies and answers everything, and Vela reads it.
        fn settle(&mut self) {
            while !self.mpv.asked.borrow().is_empty()
                || !self.mpv.answers.is_empty()
                || self.tagging.waiting_for_room()
            {
                while self.applies() {}
                self.drains();
            }
        }

        fn pass(&mut self, time: Duration) {
            self.now += time;
        }

        fn frame(&mut self, new_frame: bool) -> Frame {
            self.frame_with(new_frame, false)
        }

        /// One turn of the play's loop at the frame: decide, then do it.
        fn frame_with(&mut self, new_frame: bool, redraw: bool) -> Frame {
            let decision = self.tagging.frame(new_frame, redraw, self.now);
            self.trail.push(format!("frame (mpv has one: {new_frame}): {decision:?}"));
            // A frame mpv hands over is drawn and reported, always; one is
            // held only when there is one.
            if new_frame {
                assert!(
                    matches!(decision, Frame::Hold | Frame::Show | Frame::HoldAndBlack),
                    "mpv's frame is drawn: {:#?}",
                    self.trail
                );
            } else {
                assert!(
                    !matches!(decision, Frame::Hold | Frame::HoldAndBlack),
                    "nothing to hold: {:#?}",
                    self.trail
                );
            }
            match decision {
                Frame::Nothing | Frame::Hold => {}
                Frame::Show => self.show(),
                Frame::Black | Frame::HoldAndBlack => {
                    self.present();
                    self.presented = Presented::Black;
                }
            }
            decision
        }

        fn present(&mut self) {
            if let Some(description) = self.surface.pending.take() {
                self.surface.committed = description;
            }
        }

        /// mpv's picture is presented: what mpv drew it for must be what the
        /// surface now says it is.
        fn show(&mut self) {
            assert!(
                !self.tagging.changing(),
                "never presented while mpv's side is changing: {:#?}",
                self.trail
            );
            self.present();
            let drawn: Vec<&str> = self.mpv.applied.iter().map(String::as_str).collect();
            match self.surface.committed {
                Some(target) => assert_eq!(
                    drawn,
                    ["pq", "bt.2020", target.peak.to_string().as_str()],
                    "the surface is tagged for {} nits: {:#?}",
                    target.peak,
                    self.trail
                ),
                None => assert_eq!(drawn, AUTO, "the surface is untagged: {:#?}", self.trail),
            }
            self.presented = Presented::Video(self.surface.committed.map(|target| target.peak));
        }
    }

    /// Something the compositor does, in the order a script gives.
    #[derive(Debug, Clone, Copy)]
    enum Compositor {
        Describes(Presentation),
        Answers(bool),
    }

    impl Rig {
        fn act(&mut self, act: Compositor) {
            match act {
                Compositor::Describes(wanted) => {
                    self.describes(wanted);
                }
                Compositor::Answers(made) => {
                    self.answers(made);
                }
            }
        }
    }

    /// Run `script` against `rig` in every order the real pieces allow:
    /// between any two of the compositor's steps mpv may apply any number of
    /// asks and Vela may read any number of answers, or read mpv's queue
    /// empty when a change is waiting for room in it, and a frame is offered
    /// after every single step, once with mpv always having one and once
    /// paused. Every presented frame is checked (`Rig::show`); at every end
    /// nothing is left in flight or pending, and `end` checks the rest.
    /// Returns how many distinct states were reached.
    fn every_order(rig: &Rig, script: &[Compositor], end: &dyn Fn(&Rig)) -> usize {
        let mut seen = HashSet::new();
        for playing in [true, false] {
            let mut rig = rig.clone();
            rig.playing = playing;
            explore(rig, script, &mut seen, &mut HashSet::new(), end);
        }
        seen.len()
    }

    /// `path` holds the states on the way to this one: reaching one of them
    /// again is a play that goes round without end, which no order may do.
    fn explore(
        rig: Rig,
        script: &[Compositor],
        seen: &mut HashSet<String>,
        path: &mut HashSet<String>,
        end: &dyn Fn(&Rig),
    ) {
        let state = format!("{script:?}{}", rig.key());
        assert!(!path.contains(&state), "goes round without end: {:#?}", rig.trail);
        if !seen.insert(state.clone()) {
            return;
        }
        let mut moves = Vec::new();
        if let Some((next, rest)) = script.split_first() {
            let mut moved = rig.clone();
            moved.act(*next);
            moves.push((moved, rest));
        }
        if !rig.mpv.asked.borrow().is_empty() {
            let mut moved = rig.clone();
            moved.applies();
            moves.push((moved, script));
        }
        if !rig.mpv.answers.is_empty() {
            let mut moved = rig.clone();
            moved.reads();
            moves.push((moved, script));
        }
        // mpv's queue read empty, where that changes anything. Until then it
        // stays as full as the test made it, in every order.
        if rig.tagging.waiting_for_room() {
            let mut moved = rig.clone();
            moved.drains();
            moves.push((moved, script));
        }
        if moves.is_empty() {
            let mut rig = rig;
            assert!(!rig.tagging.changing(), "nothing is left in flight: {:#?}", rig.trail);
            // One more turn with nothing new from mpv.
            rig.frame(false);
            assert_eq!(rig.surface.pending, None, "nothing is left unpresented: {:#?}", rig.trail);
            end(&rig);
            return;
        }
        path.insert(state.clone());
        for (mut moved, rest) in moves {
            let playing = moved.playing;
            moved.frame(playing);
            explore(moved, rest, seen, path, end);
        }
        path.remove(&state);
    }

    use Compositor::{Answers, Describes};

    fn hdr_at(peak: u32) -> Compositor {
        Describes(Presentation::Hdr(target(peak)))
    }

    const SDR: Compositor = Describes(Presentation::Sdr);

    /// The end a script must reach: what is presented, with what mpv draws
    /// for, and how many times HDR was reported refused.
    fn ends_at(peak: Option<u32>, refusals: usize) -> impl Fn(&Rig) {
        move |rig: &Rig| {
            assert_eq!(rig.presented, Presented::Video(peak), "{:#?}", rig.trail);
            let drawn: Vec<String> = rig.mpv.applied.to_vec();
            let wanted: Vec<String> = match peak {
                Some(peak) => vec!["pq".into(), "bt.2020".into(), peak.to_string()],
                None => AUTO.map(String::from).to_vec(),
            };
            assert_eq!(drawn, wanted, "{:#?}", rig.trail);
            assert_eq!(rig.refusals, refusals, "HDR reported refused: {:#?}", rig.trail);
            let in_effect = peak.map_or(Presentation::Sdr, |peak| Presentation::Hdr(target(peak)));
            assert_eq!(rig.tagging.in_effect(), in_effect, "{:#?}", rig.trail);
        }
    }

    #[test]
    fn every_presented_frame_agrees_with_its_surface_in_every_order() {
        let scripts: [(&[Compositor], Option<u32>); 7] = [
            (&[hdr_at(356), Answers(true)], Some(356)),
            // The brightness slider moves.
            (&[hdr_at(356), Answers(true), hdr_at(1137), Answers(true)], Some(1137)),
            // HDR switched off.
            (&[hdr_at(356), Answers(true), SDR], None),
            // A new description is refused: the accepted one stays.
            (&[hdr_at(356), Answers(true), hdr_at(1137), Answers(false)], Some(356)),
            // A change of mind before the compositor answers.
            (&[hdr_at(356), hdr_at(1137), Answers(true)], Some(1137)),
            // Off and on again.
            (&[hdr_at(356), Answers(true), SDR, hdr_at(356), Answers(true)], Some(356)),
            // Three changes in a row.
            (
                &[hdr_at(356), Answers(true), hdr_at(1137), Answers(true), SDR],
                None,
            ),
        ];
        for (script, peak) in scripts {
            let states = every_order(&Rig::plain(), script, &ends_at(peak, 0));
            assert!(states > 4 * script.len(), "{script:?} is run in more than one order");
        }
    }

    #[test]
    fn mpv_refusing_any_one_option_leaves_both_sides_in_standard_range_in_every_order() {
        for refused in TARGET_OPTIONS {
            let rig = Rig::new(FakeMpv::refusing(refused), FakeSurface::default());
            every_order(&rig, &[hdr_at(356), Answers(true)], &|end: &Rig| {
                ends_at(None, 1)(end);
                // The same output described again is tried again rather
                // than taken for shown.
                let mut again = end.clone();
                assert_eq!(
                    again.tagging.output_described(Presentation::Hdr(target(356))),
                    Step::Describe(target(356)),
                    "{refused}"
                );
            });
        }
    }

    #[test]
    fn a_refusal_while_hdr_is_showing_takes_both_sides_out_of_it_in_every_order() {
        let mut rig = Rig::showing(356);
        // The next target is one mpv will not take.
        rig.mpv.refuses = Some("target-peak");
        every_order(&rig, &[hdr_at(1137), Answers(true)], &ends_at(None, 1));
    }

    #[test]
    fn a_surface_that_cannot_be_tagged_never_gives_mpv_an_hdr_target() {
        let surface = FakeSurface {
            cannot_tag: true,
            ..FakeSurface::default()
        };
        let rig = Rig::new(FakeMpv::new(), surface);
        every_order(&rig, &[hdr_at(356), Answers(true)], &|end: &Rig| {
            ends_at(None, 1)(end);
            assert!(!end.mpv.given_pq.get(), "not even for a moment");
        });
    }

    // mpv's queue for Vela can fill (with events of every kind), and an ask
    // then cannot be sent. Once it has filled, mpv takes no ask until Vela
    // has read it empty; asks made before it filled are still applied and
    // answered (client.c, `reserve_reply` and `choked`; measured with the
    // shipped library).

    #[test]
    fn an_ask_that_cannot_be_sent_counts_as_refused_in_every_order() {
        // The queue fills before the first, second or third HDR ask.
        for room in 0..TARGET_OPTIONS.len() {
            let rig = Rig::plain();
            rig.mpv.fills_after(room);
            every_order(&rig, &[hdr_at(356), Answers(true)], &ends_at(None, 1));
        }
    }

    #[test]
    fn a_change_none_of_whose_asks_could_be_sent_is_asked_again_once_there_is_room() {
        let mut rig = Rig::plain();
        rig.mpv.fills_after(0);
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        assert_eq!(rig.mpv.asks.get(), 6, "three for HDR, three to take it back");
        assert!(rig.tagging.waiting_for_room(), "standard range is still to be asked for");
        assert!(!rig.tagging.settled());
        assert_eq!(rig.surface.pending, Some(None), "untagged, not yet presented");
        assert_eq!(rig.frame(true), Frame::Hold, "held until mpv's side is known");
        // Reading answers is not reading the queue empty.
        rig.reads();
        assert_eq!(rig.mpv.asks.get(), 6, "nothing is asked into a full queue");
        rig.drains();
        assert_eq!(rig.mpv.asks.get(), 9, "asked again, once there is room");
        assert!(!rig.tagging.waiting_for_room());
        rig.drains();
        assert_eq!(rig.mpv.asks.get(), 9, "and once only");
        rig.settle();
        assert_eq!(rig.frame(false), Frame::Show, "the change is presented");
        ends_at(None, 1)(&rig);
    }

    // The way back to standard range cannot be sent while asks made before
    // the queue filled are still to land: an HDR change in flight (`plain`),
    // or none (`showing`). Ended there, the surface would be untagged while
    // mpv drew for HDR, for the rest of the play.
    #[test]
    fn a_way_back_that_cannot_be_sent_waits_for_room_whatever_is_still_to_land() {
        let hdr_in_flight = || {
            let mut rig = Rig::plain();
            rig.describes(Presentation::Hdr(target(356)));
            rig.answers(true);
            rig
        };
        for room in 0..TARGET_OPTIONS.len() {
            for rig in [hdr_in_flight(), Rig::showing(356)] {
                rig.mpv.fills_after(room);
                every_order(&rig, &[SDR], &ends_at(None, 0));
            }
        }

        // Step by step: the earlier asks land and are read while the queue
        // is still full; nothing is presented and nothing is asked until it
        // has been read empty.
        let mut rig = hdr_in_flight();
        rig.mpv.fills_after(0);
        rig.describes(Presentation::Sdr);
        while rig.applies() {}
        assert_eq!(rig.mpv.applied, ["pq", "bt.2020", "356"]);
        rig.reads();
        assert_eq!(rig.mpv.asks.get(), 6, "three for HDR, three for standard range");
        assert!(rig.tagging.waiting_for_room(), "answers read do not make room");
        assert_eq!(rig.frame(true), Frame::Hold, "mpv draws for HDR; the surface is untagged");
        rig.drains();
        assert_eq!(rig.mpv.asks.get(), 9);
        assert_eq!(rig.frame(true), Frame::Hold);
        rig.settle();
        assert_eq!(rig.frame(true), Frame::Show);
        ends_at(None, 0)(&rig);
    }

    #[test]
    fn answers_to_a_replaced_change_do_not_finish_the_new_one() {
        let mut rig = Rig::showing(356);
        rig.describes(Presentation::Hdr(target(600)));
        rig.answers(true);
        // Replaced before mpv has applied any of it.
        rig.describes(Presentation::Hdr(target(1137)));
        rig.answers(true);
        // mpv applies and answers the first change's three asks; Vela reads
        // them while the second's are not yet applied.
        for _ in 0..3 {
            rig.applies();
        }
        rig.reads();
        assert!(rig.tagging.changing(), "those answers are for the change that was replaced");
        assert_eq!(rig.frame(true), Frame::Hold);
        rig.settle();
        assert_eq!(rig.frame(true), Frame::Show);
        assert_eq!(rig.presented, Presented::Video(Some(1137)));
    }

    // Each ask's request says which option of which change it is, so an
    // answer counts once, for its own option (the part review finding hd-2
    // builds on).
    #[test]
    fn an_answer_counts_once_and_only_for_the_option_it_was_asked_for() {
        let mut rig = Rig::plain();
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        let requests: Vec<u64> = rig.mpv.asked.borrow().iter().map(|ask| ask.0).collect();
        assert_eq!(requests.len(), 3);
        while rig.applies() {}
        // Answers given by hand below, in place of the fake's.
        rig.mpv.answers.clear();
        let mut answer = |request: u64, taken: bool| {
            answered(request, taken, &mut rig.tagging, &rig.mpv, &mut rig.surface, rig.now)
        };
        assert_eq!(answer(requests[0] + 3, true), Change::Nothing, "no fourth option");
        // The first option answered again and again, the last time refused:
        // only its first answer counts.
        assert_eq!(answer(requests[0], true), Change::Nothing);
        assert_eq!(answer(requests[0], true), Change::Nothing);
        assert_eq!(answer(requests[0], false), Change::Nothing);
        // The other two, in the other order.
        assert_eq!(answer(requests[2], true), Change::Nothing, "one option is unanswered");
        assert_eq!(answer(requests[1], true), Change::Hdr(356));
    }

    #[test]
    fn a_frame_drawn_while_mpv_has_applied_some_of_the_three_is_never_presented() {
        // Real mpv applies each option before it answers, one at a time, and
        // a draw in between uses what it has (measured: a frame with only
        // target-trc=pq, about 35% darker).
        let mut rig = Rig::plain();
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        for _ in 0..3 {
            assert!(rig.applies());
            assert_eq!(rig.frame(true), Frame::Hold);
        }
        // All three applied and answered; not read yet.
        assert_eq!(rig.frame(true), Frame::Hold);
        rig.reads();
        assert_eq!(rig.frame(true), Frame::Show);
        assert_eq!(rig.presented, Presented::Video(Some(356)));
    }

    #[test]
    fn a_finished_change_is_presented_even_when_mpv_has_no_new_frame() {
        let mut rig = Rig::plain();
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        assert_eq!(rig.frame(false), Frame::Nothing, "paused, and still changing");
        rig.settle();
        assert_eq!(rig.frame(false), Frame::Show, "the paused picture, redrawn");
        assert_eq!(rig.presented, Presented::Video(Some(356)));
        assert_eq!(rig.frame(false), Frame::Nothing, "once");
    }

    // At the start of a play no frame is decided until set-up is over, so one
    // change can finish there, owing a frame, and the next begin before any
    // frame is decided. What is owed then waits for the newer change too.
    #[test]
    fn a_frame_owed_by_a_finished_change_waits_for_the_change_after_it() {
        let mut rig = Rig::plain();
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        rig.settle();
        rig.describes(Presentation::Hdr(target(1137)));
        rig.answers(true);
        assert!(rig.tagging.changing());
        assert_eq!(rig.frame(false), Frame::Nothing, "owed, but mpv's side is changing");
        assert_eq!(rig.presented, Presented::Video(None));
        rig.settle();
        assert_eq!(rig.frame(false), Frame::Show);
        assert_eq!(rig.presented, Presented::Video(Some(1137)));
    }

    #[test]
    fn a_redraw_is_presented_when_nothing_is_changing_and_waits_while_something_is() {
        let mut rig = Rig::plain();
        assert_eq!(rig.frame_with(false, true), Frame::Show);
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        assert_eq!(rig.frame_with(false, true), Frame::Nothing);
        rig.settle();
        assert_eq!(rig.frame(false), Frame::Show);
    }

    // ---- what is in effect while mpv's side is changing (the goal)

    #[test]
    fn hdr_switched_off_while_mpv_is_still_changing_to_it_is_switched_off() {
        let mut rig = Rig::plain();
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        assert!(rig.tagging.changing());
        assert_eq!(rig.describes(Presentation::Sdr), Step::ShowSdr);
        rig.settle();
        rig.frame(true);
        ends_at(None, 0)(&rig);
    }

    #[test]
    fn the_brightness_moving_back_while_mpv_is_still_changing_is_followed() {
        let mut rig = Rig::showing(356);
        rig.describes(Presentation::Hdr(target(1137)));
        rig.answers(true);
        assert!(rig.tagging.changing());
        assert_eq!(
            rig.describes(Presentation::Hdr(target(356))),
            Step::Describe(target(356)),
            "356 is not what is on its way"
        );
        assert_eq!(rig.answers(true), Step::ShowHdr(target(356)));
        rig.settle();
        rig.frame(true);
        ends_at(Some(356), 0)(&rig);
    }

    #[test]
    fn the_same_output_described_again_while_mpv_is_still_changing_changes_nothing() {
        let mut rig = Rig::plain();
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        assert!(rig.tagging.changing());
        assert_eq!(rig.describes(Presentation::Hdr(target(356))), Step::Nothing);
        rig.settle();
        rig.frame(true);
        ends_at(Some(356), 0)(&rig);
    }

    #[test]
    fn a_play_has_not_settled_while_mpv_is_changing() {
        let mut rig = Rig::plain();
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        assert!(rig.tagging.changing());
        assert!(!rig.tagging.settled(), "mpv has not answered");
        rig.settle();
        assert!(rig.tagging.settled());
    }

    // ---- the limit on holding

    #[test]
    fn a_change_mpv_does_not_answer_is_held_then_black_and_its_picture_never_shown() {
        let mut rig = Rig::plain();
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        rig.pass(HOLD_LIMIT - Duration::from_millis(1));
        assert_eq!(rig.frame(true), Frame::Hold);
        assert_eq!(rig.presented, Presented::Video(None), "the last picture stays");
        rig.pass(Duration::from_millis(1));
        assert_eq!(rig.frame(true), Frame::HoldAndBlack);
        assert_eq!(rig.presented, Presented::Black);
        // mpv's frames are still drawn and reported, and black stays.
        assert_eq!(rig.frame(true), Frame::Hold);
        assert_eq!(rig.frame(false), Frame::Nothing);
        // A resized window gets black of its new size.
        assert_eq!(rig.frame_with(false, true), Frame::Black);
        // mpv answers at last.
        rig.settle();
        assert_eq!(rig.frame(false), Frame::Show);
        assert_eq!(rig.presented, Presented::Video(Some(356)));
        // The next change has the whole limit again...
        rig.describes(Presentation::Hdr(target(1137)));
        rig.answers(true);
        rig.pass(HOLD_LIMIT - Duration::from_millis(1));
        assert_eq!(rig.frame(true), Frame::Hold);
        assert_eq!(rig.presented, Presented::Video(Some(356)));
        // ...and when that runs out too, black is presented again.
        rig.pass(Duration::from_millis(1));
        assert_eq!(rig.frame(true), Frame::HoldAndBlack, "each hold that runs out shows black");
        assert_eq!(rig.presented, Presented::Black);
    }

    // The longest unbroken stretch measured in ordinary use: changes
    // replacing each other for ten seconds, as a dragged brightness slider
    // makes them, held mpv for 667 ms at 24 frames a second (the shipped
    // library; review finding hd-3). A stretch that long is held, not black.
    #[test]
    fn the_longest_stretch_measured_is_held_without_black() {
        let mut rig = Rig::plain();
        let mut peak = 356;
        for turn in 1..=14 {
            peak = if peak == 356 { 1137 } else { 356 };
            rig.describes(Presentation::Hdr(target(peak)));
            rig.answers(true);
            rig.pass(Duration::from_millis(50));
            assert_eq!(rig.frame(true), Frame::Hold, "held {} ms", turn * 50);
        }
        assert_eq!(rig.presented, Presented::Video(None), "the last picture, not black");
        rig.settle();
        assert_eq!(rig.frame(true), Frame::Show);
    }

    #[test]
    fn replacing_a_change_does_not_start_the_hold_again() {
        let mut rig = Rig::plain();
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        rig.pass(HOLD_LIMIT / 2);
        rig.describes(Presentation::Hdr(target(1137)));
        rig.answers(true);
        rig.pass(HOLD_LIMIT / 2);
        assert_eq!(rig.frame(true), Frame::HoldAndBlack, "one limit for the whole stretch");
    }

    #[test]
    fn a_refused_change_and_its_way_back_are_one_stretch() {
        let mut rig = Rig::new(FakeMpv::refusing("target-peak"), FakeSurface::default());
        rig.describes(Presentation::Hdr(target(356)));
        rig.answers(true);
        rig.pass(HOLD_LIMIT / 2);
        while rig.applies() {}
        rig.reads();
        assert!(rig.tagging.changing(), "now changing back to standard range");
        rig.pass(HOLD_LIMIT / 2);
        assert_eq!(rig.frame(true), Frame::HoldAndBlack);
    }

    // ---- against the library Vela ships (review finding hd-3)
    //
    // No fake can say whether libmpv.rs's hand-written table, its reading
    // of mpv's events and its constants are the library's own. These tests
    // load the shipped set and drive a real player through the calls the
    // play makes. They live here, not in libmpv.rs, because this is the one
    // file that names mpv's target options. CI has no such set, so they are ignored there and run by
    // `npm run test:libmpv`, which every full `npm run e2e -- --embedded`
    // run runs first (tests/e2e/README.md). A missing set fails them.

    use super::super::libmpv::{Event, LibMpv, Player, LIBRARY_FILE};
    use std::path::{Path, PathBuf};

    fn shipped_player() -> Player {
        let directory = std::env::var_os("VELA_LIBMPV_DIR")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../.libmpv/out/libmpv")
            });
        let directory = std::fs::canonicalize(&directory)
            .unwrap_or_else(|_| panic!("no libmpv set at {}", directory.display()));
        let lib = LibMpv::load(&directory.join(LIBRARY_FILE))
            .unwrap_or_else(|reason| panic!("the shipped library did not load: {reason}"));
        let player = Player::create(std::sync::Arc::new(lib)).expect("a player");
        // No window, no sound, nobody's configuration, and nothing to play.
        for (name, value) in [
            ("config", "no"),
            ("vo", "null"),
            ("ao", "null"),
            ("idle", "yes"),
            ("terminal", "no"),
        ] {
            player.set_option(name, value).expect(name);
        }
        player.initialize().expect("mpv starts");
        player
    }

    /// The answers to `requests`, in the order they came, waiting at most
    /// five seconds for all of them.
    fn answers(player: &Player, requests: &[u64]) -> Vec<(u64, bool)> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut answers = Vec::new();
        while answers.len() < requests.len() && Instant::now() < deadline {
            match player.wait_for_event(Duration::from_millis(100)) {
                Event::Answer { request, taken } => answers.push((request, taken)),
                Event::None | Event::Other => {}
                Event::Shutdown => panic!("mpv shut down"),
            }
        }
        answers
    }

    #[test]
    #[ignore = "needs the shipped libmpv set: npm run test:libmpv"]
    fn real_library_answers_each_ask_with_its_request_and_takes_the_value() {
        let player = shipped_player();
        assert_eq!(player.next_event(), Event::None, "nothing is waiting yet");
        for (request, option, value) in [
            (41, "target-trc", "pq"),
            (42, "target-prim", "bt.2020"),
            (43, "target-peak", "1137"),
        ] {
            assert!(player.ask(request, option, value), "{option} was sent");
        }
        assert_eq!(
            answers(&player, &[41, 42, 43]),
            [(41, true), (42, true), (43, true)],
            "answered in the order asked, each with its own request"
        );
        // The value arrived as it was given: the pointer was the right one.
        assert_eq!(player.property("target-trc").as_deref(), Some("pq"));
        assert_eq!(player.property("target-prim").as_deref(), Some("bt.2020"));
        assert_eq!(player.property("target-peak").as_deref(), Some("1137"));
        // And handed back.
        for (request, option) in [(44, "target-trc"), (45, "target-prim"), (46, "target-peak")] {
            assert!(player.ask(request, option, "auto"));
        }
        assert_eq!(answers(&player, &[44, 45, 46]), [(44, true), (45, true), (46, true)]);
        assert_eq!(player.property("target-peak").as_deref(), Some("auto"));
    }

    #[test]
    #[ignore = "needs the shipped libmpv set: npm run test:libmpv"]
    fn real_library_answers_a_refused_value_as_refused() {
        let player = shipped_player();
        // Sending succeeds; the refusal is in the answer.
        assert!(player.ask(7, "target-peak", "5"), "below mpv's range of 10 to 10000");
        assert!(player.ask(8, "target-trc", "no-such-curve"));
        assert_eq!(answers(&player, &[7, 8]), [(7, false), (8, false)]);
        assert_eq!(player.property("target-peak").as_deref(), Some("auto"));
    }

    /// A surface that takes every description, so that what is tested is
    /// mpv's side alone.
    struct TakesEverything;

    impl Describable for TakesEverything {
        fn describe(&mut self, _target: &HdrTarget) {}
        fn tag(&mut self) -> bool {
            true
        }
        fn untag(&mut self) {}
    }

    /// Hand mpv's answers to `answered`, and an empty queue to `drained`,
    /// the way a play does, until the change in flight has finished; what it
    /// finished as.
    fn finished(player: &Player, tagging: &mut Tagging, surface: &mut impl Describable) -> Change {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut finished = Change::Nothing;
        while tagging.changing() && Instant::now() < deadline {
            match player.wait_for_event(Duration::from_millis(100)) {
                Event::Answer { request, taken } => {
                    let change = answered(request, taken, tagging, player, surface, Instant::now());
                    if change != Change::Nothing {
                        finished = change;
                    }
                }
                Event::None => drained(tagging, player, surface, Instant::now()),
                Event::Other => {}
                Event::Shutdown => panic!("mpv shut down"),
            }
        }
        assert!(!tagging.changing(), "mpv answered every ask");
        finished
    }

    // What `hdr` itself asks for, by its own option names and values, is
    // what the library takes: a misspelt option would be refused there, and
    // HDR would never be shown on any machine.
    #[test]
    #[ignore = "needs the shipped libmpv set: npm run test:libmpv"]
    fn real_library_takes_the_targets_hdr_asks_for_and_hands_them_back() {
        let player = shipped_player();
        let mut tagging = Tagging::new();
        let step = Step::ShowHdr(target(1137));
        carry_out(step, &mut tagging, &player, &mut TakesEverything, Instant::now());
        assert!(tagging.changing(), "answered later");
        assert_eq!(finished(&player, &mut tagging, &mut TakesEverything), Change::Hdr(1137));
        assert_eq!(player.property("target-trc").as_deref(), Some("pq"));
        assert_eq!(player.property("target-prim").as_deref(), Some("bt.2020"));
        assert_eq!(player.property("target-peak").as_deref(), Some("1137"));

        let step = Step::ShowSdr;
        carry_out(step, &mut tagging, &player, &mut TakesEverything, Instant::now());
        assert!(tagging.changing(), "answered later");
        assert_eq!(finished(&player, &mut tagging, &mut TakesEverything), Change::Sdr);
        for option in ["target-trc", "target-prim", "target-peak"] {
            assert_eq!(player.property(option).as_deref(), Some("auto"), "{option}");
        }
    }

    /// A surface that says whether it is tagged.
    struct Tracked {
        tagged: bool,
    }

    impl Describable for Tracked {
        fn describe(&mut self, _target: &HdrTarget) {}
        fn tag(&mut self) -> bool {
            self.tagged = true;
            true
        }
        fn untag(&mut self) {
            self.tagged = false;
        }
    }

    // mpv's queue for Vela fills while HDR
    // asks are in flight, so the way back to standard range cannot be sent.
    // The real queue, unlike a fake, takes no ask until it has been read
    // empty, and the HDR asks still land; the play must end with mpv and
    // the surface agreeing.
    #[test]
    #[ignore = "needs the shipped libmpv set: npm run test:libmpv"]
    fn real_library_a_way_back_that_cannot_be_sent_is_asked_again_once_the_queue_is_read() {
        let player = shipped_player();
        while player.wait_for_event(Duration::from_millis(50)) != Event::None {}
        let mut tagging = Tagging::new();
        let mut surface = Tracked { tagged: false };
        carry_out(Step::ShowHdr(target(356)), &mut tagging, &player, &mut surface, Instant::now());
        // More events than mpv's queue for Vela holds (1000): each of these
        // is delivered to every client.
        for _ in 0..1100 {
            player.command(&["script-message", "vela-test"]).expect("a script message");
        }
        carry_out(Step::ShowSdr, &mut tagging, &player, &mut surface, Instant::now());
        assert!(tagging.waiting_for_room(), "the queue was full: nothing could be sent");
        assert!(!surface.tagged);
        assert_eq!(finished(&player, &mut tagging, &mut surface), Change::Sdr);
        assert!(!surface.tagged);
        for option in ["target-trc", "target-prim", "target-peak"] {
            assert_eq!(player.property(option).as_deref(), Some("auto"), "{option}");
        }
    }

    #[test]
    #[ignore = "needs the shipped libmpv set: npm run test:libmpv"]
    fn real_library_says_when_it_has_shut_down_and_when_nothing_is_waiting() {
        let player = shipped_player();
        // Drain whatever start-up said.
        while player.wait_for_event(Duration::from_millis(50)) != Event::None {}
        // With nothing waiting, the running play's read returns at once: it
        // is called on every turn of the loop that draws.
        let began = Instant::now();
        for _ in 0..20 {
            assert_eq!(player.next_event(), Event::None);
        }
        assert!(
            began.elapsed() < Duration::from_millis(20),
            "twenty empty reads took {:?}",
            began.elapsed()
        );
        player.command(&["quit"]).expect("quit is a command");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match player.wait_for_event(Duration::from_millis(100)) {
                Event::Shutdown => break,
                Event::None | Event::Other | Event::Answer { .. } => {}
            }
            assert!(Instant::now() < deadline, "mpv says it has shut down");
        }
    }
}
