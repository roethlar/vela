//! Automatic quality: deciding WHEN playback is failing badly enough to be
//! worth interrupting.
//!
//! Detection only. This module never touches mpv, the sources, or config — it
//! consumes observations and returns a verdict, so every threshold in
//! `.agents/plans/server-transcoding.md` can be proven without a player.
//!
//! Stepping is ONE-WAY (owner, 2026-07-25). A tier change means killing mpv and
//! relaunching at the current position — a black flash, an audio gap, a
//! re-buffer. Spending that to rescue broken playback is worth it; spending it
//! on speculation that a better tier might now fit is not, and it invites
//! flapping. So there is no step-up and nothing here looks for one.

// Nothing calls this yet: the detector lands before the sampler and the
// relaunch so its thresholds can be red-proven without a player in the way.
// DELETE this allow in the commit that wires `AutomaticDetector` into the mpv
// sampler — after that, an unused item here is a real finding.
#![allow(dead_code)]

use std::time::Duration;

/// How often playback is sampled. Every window below is expressed in seconds
/// rather than sample counts so the tick can change without silently retuning
/// the thresholds.
pub const SAMPLE_TICK: Duration = Duration::from_secs(2);

/// Both signals are ignored this long after a play starts and after a seek. A
/// filling cache and a burst of drops are normal there — without this, a
/// step-down would fire on essentially every play.
const WARM_UP: Duration = Duration::from_secs(10);

/// After a step-down, ignore both signals this long: the replacement stream has
/// to establish, and its first seconds look exactly like the failure that
/// triggered the step.
const COOLDOWN: Duration = Duration::from_secs(30);

/// A link that is bad throughout must not march the user down the whole ladder.
/// Accepted consequence (owner, 2026-07-25): a genuinely bad link stops two
/// rungs below Original rather than reaching the floor.
const MAX_STEPS_PER_PLAY: u32 = 2;

/// Drop-storm window, and what must be true across it.
const DROP_WINDOW: Duration = Duration::from_secs(10);
/// At 24fps this is >8% of frames for ten seconds — far beyond a seek's hiccup.
const DROP_GROWTH: u64 = 50;
/// Sustained, not one spike: the growth must appear in this many of the
/// window's sample-to-sample deltas.
const DROP_RISING_SAMPLES: usize = 4;

/// Starving-cache window, and what must be true across it.
const CACHE_WINDOW: Duration = Duration::from_secs(15);
/// Less than a second of buffered media means the next stall is imminent.
const CACHE_FLOOR_SECONDS: f64 = 1.0;
const CACHE_STARVED_SAMPLES: usize = 3;
/// The cache legitimately empties at the end of a file — there is nothing left
/// to read. Without this exclusion every complete playthrough would end with a
/// spurious step-down.
const END_OF_FILE_GRACE: Duration = Duration::from_secs(20);

/// One observation of how playback is coping.
#[derive(Clone, Copy, Debug)]
pub struct HealthSample {
    /// Time since this play started, from the sampler's own clock rather than
    /// mpv's position: a stalled player still ages.
    pub at: Duration,
    pub position: Duration,
    /// mpv's `decoder-frame-drop-count`, which is cumulative for the play.
    pub decoder_drops: u64,
    /// mpv's `demuxer-cache-duration`. `None` while mpv has not reported one.
    pub cache_seconds: Option<f64>,
    pub paused: bool,
}

/// Why a step-down was called for. Carried into the log and the OSD notice so a
/// step is never unexplained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepDownReason {
    /// The machine cannot decode this stream fast enough.
    DropStorm,
    /// The link cannot deliver this stream fast enough.
    StarvingCache,
}

/// Watches one play and says when to step down. One instance per play.
pub struct AutomaticDetector {
    /// The file's length, when known. Without it the end-of-file grace cannot
    /// be applied, so the cache signal is held rather than risked.
    duration: Option<Duration>,
    samples: Vec<HealthSample>,
    steps_taken: u32,
    /// Start of the current warm-up or cooldown; signals are ignored until it
    /// has elapsed.
    quiet_until: Duration,
}

impl AutomaticDetector {
    pub fn new(duration: Option<Duration>) -> Self {
        Self {
            duration,
            samples: Vec::new(),
            steps_taken: 0,
            quiet_until: WARM_UP,
        }
    }

    /// A seek restarts the warm-up: the cache is refilling and the decoder is
    /// catching up, which is indistinguishable from the failures below.
    pub fn note_seek(&mut self, at: Duration) {
        self.samples.clear();
        self.quiet_until = at + WARM_UP;
    }

    /// Record that the caller acted on a verdict. Kept separate from `observe`
    /// so a caller that could not carry out the step does not consume one.
    pub fn note_step_down(&mut self, at: Duration) {
        self.steps_taken += 1;
        self.samples.clear();
        self.quiet_until = at + COOLDOWN;
    }

