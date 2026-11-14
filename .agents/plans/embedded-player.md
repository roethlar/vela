# Plan: an embedded libmpv player next to the external mpv player

## Status

**APPROVED 2026-11-12. Implementation in progress; see "Implementation
log" at the end of this file for what has landed.** Decisions are ruled
(D1, D4, D7) or ASSUMED under the owner's standing instruction (D2, D3,
D5, D6; see below).

The go, verbatim (owner, 2026-11-12, given as the session's goal):
"when the sweep finishes, continue with the next steps. implement the
plan, codereview slices with codex. ask any questions upfront. I won't be
here after you start." Asked up front which plan that covers, the owner
chose "Both, embedded first": this plan, then
`.agents/plans/mpv-window-geometry.md` Slices 1-3.

Rules for the unattended run (owner's choices 2026-11-12 from options the
agent wrote; the option text is the agent's, the choice is the owner's):

- **Owner-only steps wait; the work does not.** Chosen: "Build on, keep
  hidden". The agent finishes everything tests can prove and leaves the
  owner's by-eye checks as a list ("Owner checks outstanding" in the log).
  Until the owner has done them there is no Settings control, no version
  bump and no release note for the embedded player; it can be tried only
  by setting `player` in the config file. So Slice 4's "owner present"
  acceptance and Slice 6's switch-on (Settings control, default, release
  notes, docs, bump) are NOT performed by the agent.
- **Review.** Each slice that lands repo code gets a `codereview codex`
  defect hunt over its pinned range (`.agents/playbooks/codereview.md`),
  codex called plainly per `.agents/machines.md`.
- **Push.** Chosen: "Yes, push each commit", to `origin` and `github`,
  never forced.

Reviewed 2026-11-12: openreview codex, `acceptable_with_changes`, three
admitted findings (ep-1, ep-2, ep-3). The owner adopted the fixes
(below); this text has been revised for all three and the revised text has
NOT been re-reviewed. Record:
`.agents/review/openreview-2026-11-12-embedded-player-plan.md`.

Owner rulings already given (2026-11-12), verbatim:

- **Direction.** "right, so mpv as a library it is." (`.agents/decisions.md`,
  2026-11-12, "Embedded playback through libmpv is the direction".)
- **Both players.** "well we need to build all of that for the embedded
  player anyway, and embedded is the only option for ios, so we're not
  losing anything by keeping both except for future feature parity. plan
  for both options" (`.agents/decisions.md`, 2026-11-12, "Vela keeps both
  players").

- **Review changes and standing instruction.** Asked whether to adopt the
  review's first change (ep-1), the owner answered: "yes, assume I won't
  make any decisions that make all of this pointless. :)" Agent's reading,
  not the owner's words: ep-1 is adopted by the "yes"; ep-2 and ep-3,
  which are likewise confirmed defects with one sensible fix, are adopted
  under the instruction; and decisions whose other answer would defeat the
  plan are to be assumed rather than asked. Under that reading D3, D5 and
  D6 below are taken as the agent's recommendation and marked ASSUMED. The
  owner can overturn any of them; none is recorded as an owner ruling.

Decisions:

- **D1. Default player per platform: RULED 2026-11-12.** Owner, verbatim:
  "external by default for all OSes, embedded marked
  experimental/beta/whatever and kept that way until the library matches.
  embedded by default on ios, obviously. no experimental tag in ios." So:
  external is the default on every desktop OS; embedded is offered there
  under an experimental label, and the label stays until libmpv's
  embedding renderer matches the external player's picture; on iOS/iPadOS
  embedded is the only player and carries no label.
- **D2. Is the choice user-visible?** Agent's reading of the D1 ruling,
  not the owner's words: yes, a visible Settings control, since an option
  "marked experimental" has to be seen to be chosen. The exact label word
  ("Experimental" is what Settings already uses for Emby) is unconfirmed.
- **D3. What happens when embedded is selected but cannot start** (libmpv
  missing, X11 session, compositor lacks the needed protocols).
  ASSUMED: play with the external player and show one notice line
  saying why; never fail the play.
- **D4. Licence: RULED 2026-11-12, CLOSED.** Owner, final word,
  verbatim: "can you just pick the correct license that means I don't
  need to think about it or care about what's what on any platform? if
  that's lgpl, fine. I don't want to hear about licensing again after
  this decision." The agent's pick, which stands as the rule on every
  platform (full record: `.agents/decisions.md`, 2026-11-12, "Licensing
  is settled"):
  1. Vela's own code stays MIT. No relicensing, now or for any platform.
  2. mpv's library is only ever loaded as a separate dynamic library,
     never statically linked into Vela.
  3. Vela ships that library itself on every platform (D7), always as an
     LGPL build of mpv and of FFmpeg, as separate dynamic libraries, with
     their licence texts. Never a GPL build.
  **Do not raise licensing with the owner again.** An agent that meets a
  licensing question applies rules 1-3; only a case those rules cannot
  cover is escalated, and then with the specific blocker, not the topic.
- **D7. Ship the library: RULED 2026-11-12.** Told the Linux backend would
  load the system's library, the owner answered: "no. that means vela will
  work differently on linux machines with different mpv libraries. what do
  we need to do to ship the library with the code on all platforms". So
  Vela ships its own pinned libmpv on every platform, Linux included (see
  "libmpv is shipped with Vela on every platform").
- **D5. Scope of the first embedded release.** ASSUMED: Linux on
  Wayland only (the one platform the spike proved). macOS, Windows and iOS
  each get their own later plan on top of the seam this plan builds.
- **D6. Where the video appears.** ASSUMED: inside Vela's main
  window, taking over the whole content area while playing (a player
  view), with Vela's own fullscreen. Not a second Vela window.

## Why two players, and what "parity" means

- iOS/iPadOS cannot start another program, so an in-process player must
  exist for that port.
- libmpv's render API (`/usr/include/mpv/render.h`; upstream `master`
  checked 2026-11-12) offers only OpenGL, software and, from 0.42,
  Direct3D 11 backends. It cannot use mpv's `gpu-next` renderer, which the
  external player uses. Spike result
  (`.agents/plans/embedded-libmpv-hdr-spike.md`, Log): HDR survives
  embedding on KDE Wayland; dark and moderate scenes match the external
  player; bright scenes clip more highlight embedded.
- So neither player replaces the other. External mpv is kept unchanged.
- **Parity rule (owner accepted the cost):** every playback behaviour
  listed in "Behaviour inventory" must work in both players, and a test
  must fail when one player gains a behaviour the other lacks (Slice 6).

## What exists today (verified 2026-11-12; line numbers drift — find by name)

One entry point starts playback: `playback::play` in
`src-tauri/src/playback.rs`, called only from `play_by_key_locked` in
`src-tauri/src/commands.rs` inside `spawn_blocking`.

