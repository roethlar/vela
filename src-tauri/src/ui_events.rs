//! One-way signals from background work to the webview. The app handle is
//! published once at setup; before that (or in tests) every emit is a no-op,
//! so background code can signal unconditionally.

use std::sync::OnceLock;

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn set_app_handle(app: tauri::AppHandle) {
    let _ = APP.set(app);
}

/// A background listing revalidation found changes: the frontend should
/// re-fetch the view it is showing. Carries no payload — never paths, URLs,
/// or anything token-bearing.
pub fn listings_updated() {
    if let Some(app) = APP.get() {
        use tauri::Emitter;
        let _ = app.emit("listings-updated", ());
    }
}
