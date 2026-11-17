// Prevents an extra console window on Windows in release. DO NOT REMOVE.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // First thing, before anything else exists: decide how this run creates
    // its window. It is "exactly as before" unless the user has chosen the
    // embedded player and nothing known rules it out.
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
    let embedded_window = vela_lib::prepare_startup();

    // WebKitGTK's DMABUF renderer crashes under Wayland on the NVIDIA proprietary
    // driver ("Error 71 (Protocol error) dispatching to Wayland display"). Disable
    // it before the webview initializes. Must be set before GTK/WebKit starts.
    // Linux-only: macOS uses WKWebView and Windows uses WebView2, where this var
    // has no effect.
    //
    // Embedded window mode needs that renderer (a transparent window does not
    // repaint without it), and every start is that mode unless something
    // rules it out (`player/embedded/startup.rs`; owner, 2026-11-17, after
    // Vela's window played on an NVIDIA RTX 5090 with WebKitGTK 2.54). So
    // the renderer is turned off only for a start that cannot be ready; a
    // start that fails is caught by the start-up marker.
    #[cfg(target_os = "linux")]
    if !embedded_window && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    vela_lib::run()
}