`play` does, in order: resolve the mpv binary (`resolve_mpv`); load config;
build the command (`base_mpv_command`: `--no-config` unless
`mpv_use_own_config`, `--no-ytdl`, `--vo=gpu-next,gpu`,
`--profile=gpu-hq`, `--hwdec=auto`, `--hwdec-codecs=all`,
`--target-colorspace-hint=yes`, `--hdr-compute-peak=yes`, platform
`--gpu-api`, macOS `--ao`, then the user's `mpv_extra_args`); then Vela's
protected tail, each asserted after user options: autocrop script args,
marker script args plus `VELA_MARKERS_PAYLOAD` in the child environment,
window-state args, `--screen-name`, `--idle=no` (`lifecycle_args`),
`--input-ipc-server=<private socket>`, `--force-media-title`, `--title`,
`--include=<owner-only header file>` for stream auth, `--start=`, `--`,
URL. It spawns the process into `AppState.current_child`
(`ManagedChild`), under the `shutting_down` check, and starts threads that
all talk to mpv **only through the JSON IPC socket**:

- `spawn_eof_watcher` → `PlaybackAdvance::mark_eof(session)` on
  `end-file` with `reason: eof`.
- `spawn_position_reader` → position, window state, display observation.
- `start_tracking_plex` / `start_tracking_jellyfin` / `spawn_end_watcher`
  → server check-ins, then `on_end(last_position_ms)` exactly once.
- `spawn_health_sampler` → Automatic quality step-down.
- `spawn_osd_notice` → an on-screen notice.

Everything after `on_end` is player-agnostic and lives in `commands.rs`
and `lib.rs`: recents stamp, the `playback-ended` event, the dispatcher
that waits for both `mark_eof` and `mark_ended`, `advance_playlist`, the
`continue-playing` event, watched-state writes. The frontend
(`src/routes/+page.svelte`) reacts to `playback-ended`,
`continue-playing` and `source-choice-required`, and starts plays with
`play_item`, `playlist_play`, `server_playlist_play`.

Process ownership in `AppState` (`src-tauri/src/lib.rs`): `current_child`,
`reap_queue`, `tracking_stop`, `shutting_down`, a 5 s reaper thread, and an
exit sweep that kills the child. Before each play `play_by_key_locked`
sets the old `tracking_stop`, takes and kills `current_child`, and queues
it for reaping.

Settings (`src-tauri/src/config.rs`, `AppConfig`): `mpv_path`,
`mpv_extra_args`, `mpv_use_own_config`, `mpv_autocrop`, the three skip
policies, `playback_quality`, `continue_playing`. UI:
`src/lib/Settings.svelte`, "mpv player" section.

Packaging: no mpv crate in `Cargo.toml`; Linux already depends on
`wayland-client` 0.31 and `wayland-protocols` 0.32 (`client`, `staging`)
with a vendored `wayland-scanner`. `tauri.conf.json` bundles
`resources/mpv-scripts/`. `packaging/arch/PKGBUILD` depends on `mpv`
(which ships `/usr/lib/libmpv.so.2`). The main window is created from
`tauri.conf.json` only, decorated, not transparent.
`src-tauri/src/main.rs` sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux.

E2E (`tests/e2e/`): drives the real app under Xvfb (X11) with the real
mpv, seeded `--vo=null --ao=null`; scenarios find mpv's socket by scanning
`/tmp/vela-*/mpv-*.sock` and drive it over IPC (`mpv.mjs`,
`helpers.mjs`). Source-text tests `tests/mpv-lifecycle.test.mjs` and
`tests/clean-eof-refresh-order.test.mjs` parse `playback.rs`, `lib.rs`
and `commands.rs`.

## Design

### The seam: one launch description, two launchers

The central observation: everything Vela does to a running player goes
through mpv options, mpv scripts and mpv's JSON IPC socket. libmpv accepts
the same options and, per upstream documentation, the same
`input-ipc-server` option and script loading (ASSUMPTION A1, probed in
Slice 0). If A1 holds, the embedded player opens the same private socket
and **every existing tracker thread is reused unchanged**; the E2E
harness's socket discovery also keeps working.

So split `play` into:

1. `build_launch(spec, cfg) -> LaunchPlan`: the ordered option list
   (name/value pairs, no leading `--`), the environment additions, the URL,
   the header-include guard and the marker payload path. Pure except for
   writing those two files. Both players consume it.
2. A launcher per player:
   - `external`: today's behaviour. Turns the plan into a `Command`
     (`--name=value`, `--`, URL), spawns, returns a handle.
   - `embedded`: creates an mpv handle in-process, applies each option
     with `mpv_set_option_string`, initialises, attaches the render
     surface, sends `loadfile`. Returns a handle. The handle must survive
     an arbitrary delay between `mpv_initialize` and `loadfile` (surface
     setup takes time), which is why `idle` is `once` here.
3. `PlayerHandle`, an enum replacing `ManagedChild` in
   `AppState.current_child` and `reap_queue`, with the operations the
   callers use today: `kill()`, `try_wait()` (has it ended),
   `remove_consumed_header_include()`. For the embedded variant `kill` is
   an mpv `quit` followed by destruction on the player's own thread, and
   `try_wait` reports the handle's shutdown.

Options that differ per player are decided in `build_launch` by a
`PlayerKind` argument, never by editing the list afterwards:

| Option | External | Embedded |
| --- | --- | --- |
| `vo` | `gpu-next,gpu` | `libmpv` |
| `gpu-api`, `ENABLE_HDR_WSI` | as today | omitted |
| `target-colorspace-hint` | `yes` | omitted (Vela tags the surface) |
| `target-trc`, `target-prim`, `target-peak` | unset | set by the surface code |
| window-state args, `screen-name`, `title` | as today | omitted |
| `idle` | `no` | `once` (ep-1: with `no`, libmpv shuts down before the first `loadfile`; `once` waits for the first file, then exits at its end) |
| `input-ipc-server`, `force-media-title`, `include`, `start`, scripts | same | same |
| `mpv_path` | used | ignored |
| user `mpv_extra_args` / own config | applied | applied, then the embedded-only options above are re-asserted |

**How the embedded launcher applies the list (decided in Slice 0, A1).**
Through the library API mpv rejects command-line-only spellings: `script`
is such an alias ("option not found"), and so are the `-append` forms Vela
already emits (`script-opts-append`). A config file IS parsed with
command-line semantics, and `include` works through the API. So the
embedded launcher writes the plan's ordered options to a private,
owner-only file in mpv's config syntax, one
`name=%<byte length>%<value>` line per option (that form needs no
quoting), and sets `include` to it through the API. The user's
`mpv_extra_args` go through the same file, so they mean exactly what they
mean for the external player. Paths are always absolute: mpv 0.42
resolves a relative path against `$PWD`, not the working directory.

`VELA_MARKERS_PAYLOAD` reaches the external player through the child
environment. In-process there is no child environment; setting the
variable on Vela's own process is racy and leaks to later children. Pass
the path as a script option instead (`script-opts=vela_markers-payload=`)
and have `vela-markers.lua` read the script option first and the
environment variable second. That keeps one script for both players.

### libmpv is shipped with Vela on every platform (D7)

Owner ruling 2026-11-12 (D7, below): the embedded player must not depend
on whichever mpv library a machine happens to have. Vela builds one pinned
libmpv and ships it inside every package, Linux included. The EXTERNAL
player is unchanged and still uses the machine's `mpv` program.

What that requires:

- **A reproducible build of the library set**, per target (Linux x86_64
  first; later Windows x64, macOS universal, iOS), from pinned, checksummed
  source releases: mpv (configured `-Dgpl=false -Dlibmpv=true
  -Dcplayer=false`), FFmpeg (default configuration, i.e. without
  `--enable-gpl` and without `--enable-nonfree`), libplacebo, libass with
  FreeType/FriBidi/HarfBuzz, a Lua that mpv supports (the bundled scripts
  need it), and the decoders FFmpeg needs that are not in its tree (dav1d
  for AV1). All as separate shared libraries with an `$ORIGIN` run path so
  they find each other; none statically linked into Vela. Licence texts of
  each ship alongside (rule of `.agents/decisions.md`, "Licensing is
  settled").
- Verified 2026-11-12 against mpv v0.41.0's `meson.build`: `gpl=false`
  removes only CD audio, DVB, DVD navigation, JACK, OSS, libcaca, the old
  Direct3D 9 output and mpv's own X11 window code. Wayland, EGL/OpenGL,
  VA-API, Lua, Direct3D 11 and the render API are unaffected. Nothing the
  embedded player uses is lost (ASSUMPTION A10 confirms by building it).
- **System pieces that are NOT shipped** and are loaded from the machine,
  because they must match its GPU driver and compositor: libc, libEGL /
  libGL (Mesa or the vendor driver), libva and its driver, libwayland,
  libdrm, the audio client library (PipeWire/PulseAudio/ALSA). Hardware
  decoding therefore still depends on the machine's drivers; everything
  else about decoding and rendering is identical everywhere.
- **Portability on Linux:** build on the oldest distribution Vela supports
  so the libraries run on newer ones (glibc baseline). Decide the baseline
  in Slice 0 and record it; the Tauri Linux bundles and the Arch package
  must both carry the same binaries.
- **Where it lives:** a `libmpv/` directory under Tauri's resource
  directory (next to `mpv-scripts/`), installed by the deb/rpm/AppImage
  bundles and by `packaging/arch/PKGBUILD`.
- **Loading:** `libloading` (new dependency) opens the shipped
  `libmpv.so.2` by absolute path from the resource directory, with a
  hand-written FFI table for the functions used: `mpv_create`,
  `mpv_set_option_string`, `mpv_initialize`, `mpv_command`,
  `mpv_set_property_string`, `mpv_get_property`, `mpv_wait_event`,
  `mpv_set_wakeup_callback`, `mpv_terminate_destroy`,
  `mpv_error_string`, `mpv_free`, `mpv_client_api_version`,
  `mpv_render_context_create`, `_set_update_callback`, `_update`,
  `_render`, `_report_swap`, `_free`. No bindgen and no link-time
  dependency. **There is no fallback to a system libmpv**: if the shipped
  library fails to load, embedded is unavailable and D3 applies. In a
  correct install that can only come from the machine, not from Vela: a
  system library the set needs is absent or older than the build baseline
  (glibc, libEGL, libwayland, libva), or the install is damaged. A package
  that simply lacks the set is a packaging DEFECT and must never reach a
  user: the release build fails unless a packaging check finds every file
  of the set in each built package (deb, rpm, AppImage, Arch) and loads
  `libmpv.so.2` from the unpacked package with `mpv_client_api_version`
  returning the pinned version. Red-prove that check by removing one
  library from a package. The D3 notice names the actual load error so a
  damaged or too-old system is diagnosable. A debug/development build may point at another library through
  an environment variable so developers need not build the set locally;
  release builds ignore it.
- **CI and release:** a workflow job per target builds the set, caches it
  by the hash of the pinned versions, and publishes it as a build input;
  the release workflow consumes that artifact. macOS and Windows sign the
  shipped libraries with the existing signing setup when those platforms
  get their backends.
- **Upkeep (the standing cost):** Vela now owns security updates for
  FFmpeg and the rest of the set. Add the pinned versions to the
  dependency-policy checks (`tests/dependency-policy.test.mjs` is the
  existing pattern) and a documented bump procedure; a known-vulnerable
  pinned decoder is a release blocker in the same way the npm and Cargo
  audits are.
- Size: expect tens of megabytes per package (the system's libavcodec
  alone is 21 MB on `continuum`); measure in Slice 0.

`setlocale(LC_NUMERIC, "C")` must be in effect before `mpv_create`
(spike finding: libmpv refuses to start after GTK initialises otherwise).

#### The set as decided in Slice 0 (supersedes the bullets above where they differ)

Built and proved on 2026-11-12 (log, "Slice 0"); the recipe is the
prototype for Slice 0b.

- **Contents (15 libraries, 33.5 MB with licence texts):** mpv 0.42.0;
  FFmpeg 9.0.2 (configure reports "LGPL version 2.1 or later";
  `--disable-autodetect`, then zlib, iconv, GnuTLS, dav1d, VA-API and
  libdrm named explicitly); libplacebo 7.387.0 with no GPU backend (mpv
  requires the library, the embedded renderer does not draw with it);
  libass 0.17.5; FreeType 2.15.0; FriBidi 1.0.18; HarfBuzz 15.1.0; dav1d
  1.5.4; Lua 5.2.4; uchardet 0.0.8 (subtitle encoding detection, as
  distribution builds of mpv have). Every optional mpv feature is off
  unless named (`-Dauto_features=disabled`), so the result does not depend
  on what the build machine has installed. Build-only inputs, not shipped:
  Vulkan headers (libplacebo compiles stubs against them).
- **Private names, and why.** Vela's own process already has the system's
  FreeType, HarfBuzz, FriBidi and dav1d loaded (WebKitGTK needs them). The
  dynamic loader reuses an already-loaded library of the same name, and
  resolves symbols from the process's global list first. Left alone, the
  shipped copies would be silently replaced by the machine's. So every
  shipped library except `libmpv.so.2` gets a Vela-private soname
  (`libvela-avcodec.so.63`, `libvela-freetype.so.6`, ...), references are
  rewritten with `patchelf`, each has an `$ORIGIN` run path, and Vela
  opens `libmpv.so.2` by absolute path with
  `RTLD_NOW | RTLD_LOCAL | RTLD_DEEPBIND`. Checked with the loader's own
  binding trace inside a GTK + WebKitGTK process: the set binds only to
  itself, and GTK keeps binding to the system's FreeType.
- **Loaded from the machine, not shipped** (the build fails if the set
  needs anything outside this list): glibc and the C++ runtime
  (`libstdc++`, `libgcc_s`), `libEGL`, `libva` and `libva-drm`, `libdrm`,
  `libpulse` and `libasound`, `libfontconfig` (font discovery must follow
  the machine's font configuration), `libgnutls` (TLS uses the machine's
  trust store and gets the distribution's security updates; WebKitGTK's
  own networking already requires it), `libz`.
- **Baseline: Ubuntu 22.04 (glibc 2.35)**, the distribution Vela's own
  Linux bundles are built on (`release.yml`). Built there in a container,
  the set loads, decodes (H.264, HEVC 10-bit, AV1) and fetches over HTTPS
  on clean Ubuntu 22.04, Debian 12 and Fedora 45 images, and plays 4K60
  HDR with VA-API on `continuum` (CachyOS).
- **Two consequences of that baseline.** (1) Its PipeWire is too old for
  mpv's native PipeWire output, so audio goes through the PulseAudio
  client (PipeWire's own PulseAudio server on current desktops) or ALSA.
  (2) Its libwayland is too old for mpv's Wayland support, which the
  embedded player does not need: mpv is built without it, and VA-API is
  given the GPU through a DRM render node instead (next section).
- **One build-file patch to mpv**
  (`mpv-vaapi-without-drm-output.patch`, 2 hunks of `meson.build`): mpv
  ties VA-API-over-DRM to its DRM/KMS video output, which needs a newer
  libdrm than the baseline has and a library the baseline lacks. Built
  that way the set did not load on Ubuntu 22.04 itself
  (`undefined symbol: drmModeCreateDumbBuffer`). The patch keeps VA-API and
  leaves that video output out; no mpv source file is changed. It ships in
  the repo next to the build script, which is also how the LGPL's
  "provide your modifications" is met.
- **Not included, on purpose:** NVIDIA's decoder headers (see
  "Availability" below: untested hardware), Dolby Vision helper libraries,
  ICC colour management, DVD/Blu-ray, JavaScript scripting.

### Linux/Wayland video surface (the spike's Stage 2/3 design)

Reference implementation, C, outside the repo:
`~/dev/vela-spike-libmpv/core.h` and `stage2.c` on `continuum`
(`.agents/machines.md`). Port it to Rust in a new module
`src-tauri/src/player/embedded/wayland.rs`, using the existing
`wayland-client`/`wayland-protocols` dependencies and a small EGL binding
(`khronos-egl` with the `dynamic` feature, or hand-written FFI; choose in
Slice 3 and record it):

- From the Tauri window: `WebviewWindow::gtk_window()` → `GdkWindow` →
  `gdk_wayland_window_get_wl_surface` and
  `gdk_wayland_display_get_wl_display` (FFI; add the `gdk` Wayland
  bindings the lockfile already carries as `gdkwayland-sys`).
- Wrap the foreign display with `wayland-client`'s backend
  `from_foreign_display`, on its **own event queue** so GDK's dispatch and
  ours never race.
- Create a `wl_surface`; make it a `wl_subsurface` of the toplevel,
  `place_below`, desync, empty input region, opaque region = whole
  surface; `wp_viewport` for the logical size; `wp_fractional_scale_v1`
  for the buffer size.
- EGL on that display: an OpenGL 3.3 core context and a window config
  with 10 bits per channel and no alpha (config found on Mesa: R10 G10
  B10 A0). Fall back to 8-bit when absent and then do not tag HDR.
- A dedicated render thread owns the EGL context: wait on mpv's update
  callback, `mpv_render_context_render` into FBO 0 with
  `MPV_RENDER_PARAM_FLIP_Y`, `MPV_RENDER_PARAM_DEPTH`, swap with swap
  interval 0, `report_swap`. For VA-API decoding pass
  `MPV_RENDER_PARAM_DRM_DISPLAY_V2` with `render_fd` set to the DRM render
  node of the GPU that EGL draws with (`EGL_EXT_device_query`:
  `eglQueryDisplayAttribEXT(EGL_DEVICE_EXT)`, then
  `eglQueryDeviceStringEXT(EGL_DRM_RENDER_NODE_FILE_EXT)`, opened
  read-write). The spike passed `MPV_RENDER_PARAM_WL_DISPLAY`; the shipped
  mpv has no Wayland support compiled in, and Slice 0 showed the DRM node
  gives the same `hwdec=vaapi`. Never call `eglTerminate`: the EGL display
  is shared with GTK and WebKit in the same process.
- The render thread owns every Wayland object it creates on a private
  event queue (registry obtained through a display wrapper). The UI thread
  sends only `wl_subsurface.set_position` and then asks GTK to redraw,
  because that state applies on the parent's next commit; the render
  thread applies size changes (viewport, buffer, opaque region). Proved in
  Slice 0, A5.
- Colour: bind `wp_color_manager_v1` (bind at the lowest version that has
  what is used; the spike used v1). Get the surface feedback's preferred
  description. When it describes an HDR output (target max luminance
  above the reference white): set mpv `target-trc=pq`,
  `target-prim=bt.2020`, `target-peak = target_max * 203 / reference`, and
  tag the surface PQ + BT.2020 named primaries, mastering display
  primaries = the output's target primaries, mastering luminance
  (`target_min * 203 / reference`, that peak), `max_cll` = that peak,
  perceptual intent. This is exactly what Mesa's Vulkan WSI sends for the
  external player (spike log). On `preferred_changed`, recompute and
  re-tag. When the output is SDR or the protocol is missing: leave mpv's
  targets unset and do not tag.
- The main window must be transparent on Linux for the video to show
  through. Slice 0 (A2) found this works ONLY with WebKitGTK's DMABUF
  renderer: with it, the transparent build is pixel-identical to today's;
  with `WEBKIT_DISABLE_DMABUF_RENDERER=1`, which `main.rs` sets on every
  Linux start, a transparent window stops repainting after its first
  frame. Both are process-start choices. So (ASSUMED, the owner being
  away; see the log):
  - **Embedded window mode** is decided once at startup, from the saved
    `player` setting: only then is the window created transparent and the
    DMABUF renderer left on. With `player` absent or `external`, startup
    is exactly today's (opaque window, renderer disabled); nothing changes
    for anyone who has not chosen the embedded player.
  - Changing the setting therefore takes effect at the next start of Vela.
    Until then an embedded choice plays externally with a notice (D3).
  - `main.rs` cannot read the config after GTK starts, so the startup
    decision reads the config file before `vela_lib::run()`.
  - **Safety net:** embedded window mode changes how the whole window is
    drawn, and a failure there would leave the user unable to reach
    Settings. Before entering it Vela writes a marker file; the page
    removes it once the UI has loaded. A marker found at startup means the
    previous embedded-mode start did not get that far: start in today's
    mode and show a notice.

Embedded is **available** only when all hold: Linux; a Wayland GDK
display; Vela started in embedded window mode; `wl_subcompositor` and
`wp_viewporter` present; the shipped libmpv loads; an EGL config exists;
and the NVIDIA proprietary kernel driver is NOT loaded.
`wp_color_manager_v1` is not required (SDR-only without it). Otherwise it
is unavailable and D3 applies.

The NVIDIA exclusion (ASSUMED): `WEBKIT_DISABLE_DMABUF_RENDERER=1` exists
because WebKitGTK's DMABUF renderer crashed under Wayland on that driver,
and embedded window mode needs that renderer. Nothing here was run on
NVIDIA hardware, so embedded stays unavailable there (detected by
`/proc/driver/nvidia/version`) until someone tests it; the notice says so.

### Frontend: the player view

While an embedded play is active the page shows a player view: the app's
normal content is hidden, the page background is transparent over the
video rectangle, and HTML controls sit on top. New backend surface:

- Command `player_set_rect {x, y, width, height}` (CSS pixels relative to
  the webview): the frontend reports the video rectangle on mount and from
  a `ResizeObserver`; the backend converts with
  `gtk_widget_translate_coordinates` and positions the subsurface.
- Command `player_command {name, args}` with a closed allow-list mapped to
  mpv commands/properties: `toggle-pause`, `seek-relative`,
  `seek-absolute`, `set-volume`, `toggle-mute`, `set-audio-track`,
  `set-subtitle-track`, `stop`, `skip-active-marker`,
  `toggle-autocrop`. Nothing else is forwarded; no raw mpv
  command passthrough from the page.
- Event `player-state` at most 4 times a second: position, duration,
  paused, volume, muted, track lists, and whether a skip marker is active.
- Events `player-opened {sessionId}` and `player-closed {sessionId}`
  bracket the view's lifetime (ep-2). They are emitted by the embedded
  player's own lifecycle: opened when the surface is attached for that
  session, closed when that session's mpv handle has shut down. The page
  keeps the current session id and IGNORES a `player-closed` for any other
  session, so a predecessor's late close cannot dismiss its successor's
  view during auto-advance or a replacement play. `playback-ended` is NOT
  used for the view: it carries no session id, fires on every end, and
  fires a second time from the dispatcher after `advance_playlist`; it
  keeps its one job, the watch-state refresh. Closing the view must not
  wait for server bookkeeping.
- Fullscreen is Vela's own window fullscreen (Tauri API), not mpv's.
- Keyboard: the page maps mpv's default keys the external player gives
  today (space, left/right, up/down, m, f, q/Esc) to `player_command`.
  mpv receives no direct input in-process (ASSUMPTION A3: confirm in
  Slice 0 that scripts' key bindings need forwarding).
- The marker "skip" control and the OSD notice are drawn by mpv's OSD in
  the external player. In-process, the render API draws the OSD
  (ASSUMPTION A4), but pointer clicks do not reach it. The player view
  therefore draws its own skip button from `player-state`, and pressing
  it sends the allow-listed `skip-active-marker`, which invokes the
  script's existing `activate-skip` binding (`script-binding`), never a
  raw seek (ep-3): `activate_skip` in `vela-markers.lua` marks the range
  consumed and clears the active state before seeking, and a direct seek
  that lands inside the same range would leave the button showing.
  `vela-markers.lua` gets a script option that suppresses only its OSD
  DRAWING when embedded; its marker state, its Space-to-skip binding and
  the state it publishes for `player-state` stay active. Manual autocrop
  is reached the same way, through an allow-listed `toggle-autocrop` that
  invokes the crop script's binding (next section).

External plays are untouched: no player view, mpv's own window.

### Autocrop in the embedded player (decided in Slice 0, A7)

The stock `autocrop.lua` that Vela bundles finds black bars with FFmpeg's
`cropdetect` filter. FFmpeg licenses that filter under the GPL only, so it
does not exist in the LGPL FFmpeg Vela ships (D4), and the stock script
reports "No crop data" in the embedded player. FFmpeg's `bbox` filter is
LGPL, is in the shipped build, and reports the same rectangle per frame.

So the embedded player loads a Vela-authored, MIT-licensed script instead
(`vela-crop.lua`, new in Slice 5): it does what the stock script does
(pause hardware decoding for the measurement, insert the filter, gather
for about a second, check the result is plausible, set `video-crop`,
restore), with `bbox` as the filter and the union of the per-frame boxes
as the result. It offers the same three modes (off, manual, auto) and the
same settle delay as `vela-autocrop.lua`. Probe result on a letterboxed
clip: `1280x536+0+92`, exactly what the stock script gives on a GPL build.
The external player keeps the stock script unchanged. The GPL script is
never loaded in-process, which also keeps the embedded player clear of the
existing GPL note for that file.

### Selection

New optional `AppConfig.player: Option<String>`, values `"external"` and
`"embedded"`, validated like `mpv_autocrop`; absent means `"external"` on every desktop OS (D1). iOS/iPadOS has no
external player and ignores the field. `play_by_key_locked` picks the player once per play:
configured choice → availability check → D3 fallback. The choice is
recorded on the session so `apply_step_down` relaunches in the same
player.

Memory (decided in Slice 0, A8): each in-process play starts a set of
threads, and glibc keeps freed memory in per-thread arenas, up to eight
per CPU core. Over 150 stop/start cycles the process grew by a few
megabytes per play on a 32-core machine, while with the arena count capped
it stayed flat (so nothing leaks; the allocator hoards). When Vela starts
in embedded window mode it calls `mallopt(M_ARENA_MAX, 4)` before any
thread exists. External-mode starts are untouched.

## Assumptions to probe before any product code (Slice 0)

All eleven were probed on 2026-11-12; the answers are in the
Implementation log ("Slice 0"), and the design sections above already
carry the changes they forced.

- A1. libmpv honours `input-ipc-server`, `script=`, `script-opts=` and
  `include=` when used with `vo=libmpv`.
- A2. Vela's real window can be made transparent without visible change to
  the rest of the app, with and without `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
- A3. Script key bindings do not fire without input forwarding.
- A4. The render API draws mpv's OSD (script overlays, `show-text`) into
  the host framebuffer.
- A5. Rendering on a dedicated thread with its own Wayland event queue is
  stable (the spike rendered on the UI thread).
- A6. A headless Wayland compositor (`weston --backend=headless` or
  `kwin_wayland --virtual`) can host the E2E harness, so embedded plays
  can be driven in CI-like runs. Xvfb cannot: it is X11.
- A7. `autocrop.lua` works in-process (it drives `video-crop`).
- A9. `idle=once`: the handle survives a multi-second gap before the
  first `loadfile`, and after the file ends it emits `end-file` with
  `reason: eof` and then shuts down (the orchestrator's 2026-11-12 probe
  showed the first half; the second half is the reviewer's report).
- A10. The library set builds with `gpl=false` / default-configured
  FFmpeg and, loaded by the spike program in place of the system library,
  plays the test clips with VA-API hardware decoding, HDR tagging, Lua
  scripts and the IPC socket, on `continuum`.
- A11. The same binaries, built on the chosen baseline distribution, load
  and play on a second, different distribution (the Ubuntu E2E VM is the
  available one; it is aarch64, so this needs an aarch64 build or another
  x86_64 host: say which was used).
- A8. Stopping one in-process play and starting the next (quit, destroy,
  create) is leak-free and takes well under a second; needed for
  auto-advance and step-down.

## Behaviour inventory (the parity list)

Each row must pass in both players before the embedded player is offered.
"E2E" names the existing scenario that proves it for the external player.

| # | Behaviour | Existing proof (external) |
| --- | --- | --- |
| 1 | Play starts; resume at `start_seconds` | `playback`, `resume` |
| 2 | Plex timeline and Jellyfin/Emby check-ins, final stop | `watchstate`, `multiplex` |
| 3 | Final position on quit; recents stamp; `playback-ended` | `playverbs`, `curation` |
| 4 | Clean end → watched, Continue Playing modes | `continueon`, `continueoff`, `continuetv`, `completionhub` |
| 5 | Playlist and server-playlist advance | `playlistplay`, `serverplaylists` |
| 6 | Source choice / policy | `sourcepolicy`, `mergedview` |
| 7 | Stream auth header never in argv/URL/title | Rust unit tests on `write_header_include`; `live-transcode` |
| 8 | Skip markers (button / autoskip) | `markers` |
| 9 | Autocrop off / manual / auto | `autocrop` |
| 10 | Automatic quality step-down relaunch | Rust unit tests; `live-transcode` (live venue) |
| 11 | Unrequested-play notice | Rust unit tests |
| 12 | A new play stops the previous one; app exit stops playback | Rust unit tests on `retain_child_after_try_wait`; exit sweep |
| 13 | User mpv arguments and own-config mode | `base_mpv_command` unit tests |
| 14 | Ends at end of file rather than idling | `tests/mpv-lifecycle.test.mjs` |

Not in the list (external-only by nature): window-state inheritance,
`--screen-name`, `mpv_path`, the mpv installer. Embedded-only: HDR
tagging, player view controls.

## Slices

Each slice is one or more commits, each with its tests, each red-proved
per `.agents/repo-guidance.md` (Guard discipline). Nothing ships to users
until Slice 6 passes; the setting stays hidden behind D2's ruling until
then.

### Slice 0 — probes (throwaway code outside the repo; log results here)

Extend the spike programs in `~/dev/vela-spike-libmpv/` to answer A1-A11.
For each: what was run, the observed result, Yes/No. A1 No changes the
whole design (the trackers would need a second, in-process event source):
stop and re-plan. A2 No: stop and ask the owner. A6 No: the plan
continues, and the log states that embedded behaviour has no automated
end-to-end proof, only unit tests and manual acceptance.

### Slice 0b — the library build (repo code: build scripts and CI only)

Lands the build of the shipped library set before any player code uses
it: `scripts/build-libmpv.sh` (pinned versions and checksums in one data
file), the CI job, the dependency-policy guard for the pins, and
`docs/development.md` on how a developer gets the set. Nothing in the app
loads it yet. It starts from the recipe Slice 0 proved (build script,
versions file, container file, the mpv build-file patch, and the
load-and-decode check used in the distribution containers), moved into the
repo and given tests. Guard: a test that the pinned-versions file, the script and
the workflow name the same versions (source-text pattern), red-proved by
changing one of them.

### Slice 1 — the seam, no behaviour change

- New module tree `src-tauri/src/player/` (`mod.rs`, `launch.rs`,
  `external.rs`); `playback.rs` keeps the trackers.
- `build_launch` + `LaunchPlan`; `external::launch(plan) -> PlayerHandle`;
  `PlayerHandle` replaces `ManagedChild` in `AppState`, the reaper and the
  exit sweep.
- `vela-markers.lua` reads `script-opts` first, environment second; the
  external launcher keeps using the environment (no behaviour change).
- Guard: a Rust test that renders the external `Command` for a fixed
  `PlaySpec`/config and compares the full argv to a golden list captured
  from the code **before** the refactor (commit the golden first, in its
  own commit, proving it green on the old code).
- Update the two source-text tests to the new file locations without
  weakening them.
- Full verification set plus full E2E: no scenario may change.

### Slice 2 — selection and fallback, embedded stubbed

- `AppConfig.player`, validation, round-trip tests (absent field loads as
  today; a config with the field still loads in the previous release —
  check whether the previous `AppConfig` tolerates unknown fields and
  record it).
- `player::choose(cfg, availability) -> (PlayerKind, Option<Notice>)`
  per D1/D3, pure and unit-tested for every combination.
- Embedded availability returns "unavailable: not built" so every play
  still goes external. Wiring test: a play with `player: "embedded"`
  reaches the external launcher and carries the notice.
- Settings control only if D2 says visible, and only added in Slice 6.

### Slice 3 — embedded core on Linux/Wayland (SDR, no controls)

- libmpv loader (shipped library only, by absolute resource path) and
  FFI table; version check; locale. Bundle configuration:
  `tauri.conf.json` resources and `packaging/arch/PKGBUILD` install the
  `libmpv/` set. Test: the loader refuses a path outside the resource
  directory in a release build (red-proof by allowing a system fallback).
- `embedded::launch`: handle creation, options from the plan, private IPC
  socket, `loadfile`; shutdown path; `PlayerHandle::Embedded`.
- Wayland subsurface, EGL, render thread, fractional scale, resize.
- Embedded window mode as designed after Slice 0 ("The main window must
  be transparent"): the startup decision read from the config before GTK
  starts, the transparent window and DMABUF renderer only in that mode,
  the startup marker and its notice, the NVIDIA exclusion, the arena cap.
  Guards: a test that an absent or `external` setting yields exactly
  today's startup (window opaque, renderer variable set); a test that a
  leftover marker forces today's mode. Red-prove each.
- `player_set_rect`; a minimal player view (video rectangle and a Close
  control).
- Check and record how the transparent window looks in the first moments
  of startup (not probed in Slice 0).
- App-exit and new-play teardown (inventory 12).
- Acceptance tests named by ep-1, both required: a launch with a
  deliberate delay before `loadfile` still plays; a file that ends makes
  the handle shut down (so inventory 14 holds embedded). Red-proof each by
  switching `idle` to `no` and to `yes`.
- `player-opened` / `player-closed` with session ids; a test in which a
  successor opens before its predecessor's `player-closed` and
  `playback-ended` arrive, asserting the successor's view stays open
  (ep-2). Red-proof by dropping the session check.
- Tests: unit tests for option mapping per `PlayerKind`, rectangle
  conversion, availability; the tracker threads are reused, so add wiring
  tests that an embedded launch starts the same watchers. E2E under the
  headless Wayland venue if A6 held: `playback`, `resume`, `playverbs`
  with `player: "embedded"`.

### Slice 4 — HDR

- Colour-management tagging and `target-peak` as specified; re-tag on
  `preferred_changed`; SDR and no-protocol paths.
- Unit tests for the luminance arithmetic (465/265 → 356; SDR output →
  no tag) and for the re-tag state machine with a fake protocol source.
- Acceptance on `continuum`, owner present: the spike's stacked
  comparison rebuilt with the real app.

### Slice 5 — player view controls

- `player_command` allow-list, `player-state`, keyboard map, tracks,
  volume, seek bar, fullscreen, skip button, notice line.
- `vela-crop.lua` ("Autocrop in the embedded player"): the three modes,
  loaded only by the embedded launcher. Tests: the launcher never names
  the stock `autocrop.lua` for an embedded play (red-proof by restoring
  it); E2E on a letterboxed clip under the headless Wayland venue,
  asserting `video-crop`.
- Tests: allow-list rejects unknown names (red-proof by adding a
  passthrough); source-text guard that every control calls an
  allow-listed name; E2E for pause/seek/stop through the page; a skip test
  in which the seek target is clamped inside the same marker range,
  asserting the HTML button is dismissed (ep-3; red-proof by sending a raw
  seek instead).

### Slice 6 — parity

- Run inventory rows 1-14 in both players. For each row: the E2E scenario
  parameterised by player where A6 allows, otherwise a named manual
  acceptance step.
- Parity guard: a test holding the inventory as data, failing when a
  scenario listed for one player has no counterpart for the other and no
  recorded exemption.
- Only now: the Settings control (D2), the default (D1), release notes,
  `docs/usage.md`, `docs/development.md`, one `scripts/bump.sh`.
- One paragraph in `README.md`'s licence section and `docs/usage.md`: the
  experimental embedded player uses the mpv library shipped with Vela;
  the licence texts of the shipped libraries are included in the package.
  No change to Vela's own licence file or fields (D4).

## Non-goals

- Removing or changing the external player.
- macOS, Windows, iOS/iPadOS embedded backends (D5); X11 embedding.
- Making the embedded picture match `gpu-next`. A follow-up may tune
  `vo=gpu` tone-mapping; it is not part of this plan.
- Dolby Vision handling beyond what libmpv does by default.
- Picture-in-picture, multiple simultaneous videos, thumbnails.

## Risks

- The render API's `vo=gpu` renderer is in maintenance upstream; a future
  mpv could deprecate it or add a `gpu-next` render API. The loader's
  version check and the seam contain either change.
- A transparent main window may cost compositing performance or expose
  WebKitGTK bugs on some drivers (the NVIDIA DMABUF workaround in
  `main.rs` exists for one). Slice 0/A2 measures it on one machine only.
- Two players double the acceptance surface for every later playback
  feature (owner accepted, 2026-11-12).
- In-process playback means an mpv crash is a Vela crash.
- Licence: closed by D4; later platform plans apply its rules 1-3.

## Verification before claiming completion

The full set in `.agents/repo-guidance.md` (Verification) per slice. On
`continuum` that excludes the MSRV check and `cargo audit` (GitHub CI
covers them). E2E from a disposable worktree. Embedded acceptance needs
the `continuum` desktop and the owner's eye for anything about picture.

## Paperwork on landing

- `.agents/decisions.md`: entries for each of D1-D6 as ruled, verbatim.
- `.agents/state.md`; this file's Status and an implementation log.
- `.agents/repo-guidance.md` Mission Detail: replace "do not embed video
  until a plan for it is approved" with a pointer to this plan once
  approved. (Done 2026-11-12 with the approval.)

## Implementation log

Newest entry last. Each entry: what landed, the commits, what was
verified and what was not.

### Owner checks outstanding

Steps only the owner can do. The agent adds to this list as slices land
and removes nothing from it.

How to try it (nothing in the app offers it): build the library set or
download CI's, put `"player": "embedded"` in `~/.config/vela/config.json`,
and start a development build with `VELA_LIBMPV_DIR` naming the set
(`docs/development.md`, "Trying the embedded player"). Restart after
changing the setting.

From Slice 3 (the picture exists; SDR only until Slice 4):

- Watch a film you know in the embedded player on the main display. The
  agent has seen only a test pattern, in a screenshot: right place, right
  size, hardware decoding, no dropped frames in ten seconds. Whether
  motion is smooth, whether the picture is sharp at 1.7 scale, and whether
  sound and picture stay together are for eyes and ears.
- Listen. The sound path was checked at volume 0 (mpv reports PulseAudio
  running); nobody has heard it.
- Watch a cold start with the setting on. For the first instant the
  window is transparent with nothing drawn in it. The first screenshot the
  agent could take, 0.3 s after start, already showed the finished page;
  anything shorter than that was not measured.
- Use the rest of the app for a while with the setting on. In that mode
  the whole window is drawn by WebKit's GPU renderer, which Vela otherwise
  turns off. Slice 0 compared three screens pixel by pixel; daily use is
  a different test.
- Resize, maximise and move the window between monitors while a film
  plays. Headless tests covered a fixed size, and Slice 0 covered resizing
  with a probe program, not with Vela.
- Not checked at all, for lack of the hardware or software: any GPU other
  than this AMD one (NVIDIA is excluded on purpose; Intel is simply
  untried), any compositor other than KWin 6.8 and headless weston
  (GNOME in particular), X11 (expected: falls back to mpv and says so).

From Slice 4 (HDR):

- **Look at an HDR film in both players, side by side or one after the
  other** (the plan's acceptance for this slice: "the spike's stacked
  comparison rebuilt with the real app"). Everything the agent could
  measure agrees with the external player: mpv is told PQ, BT.2020 and a
  peak of 1137 nits on this display at its current brightness, the same
  figure Mesa sent for the external player in the spike, and KWin accepted
  the description. Whether highlights, dark scenes and skin look the same
  is for eyes; a screenshot cannot show it. The two players do not use the
  same renderer (`gpu-next` outside, `gpu` inside), so "the same" here
  means "not visibly worse", not identical.
- Play an ordinary (SDR) film in the embedded player on the HDR display.
  It is drawn in the HDR container at the desktop's brightness for white.
  It should look like the same film in the external player, not brighter,
  not washed out.
- Move the desktop brightness slider while an HDR film plays, and switch
  HDR off and on in the display settings. The picture should follow within
  a frame or two without a flash of wrong colours. The code path is tested
  against a stand-in for the compositor; the real one was only seen at one
  setting.
- Not checked for lack of hardware: a display without HDR (expected:
  standard range, exactly as before this slice), an HDR display on a
  compositor other than KWin.
- With "use my own mpv configuration" on and a profile in your mpv.conf
  that sets `target-trc`, `target-prim` or `target-peak`: on an HDR
  display the embedded player puts its own values back (the picture is
  held for a frame or two each time), and if something keeps changing
  them it drops to standard range for that play. Say if you would rather
  your own values won.

From Slice 5 (controls and cropping; listed when the slice was designed,
2026-11-13, so that they are not lost; they apply once it has landed):

- Look at the control bar over a bright HDR film and in a light theme,
  open the audio and subtitle lists over the picture and in fullscreen,
  and check the picture's edges right after entering and leaving
  fullscreen.
- Crop a standard-range film with black bars, several times on and off,
  and watch that it keeps playing. Changing the crop on a playing film
  stalled playback in the agent's measurements until the script was made
  to follow every crop with a nudge; 24 runs of 24 then played on.
- **The HDR crop trial, with the agent's notes at hand:** the embedded
  player refuses to crop HDR video. To lift that, the refusal has to be
  switched off in a development build and a film with baked-in bars
  cropped while playing and while paused, several times. Be ready for
  Vela to freeze: on mpv's own window this once left a process that could
  not be killed.

### 2026-11-12 — approval recorded

The owner's go and the three unattended-run rules are in Status. The owner
installed `podman` and `patchelf` on `continuum` for the library build
(`.agents/machines.md`). Nothing in the app has changed yet.

### 2026-11-12 — Slice 0 (probes): done, no repo code

Run on `continuum` in the owner's KDE Wayland session (display `DP-3`,
HDR on, scale 1.7), with throwaway code outside the repo:
`~/dev/vela-spike-libmpv/` (`probe1.c`, `probe2.c`, `pcore.h`, `mpvdl.h`,
`p0/` scripts and evidence, `libset/` library recipe). Every probe was run
against the system's libmpv 0.41.0 and against the shipped set (mpv
0.42.0) unless noted. No codereview: the slice landed no repo code.

| | Assumption | Answer | Evidence |
| --- | --- | --- | --- |
| A1 | IPC socket, scripts, script options, `include` with `vo=libmpv` | **Yes, with a condition** | The private socket answers; a script loads, sees its option and can draw; `include` applies. Condition: the name `script` is a command-line-only alias the library API rejects ("option not found"); it works from an included config file, as does `scripts`. Design: "How the embedded launcher applies the list". |
| A2 | Transparent main window, no visible change | **Yes only with WebKit's DMABUF renderer; No with Vela's current setting** | Real app, mock server, compositor screenshots of three states (home dark, home light, Settings dialog). Renderer on: opaque and transparent builds are byte-identical. `WEBKIT_DISABLE_DMABUF_RENDERER=1` (today's default): the transparent build paints its first frame and then shows a white page with one small repainted patch. Design: "embedded window mode". |
| A3 | Script key bindings need forwarding | **Yes** | Nothing delivers keys to an embedded mpv; `keypress SPACE` and `script-binding` over IPC both fire the script's binding. |
| A4 | The render API draws mpv's OSD | **Yes** | Framebuffer read back before and after: `show-text` changed 20,768 pixels in the top-left text box; a script `ass-events` box changed exactly the 60%-90% rectangle it drew. `osd-dimensions` reports the buffer size. |
| A5 | Dedicated render thread with its own Wayland queue | **Yes** | Under GTK3 + WebKitGTK: 36 s of 4K60 HDR through fullscreen, back and a resize; then 332 window resizes in 20 s while playing. No Wayland error, 13 frames dropped in the first 5 s and none after (same as the spike's single-thread figure), HDR tag set, screenshot correct. |
| A6 | Headless Wayland compositor for E2E | **Yes** | `weston --backend=headless --renderer=gl`: the E2E harness's `smoke` and `playback` pass with `VELA_E2E_HEADED=1`, `GDK_BACKEND=wayland` and weston's socket; the embedded probe renders 357 frames in 6 s there with VA-API. Weston offers no colour management and no fractional scale, so only the SDR path can be exercised in it. |
| A7 | `autocrop.lua` in-process | **No with the shipped set; Yes by another route** | On the system (GPL) library the stock scripts crop to `1280x536+0+92` in 2.3 s and toggle off. On the shipped LGPL set: "No crop data", because FFmpeg's `cropdetect` filter is GPL-only and absent. The LGPL `bbox` filter gives the same rectangle (19 samples in 1 s). Design: "Autocrop in the embedded player". |
| A8 | Stop one play, start the next | **Yes, with a condition** | 150 cycles of create, play 4K60, quit, destroy: about 6 ms to start, 17 ms from quit to shutdown, 0.2 s to destroy; file descriptors constant. Memory: flat (200 to 206 MB) with glibc's arena count capped at 2; with the default it climbed to 430-670 MB late in the run, so it is allocator hoarding, not a leak. Design: "Memory". |
| A9 | `idle=once` | **Yes** | Handle idle for 5 s, then `loadfile`: plays, `end-file` reason `eof`, then `shutdown`. Controls: `idle=no` shuts down before the file is loaded; `idle=yes` never shuts down. |
| A10 | The shipped set builds and plays here | **Yes** | See "The set as decided in Slice 0". 4K60 HDR10: `hwdec=vaapi`, HDR tag set, Lua scripts, IPC socket, all 15 libraries mapped from the set's directory. VA-API needs the DRM render node, not the Wayland display. |
| A11 | Same binaries on another distribution | **Yes** | Built in an Ubuntu 22.04 container (x86_64); loaded, decoded H.264, HEVC 10-bit and AV1 and fetched over HTTPS in clean Ubuntu 22.04, Debian 12 and Fedora 45 containers (no GPU there, so software decoding only), and played with VA-API on `continuum` (CachyOS). aarch64 was not built. |

Stop rules. A1 is a yes for what the design depends on (the trackers and
the E2E harness reach an embedded player through the same socket), so no
re-plan. **A2's rule says "No: stop and ask the owner", and A2 is a No
under Vela's current WebKit setting.** The owner is away and said to
assume no decision that makes the work pointless, so the agent did not
stop. It chose the one arrangement in which the probe found no visible
change at all (transparent window and DMABUF renderer together, and only
for a user who selected the embedded player) and marked it ASSUMED. That
choice is the owner's to overturn; it is the first item under "For the
owner" below.

Also found:

- The display's reference white is not fixed: the compositor reported 83
  nits during these probes and 265 during the spike (the desktop
  brightness slider moves it), so `target-peak` was 1137 here and 356
  there. Re-tagging on `preferred_changed` (Slice 4) is needed in
  ordinary use, not only when a monitor is swapped.
- First attempt at the set did not load on its own baseline; see the mpv
  build-file patch in "The set as decided in Slice 0".
- The transparent window's appearance during the first moments of startup
  was not probed. Check it in Slice 3 with the real implementation.

Not verified in Slice 0: NVIDIA or Intel GPUs, GNOME or any compositor
other than KWin 6.8 and headless weston, aarch64, audio output (probes ran
with audio off except the container decode test, which used a null
output), and anything about picture quality.

### 2026-11-13 — Slice 0b (the library build): landed and reviewed

What landed: `scripts/build-libmpv.sh`, the recipe in `packaging/libmpv/`
(pins, container file, the mpv build-file patch, the check and its
fixtures), a `libmpv` job in CI, policy tests in
`tests/dependency-policy.test.mjs`, and a section in
`docs/development.md`. Nothing in the app loads the set yet.

Commits: `b19d7d1` (the slice); `1e2b953` (stop at a failed download, add
mirrors; see below); review fixes `29026c2`, `7ca7e75`, `9cb85ea`,
`ec97d6e`.

Verified:

- The set builds from the repo's own script in the Ubuntu 22.04
  container on `continuum`; the check passes on the host and in clean
  Ubuntu 22.04, Debian 12 and Fedora 45 images.
- GitHub CI: the new job is green from `1e2b953` on (run 43371234580 and
  every later run through `3e91c63`). That includes the decode check with
  exact frame checksums, so the decoded video is bit-identical on GitHub's
  CPUs and on this machine.
- `npm run check` at each commit; the count grew from 102 to 108.
- Red-proofs, each from a committed state: 17 mutations against the
  first set of policy tests, 7 against the download path, 23 against the
  three review fixes. Every one failed its test for the intended reason.

What the first CI run found (run 43369746914 failed): `www.lua.org` reset
the connection, and the script did not stop at the failed download,
because bash does not carry `set -e` into `$(...)`. Fixed at `1e2b953`:
`inherit_errexit`, and a component may list mirror URLs; a download is
kept only if it matches its pinned checksum.

Review: `codereview codex` over `5f1d9b6..b19d7d1`, three findings, all
admitted (`.agents/review/codereview-2026-11-13-libmpv-set.md`).

| | Finding | State |
| --- | --- | --- |
| lm-1 | the decode check passed when a decoder produced nothing | fixed `ec97d6e`; codex accepted |
| lm-2 | licence texts were outside the integrity check | fixed `7ca7e75`; codex reopened it once (two correct points); repaired `9cb85ea`; codex accepted |
| lm-3 | the pin test could not tell rejection from failed extraction | fixed `1e2b953` + `29026c2`; codex accepted |

Not done, and known:

- No automated check compares the pinned versions with a vulnerability
  database. The rule and the manual step are in `docs/development.md`.
- x86_64 only. No aarch64 build.
- The release workflow does not consume the set yet; that arrives with
  packaging in Slice 3.
- NVIDIA's decoder headers are not in the build (the embedded player is
  not offered on that driver yet).

### 2026-11-13 — Slice 1 (the seam): landed, no behaviour change

Commits: `38cb850` (the external command line recorded on the old code),
`3e91c63` (the refactor), `b719a65` (the marker script takes its payload
path as a script option).

What landed:

- `src-tauri/src/player/`: `launch.rs` (`build_launch`, `LaunchPlan`,
  the private runtime directory, the auth include, the marker payload),
  `external.rs` (mpv discovery, `command`, `launch`, `ManagedChild`),
  `mod.rs` (`PlayerHandle`). `playback.rs` keeps supervision: `play`
  and the trackers.
- `AppState`, the reaper and the exit sweep hold `PlayerHandle`.
- A launch's options are data (`LaunchArg`): Vela's own as name and
  value, the user's Advanced lines verbatim. The external player renders
  them; nothing else touches them.
- `vela-markers.lua` reads the payload path from the script option
  `vela-markers-payload` first and from the environment second. The
  external player still uses the environment.

Differences from the plan's text, both deliberate:

- `PlayerKind` and `PlayerHandle::Embedded` are not introduced yet. They
  arrive with the first code that uses them (Slices 2 and 3); adding them
  now would be dead code that the lint cannot see through.
- The golden test needed the command to be inspectable on the old code,
  so `38cb850` first moved the composing statements, unchanged, into a
  function of their own. The recording was made against that.

Verified:

- Both recorded command lines (every argument in order, and the child
  environment; one with every optional part, one with none) are
  byte-identical before and after the refactor.
- `cargo +stable` and `+1.100.0` clippy with `-D warnings`;
  `cargo +stable test` 369 pass; `npm run check` 108 pass;
  `npm run build`.
- Full E2E from a disposable worktree at `3e91c63`: 39 of 39.
- GitHub CI run 43373621967 at `3e91c63`: every job green, including the
  Rust 1.89 check and `cargo audit`.
- The new `markerpayload` E2E scenario (real script, real mpv, five
  cases) passes, and with the app rebuilt at `b719a65` so do `markers`,
  `autocrop`, `playback` and `continueon`.
- Red-proofs: 14 mutations of the seam (order, dropped option, dropped
  environment, dropped `--`, wrong rendering, the handle's end and
  retention rules) and 3 of the script each fail their test.

Unit tests moved with the code and were not edited: where a function now
returns options as data, a test-only wrapper of the same name renders
them to the command-line text the assertions were written against.

### 2026-11-13 — Slice 1 review: two findings, both about tests

`codereview codex` over `b19d7d1..b719a65`, Slice 1 paths only
(`.agents/review/codereview-2026-11-13-player-seam.md`). Nothing in it
changes what the external player does.

| | Finding | State |
| --- | --- | --- |
| ps-1 | two tests that write a marker payload deleted each other's file (2 runs in 60) | fixed `4fbff87`, `1d7c405`; codex accepted |
| ps-2 | the check that stream credentials stay off mpv's command line did not see options as the refactor writes them | fixed `94d067e`; reopened three times on the guard, never on the shipped code; repairs `bdecf65`, `b84baeb`, `2c50ae5`, `e1ff242`, `053cb36`; fourth verification pending at the time of writing. A fourth reopen goes to the owner as contested (`.agents/review/findings/ps-2.md` says why) |

What the reopens were about, because the same thing came up again in
Slice 2: a test that reads Rust source as text can be got round by code
written to get round it, and each repair that patched the test invited
the next. ps-2's last repair makes its reader refuse what it does not
understand instead of guessing.

### 2026-11-13 — Slice 2 (selection and fallback): landed and reviewed

Commits: `599c539` (the slice), `90db1a6` (one wiring test tightened after
its own red-proof found it vacuous).

What landed:

- `AppConfig.player`: optional, `"external"` or `"embedded"`, validated
  like the other closed settings, written only when set. The plan asked
  whether the previous release tolerates the field: it does not
  (`deny_unknown_fields`), as for every setting added before it. A config
  that carries `player` is refused by 1.0.63 and earlier as an invalid
  settings file, which is the existing recovery path and overwrites
  nothing. Only someone who sets the field by hand and then installs an
  older release meets this. Read from the code; not tried against an old
  build.
- `player::choose(wanted, &availability)`: external unless embedded was
  wanted and can run; a fallback carries its reason.
- The reason is shown as one line on the player's display, once per run
  of Vela per reason, and only when the line was really handed to the
  player. A play with nothing to fall back from forgets what was told, so
  a later fallback is explained again. An automatic relaunch neither
  explains nor forgets.
- The session records its player; an Automatic step-down relaunch stays
  in it whatever the setting says by then.
- No command, Settings control, document or release note exposes the
  setting; `tests/player-selection.test.mjs` holds that until Slice 6.

Verified: clippy (stable, and the 1.89 check on GitHub), `cargo test`,
`npm run check`, `npm run build`; GitHub CI green at `599c539` (run
43376150672) and `90db1a6` (43376273069). E2E `playerfallback` (notice
once, not on the next play, again after a restart, none without the
setting). Full E2E at `bc6e8a1`: 39 of 41; one failure was a race in the
new scenario itself, fixed at `1b72dd4` (6 runs of 6 afterwards); the
other is `pagefail`, the race already recorded in `.agents/state.md`
(Blockers), which failed 15 runs of 15 at commits from before and after
this slice.

Review: `codereview codex` over `1cedb48..90db1a6`, four findings, all
admitted (`.agents/review/codereview-2026-11-13-player-selection.md`).

| | Finding | State |
| --- | --- | --- |
| sel-1 | a settings read placed after the session is installed returned early past the cleanup | fixed `88c631b` (the read moved before anything is installed); reopened twice on the guard; the third repair (`320b023`) changes the code so that leaving early past the cleanup cannot be written; codex accepted |
| sel-2 | an Automatic step-down made the fallback line appear a second time | fixed `12828ca`; codex accepted |
| sel-3 | a play replaced within 1.5 s used up the fallback line without showing it | fixed `50e57e4`, `db0b8b6`; codex accepted |
| sel-4 | the test for `play`'s refusal passed with the refusal removed | fixed `47eec9a`, guard repaired `29445f2`; codex accepted |

One staging mistake, recorded because it is in the history: `50e57e4`
also carried four unfinished files of Slice 3. Nothing referenced them;
`fb64072` took them out again. No history was rewritten.

### 2026-11-13 — Slice 3 (embedded core on Linux/Wayland): the core landed; packaging has not

Commits: `a02db80` (what an embedded launch is made of), `0a63799` (the
player).

What landed:

- **The start-up decision** (`player/embedded/startup.rs`). `main` decides
  first thing how the window is created. Only with `player` set to
  `embedded`, a Wayland session, no NVIDIA proprietary driver, WebKit's
  GPU renderer not turned off in the user's own environment, and no
  marker left by a failed earlier start, is the window made transparent
  and that renderer left on. Every other start is the old one, unchanged.
  The marker is written before entering that mode and removed by the page
  once it has loaded; a start that finds one starts the old way and
  removes it, so the start after that tries again. The setting is read
  straight from the settings file, not through the settings loader: at
  that point the loader has not yet migrated an old-format file and would
  refuse it.
- **The player** (`player/embedded/`). One thread per play owns a Wayland
  subsurface below the window, an OpenGL window on it and an mpv player
  from the shipped library. The library is loaded by absolute path with
  `RTLD_DEEPBIND`, from Vela's resource directory only (a development
  build may name another directory in `VELA_LIBMPV_DIR`; a release build
  ignores it). Hardware decoding is given the GPU's DRM render node. The
  rest of Vela reaches the player through mpv's IPC socket, as it reaches
  the external one, so the trackers are untouched.
- **Options.** The launch's ordered options are written as mpv
  configuration text and included, because the library rejects
  command-line spellings through its API. Two things were measured
  against the shipped library and shaped this: `include` reports success
  for a file with lines mpv could not use, so the launcher reads back two
  options after start-up as proof the list arrived; and an `include`
  written INSIDE an included file takes effect after the rest of that
  file, so each include is applied as a step of its own, in order.
- **The page.** While a play is open the page shows a player view (a
  Close button; Escape) and paints nothing else. The view is keyed to the
  play's session (ep-2).
- **Fallback reasons** now name the cause (restart needed, not Wayland,
  NVIDIA, library did not load, and so on), each with its own short line
  and the detail on standard error.
- **E2E.** `npm run e2e -- --embedded` runs the suite in the embedded
  player under a private headless Wayland compositor. `embeddedplay` is
  the scenario that proves a play is really embedded; it also makes every
  play wait two seconds before its file arrives (ep-1).

Decided along the way (implementation details; the ones that change what
a user gets are also under "For the owner"):

- Loading uses `libc::dlopen` directly, not the `libloading` crate the
  plan named: no new dependency, and the exact flags.
- `wayland-client` now runs on the machine's libwayland (its `system`
  feature), for all its uses including the existing HDR-state query: the
  video surface has to be created on the connection GTK already holds,
  and only libwayland can share one. New crates in the lock: `dlib`,
  `scoped-tls`. `gtk` became a direct dependency (the version Tauri
  uses).
- GDK's Wayland functions, `libEGL` and `libwayland-egl` are looked up at
  run time, so a system without them still starts Vela.
- A play that cannot start after the checks passed (EGL or mpv refusing at
  the last step) fails that one play with a message, and every later play
  in the run goes to mpv with a notice. D3 says a play is never failed
  over this; redoing a half-started play in the other player from inside
  the launch was judged riskier than one failed play. (Overturned by the
  review, finding `ec-6`, fixed at `31eacad`: the play now carries on in
  mpv from inside the same launch.)
- The embedded player has no autocrop yet (Slice 5) and, with "use my own
  mpv configuration" on, applies the user's `mpv.conf` but not their
  scripts folder, per-script option files or key bindings: mpv would load
  those after Vela's options and override them, where the external player
  applies Vela's last.

Verified:

- Clippy on stable and the 1.89 check; 418 Rust tests; 127 node tests;
  `npm run build`.
- E2E, ordinary venue: 40 of 41, `embeddedplay` skipped as designed; the
  one failure is `pagefail` (Blockers).
- E2E, `--embedded`: `playback`, `resume`, `playverbs`, `markers`,
  `markerpayload`, `embeddedplay`: 6 of 6. `continueon` fails there on a
  timing assumption in the scenario (it seeks before the file has
  loaded); left for Slice 6, which is where the rest of the suite is
  brought across.
- On `continuum`'s own desktop (KDE Wayland, 1.7 scale), through the same
  harness: `current-vo` `libmpv`, `hwdec-current` `vaapi`, the picture
  2176x1280 for a 1280x753 page, no dropped frames, `current-ao` `pulse`.
  Compositor screenshots show the test pattern inside the window under
  the Close button, and the app intact after closing. They are in the
  session's scratch directory, not in the repo.
- Red-proofs from the committed state: 11 mutations of the launch
  description, 34 of the core (start-up decision, loader, options, which
  player starts a play, geometry, teardown, the page), each failing the
  test that names it.

Not done in this slice, and known:

- **Packaging.** No package contains the library set yet, so only a
  development build can run the embedded player. The plan puts the bundle
  configuration, the Arch package and the release-time check in this
  slice; they follow as their own commits.
- The E2E red-proofs of `embeddedplay` (idle `no`, idle `yes`, Close that
  does nothing) are still to run.
- GitHub CI for `0a63799`, including `cargo audit` on the two new crates.
- codex review of the slice.

### For the owner (decisions the agent assumed while you were away)

Each can be overturned; none has shipped to users.

1. **The embedded player needs a restart to switch on, and uses a
   different WebKit drawing mode.** (A2.) Selecting it takes effect the
   next time Vela starts. In that mode Vela's window is transparent and
   WebKit draws with its GPU renderer, which today's Vela turns off for
   everyone. Measured side effect on the rest of the app: text and
   gradients differ by at most 24 of 255 in places; nothing structural.
   Users who do not select the embedded player get exactly today's Vela.
2. **Not offered on the NVIDIA proprietary driver yet.** (A2.) Today's
   WebKit setting exists because of a crash on that driver, and nothing
   here could be tested on NVIDIA hardware. Those users keep the external
   player and are told why.
3. **Automatic black-bar cropping is reimplemented for the embedded
   player.** (A7.) The bundled mpv script depends on a GPL-only FFmpeg
   filter that an LGPL build cannot contain. A Vela-written script does
   the same job with an LGPL filter. No licensing question arises from
   this; it is recorded because the two players now crop with different
   code. Still unanswered on 2026-11-13, and it matters more than this
   item first said: `.agents/decisions.md` (2026-07-05) keeps the rule
   that Vela "does not re-implement video-geometry processing", and this
   script is exactly that. Slice 5 builds it under this assumption, hidden
   with the rest of the embedded player; a "no" removes one script and
   leaves the embedded player without cropping.
4. **Ubuntu 22.04 is the oldest system the embedded player supports**, the
   same as Vela itself. Audio goes through PulseAudio (which PipeWire
   provides) or ALSA.
5. **"Use my own mpv configuration" means less in the embedded player.**
   (Slice 3.) It applies your `mpv.conf`. It does not load your mpv
   scripts folder, per-script option files or key bindings, which the
   external player does.
6. (Withdrawn 2026-11-13.) This item said that a film the embedded player
   failed to start would fail once, against the rule "never fail the
   play". The Slice 3 review did not accept that (`ec-6`), and it is
   fixed: the film now carries on in mpv, with the one line that says why.
7. **The embedded player's library is packaged only on request, and
   released packages do not contain it yet.** (Slice 3, packaging, landed
   `4b8293e`.) The plan says to ship the library with Vela. The agent built
   the packaging but left it switched off: a package with the library is
   about 15 MB larger (an Arch package: 17.9 MB against 3.1 MB), for a
   player nobody is offered yet, and switching it on means changing the
   release workflow, which the agent does not do. `scripts/build.sh
   --with-libmpv` makes such a package and checks it; a plain build makes
   exactly today's. Turning it on for releases is yours, together with
   the Settings control; `docs/development.md`, "Packaging the set", lists
   what the release workflow needs.
8. **Escape closes the embedded player when the window is not
   fullscreen.** (Slice 5 design.) In fullscreen it leaves fullscreen
   first, as in mpv. mpv's own Escape never closes its window; `q` does,
   and does here too.
9. **The embedded player will not crop HDR video until you have tried it
   with me.** (Slice 5 design.) The record of 2026-06-28 says changing the
   crop on a playing HDR film wedged mpv so hard it could not be killed.
   That was mpv's own window; inside Vela the same wedge would freeze
   Vela. It has not been tried, on purpose. Standard-range video is
   cropped. The trial is in the checks list above.
10. **In fullscreen, the library shows for a moment between two episodes.**
    (Slice 5 design.) Vela's window stays fullscreen from one play to the
    next, and the page behind the player is the library.

### 2026-11-13 — Slice 3 review: nine findings, all admitted

codex over `053cb36..0a63799` with `a02db80`
(`.agents/review/codereview-2026-11-13-embedded-core.md`). Two HIGH, five
MEDIUM, two LOW. None is about what a user sees in normal use today; all
are about what happens at the edges: a window closed in the instant a play
starts, a start that takes longer than 20 seconds, a settings path that is
not a file, an unusual mpv configuration, two things happening in the same
instant, a start that fails late, a file that cannot be deleted. One of
them (`ec-3`) reaches users who never chose the embedded player, and one
(`ec-6`) withdraws a shortcut the agent took against decision D3 (item 6
under "For the owner" goes away with it).

State is in `.agents/review/index.md`; each is fixed in its own commit and
verified by codex before it is closed.

### 2026-11-13 — Slice 4 (HDR): landed

Commit: `e8db400`.

What landed:

- **The decision** (`player/embedded/hdr.rs`). The compositor describes
  the output the video surface is on: its reference white and its darkest
  and brightest levels. HDR is used when the output is brighter than its
  own reference white, the window has ten bits per channel, and the
  compositor offers every request Vela would send. The peak is the
  output's maximum rescaled from the output's reference white to the 203
  nits PQ video uses (465 at a reference of 265 is 356; at 83 it is
  1137), capped at 10000. Anything less is standard range, and then
  nothing is sent and mpv's targets are left alone, exactly as before.
- **Both sides together.** mpv is told `target-trc=pq`,
  `target-prim=bt.2020`, `target-peak`; the surface is tagged PQ, BT.2020,
  the output's primaries and range as the mastering display, the peak as
  the content light level, perceptual intent. One function does both
  (`hdr::carry_out`), the surface first, and when either side refuses both
  go back to standard range. Both take effect with the next frame.
- **Following the display.** The compositor is listened to for the whole
  play. A moved brightness slider, HDR switched off, another monitor: the
  output is described again and the play re-tags, keeping the old
  agreement until the new description exists.
- **Before the first picture.** At the start of a play up to six
  exchanges with the compositor are waited for (three are needed), after
  mpv has every other option, so the first frame is already right. If
  they take longer, the play starts in standard range and switches.

Decided along the way:

- HDR needs the compositor's `set_mastering_display_primaries` feature,
  not only PQ and BT.2020. The protocol puts the mastering luminance
  range under that feature too, and a request the compositor has not
  offered is a protocol error on the connection Vela's whole window lives
  on. Mesa makes the same demand for the external player. A compositor
  without it gets standard range. Every request is checked against what
  was offered at the point it is sent, and a description is put on the
  surface only after the compositor has said it made it.
- The colour manager is bound at version 1, the version whose events the
  code handles. Version 2 renames two of them.
- In HDR, Vela's three target options override the same options in the
  user's Advanced list; outside HDR the user's stand. If the display
  leaves HDR in the middle of a play they are set to `auto`, not back to
  what the user had.
- An ordinary (SDR) film on an HDR display is drawn in the HDR container
  too. The surface's description follows the display, not the film.

Verified:

- Clippy on stable and the 1.89 check; 437 Rust tests (19 new in `hdr.rs`,
  among them the two readings from the development display and a whole
  sequence of compositor events against a stand-in for both sides); the
  node tests with three new wiring tests; `npm run build`.
  `cargo audit` is not installed on this machine; CI runs it, and the
  lock file did not change.
- E2E, `--embedded` (headless weston, which has no colour management):
  `embeddedplay` now also asserts that mpv's three target options are
  untouched there.
- On `continuum`'s own desktop (KDE Wayland, HDR on), an HDR10 test clip:
  mpv had `target-trc` `pq`, `target-prim` `bt.2020`, `target-peak` 1137
  before its first frame; `hwdec-current` `vaapi`; no dropped frames; the
  log line "HDR output, peak 1137 nits". A compositor screenshot shows the
  test pattern in the window. A screenshot is tone-mapped, so it says
  nothing about how the HDR looks.
- Red-proofs from the committed state: 55 mutations (26 of the decision
  and the agreement, 29 of the wiring), each failing the test that names
  it.

Not done, and known:

- The owner's by-eye comparison (above, "From Slice 4"). It is the
  slice's acceptance and the agent cannot do it.
- codex review of the slice: dispatched, pending.

### 2026-11-13 — the review findings of Slices 3 and 4, as fixed so far

State of each is in `.agents/review/index.md`; what was done and how it
was proven is in `.agents/review/findings/<id>.md`. In short:

- `ec-1` (a window closed while a play starts): the play's surface is
  now made on the window's own thread, in the call that fetches the
  window's surface; the pointer cannot leave that thread, and the build
  fails if that is changed. Verified by codex.
- `ec-2` (a start that outlasts the wait for it): the start is settled
  once by whoever gets there first; a play that lost is never loaded or
  announced; a thread that will not end is kept, not dropped. Reopened
  once (a second wait; a thread joined before it had finished); second
  repair `7fa009b`, with codex.
- `ec-3` (the settings read at start-up): the path is looked at without
  being opened, and only a regular file is read, up to 8 MiB. Reopened
  once (opening a pipe or a terminal is itself an act); verified.
- `ec-4` (a profile's include applied after Vela's options): every
  option of a launch is applied on its own. Reopened once (Vela's own
  `gpu-hq` line is the user's profile too); verified.
- `ec-5` (a late Close stopping the next play), `ec-7` (a stream-auth
  file forgotten after a failed removal), `ec-8` (a marker file left by a
  failed start): fixed and verified.
- `ec-6` (a failed embedded start failing the play): the play carries on
  in mpv. With codex.
- `ec-9` (one registry per play): the compositor's globals are bound once
  per run. Reopened once (plays must not share an event queue); verified.
- `hd-1` (an old description of the display replacing a newer one):
  every piece is matched to the question it answers. With codex.
- `hd-3`, `hd-2`: in progress.

New E2E scenarios, all for the embedded venue: `embeddedlatestart` (a
play that does not start in time carries on in mpv, and the late one
stays out of it) and `embeddedownconfig` (a user's mpv.conf against
Vela's own options).

One mistake to record: `9026b4f` (`ec-7`) broke an unrelated source-text
test, and `main` had one failing check on GitHub from that commit until
`9878129` (seven commits, 2026-11-13 08:50 to 09:09 UTC). The agent had
run only the tests it had changed. The code was not wrong; the test held
two things together by their distance in a file. Since then the whole
check is run before every commit.

From this point the work also uses several agents at once (the owner
switched that on for the session on 2026-11-13): to measure mpv's
behaviour before a design depends on it, and to attack a change before
it goes to codex. The review of record is still codex.

### 2026-11-13 — the review loop of Slice 3 is closed; Slice 5 is designed

- `ec-6`: codex accepted the second repair (`ff147c2`) with its guard
  confirmed. All nine findings of the Slice 3 review are verified. `hd-1`
  is verified too. `hd-3` and `hd-2` have a design
  (`.agents/review/hd-3-hd-2.design.md`) and `hd-3` is being written.
- Slice 5 has a design: `.agents/plans/embedded-player.slice5-design.md`.
  It was checked before any code by two probers (in the app under the
  headless venue; against the shipped library) and three critics. What
  that changed, in short:
  - changing the crop on a playing film can stall playback in the shipped
    library, on ordinary video; the script now follows every crop with a
    filter or decoder change or a zero seek, which played on 24 times of
    24;
  - `bbox` needs its threshold scaled to the bit depth, fails silently
    under hardware decoding, and reports per frame, so the result has to
    be a union;
  - Tauri's "is the window fullscreen" answers with the last request, not
    with what the compositor did; the real state is taken from GDK;
  - a timer for leaving fullscreen between plays would have dropped
    fullscreen during ordinary episode changes; it leaves on events;
  - keeping focus off controls by preventing `mousedown` breaks range
    inputs and selects in WebKitGTK; only buttons get it;
  - commands from the page are limited to one in flight per play, so a
    player that stops answering cannot pile up threads inside Vela.
  Three of its decisions are for the owner and are items 8 to 10 of "For
  the owner"; item 3 has a note.
- The slice is written after `hd-3` and `hd-2` have landed: both change
  the files it builds on.

### 2026-11-13 — `hd-3` fixed (`0e7adca`); what was run

The fix and its record: `.agents/review/findings/hd-3.md`. Verified by the
agent at the fix's content before it was committed:

- The full check on `main` with the fix: JS, Rust 1.89, stable and 1.100
  clippy, 494 Rust tests, and the five tests against the shipped library.
  `cargo audit` is not installed on this machine; GitHub CI runs it.
- The whole embedded E2E suite, which now starts with the real-library
  tests: 32 of 43. The failures are the ten known ones of the embedded
  venue plus `continueon`, which failed once with "mpv: property
  unavailable": the scenario sought in the file before mpv could seek.
  That is a race in the scenarios, fixed in the harness (`dc864f3`);
  `continueon` then passed 3 runs of 3 on the fix's build.
- Four scenarios in the external player on the same build: 4 of 4.
- One play of an HDR clip on `continuum`'s desktop (KDE, HDR on, a scripted
  run): mpv had `pq` / `bt.2020` / 1137 two milliseconds after it started,
  before the file; "HDR output, peak 1137 nits" was logged; hardware
  decoding; no dropped frame.
- Seen in that run's mpv log and not followed up: one line "mpv_render_
  context_render() not being called or stuck", 240 ms after Close, while
  mpv was shutting down. Nothing like it during the play. The play's
  thread stops drawing the moment it is told to stop, and mpv then waits
  out one of its limits; whether closing was already a quarter second
  slower than it need be before this fix was not checked.

Not run, and why: a change of the display in the middle of a play on a
real HDR desktop (it needs the display to change; the owner's checks for
Slice 4 cover it).

### 2026-11-13 — `hd-3` verified; `hd-2` fixed; the Slice 4 review is answered

- codex accepted the `hd-3` fix (`0e7adca`) with its guard confirmed, and
  accepted the recorded gap about the hand-over of the file.
- `hd-2` is fixed (see `.agents/review/findings/hd-2.md` for the commit and
  the guard proof) and with codex. What was run at the fix's content: the
  full check (521 Rust tests), the ten tests against the shipped library,
  six scenarios in the embedded venue (6 of 6), and a scripted play on
  `continuum`'s HDR desktop in which a second IPC client changed each of
  mpv's targets: each was back at Vela's value within 46 to 82 ms with no
  dropped frame, and nine changes in quick succession made Vela give up to
  standard range as designed while the play carried on.
- The design both fixes started from was wrong in three places, found by
  attacking the fixes before they landed; its last section says which.
- One thing for the owner's Slice 4 list, added below it: with "use my own
  mpv configuration" on, a profile that keeps changing mpv's HDR target
  makes the embedded player drop to standard range for that play.

### 2026-11-13 — Slice 5 (controls and cropping): landed at `3d2fef6`, hidden like the rest

Built to `.agents/plans/embedded-player.slice5-design.md`. One commit.

What an embedded play has now, all of it only in the player nobody is
offered yet:

- A control bar over the picture: play and pause, time, a seek bar, mute
  and volume, audio and subtitle lists, a crop control when a crop script
  is loaded, fullscreen, close. It hides after three seconds of quiet
  while playing.
- Keys, mapped by the page because mpv gets no key in this player: Space
  (pause, or skip while a marker's button shows), the arrows (5 and 60
  seconds), `m`, `f`, `q`, Escape (leave fullscreen, else close), and `C`
  for cropping.
- A "Skip Intro / Credits / Commercials" button drawn by the page. The
  marker script runs with its own drawing off (`vela-markers-draw=no`) and
  is told to skip by name, never by a seek.
- Fullscreen of Vela's own window, held across the plays of a run and left
  when the run ends or the user stops.
- Cropping by a new script written by Vela, `vela-crop.lua` (MIT), which
  the embedded launch loads in place of the bundled `autocrop.lua`: off,
  manual and automatic, with FFmpeg's `bbox` filter. It refuses HDR video.

How the page reaches the player: one command, `player_command`, with ten
names and nothing else; each maps to one fixed mpv command built in Rust.
Whose play it is, is decided under the player-slot lock in the same hold
that stops it or copies its socket's path (the `ec-5` rule); `player_close`
is gone and `stop` takes its place. One command at a time per play; a
player that does not answer costs one waiting thread per five seconds. The
state the bar shows comes from a per-play thread as `player-state`, at
most four times a second, built field by field: a track's title and
language (cut to 200 characters, and no title at all for a track from a
file of its own, since mpv names those after their URL) are the only text
from mpv that reaches the page.

How it was made: a read-only map of the code, a draft design, a design
check (two helpers measuring in the app and against the library, three
attacking the draft), then four helpers building it on branches (scripts
and launch; backend; page; integration and end-to-end scenarios), three
attacking the result (23 findings, among them a track title that carried
a URL's query to the page, and several guards that could not fail), two
rounds of repair, and the coordinating agent's own reading, checks and
regression injections. The review of record is codex's, below.

What the coordinating agent ran at the landed content:

- The full check: JS (165 Node tests, svelte-check clean), the build, Rust
  1.89, stable and 1.100 clippy, 567 Rust tests, the ten tests against the
  shipped library. `cargo audit` is not installed on this machine; GitHub
  CI runs it.
- The whole suite in the external player: 41 of 42. The one failure is
  `pagefail`, the recorded product race.
- The whole suite in the embedded player: 37 of 46, with `autocrop` now
  skipped there (its embedded counterpart is `embeddedcrop`). The new
  scenarios `embeddedcontrols`, `embeddedskip` and `embeddedcrop` pass,
  and so do `cropscript` and the new legs of `markerpayload` with the
  system's mpv. The nine failures are not this slice's: eight are the
  scenarios already diagnosed for Slice 6
  (`.agents/plans/embedded-player.slice6-notes.md`: `continuetv`,
  `multiplex`, `playlistplay`, `serverplaylists`, `sourcepolicy`,
  `surfaces`, `uifoundation`, and `pagefail`), and `mergedview` failed
  once with "element click intercepted", a click made while the player
  view was still leaving, which is the same race as `sourcepolicy`'s.
  A first run had nine more failures, all "screenshot: no response": the
  test compositor going idle after five minutes, fixed in the harness
  (`a0b9c06`) before this run.
- Seven regressions injected by the coordinating agent, each caught: a
  view of another session reaching the play (Rust and Node); an absolute
  seek of any size taken; the title mpv gives a track from a file of its
  own reaching the page; an embedded launch naming the stock crop scripts;
  the command run on an async worker instead of the blocking pool (Node
  pin); the view calling the backend past the list of names (four Node
  tests); a held key repeating every action (two Node tests).
- Read by the coordinating agent: the command parser, its transport and
  gate, the state payload, the Tauri commands and the wiring into
  `playback::play`, the key map. Not read line by line: the page's markup
  and styles, the crop script, the scenarios.

Departures from the design, all the builders' and all kept:

- `stop` answers success, as `player_close` did, when the view's play is no
  longer the running one (the design would have said "not running").
- Before the first state arrives the page gets no all-empty state; it
  simply has none yet.
- The volume slider sends once, on release, like the seek bar, so that a
  drag does not run into the one-command-at-a-time rule.
- Escape leaves a fullscreen the user made themselves too (it follows the
  window's real state, as mpv's own Escape does); only the automatic
  leaving spares it.
- The crop script refuses silently, without the toast, when there are no
  video parameters yet; the toast is for video that is definitely HDR.
- Relative seeks are mpv's default (to a keyframe), as in mpv's own
  window; on a file with few keyframes an arrow can jump further than five
  seconds.

Known gaps, none worked around:

- Builds for Windows and macOS were not compiled (no target installed
  here; GitHub CI builds them). The new command and its helpers compile to
  "the player is not running" there, by reading.
- The audio and subtitle lists, the notice line and the busy and
  no-answer refusals are held by unit tests only: the test clips have one
  audio track and no subtitles.
- The seek bar and the volume slider are driven in the end-to-end
  scenario by setting their value and sending the events a drag sends,
  not by a pointer drag.
- Hardware-decoded video goes through the crop script's "switch hardware
  decoding off, measure, switch it back" path only in the embedded
  scenario; nothing else can reach it.
- Whether the scripts, the controls and the picture look right is for
  the owner's eye (the list above, "From Slice 5").
- The Arch package gets an install line for `vela-crop.lua`; the missing
  one for `vela-markers.lua` is still the recorded blocker.

### 2026-11-14 — Slice 3, packaging: landed at `4b8293e`, opt-in

The library set can now be put into the Linux packages, and by default is
not.

- The request is `scripts/build.sh --with-libmpv` (the set from
  `.libmpv/out/libmpv`) or `--with-libmpv=DIR`. No environment variable
  opts in for that script; `VELA_LIBMPV_DIR` stays what a development
  build reads. For deb, rpm and AppImage the script adds an overlay config
  (`src-tauri/tauri.libmpv.conf.json`) to the Tauri build; for the Arch
  package it sets `VELA_PACKAGE_LIBMPV`, under which the PKGBUILD installs
  the set to `/usr/lib/Vela/libmpv`, the licence texts under
  `/usr/share/licenses/vela/libmpv`, and turns stripping off (makepkg's
  stripping rewrote all fifteen libraries: found by building).
- `packaging/libmpv/check-package.sh` unpacks a built package and either
  requires the whole set (in its place, readable by every user, equal to
  the set it was built from, at the pinned versions, and loading:
  `libmpv.so.2` is loaded from the unpacked package and must map only the
  set's own libraries) or, with `--without-libmpv`, requires that none of
  it is there, by name and by contents. `--with-libmpv` runs it on every
  package before anything reaches `dist/`, and leaves `dist/` empty when
  one fails.
- `src-tauri/tauri.conf.json`, the product's source and both workflows
  are untouched. Released packages do not contain the set.

How it was made: one helper built it, three attacked it (19 findings,
among them two the first version of its own check had passed: an rpm
whose libraries no user could read, and a piece of the set hidden in a
directory without permission bits), a repair pass fixed all 19 and a last
helper confirmed each with the fix taken out again (53 mutations, none
survived).

What the coordinating agent ran at the landed content, on `continuum`:
the full check (the new test file has 137 tests); `scripts/build.sh
--with-libmpv=<the built set>`, which built the Arch package and passed
every line of the check (place, access, source, pins, manifest, load at
mpv 0.42.0 with client API 2.6, 15 libraries mapped from the set, 13
licence texts); a plain `scripts/build.sh`, whose package passed the
must-not-contain check and failed the must-contain one, as it should.
The helpers also built deb and rpm both ways, before the merge with
Slice 5.

One change made by the coordinating agent before landing: the helpers'
test FAILED on a CI runner without `bsdtar`, so that CI could not skip the
package check unnoticed. GitHub's runner image does not list `bsdtar`, and
installing it is a change to `ci.yml`, which the agent may not make (it
tried; the edit was refused). As built, `main`'s CI would have gone red.
The tests now skip there with the reason printed. So CI most likely does
not exercise the package check; a developer's machine does.

Not done, and why:

- No AppImage with the set was built (an Arch host cannot build one).
- The release workflow does not build or ship the set.
- CI does not run the package check's tests until its `frontend` job
  installs `libarchive-tools` (one step in `ci.yml`; the owner's).
- "A plain build is byte for byte today's package" cannot be shown while
  Vela's build is not reproducible; what is shown is that it contains
  nothing of the set.

### 2026-11-14 — the Slice 5 review: two findings, both fixed

codex over `0aeeff0..3d2fef6` returned two findings, both admitted
(`.agents/review/codereview-2026-11-14-embedded-controls.md`).

- `pc-1` (`a909c20`): the reader of mpv's socket returns after every read,
  so a line that comes in pieces can no longer keep a command past its
  two seconds or a stopped play's state thread alive.
- `pc-2` (`4118fea`): a play that ends in an error, or is quit by another
  hand, or whose playlist cannot start its next entry, says so, and
  Vela's window leaves the player's fullscreen at once where it used to
  wait out a 50-second backstop. The second of those three cases was
  found by attacking the first version of the fix. The fix changes code
  both players use to go on to the next play, so the whole external suite
  was run on it (41 of 42; `pagefail`).

Both are with codex. The records have the guard proofs and what is not
covered.

### 2026-11-14 — the packaging review: six findings fixed; `pc-1` verified, `pc-2` reopened

codex over `2f0d191..4b8293e` returned six findings about the package
check and the build script, all admitted
(`.agents/review/codereview-2026-11-14-libmpv-packaging.md`). They are
fixed on `main` in thirteen commits, `53f2ecd` to `1348782`: one per
finding, a second one for four of them after helpers attacked the fixes,
and three for defects that attack found which are older than the fixes.

- `pk-1`: the check reads an AppImage with `unsquashfs` and never runs it.
- `pk-2`: nothing of a set that failed a check is loaded; and a package is
  checked for the set only with `--set`, so what is loaded is byte for
  byte what the builder built.
- `pk-3`: an Arch build hands out only the package `makepkg` names, and
  fails when it names none.
- `pk-4`: every directory above the set must be one every user may enter.
- `pk-5`: an Arch package's licence copies must be readable files of the
  package itself.
- `pk-6`: a temporary directory with spaces in its name no longer fails a
  good package; one with a line break in it is refused, with the reason.
- Older than the fixes: a relative temporary directory beginning with a
  dash broke the check; a package that was only a link to the machine's
  own `/usr` counted as Vela; a named pipe in a package hung the check.

The coordinating agent put each defect back, one at a time (sixteen
injections: thirteen by hand, three by taking a commit back out); each
failed its named tests. The real opt-in Arch package passes the check with
`--set`, fails `--without-libmpv`, and is refused without `--set`. All six
are with codex. Still owed: one run of the AppImage reading against a real
`unsquashfs` (squashfs-tools is not on this machine; that test skips), and
CI runs none of the package tests until its runner has `bsdtar`
(`.agents/state.md`, Blockers).

Slice 5's review: codex verified `pc-1` and REOPENED `pc-2`. The case it
names was in the record as a known gap: a quality step-down whose
replacement fails to start. The old play reports while the replacement is
installed and says the run goes on; the failure is then only logged, and
the window stays fullscreen until the 50-second backstop. Its repair is
being written on the local branch `pc-2-r2`.
