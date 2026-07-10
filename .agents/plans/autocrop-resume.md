# Plan: autocrop doesn't engage on resume (owner defect, 2026-07-10)

## Status
**APPROVED-DIRECTION 2026-07-10 — shim approach chosen by owner ("if we
mod it, we own that fork" → companion script, upstream untouched;
rework + review + implement authorized "yes"). Plan review running
before code lands.** Owner report (autocrop playtest, 2026-07-10):
fresh plays crop automatically; resumed plays don't — the owner has to
hit Shift+C. Top of the functional queue ("queue first", owner
2026-07-10).

## Diagnosis (code-confirmed logic defect; micro-mechanism labeled)
The wiring is NOT the suspect, as the queue triage already noted: auto
mode just loads the bundled stock script
(`src-tauri/resources/mpv-scripts/autocrop.lua`, injected by
`playback.rs autocrop_args`), and a resume differs only by
`--start=<seconds>` on the same command line.

The defect is in the stock script's `on_start` (autocrop.lua:230-267),
which runs on `file-loaded`:

```lua
local playback_time = mp.get_property_native("playback-time")
local is_delay_needed = playback_time and options.auto_delay > playback_time
```

The `auto_delay` (4s) is treated as a POSITION IN THE FILE ("skip
intros/fade-ins near the start"), not as a settle delay after load:
- Fresh play: `playback-time ≈ 0 < 4` → detection is deferred
  ~5s (`auto_delay + detect_seconds`) past load. Works.
- Resume: `--start` puts `playback-time` at the resume position (≫ 4)
  → `is_delay_needed = false` → `detect_crop()` runs IMMEDIATELY at
  `file-loaded`, while mpv is still mid initial-seek/decoder/hwdec
  init. Detection then dies silently and there is NO retry path — the
  script's only other trigger is the manual Shift+C binding, exactly
  the workaround the owner reports using.

Two candidate micro-mechanisms for the silent death (either or both;
**ASSUMPTION to discriminate at implementation** via the E2E red leg
plus mpv log output — the plan's fix cures both identically):
1. `is_cropable` → `is_enough_time` reads `playtime-remaining`, which
   can still be nil at `file-loaded` mid-seek → `is_cropable` returns
   false → silent bail, no retry.
2. `detect_crop` reads `hwdec-current` to decide whether to disable
   non-copy-back hwdec before inserting cropdetect (autocrop.lua:208).
   At `file-loaded` hwdec isn't established yet (`hwdec-current` = "no")
   → no disable → hwdec engages during the 1s gather window → the
   cropdetect filter sees no frames → `detect_end` gets no metadata
   ("No crop data.") and returns without retry. The owner runs HDR
   passthrough setups where hwdec is active, making this path likely.

## Fix: Vela-owned companion shim; upstream stays pristine
Owner ruling 2026-07-10: "that script comes from mpv's repo. if we mod
it, we own that fork" — so the vendored `autocrop.lua` is NOT patched.
It stays byte-identical to upstream (refreshable against mpv's repo any
time); the behavior change lives in a new, small, clearly-Vela file.

1. **New resource `src-tauri/resources/mpv-scripts/vela-autocrop.lua`**
   (~25 lines, Vela-owned): on every `file-loaded`, start a settle
   timer (`vela-autocrop-delay`, default ~5s, overridable via the
   existing `mpv_extra_args` passthrough as
   `--script-opts-append=vela-autocrop-delay=N`); when it fires, if
   `video-crop` is still empty, invoke the stock script's own detection
   through its public binding —
   `mp.commandv("script-binding", "autocrop/toggle_crop")` — the same
   entry point Shift+C hits. One code path for fresh plays and resumes.
   The `video-crop == ""` guard means a user who already Shift+C'd a
   crop during the delay window is never un-cropped by the shim. Timer
   killed on `end-file`.
2. **`playback.rs`:** `PlaySpec` gains `autocrop_shim: Option<String>`
   (resolved by `commands.rs` alongside the existing script path, same
   resource resolver + existence check). `autocrop_args(mode, script,
   shim)` becomes:
   - `off`/unknown, or no stock script → no args (unchanged);
   - `manual` → stock script + `autocrop-auto=no` (unchanged);
   - `auto` → stock script + `autocrop-auto=no` + `--script=<shim>` —
     the stock auto trigger is DISABLED and the shim owns the trigger;
   - `auto` with the shim unresolved → degrade to today's stock-auto
     args (fresh plays keep cropping; resume stays broken) + one log
     line, rather than losing autocrop entirely.
3. No change to the config surface (`mpv_autocrop` off/manual/auto),
   Shift+C, or manual mode. Existing `autocrop_args` unit tests extend
   for the new auto shape + degradation case.

Trade-offs accepted: fresh plays now detect on the shim's fixed settle
delay (~5s wall time after load) instead of upstream's
position-conditional delay — same practical timing; and detection near
dark scenes at the resume point stays as imperfect as upstream's design
(Shift+C remains the escape hatch).

## Guard (new E2E scenario `autocrop.mjs`, red→green)
The harness already drives real mpv over IPC (`tests/e2e/mpv.mjs`), and
`resume.mjs` already proves server-position resume; combine them:
1. Seed a mock-JF movie whose clip has LETTERBOX BARS — extend
   `makeClips` (or generate locally in the scenario) with an ffmpeg
   `pad` filter: content 320x140 padded to 320x180 (bars top+bottom),
   duration ~30s so `playtime-remaining` clears the stock script's
   time-needed check at every leg. Seed `mpv_extra_args` with
   `--script-opts-append=vela-autocrop-delay=1` to keep waits short
   (the shim path is "always delay by the settle value"; the value is
   not the behavior under test).
2. Leg 1 (fresh play — guards the working path against regression):
   play from the grid, connect IPC, poll the `video-crop` property →
   must become non-empty (a crop ≈ 320x140+0+20) within
   auto_delay+detect+margin; then quit at ~6s (stores a resume point:
   mock `minResumeTicks` default 0).
3. Leg 2 (resume — RED today): play again (resolves with `--start` from
   the server position; `resume.mjs` proves that plumbing), poll
   `video-crop` → must become non-empty within the same window. Fails
   on today's stock-auto injection (immediate detection dies, no retry,
   property stays ''), passes with the shim owning the trigger.
4. Guard-proof per repo rule: run the scenario against today's
   stock-auto args (red at leg 2), switch to the shim injection, rerun
   (green), full suite for no-regression (the padded clip and IPC
   polling touch nothing shared).
**ASSUMPTION (verify on the VM before writing the scenario):** the VM's
mpv supports the `video-crop` property and lavfi cropdetect under
`--vo=null` (frames still decode through vf). If `--vo=null` starves
cropdetect, fall back to asserting the script's OSD/log line or use
`--vo=gpu` under Xvfb for this scenario only; discriminate at
implementation, record the outcome here.

## Non-goals
- No re-detection on every seek (`playback-restart` hook): re-cropping
  mid-watch on user seeks is new behavior the owner didn't ask for.
- No modification to the vendored `autocrop.lua` — it stays
  byte-identical to upstream (the owner's fork-ownership ruling is the
  reason this plan exists in its shim form).
- No upstream mpv PR in this slice (the positional-delay-on-resume
  behavior is arguably an upstream bug; recorded as an optional
  follow-up, owner's call).
- No new Vela config options (the shim delay has a script default,
  overridable via the existing `mpv_extra_args` passthrough as today).
- No change to manual mode or Shift+C behavior.

## Verification
- `playback.rs` is touched → full CI set: `npm run check`, `npm run
  build`, and from `src-tauri/` `cargo check/clippy/test --locked`
  (extended `autocrop_args` unit tests, guard-proven).
- E2E on the owner's Linux VM: new scenario red→green + full suite.
- Owner playtest on the next build: resume a mid-progress HDR/letterbox
  title → bars crop automatically within ~5s, no Shift+C; fresh play
  still crops; Shift+C still toggles.

## Review log
(plan-review pending)
