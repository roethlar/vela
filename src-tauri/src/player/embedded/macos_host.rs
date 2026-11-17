//! AppKit ownership stays on the main thread. Worker threads carry only an
//! id and the CGL drawing state, never NSView/NSWindow pointers. Preparing a
//! view may wait for main BEFORE the player slot is locked; all later view
//! operations are queued without waiting (including shutdown at app exit).
#![allow(deprecated)] // The approved CAOpenGLLayer host.

use super::{
    macos_display::DisplayQueue,
    macos_geometry::placement,
    macos_gl::{DrawingState, VideoLayer},
    session::{macos_hdr::Range, Rect},
};
use dispatch2::DispatchQueue;
use objc2::{rc::Retained, runtime::AnyClass, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSWindow, NSWindowOrderingMode};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_quartz_core::CATransaction;
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Emitter, Manager};

thread_local! {
    static VIEWS: RefCell<HashMap<u64, NativeVideo>> = RefCell::new(HashMap::new());
}
static NEXT_VIEW: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
struct ScreenState {
    queued: AtomicBool,
    headroom: Mutex<(f64, f64)>, // potential, current; ratios, NOT nits
}

struct NativeVideo {
    video: Retained<NSView>,
    page: Retained<NSView>,
    content: Retained<NSView>,
    layer: Retained<VideoLayer>,
    drawing: Arc<DrawingState>,
    reported: Option<Rect>,
    screen: Arc<ScreenState>,
}

fn web_view(view: &NSView) -> Option<Retained<NSView>> {
    let class = AnyClass::get(c"WKWebView")?;
    if view.isKindOfClass(class) {
        return Some(view.retain());
    }
    view.subviews().iter().find_map(|child| web_view(&child))
}

impl NativeVideo {
    fn create(
        app: &tauri::AppHandle,
        drawing: Arc<DrawingState>,
        reported: Option<Rect>,
        screen: Arc<ScreenState>,
    ) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("AppKit needs the main thread")?;
        let window = app
            .get_webview_window("main")
            .ok_or("Vela's window is not ready")?;
        let raw = window
            .ns_window()
            .map_err(|_| "Vela's native window is not ready")?;
        // SAFETY: main-thread Tauri call above returned its live NSWindow.
        let window =
            unsafe { Retained::retain(raw.cast::<NSWindow>()) }.ok_or("Vela's window is gone")?;
        let content = window
            .contentView()
            .ok_or("Vela's window has no content view")?;
        let page = web_view(&content).ok_or("Vela's page is not ready")?;
        let content = if std::ptr::eq(&*content, &*page) {
            // As in probe M1: retain the page across contentView replacement,
            // and restore its keyboard focus after wrapping it in a container.
            let container = NSView::initWithFrame(NSView::alloc(mtm), content.frame());
            window.setContentView(Some(&container));
            page.setFrame(container.bounds());
            page.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            container.addSubview(&page);
            window.makeFirstResponder(Some(&page));
            container
        } else {
            content
        };
        let parent = unsafe { page.superview() }.ok_or("Vela's page has no parent")?;
        let video = NSView::initWithFrame(NSView::alloc(mtm), page.frame());
        let layer = VideoLayer::new(drawing.clone(), mtm)?;
        video.setLayer(Some(&layer));
        video.setWantsLayer(true);
        parent.addSubview_positioned_relativeTo(&video, NSWindowOrderingMode::Below, Some(&page));
        let native = Self {
            video,
            page,
            content,
            layer,
            drawing,
            reported,
            screen,
        };
        let installed = native.video.layer().ok_or("the video view has no layer")?;
        let expected: &objc2_quartz_core::CALayer = &native.layer;
        if !std::ptr::eq(&*installed, expected) {
            return Err("AppKit replaced the video's OpenGL layer".into());
        }
        native.resize();
        Ok(native)
    }

    fn resize(&self) {
        self.sample_screen();
        let Some(window) = self.page.window() else {
            return;
        };
        let Some(parent) = (unsafe { self.video.superview() }) else {
            return;
        };
        let bounds = self.page.bounds();
        let scale = window.backingScaleFactor();
        let reported = self.reported.unwrap_or(Rect {
            x: 0,
            y: 0,
            width: bounds.size.width.round() as i32,
            height: bounds.size.height.round() as i32,
        });
        let frame = placement(
            (bounds.size.width, bounds.size.height),
            reported,
            self.page.isFlipped(),
            scale,
        );
        let local = NSRect::new(
            NSPoint::new(bounds.origin.x + frame.x, bounds.origin.y + frame.y),
            NSSize::new(frame.width, frame.height),
        );
        // Convert through content coordinates: this handles both flipped and
        // unflipped AppKit parents and a WKWebView nested below the content.
        let in_content = self.page.convertRect_toView(local, Some(&self.content));
        let in_parent = self.content.convertRect_toView(in_content, Some(&parent));
        self.video.setFrame(in_parent);
        self.video
            .setHidden(frame.backing_width <= 0 || frame.backing_height <= 0);
        self.layer.setFrame(self.video.bounds());
        self.layer.setContentsScale(scale);
        self.layer.redraw(self.video.mtm());
    }

    fn sample_screen(&self) {
        // Query the window's screen anew: NSScreen's potential is immutable
        // on an individual object. Do not cache an NSScreen across display
        // reconfiguration. Periodic sampling also catches headroom changes
        // with no move/resize event, including while a video is paused.
        let headroom = self
            .page
            .window()
            .and_then(|w| w.screen())
            .map_or((1.0, 1.0), |screen| {
                (
                    screen.maximumPotentialExtendedDynamicRangeColorComponentValue(),
                    screen.maximumExtendedDynamicRangeColorComponentValue(),
                )
            });
        *self
            .screen
            .headroom
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = headroom;
    }
}

