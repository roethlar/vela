//! The shared session owns events and quit; main drives Core Animation drawing.
//! Page position reports run independently in player::report over mpv's IPC.

use super::macos_hdr::{allow_restoration, decide, Decision, Range, TargetAccess, Targets};
use super::macos_timing::Timing;
use super::*;

#[derive(Clone, Copy)]
enum Phase {
    Black,
    Targets,
    Layer,
}

struct Switch {
    generation: u64,
    decision: Decision,
    phase: Phase,
    since: Instant,
}

pub(super) struct Live {
    surface: VideoTarget,
    pub(super) player: Player,
    transfer: Option<String>,
    primaries: Option<String>,
    video_registration: u64,
    targets: Targets,
    timing: Timing,
    displayed: Option<Range>,
    switching: Option<Switch>,
    generation: u64,
    last_screen_sample: Instant,
    restorations: std::collections::VecDeque<Instant>,
}

pub(super) fn set_up(
    start: &Start,
    surface: VideoTarget,
    shared: &Arc<Shared>,
) -> Result<Live, String> {
    let player = Player::create(start.lib.clone())?;
    // Same default as external macOS playback. Leave the expert's own
    // configuration alone; Advanced options below can override this default.
    if start.own_config.is_empty() {
        player.set_option("ao", "avfoundation,coreaudio")?;
    }
    configure(&player, start)?;
    player
        .initialize()
        .map_err(|reason| format!("mpv could not start ({reason})"))?;
    if player.property("input-ipc-server").as_deref() != Some(start.ipc_path.as_str())
        || player.property("vo").as_deref() != Some("libmpv")
    {
        return Err("mpv did not take the player's options".into());
    }
    // Shared survives Live and the last callback made during mpv termination.
    unsafe { player.set_wakeup(wake_from_mpv, Arc::as_ptr(shared) as *mut c_void) };
    let video_registration = 1 << 63; // disjoint from target reply/observation IDs
    if !player.observe_video_params(video_registration) {
        return Err("mpv could not report video colour parameters".into());
    }
    let timing = Timing::observe(&player, Instant::now())?;
    surface.drawing.hold(1);
    surface.drawing.start(&player)?;
    let now = Instant::now();
    Ok(Live {
        surface,
        player,
        transfer: None,
        primaries: None,
        video_registration,
        targets: Targets::new(),
        timing,
        displayed: None,
        generation: 1,
        switching: Some(Switch {
            generation: 1,
            decision: decide(None, None, 1.0, false),
            phase: Phase::Black,
            since: now,
        }),
        last_screen_sample: now,
        restorations: std::collections::VecDeque::new(),
    })
}

impl Live {
    fn start_switch(&mut self, decision: Decision) {
        self.generation += 1;
        self.surface.drawing.hold(self.generation);
        self.switching = Some(Switch {
            generation: self.generation,
            decision,
            phase: Phase::Black,
            since: Instant::now(),
        });
    }

    fn colour_turn(&mut self) -> Result<(), &'static str> {
        let now = Instant::now();
        if now.duration_since(self.last_screen_sample) >= Duration::from_millis(250) {
            self.surface.sample_screen(); // queued main-thread read, never a wait
            self.last_screen_sample = now;
        }
        let (potential, _current) = self.surface.headroom();
        let decision = decide(
            self.transfer.as_deref(),
            self.primaries.as_deref(),
            potential,
            self.surface.drawing.float_surface(),
        );
        let requested = self
            .switching
            .as_ref()
            .map(|s| s.decision.range)
            .or(self.displayed);
        if requested != Some(decision.range) {
            self.start_switch(decision);
        } else if self.targets.overridden()
            && self
                .switching
                .as_ref()
                .is_none_or(|s| matches!(s.phase, Phase::Layer))
        {
            if !allow_restoration(&mut self.restorations, now) {
                return Err("mpv repeatedly replaced protected colour targets");
            }
            self.start_switch(decision);
        }

