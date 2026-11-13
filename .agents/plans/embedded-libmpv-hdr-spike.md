# Plan: spike — does HDR survive when mpv is embedded as a library?

## Status

**RUN 2026-11-12 on the owner's go** ("perform whatever you need to to do
the spike"). **Answer: Yes-with-conditions at all three stages** on
`continuum` (KDE Wayland, AMD/Mesa). Evidence and conditions are in the
Log at the end of this file. One check is still the owner's: looking at the
screen (Log, "Not verified"). This was a SPIKE: throwaway prototype code
outside the shipped app. It changed nothing Vela ships, and it authorizes no
product work; an embedded player still needs its own approved plan.

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

## Log — 2026-11-12, `continuum`

Scratch code, clips, logs and screenshots: `~/dev/vela-spike-libmpv/`
(outside the repo, not committed; `build.sh` rebuilds, `run.sh` runs a
command in the desktop session). Versions as in "Facts established" above.
The agent ran as root in a sudo session, so it installed one package itself:
`wayland-protocols` 1.50 (protocol XML only, needed to build).

### Answer

| Stage | Result |
| --- | --- |
| 0 reference | Done. An objective test exists for everything except absolute brightness. |
| 1 bare Wayland window | **Yes.** |
| 2 under GTK3 + WebKitGTK | **Yes.** |
| 3 inside Tauri 2 | **Yes.** |

All four assumptions held: A1 (libmpv renders untouched PQ/BT.2020), A2
(10-bit and 16-bit-float EGL window configs exist), A3 (KWin accepts and
honours the tag), A4 (a subsurface below the toplevel shows through a
transparent WebKitGTK view).

### How HDR was judged (Stage 0)

The agent cannot see the monitor, so three objective checks stand in:

1. **Readback.** A generated HDR10 pattern (grey patches at 0-4000 nits,
   BT.2020 primaries; HEVC Main10, PQ, mastering 1000 nits) has a known PQ
   code per patch. `glReadPixels` on the framebuffer libmpv drew is compared
   with those codes.
2. **Protocol trace.** `WAYLAND_DEBUG=1` on external mpv, launched with
   Vela's exact arguments, shows what is sent to KWin; the prototype sends
   the same.
3. **Screenshot A/B.** KWin's screenshot (`spectacle`) converts every
   surface to SDR through its colour pipeline. Two surfaces with the same
   tag and the same pixels must screenshot the same; an untagged surface
   screenshots differently.

What no screenshot can show: patches above SDR white all saturate to 255.
Whether highlights are physically brighter than SDR white is the owner's
eye (commands under "Not verified").

Correction to "Facts established": external mpv does not tag its own
surface. With `--vo=gpu-next --gpu-api=vulkan`, **Mesa's Vulkan WSI** makes
the `wp_color_manager_v1` calls, from the swapchain colour space and HDR
metadata mpv hands it. And mpv, not KWin, does the tone-mapping: it maps to
the output's volume and tags the surface with that volume. Observed for this
output (peak 465 nits, reference white 265 nits):

```
set_primaries_named(bt2020)  set_tf_named(st2084_pq)
set_mastering_display_primaries(<the output's target primaries>)
set_mastering_luminance(min 0.0038, max 356)  set_max_cll(356)  set_max_fall(0)
set_image_description(desc, perceptual)
```

356 = 465 x 203 / 265: the output's peak restated against PQ's 203-nit
reference white.

### Stage 1 — bare Wayland window: Yes

- EGL window configs offered by Mesa: 8-bit, **10-bit (R10G10B10, no
  alpha)**, 16-bit fixed, **16-bit float**. Used: 10-bit, OpenGL 3.3 core.
- libmpv options: `vo=libmpv`, `hwdec=auto`, `target-trc=pq`,
  `target-prim=bt.2020`; render params `MPV_RENDER_PARAM_WL_DISPLAY`,
  `MPV_RENDER_PARAM_DEPTH=10`, FBO 0. With `target-peak` unset mpv does no
  tone-mapping. Readback, 10-bit codes (expected / got): 50 nits 450/449-452,
  203 nits 594/592-595, 1000 nits 769/767-770, 4000 nits 923/921-924; BT.2020
  red 520,0,0 / 519,11,11. The same within 1 code on the float config. (Black
  reads 11, about 0.005 nits, in this mode; 0 once `target-peak` is set.)
- Tag: `wp_color_manager_v1` (KWin offers v3; bound at v1) accepted the
  parametric PQ/BT.2020 description and sent `ready`; no protocol error.
  KWin advertises parametric, set_luminances,
  set_mastering_display_primaries and extended_target_volume.
- KWin honours it. Screenshot grey values, same pattern:

  | Surface | 50 | 100 | 203 | 265 nits |
  | --- | --- | --- | --- | --- |
  | prototype, untagged | 111 | 128 | 147 | 154 |
  | prototype, tagged like external mpv | 124 | 160 | 207 | 227 |
  | external mpv | 125 | 163 | 210 | 229 |

  (Both players with `tone-mapping=clip`, `hdr-compute-peak=no`, so only the
  compositor path differs. The prototype sets `target-peak=356` and the tag
  above.)