impl Drop for NativeVideo {
    fn drop(&mut self) {
        self.drawing.stop_drawing();
        self.layer.setAsynchronous(false);
        self.video.removeFromSuperview();
        self.video.setLayer(None);
    }
}

/// Non-owning placement address: queued work for an old play cannot touch a
/// replacement, even when its removal and a new start cross in the main queue.
#[derive(Clone)]
pub(super) struct PlacementTarget {
    app: tauri::AppHandle,
    id: u64,
    display_queue: Arc<DisplayQueue>,
}

impl PlacementTarget {
    pub(super) fn request_display(&self) {
        if !self.display_queue.request() {
            return;
        }
        let id = self.id;
        let queue = self.display_queue.clone();
        // Tauri's run_on_main_thread can execute inline on main. mpv can
        // signal during a render API call, so always enqueue asynchronously.
        // Capture only this play's id and queue, never an owning layer pointer.
        DispatchQueue::main().exec_async(move || {
            queue.begin();
            VIEWS.with_borrow(|views| {
                if let Some(view) = views.get(&id) {
                    view.layer.display_now(view.video.mtm());
                }
            });
        });
    }

    pub(super) fn place(&self, reported: Rect) {
        let id = self.id;
        let _ = self.app.run_on_main_thread(move || {
            VIEWS.with_borrow_mut(|views| {
                if let Some(view) = views.get_mut(&id) {
                    view.reported = Some(reported);
                    view.resize();
                }
            })
        });
    }
}

pub(crate) struct Surface {
    target: PlacementTarget,
    pub(super) drawing: Arc<DrawingState>,
    screen: Arc<ScreenState>,
}

