//! The video surface: a Wayland surface of Vela's own, placed UNDER the
//! window's surface, that mpv's pictures are drawn into.
//!
//! Vela's window is one Wayland surface, owned and drawn by GTK and the web
//! view. The video goes on a second surface made a child of it (a
//! subsurface) and stacked below it, so the page is drawn over the video and
//! the video shows wherever the page is transparent. The child never takes
//! input: every click and key still goes to the window.
//!
//! Both surfaces must be created on the SAME connection to the compositor,
//! the one GTK opened. This module joins that connection as a guest: its
//! objects get an event queue of their own, so GTK's event handling and this
//! module's never see each other's events (proved in Slice 0 of
//! `.agents/plans/embedded-player.md`, A5, by 332 resizes during playback).
//! Everything here except [`VideoSurface::subsurface`] is used from the one
//! thread that owns the surface.

use std::ffi::c_void;
use std::os::fd::{AsRawFd, BorrowedFd};

use wayland_client::backend::{Backend, ObjectId};
use wayland_client::protocol::{
    wl_compositor::WlCompositor, wl_region::WlRegion, wl_registry, wl_subcompositor::WlSubcompositor,
    wl_subsurface::WlSubsurface, wl_surface::WlSurface,
};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::{self, WpFractionalScaleV1},
};
use wayland_protocols::wp::viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter};

/// The compositor's scale for a surface, in 120ths: 120 is 1.0, 204 is 1.7.
const SCALE_ONE: u32 = 120;

#[derive(Default)]
struct State {
    compositor: Option<WlCompositor>,
    subcompositor: Option<WlSubcompositor>,
    viewporter: Option<WpViewporter>,
    fractional_scale: Option<WpFractionalScaleManagerV1>,
    /// The scale the compositor last asked this surface to be drawn at.
    scale_120: Option<u32>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        // Each is bound at the lowest version that has what is used here.
        match interface.as_str() {
            "wl_compositor" => state.compositor = Some(registry.bind(name, version.min(4), qh, ())),
            "wl_subcompositor" => state.subcompositor = Some(registry.bind(name, 1, qh, ())),
            "wp_viewporter" => state.viewporter = Some(registry.bind(name, 1, qh, ())),
            "wp_fractional_scale_manager_v1" => {
                state.fractional_scale = Some(registry.bind(name, 1, qh, ()))
            }
            _ => {}
        }
    }
}

impl Dispatch<WpFractionalScaleV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            state.scale_120 = Some(scale);
        }
    }
}

// These send nothing this module acts on. The surface's own events say which
// outputs it is on, which is not needed to draw into it.
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: WlCompositor);
delegate_noop!(State: WlSubcompositor);
delegate_noop!(State: WlSubsurface);
delegate_noop!(State: WlRegion);
delegate_noop!(State: WpViewporter);
delegate_noop!(State: WpViewport);
delegate_noop!(State: WpFractionalScaleManagerV1);

fn describe(error: impl std::fmt::Display) -> String {
    format!("the connection to the compositor failed ({error})")
}

/// Join GTK's connection as a guest and learn what the compositor offers.
///
/// # Safety
/// `wl_display` must be the live `wl_display` of Vela's window and stay valid
/// for as long as the returned objects are used.
unsafe fn join(wl_display: *mut c_void) -> Result<(Connection, EventQueue<State>, State), String> {
    // SAFETY: the caller's contract. A guest backend never closes the
    // connection; it gives its objects an event queue of their own.
    let backend = unsafe { Backend::from_foreign_display(wl_display.cast()) };
    let conn = Connection::from_backend(backend);
    let mut queue = conn.new_event_queue();
    let mut state = State::default();
    let _registry = conn.display().get_registry(&queue.handle(), ());
    // One round trip delivers the list of globals.
    queue.roundtrip(&mut state).map_err(describe)?;
    Ok((conn, queue, state))
}

/// The first protocol the video surface needs that this compositor lacks, if
/// any. Used to decide availability before a play is started.
///
/// # Safety
/// As for [`VideoSurface::create`].
pub(crate) unsafe fn missing_protocol(wl_display: *mut c_void) -> Result<(), String> {
    let (conn, _queue, state) = unsafe { join(wl_display) }?;
    let missing = if state.compositor.is_none() {
        Some("wl_compositor")
    } else if state.subcompositor.is_none() {
        Some("wl_subcompositor")
    } else if state.viewporter.is_none() {
        Some("wp_viewporter")
    } else {
        None
    };
    release_globals(&state);
    let _ = conn.flush();
    match missing {
        Some(name) => Err(format!("the compositor does not offer {name}")),
        None => Ok(()),
    }
}

