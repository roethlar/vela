# Plan: spike — does HDR survive when mpv is embedded as a library?

## Status

**DRAFT 2026-11-12. NOT APPROVED. Nothing may be built until the owner's
go.** This is a SPIKE: throwaway prototype code outside the shipped app,
whose only product is an answer written into this file. It changes nothing
Vela ships.

Direction it serves: `.agents/decisions.md`, 2026-11-12, "Embedded
playback through libmpv is the direction". That decision names this spike
as the first step and keeps external mpv as the shipped player until the
spike reports.

## The question

Can video decoded by libmpv INSIDE a Vela-like window (a UI layer drawn
over it) reach the display as HDR on KDE Wayland — the platform Vela
targets first — or is it tone-mapped to SDR?

A second question only matters if the first is yes: can that be done under
a GTK3/WebKitGTK window, which is what Tauri gives us on Linux?

## Facts established 2026-11-12 on `continuum` (no code written)

- Display `DP-3`: HDR enabled, Wide Color Gamut enabled, scale 1.7
  (`kscreen-doctor -o`). So this machine can show a real answer.
- mpv 0.41.0, libplacebo 7.360.1, KWin 6.8.2, Mesa 26.2.6, GTK 3.24.52,
  WebKitGTK 2.54.2, wry 0.55.1, tao 0.35.3.
- `/usr/include/mpv/render.h`: the libmpv render API offers exactly two
  backends, `MPV_RENDER_API_TYPE_OPENGL` and `MPV_RENDER_API_TYPE_SW`.
  There is no Vulkan/libplacebo render API. It accepts
  `MPV_RENDER_PARAM_WL_DISPLAY` (for hardware decoding interop).
- mpv's `--wid` does not exist for Wayland (decision 2026-07-14), so
  "libmpv with its own VO" can only open a separate toplevel on Wayland —
  that is today's behaviour in-process and answers nothing.

Consequence: on Wayland the only embedding route is the OpenGL render API
drawing into a surface the HOST owns. mpv then cannot tag that surface as
HDR; the host must. That is the whole risk.

## Assumptions (each is what the spike tests; none is verified)

- A1. With `target-trc=pq`, `target-prim=bt.2020` (and `target-peak` from
  the display), `vo=libmpv` renders PQ-encoded BT.2020 pixels into the
  host's framebuffer without tone-mapping.
- A2. Mesa offers an EGL config for a Wayland surface with at least 10 bits
  per channel (or 16-bit float), so those pixels are not crushed to 8 bits.
- A3. KWin 6.8 exposes the Wayland color-management protocol
  (`wp_color_manager_v1`) to ordinary clients, and tagging the host's
  surface with a PQ/BT.2020 image description (plus mastering luminance)
  makes KWin present it as HDR. Check with `wayland-info | grep -i color`.
- A4. A `wl_subsurface` placed BELOW a GTK3 toplevel's main surface is
  visible through a transparent region of a WebKitGTK webview.

## Stages (stop at the first No and record it)

All code lives in a scratch directory OUTSIDE the repo
(`~/dev/vela-spike-libmpv/`); nothing is committed to Vela except this
file's log. Language: whatever is fastest (C or Rust with `libmpv` +
`wayland-client` + EGL); quality is irrelevant.

### Stage 0 — reference (minutes, no code)

Play one HDR10 test clip with the external player exactly as Vela launches
it. Record what proves HDR on this machine: KWin's reported surface state
if a tool shows it, the monitor's own HDR indicator/OSD, and a peak-white
luminance pattern compared by eye against SDR white. Write down the method
that is used as the yes/no test in every later stage. If no objective
method exists, say so and use the owner's eye with an A/B clip.

Test clips: an HDR10 (PQ, BT.2020) pattern with a known 1000-nit patch; a
normal HDR10 film sample; a Dolby Vision profile 5 or 8 sample if the
owner has one (record separately — DV is expected to fall back to its
HDR10 base layer or be tone-mapped, as it does today).

### Stage 1 — bare Wayland window, libmpv render API, HDR tag (the kill test)

A standalone program, no GTK, no webview: one `xdg_toplevel`, an EGL
window surface with the deepest config available (A2), libmpv with
`vo=libmpv` and the options in A1, `mpv_render_context_render` into the
default framebuffer, and the surface tagged through `wp_color_manager_v1`
(A3).

- Pass: the Stage 0 test reads HDR, with highlights matching the external
  player side by side.
- Fail modes to distinguish in the log: no 10-bit config (A2); protocol
  missing or refused (A3); image is HDR-tagged but visibly wrong (A1:
  double tone-mapping, wrong primaries, banding).
- Also record: hardware decoding in use or not (`hwdec-current`), dropped
  frames over 60 s of 4K HDR, and CPU/GPU load against external mpv.

**If Stage 1 fails with no workaround, the answer is No for Wayland.**
Record it; the direction decision then needs the owner's ruling on the
fallback it already names (embedded elsewhere, external on Linux).

### Stage 2 — the same surface under a GTK3 + WebKitGTK window

A minimal GTK3 program (not Tauri yet) with a transparent WebKitGTK view
showing a page that has an opaque sidebar and a transparent "video hole".
Take the toplevel's `wl_surface` (`gdk_wayland_window_get_wl_surface`),
create a `wl_subsurface` below it, and run Stage 1's renderer on that.

- Pass: HDR video visible through the hole, HTML controls drawn over it,
  correct on resize, on a scale-1.7 display, and when the window moves
  between fullscreen and windowed.
- Record: whether KWin still treats the composition as HDR when an SDR
  surface (the webview) overlaps it, and whether SDR UI brightness looks
  right next to HDR video.

### Stage 3 — inside Tauri (only if Stage 2 passes)

A throwaway Tauri 2 app (not the Vela repo): transparent window, reach the
GTK window through Tauri's Linux handle, attach Stage 2's subsurface.
Answers only: does wry/tao get in the way (window transparency, the
compositing mode WebKitGTK picks, input reaching the webview over the
hole)?

### Not in this spike

iOS/iPadOS, macOS and Windows backends; audio routing, subtitles, the IPC
features the current player uses (resume, auto-advance, AirPlay sync,
transcode step-down); any change to Vela. Each is real work that is only
worth sizing after the answer.

## Output

Written into this file, under a dated log:

- Yes / No / Yes-with-conditions for each stage, with the evidence.
- For a Yes: the exact options, EGL config and protocol calls that worked,
  and a rough list of what a real implementation must then build.
- For a No: which assumption failed and whether it is a limit of mpv's
  render API, Mesa, KWin, or GTK/WebKitGTK — because that says whether
  waiting for upstream changes it.

Then: a `decision` entry updating the 2026-11-12 direction with the
result, and a `state.md` update.

## Owner involvement

- A go for the spike (scratch code outside the repo; opens test windows on
  the `continuum` desktop).
- The owner's eye at the HDR yes/no test if Stage 0 finds no objective
  measurement.
- An HDR10 test clip the owner is content to use, or permission to
  generate/download a public test pattern.
