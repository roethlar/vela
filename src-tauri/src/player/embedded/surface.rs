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
//! the one GTK opened. This module joins that connection as a guest
//! ([`Guest`]): its objects get event queues of their own, so GTK's event
//! handling and this module's never see each other's events (proved in Slice
//! 0 of `.agents/plans/embedded-player.md`, A5, by 332 resizes during
//! playback).
//!
//! The connection is joined ONCE, for the life of the process. Joining asks
//! the compositor for its list of globals, and the protocol has no way to
//! give that list (a `wl_registry`) back: the compositor keeps one for every
//! time it was asked, until the connection closes, and this connection is
//! the window's and never closes. Asking per play would grow the compositor
//! by one for every film (review finding ec-9). What a play makes for itself
//! (its surface and what hangs off it) it also destroys.
//!
//! Everything in [`VideoSurface`] except [`VideoSurface::subsurface`] is used
//! from the one thread that owns the surface.

use std::ffi::c_void;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::sync::{Arc, Mutex};

use wayland_client::backend::{Backend, ObjectId};
use wayland_client::protocol::{
    wl_compositor::WlCompositor, wl_region::WlRegion, wl_registry, wl_subcompositor::WlSubcompositor,
    wl_subsurface::WlSubsurface, wl_surface::WlSurface,
};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::color_management::v1::client::{
    wp_color_management_surface_feedback_v1::{self, WpColorManagementSurfaceFeedbackV1},
    wp_color_management_surface_v1::WpColorManagementSurfaceV1,
    wp_color_manager_v1::{self, WpColorManagerV1},
    wp_image_description_creator_params_v1::WpImageDescriptionCreatorParamsV1,
    wp_image_description_info_v1::{self, WpImageDescriptionInfoV1},
    wp_image_description_v1::{self, WpImageDescriptionV1},
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::{self, WpFractionalScaleV1},
};
use wayland_protocols::wp::viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter};

use super::hdr::{Describable, HdrTarget, OutputDescription};

// Values of the colour-management protocol's enumerations that are used
// here, as the protocol numbers them.
const FEATURE_PARAMETRIC: u32 = 1;
const FEATURE_MASTERING_DISPLAY: u32 = 5;
const TRANSFER_PQ: u32 = 11;
const PRIMARIES_BT2020: u32 = 6;
const INTENT_PERCEPTUAL: u32 = 0;

/// Which image description an event is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Described {
    /// The compositor's own description of the output the surface is on.
    Output,
    /// The description Vela asked for, to put on the surface.
    Surface,
}

/// What the compositor's colour manager says it supports.
#[derive(Debug, Default, Clone, Copy)]
struct ColourSupport {
    parametric: bool,
    /// The mastering display's primaries AND its luminance range: the
    /// protocol puts both requests under this one feature.
    mastering: bool,
    pq: bool,
    bt2020: bool,
    perceptual: bool,
}

/// The compositor's scale for a surface, in 120ths: 120 is 1.0, 204 is 1.7.
const SCALE_ONE: u32 = 120;

/// What the compositor offers, as learnt when the connection is joined.
#[derive(Default)]
struct Globals {
    compositor: Option<WlCompositor>,
    subcompositor: Option<WlSubcompositor>,
    viewporter: Option<WpViewporter>,
    fractional_scale: Option<WpFractionalScaleManagerV1>,
    colour_manager: Option<WpColorManagerV1>,
    colour: ColourSupport,
    /// Set once the list has been read. What the compositor announces
    /// afterwards (a monitor plugged in) is of no interest here.
    read: bool,
}

