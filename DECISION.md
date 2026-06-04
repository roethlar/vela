# Decision: adaptive render pipeline rejected; strict HDR hardware minimums chosen

**Date:** 2026-06-04

An adaptive mpv render pipeline (per-GPU/per-file quality tiering, a measured governor,
a render cache) was designed and then rejected. Vela's reason to exist is **HDR on
Wayland**: hardware below the HDR floor isn't a target user, and hardware above it runs
the single shipping profile fine — so per-file/per-GPU tiering isn't worth the
complexity. Instead we document hardware minimums in the README and otherwise leave the
choice to the user — Vela does not block, warn, or otherwise handicap below-minimum
hardware; it just won't perform well there.

## Facts that still hold

- **Shipping mpv profile** is the single hardcoded one: `--vo=gpu-next,gpu`,
  `--profile=gpu-hq`, plus the HDR flags (`--target-colorspace-hint=yes`,
  `--hdr-compute-peak=yes`) and per-OS `--gpu-api`. See `src-tauri/src/playback.rs`.
- **Release packaging** is already handled by `.github/workflows/release.yml`
  (MSI/NSIS, dmg/.app, deb/AppImage/rpm via `tauri-action`).
- **mpv stays unbundled** and runtime-detected; the in-app installer fetches it.
- **Minimums** are documented in the README only. They are *not* enforced — no runtime
  notice, no gating. Users may run Vela on below-minimum hardware and accept the
  performance/HDR tradeoff.

## Explicitly NOT built

No `RenderProfile` abstraction, no quality ladder, no media classification, no render
cache, no measured governor, no IPC dispatcher (for this purpose), no
`Auto | Quality | Performance` render mode.
