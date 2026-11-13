# Plan: an embedded libmpv player next to the external mpv player

## Status

**DRAFT 2026-11-12. NOT APPROVED. No implementation may start, including
Slice 0's probes.** Every decision is ruled (D1, D4) or ASSUMED under the
owner's standing instruction (D2, D3, D5, D6; see below). The plan now
needs only the owner's explicit go.

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
  library is missing or fails to load, embedded is unavailable and D3
  applies. A debug/development build may point at another library through
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
  interval 0, `report_swap`. Pass `MPV_RENDER_PARAM_WL_DISPLAY` at context
  creation for VA-API decoding.
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
  through (`"transparent": true`, or the builder equivalent). Its effect
  on the rest of the app and on the `WEBKIT_DISABLE_DMABUF_RENDERER=1`
  setting is ASSUMPTION A2, probed in Slice 0.

Embedded is **available** only when all hold: Linux; a Wayland GDK
display; `wl_subcompositor` and `wp_viewporter` present; the shipped
libmpv loads; an EGL config exists. `wp_color_manager_v1` is not required (SDR-only
without it). Otherwise it is unavailable and D3 applies.

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
  invokes the stock script's binding.

External plays are untouched: no player view, mpv's own window.

### Selection

New optional `AppConfig.player: Option<String>`, values `"external"` and
`"embedded"`, validated like `mpv_autocrop`; absent means `"external"` on every desktop OS (D1). iOS/iPadOS has no
external player and ignores the field. `play_by_key_locked` picks the player once per play:
configured choice → availability check → D3 fallback. The choice is
recorded on the session so `apply_step_down` relaunches in the same
player.

## Assumptions to probe before any product code (Slice 0)

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
loads it yet. Guard: a test that the pinned-versions file, the script and
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
- Transparent main window on Linux; `player_set_rect`; a minimal player
  view (video rectangle and a Close control).
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
  approved.