- Hardware decoding: `vaapi` (external mpv picks `vulkan`).
- 4K60 HEVC 10-bit HDR, fullscreen at 3840x2161, 58 s, Vela's `gpu-hq` and
  `hdr-compute-peak=yes`, two runs each:

  | | Frames dropped | CPU time | GPU busy (idle 6%) |
  | --- | --- | --- | --- |
  | prototype | 13-14, all in the first 5 s | 1.9 s | 21-22% |
  | external mpv | 0 | 2.8 s | 20% |

### Stage 2 — GTK3 + WebKitGTK: Yes

- A GTK3 toplevel with an RGBA visual, `app-paintable`, a transparent CSS
  background, and a `WebKitWebView` with a transparent background colour.
  The video is a `wl_subsurface` of the toplevel's `wl_surface`
  (`gdk_wayland_window_get_wl_surface`), `place_below`, desync, empty input
  region, own `wp_viewport` and `wp_fractional_scale_v1`.
- Screenshot: opaque HTML sidebar exactly its CSS colour (32,36,44); video
  visible in the hole with the Stage 1 values (125/161/209/226); translucent
  HTML bar and an opaque white swatch drawn over the video.
- Resize to 1500x700, fullscreen, and back: the hole follows, the buffer is
  re-sized to native pixels (3330x2161 fullscreen at scale 1.7), values
  unchanged, the tag survives.
- GTK3 itself renders at integer scale 2 and is downscaled; the video
  surface is at true 1.7.
- 4K60 fullscreen with rendering on the GTK main thread: 13 drops, all in
  the first 5 s, none in the next 32 s.
- libmpv refuses to start after `gtk_init` until
  `setlocale(LC_NUMERIC, "C")` is called.

### Stage 3 — Tauri 2: Yes

- Throwaway app on Vela's own pinned versions (tauri 2.11.5, wry 0.55.1,
  tao 0.35.3, from a copy of Vela's `Cargo.lock`), window
  `"transparent": true`. In `setup`, `WebviewWindow::gtk_window()` gives the
  `GtkApplicationWindow`; the Stage 2 C code is linked in and attaches to it
  unchanged.
- tao's window arrives realized, mapped, RGBA and app-paintable, with
  client-side decorations: the webview sits at (45,93) inside the surface
  and the subsurface offset comes from `gtk_widget_translate_coordinates`.
- Readback and fullscreen screenshot match Stage 2 exactly. Windowed,
  `set_fullscreen(true)` and back all track.
- 4K60 fullscreen: 13 drops in the first 5 s, none in the next 30 s.

### Conditions and differences found

- **Not the same renderer.** The render API draws with mpv's older `vo=gpu`
  renderer, not `gpu-next`/libplacebo, which Vela uses today. Measured on
  the pattern with each player's default tone-mapping: identical up to
  ~265 nits; above that the embedded picture is brighter (a 1000-nit patch
  screenshots 230 against 205). External mpv also maps saturated BT.2020
  colours into the display's gamut; the embedded one did not. Picture
  quality on real films therefore needs an owner comparison before any plan
  treats the two as equal.
- **Vela must do what Mesa does for mpv today:** read the output's
  preferred description, set `target-peak`, tag the surface, and redo both
  when the preferred description changes (another monitor, HDR toggled). The
  prototype does it once.
- A few frames drop at start-up; none in steady state.
- A one-pixel seam is visible at the hole's left edge in the windowed Tauri
  screenshot (fractional-scale rounding of the subsurface position).

### Not verified

- **The owner's eye.** That highlights are physically brighter than SDR
  white, and that SDR UI looks right beside HDR video. From a terminal on
  the `continuum` desktop:
  `cd ~/dev/vela-spike-libmpv && ./stage2 --tone display clips/pattern-hdr10.mp4`
  beside
  `mpv --no-config --vo=gpu-next --gpu-api=vulkan --target-colorspace-hint=yes clips/pattern-hdr10.mp4`.
  The patches right of the fifth grey (265 nits) should be visibly brighter
  than the white HTML swatch in the bottom bar, and match external mpv.
- **Pointer input over the hole.** Not exercised: the agent has no pointer.
  The video surface has an empty input region and sits below the webview, so
  clicks should reach the page; both test apps print a line when either
  button is clicked.
- A real film sample and Dolby Vision. Only generated clips were used (the
  one public HDR10 sample URL tried returned 404).
- Audio (disabled in every run), subtitles, OSD.
- Any other compositor, GPU vendor or driver: one machine, KWin 6.8.2 on
  AMD/Mesa. NVIDIA, GNOME and X11 are untested.
- Rendering on a thread of its own. The prototype draws on the UI thread
  and saves/restores the toolkit's EGL state around each frame.

### What a real implementation must then build

Rough list, unsized: a libmpv binding and render thread; the Wayland
subsurface and its geometry sync with the page layout (and the seam);
colour-management tagging that follows the output; a decision on the
tone-mapping quality gap; replacement of everything the external player's
IPC does today (resume, auto-advance, watched state, transcode step-down,
skip markers, AirPlay ordering on macOS); audio; on-video controls in HTML;
a fallback when the compositor lacks `wp_color_manager_v1`; and separate
backends for X11, macOS, Windows and iOS, none of which this spike touched.
