//! The embedded player: mpv's library drawing inside Vela's own window.
//!
//! Linux on Wayland only (decision D5 of
//! `.agents/plans/embedded-player.md`). Experimental and off by default; the
//! external player stays the default and is never removed.
//!
//! How the pieces fit: `startup` decides, before any window exists, whether
//! this run's window can have video under it. `host` answers whether an
//! embedded play can run and starts one. A play is a `session`: one thread
//! owning a `surface` under the window, an OpenGL window on it (`egl`), and
//! an mpv player from the shipped library (`libmpv`) configured through
//! `options`.

mod egl;
mod host;
mod libmpv;
mod options;
mod session;
pub(crate) mod startup;
mod surface;

pub(crate) use host::{Host, Launcher};
pub(crate) use session::{Rect, Session};

/// Turn an address the system's loader returned into a function pointer of
/// type `F`. Refuses, at compile time, an `F` that is not pointer-sized.
///
/// # Safety
/// `address` must be the address of a function whose signature is exactly
/// `F`. Nothing can check that; it is transcribed from the library's header
/// at each place this is called.
unsafe fn function_at<F: Copy>(address: *mut std::ffi::c_void) -> F {
    const { assert!(std::mem::size_of::<F>() == std::mem::size_of::<*mut std::ffi::c_void>()) };
    // SAFETY: the caller's contract, and the size check above.
    unsafe { std::mem::transmute_copy::<*mut std::ffi::c_void, F>(&address) }
}
