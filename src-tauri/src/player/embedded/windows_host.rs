//! The native Windows parent and fullscreen notifications.

use super::*;

/// HWND is an opaque integer to mpv (`wid`), never dereferenced in Rust.
/// Read on the window thread before locking the player slot. mpv notices
/// parent destruction; the shared stop/leftover handling owns its lifetime.
pub(super) fn window_id(app: &tauri::AppHandle) -> Result<usize, EmbeddedUnavailable> {
    let window = app
        .get_webview_window("main")
        .ok_or(EmbeddedUnavailable::WindowNotReady)?;
    window
        .hwnd()
        .map(|hwnd| hwnd.0 as usize)
        .map_err(|_| EmbeddedUnavailable::WindowNotReady)
}

/// Windows has no compositor fullscreen event like GTK's. Follow Tauri's
/// native window state when its bounds/focus change; mpv's parent hook fits
/// its child to those bounds, without a separate fullscreen command to mpv.
pub(crate) fn follow_fullscreen(app: &tauri::AppHandle) {
    use std::sync::atomic::{AtomicBool, Ordering};
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
        ) {
            return;
        }
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
