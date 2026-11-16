//! The embedded player: mpv's library drawing inside Vela's own window.
//!
//! Linux draws through the render API into a Wayland/EGL surface; Windows
//! gives mpv the main window's HWND and lets gpu-next own its D3D11 child.
//! macOS uses the render API in a CGL layer below WKWebView. All share the
//! startup recovery, launch, session and IPC lifecycle.

#[cfg(target_os = "linux")]
mod egl;
#[cfg(target_os = "linux")]
mod hdr;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
mod host;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
mod libmpv;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
mod options;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
mod session;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod startup;
#[cfg(target_os = "linux")]
mod surface;

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) use host::{follow_fullscreen, Host, Launcher};
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use session::Rect;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) use session::{wait_for_unfinished_plays, Session};

/// Turn an address the system's loader returned into a function pointer of
/// type `F`. Refuses, at compile time, an `F` that is not pointer-sized.
///
/// # Safety
/// `address` must be the address of a function whose signature is exactly
/// `F`. Nothing can check that; it is transcribed from the library's header
/// at each place this is called.
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
unsafe fn function_at<F: Copy>(address: *mut std::ffi::c_void) -> F {
    const { assert!(std::mem::size_of::<F>() == std::mem::size_of::<*mut std::ffi::c_void>()) };
    // SAFETY: the caller's contract, and the size check above.
    unsafe { std::mem::transmute_copy::<*mut std::ffi::c_void, F>(&address) }
}

#[cfg(target_os = "macos")]
mod macos_host;
#[cfg(target_os = "macos")]
mod macos_gl;
#[cfg(any(target_os = "macos", test))]
mod macos_geometry;
