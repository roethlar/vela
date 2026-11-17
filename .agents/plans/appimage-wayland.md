# Plan: the Linux AppImage starts Vela on Wayland

Status: APPROVED by the owner's ruling of 2026-11-17 ("do not drop the
appimage if it can be fixed ... just do it correctly";
`.agents/decisions.md`, that date). In progress; `.agents/state.md` says
how far it has landed.

## The defects, found 2026-11-17

1. Tauri builds the AppImage with its fork of linuxdeploy's GTK plugin,
   whose AppRun hook exports `GDK_BACKEND=x11` unconditionally
   (`linuxdeploy-plugin-gtk.sh` line 246, citing tauri-apps/tauri#8541), and
   tauri-cli 2.11.4 offers no way to turn it off. Every AppImage start went
   to X11, which Vela does not support in any way.
2. With that line removed, WebKit's web process inside the AppImage aborts:
   "Could not create default EGL display: EGL_BAD_PARAMETER" (core dumps of
   `WebKitWebProcess`, on this AMD machine under a headless weston; on the
   owner's NVIDIA machine the page was lost two seconds into a play). Proven
   cause: the AppImage bundles the build machine's `libwayland-client`,
   `-egl`, `-cursor` and `-server` (Ubuntu 22.04). With only those removed
   from an unpacked copy, so that the host's are used, the AppImage played
   in Vela's window, its clock advancing 0:02 to 0:19 over 20 s
   (`scratchpad/appimg/probe-play.mjs`).

## The fix

- `packaging/appimage/fix-appimage.sh`: unpack the AppImage, delete the
  hook's `GDK_BACKEND` line, remove `usr/lib/libwayland-*`, pack it again in
  place with appimagetool 1.9.1 and the type2 runtime 20251108, both pinned
  by SHA-256. Wayland's client libraries, like libGL and libEGL, must match
  the host's graphics stack, so they come from the host.
- `.github/workflows/release.yml` ("Make the AppImage start on Wayland") and
  `scripts/build.sh` run it on every AppImage before any check or copy.
- `packaging/libmpv/check-package.sh` holds the result for every AppImage it
  checks: no AppRun hook sets `GDK_BACKEND`, no bundled `libwayland-*`. It
  fails the AppImage of release run 44098988278 on both.

## Proof

- The check fails the old AppImage and passes the fixed one (the release
  workflow runs it on every release).
- The fixed AppImage of a release run plays in Vela's window on this AMD
  machine (the probe) and on the owner's NVIDIA machine.
- A codex hunt over the change.