/// Tell the compositor the globals are no longer used, for those that can be
/// released at all.
fn release_globals(state: &State) {
    if let Some(subcompositor) = &state.subcompositor {
        subcompositor.destroy();
    }
    if let Some(viewporter) = &state.viewporter {
        viewporter.destroy();
    }
    if let Some(fractional_scale) = &state.fractional_scale {
        fractional_scale.destroy();
    }
}

/// The pixel size of a surface that is `logical` units wide at `scale_120`.
pub(crate) fn pixels(logical: i32, scale_120: u32) -> i32 {
    let exact = i64::from(logical.max(1)) * i64::from(scale_120);
    // Round half up, as the fractional-scale protocol specifies.
    (((exact + i64::from(SCALE_ONE) / 2) / i64::from(SCALE_ONE)).max(1)) as i32
}

pub(crate) struct VideoSurface {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
    surface: WlSurface,
    subsurface: WlSubsurface,
    viewport: WpViewport,
    fractional: Option<WpFractionalScaleV1>,
    /// The size in the window's own units, which is what the page reports.
    logical: (i32, i32),
    /// The scale the current buffers were sized for.
    applied_scale_120: u32,
}

impl VideoSurface {
    /// Create the surface under `parent_surface`, `width` by `height` in the
    /// window's units, at the top left of the window until it is positioned.
    ///
    /// # Safety
    /// `wl_display` and `parent_surface` must be the live `wl_display` and
    /// `wl_surface` of Vela's window and must outlive the returned value.
    pub(crate) unsafe fn create(
        wl_display: *mut c_void,
        parent_surface: *mut c_void,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        let (conn, mut queue, mut state) = unsafe { join(wl_display) }?;
        let qh = queue.handle();
        let (Some(compositor), Some(subcompositor), Some(viewporter)) = (
            state.compositor.clone(),
            state.subcompositor.clone(),
            state.viewporter.clone(),
        ) else {
            return Err("the compositor lacks a protocol the video surface needs".to_string());
        };
        // SAFETY: the caller's contract: a live `wl_surface` on this connection.
        let parent_id = unsafe { ObjectId::from_ptr(WlSurface::interface(), parent_surface.cast()) }
            .map_err(|_| "the window's Wayland surface is not usable".to_string())?;
        let parent = WlSurface::from_id(&conn, parent_id)
            .map_err(|_| "the window's Wayland surface is not usable".to_string())?;

        let surface = compositor.create_surface(&qh, ());
        let viewport = viewporter.get_viewport(&surface, &qh, ());
        let fractional = state
            .fractional_scale
            .as_ref()
            .map(|manager| manager.get_fractional_scale(&surface, &qh, ()));
        // No input region at all: the pointer and the keyboard go to the
        // window above, as if this surface were not there.
        let nothing = compositor.create_region(&qh, ());
        surface.set_input_region(Some(&nothing));
        nothing.destroy();

        let subsurface = subcompositor.get_subsurface(&surface, &parent, &qh, ());
        subsurface.place_below(&parent);
        // Not tied to the window's own redraws: a video frame is shown when
        // it is ready, whether or not the page has anything new.
        subsurface.set_desync();
        // Learn the scale before the first buffer is sized.
        queue.roundtrip(&mut state).map_err(describe)?;

        let mut video = VideoSurface {
            conn,
            queue,
            state,
            surface,
            subsurface,
            viewport,
            fractional,
            logical: (width.max(1), height.max(1)),
            applied_scale_120: SCALE_ONE,
        };
        video.apply();
        Ok(video)
    }

    /// The raw `wl_surface`, for creating the EGL window on it.
    pub(crate) fn wl_surface(&self) -> *mut c_void {
        self.surface.id().as_ptr().cast()
    }

    /// The handle used to move the surface inside the window. Unlike the rest
    /// of this type it may be used from the window's own thread, and has to
    /// be: a position takes effect with the window's next redraw, so it is
    /// sent from the thread that causes that redraw.
    pub(crate) fn subsurface(&self) -> WlSubsurface {
        self.subsurface.clone()
    }

    fn scale_120(&self) -> u32 {
        self.state.scale_120.unwrap_or(SCALE_ONE)
    }

    /// The size the buffers must have, in pixels.
    pub(crate) fn buffer_size(&self) -> (i32, i32) {
        (
            pixels(self.logical.0, self.applied_scale_120),
            pixels(self.logical.1, self.applied_scale_120),
        )
    }

