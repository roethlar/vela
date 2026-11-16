//! The shared session owns events and quit; Core Animation owns drawing.

use super::*;

pub(super) struct Live {
    surface: VideoTarget,
    pub(super) player: Player,
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
    surface.drawing.start(&player)?;
    eprintln!("vela: embedded macOS player: standard-range sRGB output (no HDR/EDR)");
    Ok(Live { surface, player })
}

impl PlayLoop for Live {
    fn wait(&mut self, shared: &Shared, timeout_ms: i32) -> bool {
        shared.wait(timeout_ms);
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
            match self.player.next_event() {
                Event::None => return true,
                Event::Shutdown => return false,
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
