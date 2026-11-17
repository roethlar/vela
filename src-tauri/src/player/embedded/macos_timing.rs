//! Nonblocking renders require zero video lead for the entire play.
//! Unlike colour targets this needs neither a layer change nor a black hold.
use super::macos_hdr::{allow_restoration, TargetAccess};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const OPTION: &str = "video-timing-offset";
// Disjoint from colour target IDs (low bits) and video-params IDs (bit 63).
const FIRST_ID: u64 = 1 << 62;

fn needs_restoration(value: Option<&str>) -> bool {
    value.and_then(|value| value.parse::<f64>().ok()) != Some(0.0)
}

pub(super) struct Timing {
    registration: u64,
    pending: Option<u64>,
    since: Option<Instant>,
    restorations: VecDeque<Instant>,
}

impl Timing {
    pub(super) fn observe(mpv: &impl TargetAccess, now: Instant) -> Result<Self, &'static str> {
        if !mpv.observe(FIRST_ID, OPTION) {
            return Err("mpv could not observe protected video timing");
        }
        Ok(Self {
            registration: FIRST_ID,
            pending: None,
            since: Some(now),
            restorations: VecDeque::new(),
        })
    }

    pub(super) fn reported(
        &mut self,
        registration: u64,
        option: &str,
        value: Option<&str>,
        mpv: &impl TargetAccess,
        now: Instant,
    ) -> Result<(), &'static str> {
        if registration != self.registration || option != OPTION || self.pending.is_some() {
            return Ok(());
        }
        if !needs_restoration(value) {
            self.since = None;
            return Ok(());
        }
        // Independent budget: timing corrections do not count colour switches.
        if !allow_restoration(&mut self.restorations, now) {
            return Err("mpv repeatedly replaced protected video timing");
        }
        let request = self.registration + 1;
        if !mpv.ask(request, OPTION, "0") {
            return Err("mpv refused to queue protected video timing");
        }
        self.pending = Some(request);
        self.since.get_or_insert(now);
        Ok(())
    }

    pub(super) fn answered(
        &mut self,
        request: u64,
        taken: bool,
        mpv: &impl TargetAccess,
    ) -> Result<(), &'static str> {
        if self.pending != Some(request) {
            return Ok(());
        }
        // A conditional profile can replace zero in the same core iteration.
        // A fresh observation forces a report even if the value is unchanged
        // from the last (wrong) report; queued reports under the old ID lose.
        if !taken || !mpv.observe(request, OPTION) {
            return Err("mpv refused protected video timing");
        }
        mpv.unobserve(self.registration);
        self.registration = request;
        self.pending = None;
        Ok(())
    }

    pub(super) fn check_deadline(&self, now: Instant) -> Result<(), &'static str> {
        if self
            .since
            .is_some_and(|since| now.duration_since(since) >= Duration::from_secs(3))
        {
            return Err("mpv did not confirm protected video timing");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct Mpv {
        asks: RefCell<Vec<(u64, String, String)>>,
        observations: RefCell<Vec<(u64, String)>>,
        removals: RefCell<Vec<u64>>,
        refuse: Cell<bool>,
    }
    impl TargetAccess for Mpv {
        fn ask(&self, id: u64, name: &str, value: &str) -> bool {
            self.asks.borrow_mut().push((id, name.into(), value.into()));
            !self.refuse.get()
        }
        fn observe(&self, id: u64, name: &str) -> bool {
            self.observations.borrow_mut().push((id, name.into()));
            !self.refuse.get()
        }
        fn unobserve(&self, id: u64) {
            self.removals.borrow_mut().push(id);
        }
    }

    #[test]
    fn only_a_known_numeric_zero_needs_no_timing_restoration() {
        for value in ["0", "0.000000", "-0", "0e0"] {
            assert!(!needs_restoration(Some(value)), "{value}");
        }
        for value in [
            Some("0.5"),
            Some("-0.5"),
            Some("NaN"),
            Some("inf"),
            Some(""),
            None,
        ] {
            assert!(needs_restoration(value), "{value:?}");
        }
    }

    #[test]
    fn a_profile_override_is_reset_and_confirmed_by_a_fresh_observation() {
        let mpv = Mpv::default();
        let now = Instant::now();
        let mut timing = Timing::observe(&mpv, now).unwrap();
        assert_eq!(*mpv.observations.borrow(), [(FIRST_ID, OPTION.into())]);
        timing
            .reported(FIRST_ID, OPTION, Some("0.5"), &mpv, now)
            .unwrap();
        assert_eq!(
            *mpv.asks.borrow(),
            [(FIRST_ID + 1, OPTION.into(), "0".into())]
        );
        // Coalesced old observations cannot confirm an outstanding repair.
        timing
            .reported(FIRST_ID, OPTION, Some("0"), &mpv, now)
            .unwrap();
        assert!(timing.since.is_some());
        timing.answered(FIRST_ID + 1, true, &mpv).unwrap();
        assert_eq!(
            mpv.observations.borrow().last(),
            Some(&(FIRST_ID + 1, OPTION.into()))
        );
        assert_eq!(*mpv.removals.borrow(), [FIRST_ID]);
        timing
            .reported(FIRST_ID, OPTION, Some("0"), &mpv, now)
            .unwrap();
        assert!(timing.since.is_some(), "stale zero is not confirmation");
        timing
            .reported(FIRST_ID + 1, OPTION, Some("0.000000"), &mpv, now)
            .unwrap();
        assert!(timing.since.is_none());
        timing
            .reported(FIRST_ID + 1, OPTION, Some("0.5"), &mpv, now)
            .unwrap();
        assert_eq!(mpv.asks.borrow().len(), 2, "protect the whole play");
    }

    #[test]
    fn unrelated_events_cannot_change_timing() {
        let mpv = Mpv::default();
        let now = Instant::now();
        let mut timing = Timing::observe(&mpv, now).unwrap();
        timing.reported(8, OPTION, Some("0.5"), &mpv, now).unwrap();
        timing
            .reported(FIRST_ID, "other", Some("0.5"), &mpv, now)
            .unwrap();
        timing.answered(8, false, &mpv).unwrap();
        assert!(mpv.asks.borrow().is_empty());
        assert_eq!(mpv.observations.borrow().len(), 1);
    }

    #[test]
    fn refusals_and_missing_confirmation_fail_closed() {
        let mpv = Mpv::default();
        let now = Instant::now();
        mpv.refuse.set(true);
        assert!(Timing::observe(&mpv, now).is_err());
        mpv.refuse.set(false);
        let mut timing = Timing::observe(&mpv, now).unwrap();
        assert!(timing.check_deadline(now + Duration::from_secs(3)).is_err());
        assert!(timing.check_deadline(now + Duration::from_secs(2)).is_ok());
        mpv.refuse.set(true);
        assert!(timing
            .reported(FIRST_ID, OPTION, Some("0.5"), &mpv, now)
            .is_err());
        mpv.refuse.set(false);
        timing.reported(FIRST_ID, OPTION, None, &mpv, now).unwrap();
        assert!(timing.answered(FIRST_ID + 1, false, &mpv).is_err());
        mpv.refuse.set(true);
        assert!(timing.answered(FIRST_ID + 1, true, &mpv).is_err());
    }

    #[test]
    fn a_profile_that_keeps_overriding_timing_is_bounded_without_a_black_hold() {
        let mpv = Mpv::default();
        let now = Instant::now();
        let mut timing = Timing::observe(&mpv, now).unwrap();
        for i in 0..5 {
            timing
                .reported(FIRST_ID + i, OPTION, Some("0.5"), &mpv, now)
                .unwrap();
            timing.answered(FIRST_ID + i + 1, true, &mpv).unwrap();
        }
        assert!(timing
            .reported(FIRST_ID + 5, OPTION, Some("0.5"), &mpv, now)
            .is_err());
        assert_eq!(mpv.asks.borrow().len(), 5);
    }
}