/// What one play hears from the compositor about its own surface.
#[derive(Default)]
struct State {
    /// The scale the compositor last asked this surface to be drawn at.
    scale_120: Option<u32>,
    /// The compositor said its description of the output has changed.
    output_changed: bool,
    /// The output's description while its parts are still arriving.
    arriving: Option<OutputDescription>,
    /// A complete description of the output that nobody has collected yet.
    output: Option<OutputDescription>,
    /// The answer to the last description Vela asked for: made, or refused.
    answer: Option<bool>,
    /// The description last asked for has been made. Only then may it be
    /// put on a surface; before, that is a protocol error.
    made: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Globals {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if state.read {
            return;
        }
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
            "wp_color_manager_v1" => state.colour_manager = Some(registry.bind(name, 1, qh, ())),
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

impl Dispatch<WpColorManagerV1, ()> for Globals {
    fn event(
        state: &mut Self,
        _: &WpColorManagerV1,
        event: wp_color_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wp_color_manager_v1::Event;
        match event {
            Event::SupportedFeature { feature } => match u32::from(feature) {
                FEATURE_PARAMETRIC => state.colour.parametric = true,
                FEATURE_MASTERING_DISPLAY => state.colour.mastering = true,
                _ => {}
            },
            Event::SupportedTfNamed { tf } => {
                state.colour.pq |= u32::from(tf) == TRANSFER_PQ;
            }
            Event::SupportedPrimariesNamed { primaries } => {
                state.colour.bt2020 |= u32::from(primaries) == PRIMARIES_BT2020;
            }
            Event::SupportedIntent { render_intent } => {
                state.colour.perceptual |= u32::from(render_intent) == INTENT_PERCEPTUAL;
            }
            _ => {}
        }
    }
}

impl Dispatch<WpColorManagementSurfaceFeedbackV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpColorManagementSurfaceFeedbackV1,
        event: wp_color_management_surface_feedback_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_color_management_surface_feedback_v1::Event::PreferredChanged { .. } = event {
            state.output_changed = true;
        }
    }
}

impl Dispatch<WpImageDescriptionV1, Described> for State {
    fn event(
        state: &mut Self,
        description: &WpImageDescriptionV1,
        event: wp_image_description_v1::Event,
        which: &Described,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wp_image_description_v1::Event;
        match (which, event) {
            (Described::Output, Event::Ready { .. }) => {
                // Its contents arrive as events on the object this creates.
                state.arriving = Some(OutputDescription {
                    reference: 0,
                    min_e4: 0,
                    max: 0,
                    primaries: None,
                });
                description.get_information(qh, ());
            }
            (Described::Surface, Event::Ready { .. }) => {
                state.answer = Some(true);
                state.made = true;
            }
            (Described::Surface, Event::Failed { .. }) => state.answer = Some(false),
            // A description of the output that could not be made: nothing
            // new is known, and what was known stands.
            _ => {}
        }
    }
}

impl Dispatch<WpImageDescriptionInfoV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpImageDescriptionInfoV1,
        event: wp_image_description_info_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wp_image_description_info_v1::Event;
        let Some(arriving) = state.arriving.as_mut() else {
            return;
        };
        match event {
            Event::Luminances { reference_lum, .. } => arriving.reference = reference_lum,
            Event::TargetLuminance { min_lum, max_lum } => {
                arriving.min_e4 = min_lum;
                arriving.max = max_lum;
            }
            Event::TargetPrimaries {
                r_x,
                r_y,
                g_x,
                g_y,
                b_x,
                b_y,
                w_x,
                w_y,
            } => arriving.primaries = Some([r_x, r_y, g_x, g_y, b_x, b_y, w_x, w_y]),
            Event::Done => state.output = state.arriving.take(),
            // A colour profile file, when one is sent, is closed by being
            // dropped here.
            _ => {}
        }
    }
}

// These send nothing this module acts on. The surface's own events say which
// outputs it is on, which is not needed to draw into it.
delegate_noop!(State: WpColorManagementSurfaceV1);
delegate_noop!(State: WpImageDescriptionCreatorParamsV1);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: WlSubsurface);
delegate_noop!(State: WlRegion);
delegate_noop!(State: WpViewport);
delegate_noop!(Globals: WlCompositor);
delegate_noop!(Globals: WlSubcompositor);
delegate_noop!(Globals: WpViewporter);
delegate_noop!(Globals: WpFractionalScaleManagerV1);

fn describe(error: impl std::fmt::Display) -> String {
    format!("the connection to the compositor failed ({error})")
}

/// The window's connection to the compositor, as the raw pointer GTK owns.
/// GTK keeps the connection for the life of the process, and libwayland,
/// through which alone it is used, may be called from any thread.
#[derive(Clone, Copy)]
pub(crate) struct WlDisplay(pub(crate) *mut c_void);

// SAFETY: see the type's notes: valid for the life of the process, and only
// ever handed to libwayland and EGL, which are thread-safe on it.
unsafe impl Send for WlDisplay {}
unsafe impl Sync for WlDisplay {}