impl Surface {
    /// Main-thread only, called by Host::prepare through run_on_main_thread.
    pub(super) fn create(
        app: tauri::AppHandle,
        drawing: Arc<DrawingState>,
        reported: Option<Rect>,
    ) -> Result<Self, String> {
        let screen = Arc::new(ScreenState::default());
        let native = NativeVideo::create(&app, drawing.clone(), reported, screen.clone())?;
        let id = NEXT_VIEW.fetch_add(1, Ordering::Relaxed);
        VIEWS.with_borrow_mut(|views| {
            views.insert(id, native);
        });
        let target = PlacementTarget {
            app,
            id,
            display_queue: Arc::new(DisplayQueue::default()),
        };
        drawing.set_display_target(target.clone());
        Ok(Self {
            target,
            drawing,
            screen,
        })
    }

    pub(super) fn placement_target(&self) -> PlacementTarget {
        self.target.clone()
    }

    pub(super) fn headroom(&self) -> (f64, f64) {
        *self
            .screen
            .headroom
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn sample_screen(&self) {
        if self.screen.queued.swap(true, Ordering::AcqRel) {
            return;
        }
        let id = self.target.id;
        let screen = self.screen.clone();
        if self
            .target
            .app
            .run_on_main_thread(move || {
                VIEWS.with_borrow(|views| {
                    if let Some(view) = views.get(&id) {
                        view.sample_screen();
                    }
                });
                screen.queued.store(false, Ordering::Release);
            })
            .is_err()
        {
            self.screen.queued.store(false, Ordering::Release);
        }
    }

    pub(super) fn set_range(&self, generation: u64, range: Range) -> bool {
        let id = self.target.id;
        self.target
            .app
            .run_on_main_thread(move || {
                VIEWS.with_borrow(|views| {
                    let Some(view) = views.get(&id) else {
                        return;
                    };
                    if !view.drawing.still_holding(generation) {
                        return;
                    }
                    let mtm = MainThreadMarker::new().expect("main-thread dispatch");
                    // No CGL lock or drawing mutex here. Black was flushed before
                    // mpv was changed, and stays black throughout this transaction.
                    // Disable implicit animations so EDR and the colour tag change
                    // together, then submit before letting CA show video again.
                    CATransaction::begin();
                    CATransaction::setDisableActions(true);
                    let result = view.layer.set_range(range, mtm);
                    CATransaction::commit();
                    CATransaction::flush();
                    match result {
                        Ok(()) => {
                            view.drawing.release(generation, &view.layer, mtm);
                        }
                        Err(error) => view.drawing.fail(&error),
                    }
                });
            })
            .is_ok()
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // The renderer was freed by Live before this, or never created on a
        // failed prepare. A closed event loop may refuse the queue; in that
        // case its main-thread registry owns the inert view until exit.
        self.drawing.stop_drawing();
        let id = self.target.id;
        let _ = self.target.app.run_on_main_thread(move || {
            VIEWS.with_borrow_mut(|views| {
                views.remove(&id);
            })
        });
    }
}

/// Tauri's macOS state reflects native fullscreen. Bounds/backing changes
/// also reapply placement so a paused picture follows resize and Retina moves.
pub(crate) fn follow_fullscreen(app: &tauri::AppHandle) {
    use std::sync::atomic::AtomicBool;
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let observed = window.clone();
    let app = app.clone();
    let last = AtomicBool::new(window.is_fullscreen().unwrap_or(false));
    window.on_window_event(move |event| {
        if !matches!(
            event,
            tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::Moved(_)
                | tauri::WindowEvent::Focused(_)
                | tauri::WindowEvent::ScaleFactorChanged { .. }
        ) {
            return;
        }
        let _ = app.run_on_main_thread(|| {
            VIEWS.with_borrow(|views| {
                for view in views.values() {
                    view.resize();
                }
            })
        });
        let Ok(on) = observed.is_fullscreen() else {
            return;
        };
        if last.swap(on, Ordering::SeqCst) != on {
            if !on {
                app.state::<crate::AppState>()
                    .player_fullscreen_held
                    .store(false, Ordering::SeqCst);
            }
            let _ = app.emit("window-fullscreen", serde_json::json!({ "on": on }));
        }
    });
}
