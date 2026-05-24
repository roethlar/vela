// Prevents an extra console window on Windows in release. DO NOT REMOVE.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK's DMABUF renderer crashes under Wayland on the NVIDIA proprietary
    // driver ("Error 71 (Protocol error) dispatching to Wayland display"). Disable
    // it before the webview initializes. Must be set before GTK/WebKit starts.
    // Linux-only: macOS uses WKWebView and Windows uses WebView2, where this var
    // has no effect.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    vela_lib::run()
}