/// Vela's place on the window's connection to the compositor: joined once,
/// kept for the life of the process, shared by every play.
pub(crate) struct Guest {
    display: WlDisplay,
    conn: Connection,
    compositor: WlCompositor,
    subcompositor: WlSubcompositor,
    viewporter: WpViewporter,
    fractional_scale: Option<WpFractionalScaleManagerV1>,
    colour_manager: Option<WpColorManagerV1>,
    colour: ColourSupport,
    /// The queue the globals above were bound on. Nothing more is expected
    /// on it, but what does arrive (the compositor announces every later
    /// global to every registry) must be taken off it or it accumulates.
    announcements: Mutex<(EventQueue<Globals>, Globals)>,
}

impl Guest {
    /// Join GTK's connection as a guest and learn what the compositor
    /// offers. Call once per process; see the module's notes.
    ///
    /// # Safety
    /// `display` must be the live `wl_display` of Vela's window, which
    /// stays valid for the life of the process.
    pub(crate) unsafe fn join(display: WlDisplay) -> Result<Self, String> {
        // SAFETY: the caller's contract. A guest backend never closes the
        // connection; it gives its objects an event queue of their own.
        let backend = unsafe { Backend::from_foreign_display(display.0.cast()) };
        let conn = Connection::from_backend(backend);
        let mut queue = conn.new_event_queue();
        let mut globals = Globals::default();
        let _registry = conn.display().get_registry(&queue.handle(), ());
        // One round trip delivers the list of globals, a second what the
        // colour manager says about itself on being bound.
        queue.roundtrip(&mut globals).map_err(describe)?;
        queue.roundtrip(&mut globals).map_err(describe)?;
        globals.read = true;
        let lacks = |name: &str| format!("the compositor does not offer {name}");
        Ok(Guest {
            display,
            compositor: globals.compositor.take().ok_or_else(|| lacks("wl_compositor"))?,
            subcompositor: globals
                .subcompositor
                .take()
                .ok_or_else(|| lacks("wl_subcompositor"))?,
            viewporter: globals.viewporter.take().ok_or_else(|| lacks("wp_viewporter"))?,
            fractional_scale: globals.fractional_scale.take(),
            colour_manager: globals.colour_manager.take(),
            colour: globals.colour,
            announcements: Mutex::new((queue, globals)),
            conn,
        })
    }

    /// The connection this guest is on, for making an OpenGL window on one
    /// of its surfaces.
    pub(crate) fn display(&self) -> WlDisplay {
        self.display
    }

    /// Take whatever has arrived for the globals' own queue off it.
    fn drain(&self) {
        let mut guard = self.announcements.lock().unwrap_or_else(|e| e.into_inner());
        let (queue, globals) = &mut *guard;
        let _ = queue.dispatch_pending(globals);
    }
}

/// The pixel size of a surface that is `logical` units wide at `scale_120`.
pub(crate) fn pixels(logical: i32, scale_120: u32) -> i32 {
    let exact = i64::from(logical.max(1)) * i64::from(scale_120);
    // Round half up, as the fractional-scale protocol specifies.
    (((exact + i64::from(SCALE_ONE) / 2) / i64::from(SCALE_ONE)).max(1)) as i32
}

pub(crate) struct VideoSurface {
    guest: Arc<Guest>,
    queue: EventQueue<State>,
    state: State,
    surface: WlSurface,
    subsurface: WlSubsurface,
    viewport: WpViewport,
    fractional: Option<WpFractionalScaleV1>,
    /// Tells this surface when the output it is on changes character.
    feedback: Option<WpColorManagementSurfaceFeedbackV1>,
    /// Where an image description is put on this surface. Made on first use.
    colour_surface: Option<WpColorManagementSurfaceV1>,
    /// The compositor's description of the output, while it is being read.
    output_query: Option<WpImageDescriptionV1>,
    /// The description last asked for, to put on the surface.
    described: Option<WpImageDescriptionV1>,
    /// The size in the window's own units, which is what the page reports.
    logical: (i32, i32),
    /// The scale the current buffers were sized for.
    applied_scale_120: u32,
}

