//! Display-request rules, independent of AppKit so Linux tests exercise the
//! same coalescing and colour-switch signals used by the macOS layer.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Default)]
pub(super) struct DisplayQueue {
    queued: AtomicBool,
}

impl DisplayQueue {
    /// Only the first signal queues work until main begins handling it.
    pub(super) fn request(&self) -> bool {
        !self.queued.swap(true, Ordering::AcqRel)
    }

    /// Clear BEFORE displaying the layer: an update arriving during that
    /// operation must be able to queue another display request.
    pub(super) fn begin(&self) {
        self.queued.store(false, Ordering::Release);
    }
}

/// A display pass must consume a pending mpv frame even if CA declines to
/// draw (for example, a hidden or zero-sized layer). mpv's update flag is
/// destructive, so canDraw's observation stays latched until render/skip.
#[derive(Default)]
pub(super) struct DisplayFrame {
    pending: bool,
    needs_flip: bool,
}

impl DisplayFrame {
    pub(super) fn begin_display(&mut self) {
        self.needs_flip = true;
    }

    pub(super) fn update(&mut self, frame: bool) {
        self.pending |= frame;
    }

    pub(super) fn pending(&self) -> bool {
        self.pending
    }

    pub(super) fn needs_flip(&self) -> bool {
        self.needs_flip
    }

    pub(super) fn needs_skip(&self) -> bool {
        self.needs_flip && self.pending
    }

    pub(super) fn consumed(&mut self) {
        self.pending = false;
        self.needs_flip = false;
    }
}

pub(super) struct DrawingSignals {
    pub(super) redraw: AtomicBool,
    pub(super) held: AtomicU64,
    applied: AtomicU64,
}

impl DrawingSignals {
    pub(super) fn new() -> Self {
        Self {
            redraw: AtomicBool::new(true),
            held: AtomicU64::new(0),
            applied: AtomicU64::new(0),
        }
    }

    pub(super) fn redraw(&self, request_display: impl FnOnce()) {
        self.redraw.store(true, Ordering::Release);
        request_display();
    }

    pub(super) fn hold(&self, generation: u64, request_display: impl FnOnce()) {
        self.held.store(generation, Ordering::Release);
        self.redraw(request_display);
    }

    pub(super) fn release(&self, generation: u64, request_display: impl FnOnce()) {
        if self
            .held
            .compare_exchange(generation, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.applied.store(generation, Ordering::Release);
            self.redraw(request_display);
        }
    }

    pub(super) fn applied(&self, generation: u64) -> bool {
        self.applied.load(Ordering::Acquire) == generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn a_frame_seen_by_can_draw_is_skipped_if_ca_does_not_draw() {
        let mut frame = DisplayFrame::default();
        frame.begin_display();
        assert!(!frame.needs_skip(), "an idle display must not render");
        frame.update(true);
        frame.update(false); // canDraw may read the destructive flag twice
        assert!(frame.pending());
        assert!(frame.needs_skip());
        frame.consumed(); // successful skip
        assert!(!frame.pending());
        assert!(!frame.needs_skip());
    }

    #[test]
    fn a_frame_is_skipped_even_when_ca_never_calls_can_draw() {
        let mut frame = DisplayFrame::default();
        frame.begin_display();
        assert!(frame.needs_flip());
        // The fallback itself reads mpv's update flag after display/flush.
        frame.update(true);
        assert!(frame.needs_skip());
    }

    #[test]
    fn a_draw_consumes_the_pass_and_the_next_update_gets_its_own_display() {
        let mut frame = DisplayFrame::default();
        frame.begin_display();
        frame.update(true);
        frame.consumed(); // successful draw
        assert!(!frame.pending());
        assert!(!frame.needs_flip());
        frame.update(true); // arrived after the draw: don't discard it here
        assert!(!frame.needs_skip());
        frame.begin_display();
        assert!(frame.needs_skip());
    }

    #[test]
    fn update_signals_coalesce_until_main_begins_the_task() {
        let queue = DisplayQueue::default();
        assert!(queue.request());
        for _ in 0..100 {
            assert!(!queue.request());
        }
        queue.begin();
        // This update arrives while the preceding task displays the layer.
        assert!(queue.request());
        assert!(!queue.request());
        queue.begin();
        assert!(queue.request());
    }

    #[test]
    fn concurrent_updates_queue_only_one_task() {
        let queue = DisplayQueue::default();
        let barrier = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            let tasks: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        queue.request()
                    })
                })
                .collect();
            assert_eq!(
                tasks
                    .into_iter()
                    .map(|t| t.join().unwrap())
                    .filter(|queued| *queued)
                    .count(),
                1
            );
        });
    }

    fn idle() -> DrawingSignals {
        let signals = DrawingSignals::new();
        assert!(signals.redraw.swap(false, Ordering::AcqRel));
        signals
    }

    #[test]
    fn redraw_requests_one_display_even_without_a_new_frame() {
        let signals = idle();
        let displays = Cell::new(0);
        signals.redraw(|| {
            assert!(signals.redraw.load(Ordering::Acquire));
            displays.set(displays.get() + 1);
        });
        assert_eq!(displays.get(), 1);
        assert!(signals.redraw.swap(false, Ordering::AcqRel));
        assert!(!signals.redraw.swap(false, Ordering::AcqRel));
    }

    #[test]
    fn hold_requests_a_display_to_acknowledge_black() {
        let signals = idle();
        let displays = Cell::new(0);
        signals.hold(7, || {
            assert_eq!(signals.held.load(Ordering::Acquire), 7);
            assert!(signals.redraw.load(Ordering::Acquire));
            displays.set(displays.get() + 1);
        });
        assert_eq!(displays.get(), 1);
        assert!(!signals.applied(7));
    }

    #[test]
    fn release_requests_a_display_to_show_the_picture_again() {
        let signals = idle();
        signals.hold(7, || {});
        signals.redraw.store(false, Ordering::Release);
        let displays = Cell::new(0);
        signals.release(7, || {
            assert_eq!(signals.held.load(Ordering::Acquire), 0);
            assert!(signals.applied(7));
            assert!(signals.redraw.load(Ordering::Acquire));
            displays.set(displays.get() + 1);
        });
        assert_eq!(displays.get(), 1);
    }

    #[test]
    fn an_obsolete_release_cannot_release_or_redraw_a_newer_hold() {
        let signals = idle();
        signals.hold(8, || {});
        signals.redraw.store(false, Ordering::Release);
        signals.release(7, || panic!("obsolete release requested display"));
        assert_eq!(signals.held.load(Ordering::Acquire), 8);
        assert!(!signals.applied(7));
        assert!(!signals.redraw.load(Ordering::Acquire));
    }
}