    /// Send the surface's size, as currently known, to the compositor. It
    /// takes effect with the next frame drawn.
    fn apply(&mut self) {
        self.applied_scale_120 = self.scale_120();
        let (width, height) = self.logical;
        // The buffer is in pixels; this says how large it appears.
        self.viewport.set_destination(width, height);
        if let Some(compositor) = &self.state.compositor {
            // Opaque everywhere: the compositor need not draw what is behind.
            let all = compositor.create_region(&self.queue.handle(), ());
            all.add(0, 0, width, height);
            self.surface.set_opaque_region(Some(&all));
            all.destroy();
        }
    }

    /// Change the size, in the window's units. Returns the new pixel size
    /// when the buffers have to change.
    pub(crate) fn resize(&mut self, width: i32, height: i32) -> Option<(i32, i32)> {
        let wanted = (width.max(1), height.max(1));
        if wanted == self.logical && self.scale_120() == self.applied_scale_120 {
            return None;
        }
        self.logical = wanted;
        self.apply();
        Some(self.buffer_size())
    }

    /// Pick up a change of scale from the compositor. Returns the new pixel
    /// size when the buffers have to change.
    pub(crate) fn rescale(&mut self) -> Option<(i32, i32)> {
        if self.scale_120() == self.applied_scale_120 {
            return None;
        }
        self.apply();
        Some(self.buffer_size())
    }

    /// Wait until the compositor has something for this surface, `wake`
    /// becomes readable, or `timeout_ms` passes; then handle what arrived.
    pub(crate) fn wait(&mut self, wake: BorrowedFd<'_>, timeout_ms: i32) -> Result<(), String> {
        self.queue.dispatch_pending(&mut self.state).map_err(describe)?;
        let _ = self.conn.flush();
        // Reading the connection is shared with GTK's thread. This is the
        // protocol for that: announce the intent to read, wait, and then
        // either read or withdraw.
        let guard = loop {
            match self.queue.prepare_read() {
                Some(guard) => break guard,
                None => {
                    self.queue.dispatch_pending(&mut self.state).map_err(describe)?;
                }
            }
        };
        let mut fds = [
            libc::pollfd {
                fd: guard.connection_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: wake.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: two valid descriptors in an array of two.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout_ms) };
        if ready > 0 && fds[0].revents & libc::POLLIN != 0 {
            // "Would block" only means GTK's thread read first; the events
            // are in this surface's queue either way.
            if let Err(error) = guard.read() {
                let would_block = matches!(
                    &error,
                    wayland_client::backend::WaylandError::Io(io)
                        if io.kind() == std::io::ErrorKind::WouldBlock
                );
                if !would_block {
                    return Err(describe(error));
                }
            }
        } else {
            drop(guard);
        }
        self.queue.dispatch_pending(&mut self.state).map_err(describe)?;
        Ok(())
    }

    /// Send everything queued for the compositor now.
    pub(crate) fn flush(&self) {
        let _ = self.conn.flush();
    }
}

impl Drop for VideoSurface {
    fn drop(&mut self) {
        // Removing the subsurface takes the video off screen at once.
        self.viewport.destroy();
        if let Some(fractional) = &self.fractional {
            fractional.destroy();
        }
        self.subsurface.destroy();
        self.surface.destroy();
        release_globals(&self.state);
        let _ = self.conn.flush();
        // Dropping the connection afterwards discards this guest's remaining
        // objects and its event queue. GTK's connection stays open.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_buffer_is_the_logical_size_times_the_scale_rounded_half_up() {
        assert_eq!(pixels(1280, 120), 1280, "scale 1");
        assert_eq!(pixels(1280, 240), 2560, "scale 2");
        // The development machine: 1.7.
        assert_eq!(pixels(1280, 204), 2176);
        assert_eq!(pixels(720, 204), 1224);
        // 1.25 and 1.5, and sizes that do not divide evenly.
        assert_eq!(pixels(1001, 150), 1251, "1251.25 rounds down");
        assert_eq!(pixels(1002, 150), 1253, "1252.5 rounds up");
        assert_eq!(pixels(333, 180), 500, "499.5 rounds up");
        assert_eq!(pixels(7, 204), 12, "11.9 rounds up");
    }

    #[test]
    fn a_buffer_is_never_empty() {
        // A window being dragged to nothing, or a page reporting before it
        // has laid out. A zero-sized buffer is a protocol error.
        assert_eq!(pixels(0, 120), 1);
        assert_eq!(pixels(-40, 204), 2);
        assert_eq!(pixels(1, 60), 1, "half scale of one unit");
    }
}