impl VideoSurface {
    /// Create the surface under `parent_surface`, `width` by `height` in the
    /// window's units, at the top left of the window until it is positioned.
    ///
    /// This is the one place the window's own surface is touched, and it is
    /// touched only for the length of this call: the new surface is made its
    /// child and the pointer is not kept. So this is called on the window's
    /// own thread, where the window cannot go away while it runs; a pointer
    /// carried to another thread and used later could be to a window that
    /// had been closed in between (review finding ec-1). For the same reason
    /// nothing here waits for the compositor: the window's thread must not.
    /// [`VideoSurface::settle`] does the waiting, on the play's own thread.
    ///
    /// If the window goes away afterwards, the compositor unmaps this
    /// surface; nothing here refers to the window again.
    ///
    /// # Safety
    /// `parent_surface` must be the live `wl_surface` of Vela's window, on
    /// the connection `guest` joined, for the length of the call.
    pub(crate) unsafe fn create(
        guest: &Arc<Guest>,
        parent_surface: *mut c_void,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        guest.drain();
        let queue = guest.conn.new_event_queue();
        let state = State::default();
        let qh = queue.handle();
        // SAFETY: the caller's contract: a live `wl_surface` on this connection.
        let parent_id = unsafe { ObjectId::from_ptr(WlSurface::interface(), parent_surface.cast()) }
            .map_err(|_| "the window's Wayland surface is not usable".to_string())?;
        let parent = WlSurface::from_id(&guest.conn, parent_id)
            .map_err(|_| "the window's Wayland surface is not usable".to_string())?;

        let surface = guest.compositor.create_surface(&qh, ());
        let viewport = guest.viewporter.get_viewport(&surface, &qh, ());
        let fractional = guest
            .fractional_scale
            .as_ref()
            .map(|manager| manager.get_fractional_scale(&surface, &qh, ()));
        // No input region at all: the pointer and the keyboard go to the
        // window above, as if this surface were not there.
        let nothing = guest.compositor.create_region(&qh, ());
        surface.set_input_region(Some(&nothing));
        nothing.destroy();

        let subsurface = guest.subcompositor.get_subsurface(&surface, &parent, &qh, ());
        subsurface.place_below(&parent);
        // Not tied to the window's own redraws: a video frame is shown when
        // it is ready, whether or not the page has anything new.
        subsurface.set_desync();
        let feedback = guest
            .colour_manager
            .as_ref()
            .map(|manager| manager.get_surface_feedback(&surface, &qh, ()));

        Ok(VideoSurface {
            guest: guest.clone(),
            queue,
            state,
            surface,
            subsurface,
            viewport,
            fractional,
            feedback,
            colour_surface: None,
            output_query: None,
            described: None,
            logical: (width.max(1), height.max(1)),
            applied_scale_120: SCALE_ONE,
        })
    }

    /// Learn from the compositor the scale this surface is drawn at, and
    /// size it. Waits for one exchange with the compositor, so it belongs to
    /// the play's own thread; call it once, before the first buffer is made.
    pub(crate) fn settle(&mut self) -> Result<(), String> {
        self.queue.roundtrip(&mut self.state).map_err(describe)?;
        self.apply();
        Ok(())
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
        // Opaque everywhere: the compositor need not draw what is behind.
        let all = self.guest.compositor.create_region(&self.queue.handle(), ());
        all.add(0, 0, width, height);
        self.surface.set_opaque_region(Some(&all));
        all.destroy();
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
        let _ = self.guest.conn.flush();
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
        let _ = self.guest.conn.flush();
    }

    /// Send what is queued and handle everything the compositor has to say
    /// in answer. Blocks for one exchange.
    pub(crate) fn roundtrip(&mut self) -> Result<(), String> {
        self.queue.roundtrip(&mut self.state).map(|_| ()).map_err(describe)
    }

    /// Whether the compositor can be told everything [`VideoSurface::describe`]
    /// and [`VideoSurface::tag`] say: every request and every value they
    /// send is one the compositor has offered. Sending one it has not
    /// offered is a protocol error, which ends the connection, and the
    /// connection is the one Vela's window lives on.
    pub(crate) fn colour_support(&self) -> bool {
        let colour = self.guest.colour;
        self.guest.colour_manager.is_some()
            && self.feedback.is_some()
            && colour.parametric
            && colour.mastering
            && colour.pq
            && colour.bt2020
            && colour.perceptual
    }

