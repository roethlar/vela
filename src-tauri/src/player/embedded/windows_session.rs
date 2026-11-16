//! mpv owns the D3D11 child and its swap chain. Session still owns the play.

use super::super::options;
use super::*;

pub(super) struct Live {
    pub(super) player: Player,
}

/// Windows draws and paces frames on mpv's threads, with no render context
/// and no Wayland colour targets. The same PlayLoop owns events and quit.
pub(super) fn set_up(
    start: &Start,
    wid: VideoTarget,
    shared: &Arc<Shared>,
) -> Result<Live, String> {
    let player = Player::create(start.lib.clone())?;
    configure(&player, start)?;
    // These native options all have client-API spellings (unlike script and
    // append options in the launch), so they need no configuration include.
    for arg in options::windows_window_options(wid) {
        let crate::player::launch::LaunchArg::Option {
            name,
            value: Some(value),
        } = arg
        else {
            unreachable!("only native window options")
        };
        player.set_option(name, &value)?;
    }
    let wid = wid.to_string();
    player
        .initialize()
        .map_err(|reason| format!("mpv could not start ({reason})"))?;
    if player.property("input-ipc-server").as_deref() != Some(start.ipc_path.as_str())
        || player.property("vo").as_deref() != Some("gpu-next")
        || player.property("wid").as_deref() != Some(wid.as_str())
    {
        return Err("mpv did not take the player's options".into());
    }
    // SAFETY: Shared outlives Live and thus mpv's last callback, as on Linux.
    unsafe { player.set_wakeup(wake_from_mpv, Arc::as_ptr(shared) as *mut c_void) };
    Ok(Live { player })
}

impl PlayLoop for Live {
    fn wait(&mut self, shared: &Shared, timeout_ms: i32) -> bool {
        shared.wait(timeout_ms);
        true
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
