//! AppKit ownership stays on the main thread. Worker threads carry only an
//! id and the CGL drawing state, never NSView/NSWindow pointers. Preparing a
//! view may wait for main BEFORE the player slot is locked; all later view
//! operations are queued without waiting (including shutdown at app exit).
#![allow(deprecated)] // The approved CAOpenGLLayer host.

use super::{
    macos_geometry::placement,
    macos_gl::{DrawingState, VideoLayer},
    session::Rect,
};
use objc2::{rc::Retained, runtime::AnyClass, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSWindow, NSWindowOrderingMode};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tauri::{Emitter, Manager};

thread_local! {
    static VIEWS: RefCell<HashMap<u64, NativeVideo>> = RefCell::new(HashMap::new());
}
static NEXT_VIEW: AtomicU64 = AtomicU64::new(1);

struct NativeVideo {
    video: Retained<NSView>,
    page: Retained<NSView>,
    content: Retained<NSView>,
    layer: Retained<VideoLayer>,
    drawing: Arc<DrawingState>,
    reported: Option<Rect>,
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
        let layer = VideoLayer::new(drawing.clone());
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
        self.drawing.redraw();
        self.layer.setNeedsDisplay();
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
}

impl PlacementTarget {
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
}

impl Surface {
    /// Main-thread only, called by Host::prepare through run_on_main_thread.
    pub(super) fn create(
        app: tauri::AppHandle,
        drawing: Arc<DrawingState>,
        reported: Option<Rect>,
    ) -> Result<Self, String> {
        let native = NativeVideo::create(&app, drawing.clone(), reported)?;
        let id = NEXT_VIEW.fetch_add(1, Ordering::Relaxed);
        VIEWS.with_borrow_mut(|views| {
            views.insert(id, native);
        });
        Ok(Self {
            target: PlacementTarget { app, id },
            drawing,
        })
    }

    pub(super) fn placement_target(&self) -> PlacementTarget {
        self.target.clone()
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