        let Some(switch) = self.switching.as_mut() else {
            return Ok(());
        };
        match switch.phase {
            Phase::Black => {
                // Poll once and return to the event loop. Waiting for a real
                // black draw must not block events, quit, or the IPC reporter.
                if self.surface.drawing.black_ready(switch.generation) {
                    if !self.targets.begin(switch.decision.range, &self.player) {
                        return Err("mpv refused to queue protected colour targets");
                    }
                    switch.phase = Phase::Targets;
                    switch.since = now;
                }
            }
            Phase::Targets => {
                if self.targets.ready() {
                    if !self
                        .surface
                        .set_range(switch.generation, switch.decision.range)
                    {
                        return Err("the main thread could not apply the layer colour space");
                    }
                    switch.phase = Phase::Layer;
                } else if self.targets.settled() && self.targets.overridden() {
                    // Reassert protected values after a load/conditional
                    // profile, as Linux does. Repeated hostile rewrites must
                    // not expose mismatched pixels or spin forever in SDR.
                    if !allow_restoration(&mut self.restorations, now) {
                        return Err("mpv repeatedly replaced protected colour targets");
                    }
                    if !self.targets.begin(switch.decision.range, &self.player) {
                        return Err("mpv refused to restore protected colour targets");
                    }
                }
                if now.duration_since(switch.since) > Duration::from_secs(3) {
                    return Err("mpv did not confirm protected colour targets");
                }
            }
            Phase::Layer => {
                if self.surface.drawing.applied(switch.generation) {
                    if self.displayed != Some(switch.decision.range) {
                        eprintln!(
                            "vela: embedded macOS range: {}; {}",
                            switch.decision.range.label(),
                            switch.decision.reason
                        );
                    }
                    self.displayed = Some(switch.decision.range);
                    self.switching = None;
                }
            }
        }
        Ok(())
    }
}

impl PlayLoop for Live {
    fn wait(&mut self, shared: &Shared, timeout_ms: i32) -> bool {
        shared.wait(if self.switching.is_some() {
            timeout_ms.min(10)
        } else {
            timeout_ms
        });
        !self.surface.drawing.failed()
    }

    fn request_quit(&self) {
        self.player.request_quit();
    }

    fn turn(&mut self, _shared: &Shared, stopping: &Stopping) -> bool {
        loop {
            if stopping.expired(Instant::now()) {
                return false;
            }
            if let Err(reason) = self.timing.check_deadline(Instant::now()) {
                eprintln!("vela: embedded macOS timing protection stopped: {reason}");
                return false;
            }
            match self.player.next_event() {
                Event::None => {
                    if let Err(reason) = self.colour_turn() {
                        eprintln!("vela: embedded macOS colour switch stopped: {reason}");
                        return false;
                    }
                    return true;
                }
                Event::Shutdown => return false,
                Event::StartFile => {
                    self.transfer = None;
                    self.primaries = None;
                    let previous = self.video_registration;
                    self.video_registration += 1;
                    if !self.player.observe_video_params(self.video_registration) {
                        return false;
                    }
                    self.player.unobserve(previous);
                }
                Event::VideoParams {
                    registration,
                    transfer,
                    primaries,
                } => {
                    if registration == self.video_registration {
                        self.transfer = transfer;
                        self.primaries = primaries;
                    }
                }
                Event::Answer { request, taken } => {
                    if let Err(reason) = self.timing.answered(request, taken, &self.player) {
                        eprintln!("vela: embedded macOS timing protection stopped: {reason}");
                        return false;
                    }
                    if !self.targets.answered(request, taken, &self.player) {
                        eprintln!("vela: embedded macOS colour switch stopped: mpv refused a protected target");
                        return false;
                    }
                }
                Event::TargetChanged {
                    registration,
                    option,
                    value,
                } => {
                    if let Err(reason) = self.timing.reported(
                        registration, &option, value.as_deref(), &self.player, Instant::now(),
                    ) {
                        eprintln!("vela: embedded macOS timing protection stopped: {reason}");
                        return false;
                    }
                    self.targets
                        .reported(registration, &option, value.as_deref());
                }
                _ => {}
            }
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Err(error) = self.surface.drawing.shutdown() {
            // Never free a renderer without its current CGL context, or
            // terminate mpv while the renderer still holds it. Keep this
            // thread and its owners alive; the shared leftover policy then
            // refuses new embedded plays and bounds the app's exit wait.
            eprintln!("vela: cannot free the macOS renderer safely: {error}; retaining the play");
            loop {
                std::thread::park();
            }
        }
        // Fields drop next: queue AppKit removal, then terminate mpv.
    }
}
