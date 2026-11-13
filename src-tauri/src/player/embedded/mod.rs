//! The embedded player: mpv's library drawing inside Vela's own window.
//!
//! Linux on Wayland only (decision D5 of
//! `.agents/plans/embedded-player.md`). Experimental and off by default; the
//! external player stays the default and is never removed.
#![allow(dead_code)] // TEMPORARY while the module is being wired; see repo-guidance, Guard discipline.

pub(crate) mod libmpv;
pub(crate) mod options;