    pub fn steps_taken(&self) -> u32 {
        self.steps_taken
    }

    /// Feed one observation and get a verdict.
    pub fn observe(&mut self, sample: HealthSample) -> Option<StepDownReason> {
        self.samples.push(sample);
        self.forget_before(sample.at.saturating_sub(CACHE_WINDOW.max(DROP_WINDOW)));

        if sample.at < self.quiet_until || self.steps_taken >= MAX_STEPS_PER_PLAY {
            return None;
        }
        // Drops first: the decoder failing is the more specific diagnosis, and a
        // machine that cannot decode also tends to let the cache drift.
        if self.drop_storm(sample.at) {
            return Some(StepDownReason::DropStorm);
        }
        if self.starving_cache(sample.at) {
            return Some(StepDownReason::StarvingCache);
        }
        None
    }

    fn forget_before(&mut self, cutoff: Duration) {
        self.samples.retain(|s| s.at >= cutoff);
    }

    fn window(&self, now: Duration, span: Duration) -> impl Iterator<Item = &HealthSample> {
        let cutoff = now.saturating_sub(span);
        self.samples.iter().filter(move |s| s.at >= cutoff)
    }

    fn drop_storm(&self, now: Duration) -> bool {
        let window: Vec<_> = self.window(now, DROP_WINDOW).collect();
        let (Some(first), Some(last)) = (window.first(), window.last()) else {
            return false;
        };
        let growth = last.decoder_drops.saturating_sub(first.decoder_drops);
        let rising = window
            .windows(2)
            .filter(|pair| pair[1].decoder_drops > pair[0].decoder_drops)
            .count();
        growth >= DROP_GROWTH && rising >= DROP_RISING_SAMPLES
    }

    fn starving_cache(&self, now: Duration) -> bool {
        self.window(now, CACHE_WINDOW)
            .filter(|s| !s.paused && !self.near_end(s.position))
            .filter(|s| s.cache_seconds.is_some_and(|c| c < CACHE_FLOOR_SECONDS))
            .count()
            >= CACHE_STARVED_SAMPLES
    }

