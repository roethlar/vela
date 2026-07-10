# Plan: autocrop doesn't engage on resume (owner defect, 2026-07-10)

## Status
**DRAFT 2026-07-10 — awaiting plan review + owner go before any code.**
Owner report (autocrop playtest, 2026-07-10): fresh plays crop
automatically; resumed plays don't — the owner has to hit Shift+C.
Top of the functional queue ("queue first", owner 2026-07-10).

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

## Fix (minimal patch to the bundled script)
In `on_start`, make the settle delay unconditional — the delay becomes
"time after (re)start" instead of "position in file":

- Replace the `is_delay_needed` branch with a single delayed path:
  always schedule `detect_crop` via `timers.auto_delay` after
  `auto_delay + detect_seconds`, keeping the existing `is_cropable`
  pre-check and timer bookkeeping. (The immediate-path `else` branch is
  removed; a resumed file simply detects ~5s after it starts playing,
  same as a fresh file does today.)
- Comment the change in place as a Vela deviation from the upstream
  mpv-scripts version, with the defect one-liner and a pointer to this
  plan — the file is vendored, so the note is what prevents a future
  refresh from silently reintroducing the bug.
- No change to `playback.rs`, the config surface (`mpv_autocrop`
  off/manual/auto), Shift+C, or the `autocrop-auto=no` manual-mode
  injection. `autocrop_args` unit tests stand unchanged.

Trade-off accepted: a fresh play that starts INSIDE the first 4 seconds
(rare: only via a sub-4s stored resume point) waits the same ~5s as
everyone else; detection near dark scenes at the resume point remains
as imperfect as upstream's design (Shift+C stays the escape hatch).

## Guard (new E2E scenario `autocrop.mjs`, red→green)
The harness already drives real mpv over IPC (`tests/e2e/mpv.mjs`), and
`resume.mjs` already proves server-position resume; combine them:
1. Seed a mock-JF movie whose clip has LETTERBOX BARS — extend
   `makeClips` (or generate locally in the scenario) with an ffmpeg
   `pad` filter: content 320x140 padded to 320x180 (bars top+bottom),
   duration ~30s so `playtime-remaining` clears the script's
   time-needed check at every leg. Seed `mpv_extra_args` with
   `--script-opts-append=autocrop-auto_delay=1` to keep waits short
   (the patched path is "always delay by auto_delay"; the value is not
   the behavior under test).
2. Leg 1 (fresh play — guards the working path against regression):
   play from the grid, connect IPC, poll the `video-crop` property →
   must become non-empty (a crop ≈ 320x140+0+20) within
   auto_delay+detect+margin; then quit at ~6s (stores a resume point:
   mock `minResumeTicks` default 0).
3. Leg 2 (resume — RED today): play again (resolves with `--start` from
   the server position; `resume.mjs` proves that plumbing), poll
   `video-crop` → must become non-empty within the same window. Fails
   on the stock script (immediate detection dies, no retry, property
   stays ''), passes with the patch.
4. Guard-proof per repo rule: run the scenario against the unpatched
   script (red at leg 2), apply the patch, rerun (green), full suite
   for no-regression (the padded clip and IPC polling touch nothing
   shared).
**ASSUMPTION (verify on the VM before writing the scenario):** the VM's
mpv supports the `video-crop` property and lavfi cropdetect under
`--vo=null` (frames still decode through vf). If `--vo=null` starves
cropdetect, fall back to asserting the script's OSD/log line or use
`--vo=gpu` under Xvfb for this scenario only; discriminate at
implementation, record the outcome here.

## Non-goals
- No re-detection on every seek (`playback-restart` hook): re-cropping
  mid-watch on user seeks is new behavior the owner didn't ask for.
- No upstream-sync policy for the vendored script beyond the in-file
  deviation note.
- No new config options (delay stays script-default; overridable via
  the existing `mpv_extra_args` passthrough as today).
- No change to manual mode or Shift+C behavior.

## Verification
- Full CI set is NOT triggered by the .lua resource alone, but the new
  E2E scenario is code: `npm run check`/`build` untouched-but-cheap,
  `cargo` set only if `playback.rs` ends up touched (not planned).
- E2E on the owner's Linux VM: new scenario red→green + full suite.
- Owner playtest on the next build: resume a mid-progress HDR/letterbox
  title → bars crop automatically within ~5s, no Shift+C; fresh play
  still crops; Shift+C still toggles.

## Review log
(plan-review pending)