    /// Ask the compositor to describe the output this surface is on. The
    /// answer is collected with [`VideoSurface::output_described`].
    pub(crate) fn ask_about_output(&mut self) {
        // The parametric form is the one whose numbers can be read; it may
        // only be asked for where the compositor says it has it.
        let (Some(feedback), true) = (&self.feedback, self.guest.colour.parametric) else {
            return;
        };
        if let Some(previous) = self.output_query.take() {
            previous.destroy();
        }
        self.state.arriving = None;
        self.output_query =
            Some(feedback.get_preferred_parametric(&self.queue.handle(), Described::Output));
    }

    /// Whether the compositor has said, since the last call, that its
    /// description of the output changed.
    pub(crate) fn output_changed(&mut self) -> bool {
        std::mem::take(&mut self.state.output_changed)
    }

    /// A description of the output that has arrived in full since the last
    /// call.
    pub(crate) fn output_described(&mut self) -> Option<OutputDescription> {
        self.state.output.take()
    }

    /// The compositor's answer to the last [`VideoSurface::describe`], once:
    /// `true` when the description was made, `false` when it was refused.
    pub(crate) fn description_answer(&mut self) -> Option<bool> {
        self.state.answer.take()
    }
}

impl Describable for VideoSurface {
    /// Ask the compositor for an image description of `target`: PQ, BT.2020,
    /// with the output's own range as the mastering display. An earlier
    /// request that has not been answered is dropped. The answer is
    /// collected with [`VideoSurface::description_answer`].
    fn describe(&mut self, target: &HdrTarget) {
        if !self.colour_support() {
            return;
        }
        let Some(manager) = self.guest.colour_manager.clone() else {
            return;
        };
        let (Ok(pq), Ok(bt2020)) = (
            wp_color_manager_v1::TransferFunction::try_from(TRANSFER_PQ),
            wp_color_manager_v1::Primaries::try_from(PRIMARIES_BT2020),
        ) else {
            return;
        };
        let qh = self.queue.handle();
        let parameters = manager.create_parametric_creator(&qh, ());
        parameters.set_tf_named(pq);
        parameters.set_primaries_named(bt2020);
        if let Some([r_x, r_y, g_x, g_y, b_x, b_y, w_x, w_y]) = target.primaries {
            parameters.set_mastering_display_primaries(r_x, r_y, g_x, g_y, b_x, b_y, w_x, w_y);
        }
        parameters.set_mastering_luminance(target.min_e4, target.peak);
        parameters.set_max_cll(target.peak);
        if let Some(previous) = self.described.take() {
            previous.destroy();
        }
        self.state.answer = None;
        self.state.made = false;
        // Creating the description consumes the parameters object.
        self.described = Some(parameters.create(&qh, Described::Surface));
    }

    /// Put the description that was made on the surface. It takes effect
    /// with the next frame drawn. Returns whether it was put there.
    fn tag(&mut self) -> bool {
        if !self.colour_support() || !self.state.made {
            return false;
        }
        let (Some(manager), Some(description)) = (&self.guest.colour_manager, &self.described)
        else {
            return false;
        };
        let Ok(perceptual) = wp_color_manager_v1::RenderIntent::try_from(INTENT_PERCEPTUAL) else {
            return false;
        };
        let qh = self.queue.handle();
        let colour_surface = self
            .colour_surface
            .get_or_insert_with(|| manager.get_surface(&self.surface, &qh, ()));
        colour_surface.set_image_description(description, perceptual);
        true
    }

    /// Take the description off the surface again. It takes effect with the
    /// next frame drawn.
    fn untag(&mut self) {
        if let Some(colour_surface) = &self.colour_surface {
            colour_surface.unset_image_description();
        }
    }
}

impl Drop for VideoSurface {
    fn drop(&mut self) {
        for description in [self.described.take(), self.output_query.take()]
            .into_iter()
            .flatten()
        {
            description.destroy();
        }
        if let Some(colour_surface) = &self.colour_surface {
            colour_surface.destroy();
        }
        if let Some(feedback) = &self.feedback {
            feedback.destroy();
        }
        // Removing the subsurface takes the video off screen at once.
        self.viewport.destroy();
        if let Some(fractional) = &self.fractional {
            fractional.destroy();
        }
        self.subsurface.destroy();
        self.surface.destroy();
        let _ = self.guest.conn.flush();
        // Everything this play made is destroyed above, by name: the
        // connection it was made on is the process's and outlives it.
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