    /// Inside the tail of the file, where an empty cache means "finished
    /// reading", not "cannot keep up". Unknown length is treated as near the
    /// end, so an unmeasurable stream never triggers on the cache signal.
    fn near_end(&self, position: Duration) -> bool {
        match self.duration {
            Some(duration) => position + END_OF_FILE_GRACE >= duration,
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILM: Option<Duration> = Some(Duration::from_secs(3600));

    /// Build the sample the sampler would take at `tick` (0-based), with
    /// everything healthy unless a test says otherwise.
    struct Feed {
        detector: AutomaticDetector,
        tick: u32,
        drops: u64,
    }

    impl Feed {
        fn new(duration: Option<Duration>) -> Self {
            Self {
                detector: AutomaticDetector::new(duration),
                tick: 0,
                drops: 0,
            }
        }

        fn at(&self) -> Duration {
            SAMPLE_TICK * self.tick
        }

        /// One tick with an explicit drop increment and cache reading.
        fn step(&mut self, new_drops: u64, cache: f64) -> Option<StepDownReason> {
            self.drops += new_drops;
            let sample = HealthSample {
                at: self.at(),
                // A play that is progressing normally; well clear of the end.
                position: self.at(),
                decoder_drops: self.drops,
                cache_seconds: Some(cache),
                paused: false,
            };
            self.tick += 1;
            self.detector.observe(sample)
        }

        /// Healthy: no new drops, a comfortable cache.
        fn healthy(&mut self) -> Option<StepDownReason> {
            self.step(0, 30.0)
        }

        /// Run the warm-up out with healthy samples so later ticks are live.
        fn finish_warm_up(&mut self) {
            while self.at() < WARM_UP {
                assert_eq!(self.healthy(), None, "warm-up must never trigger");
            }
        }

        fn run(&mut self, ticks: u32, new_drops: u64, cache: f64) -> Option<StepDownReason> {
            let mut verdict = None;
            for _ in 0..ticks {
                verdict = verdict.or(self.step(new_drops, cache));
            }
            verdict
        }
    }

    #[test]
    fn a_sustained_drop_storm_steps_down() {
        let mut feed = Feed::new(FILM);
        feed.finish_warm_up();
        // 15 new drops per 2s tick: >50 across the window, rising every sample.
        assert_eq!(
            feed.run(6, 15, 30.0),
            Some(StepDownReason::DropStorm),
            "sustained decoder drops must step down"
        );
    }

    #[test]
    fn a_starving_cache_steps_down() {
        let mut feed = Feed::new(FILM);
        feed.finish_warm_up();
        assert_eq!(
            feed.run(4, 0, 0.2),
            Some(StepDownReason::StarvingCache),
            "a cache repeatedly under a second must step down"
        );
    }

    #[test]
    fn healthy_playback_never_steps_down() {
        let mut feed = Feed::new(FILM);
        for _ in 0..120 {
            assert_eq!(feed.healthy(), None, "healthy playback must be left alone");
        }
    }

    /// The startup burst is the one every play produces. If it triggered, every
    /// play would step down.
    #[test]
    fn the_startup_burst_is_ignored() {
        let mut feed = Feed::new(FILM);
        // Worse than either threshold, for the whole warm-up.
        while feed.at() < WARM_UP {
            assert_eq!(
                feed.step(40, 0.1),
                None,
                "nothing may trigger during warm-up"
            );
        }
    }

    #[test]
    fn a_seek_restarts_the_warm_up() {
        let mut feed = Feed::new(FILM);
        feed.finish_warm_up();
        let at = feed.at();
        feed.detector.note_seek(at);
        // The same burst that would otherwise trigger, immediately after a seek.
        for _ in 0..4 {
            assert_eq!(feed.step(40, 0.1), None, "a seek's refill must be forgiven");
        }
    }

    /// One spike — a seek, a display change, a momentary stall — is not a storm.
    #[test]
    fn a_single_drop_spike_is_not_a_storm() {
        let mut feed = Feed::new(FILM);
        feed.finish_warm_up();
        assert_eq!(feed.step(500, 30.0), None, "one spike is not sustained");
        assert_eq!(feed.run(5, 0, 30.0), None, "and it must not linger");
    }

    /// The signal that would otherwise fire on every complete playthrough.
    #[test]
    fn an_emptying_cache_at_the_end_of_the_file_is_not_starvation() {
        let mut detector = AutomaticDetector::new(Some(Duration::from_secs(600)));
        for tick in 0..40u32 {
            let at = Duration::from_secs(300) + SAMPLE_TICK * tick;
            let verdict = detector.observe(HealthSample {
                at,
                // Playing out the last ten seconds, where the cache is empty
                // because there is nothing left to read.
                position: Duration::from_secs(595),
                decoder_drops: 0,
                cache_seconds: Some(0.0),
                paused: false,
            });
            assert_eq!(verdict, None, "the tail of a file must not step down");
        }
    }

    #[test]
    fn a_paused_player_is_not_starving() {
        let mut feed = Feed::new(FILM);
        feed.finish_warm_up();
        for _ in 0..10 {
            let at = feed.at();
            feed.tick += 1;
            let verdict = feed.detector.observe(HealthSample {
                at,
                position: at,
                decoder_drops: 0,
                cache_seconds: Some(0.0),
                paused: true,
            });
            assert_eq!(verdict, None, "a paused player is not failing");
        }
    }

    #[test]
    fn a_step_down_is_followed_by_a_cooldown() {
        let mut feed = Feed::new(FILM);
        feed.finish_warm_up();
        assert!(feed.run(6, 15, 30.0).is_some());
        let at = feed.at();
        feed.detector.note_step_down(at);
        // The replacement stream's own first seconds look exactly like failure.
        while feed.at() < at + COOLDOWN {
            assert_eq!(
                feed.step(40, 0.1),
                None,
                "the new stream must be given time to establish"
            );
        }
    }

    #[test]
    fn stepping_stops_at_the_cap() {
        let mut feed = Feed::new(FILM);
        feed.finish_warm_up();
        for step in 0..MAX_STEPS_PER_PLAY {
            let verdict = feed.run(30, 15, 30.0);
            assert!(verdict.is_some(), "step {step} should have been called for");
            let at = feed.at();
            feed.detector.note_step_down(at);
        }
        assert_eq!(feed.detector.steps_taken(), MAX_STEPS_PER_PLAY);
        // Playback still failing, but the ladder walk is over.
        assert_eq!(
            feed.run(60, 40, 0.1),
            None,
            "no more than {MAX_STEPS_PER_PLAY} steps in one play"
        );
    }

    /// A verdict the caller could not act on must not consume one of the two
    /// steps — that is why `note_step_down` is separate from `observe`.
    #[test]
    fn an_unacted_verdict_costs_no_step() {
        let mut feed = Feed::new(FILM);
        feed.finish_warm_up();
        assert!(feed.run(6, 15, 30.0).is_some());
        assert_eq!(feed.detector.steps_taken(), 0);
    }

    /// Without a known length the end-of-file grace cannot be applied, so the
    /// cache signal is held rather than risked on a live or unmeasurable stream.
    #[test]
    fn an_unmeasurable_stream_never_triggers_on_the_cache() {
        let mut feed = Feed::new(None);
        feed.finish_warm_up();
        assert_eq!(feed.run(20, 0, 0.0), None);
        // The drop signal still works there — it needs no length.
        assert_eq!(feed.run(6, 15, 30.0), Some(StepDownReason::DropStorm));
    }
}
