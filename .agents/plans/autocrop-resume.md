# Plan: autocrop doesn't engage on resume (owner defect, 2026-07-10)

## Status
**IMPLEMENTED 2026-07-10 — awaiting owner playtest.** Plan-review loop
CLOSED accepted at r3 (r1: 6 findings fixed; r2: 1 finding fixed; r3:
clean). Guard complete: mac-host probe red→green (Part A, tables
below), VM E2E sed-red + green with the load-marker assertion, full
suite 12/12. Shim approach chosen by owner ("if we mod it, we own that
fork" → companion script, upstream untouched; rework + review +
implement authorized "yes"). Owner report (autocrop playtest,
2026-07-10): fresh plays crop automatically; resumed plays don't — the
owner has to hit Shift+C. Was top of the functional queue ("queue
first", owner 2026-07-10).

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

**Micro-mechanism CONFIRMED by probe (2026-07-10, owner's mac host,
mpv 0.41, Vela's real video args `--profile=gpu-hq --hwdec=auto
--hwdec-codecs=all` from `playback.rs:507-509`; letterboxed 30s test
clip, stock script, `video-crop` read over IPC):**

| leg | vo | hwdec-current seen | video-crop |
|---|---|---|---|
| fresh | null | videotoolbox-copy | `320x140+0+20` ✓ |
| resume `--start=10` | null | videotoolbox-copy | `320x140+0+20` ✓ |
| fresh | gpu | videotoolbox → no | `320x140+0+20` ✓ (guard disabled hwdec) |
| **resume `--start=10`** | **gpu** | **videotoolbox (never disabled)** | **`''` — never crops** |

The confirmed chain: resume → immediate detection at `file-loaded` →
`hwdec-current` not yet established at that instant → the script's
hwdec guard (autocrop.lua:208-213, disable non-copy-back hwdec before
cropdetect) never fires → real non-copy-back hwdec (videotoolbox here;
vaapi/nvdec on Linux) engages during the 1s gather window → cropdetect
sees no frames → `detect_end` gets no metadata ("No crop data.") and
returns with NO retry. Fresh plays work only because the positional
delay defers detection past hwdec init, so the guard reads the real
value. The alternate candidate (`playtime-remaining` nil at
`file-loaded` → `is_cropable` bail) is ELIMINATED: under copy-back or
software decode the immediate path succeeds, so the time check passes.

Guard-strategy consequence (also found independently by plan-review r1,
finding 6): **the failure cannot reproduce under `--vo=null`** — hwdec
falls back to copy-back (`videotoolbox-copy` above; the Linux VM has no
hardware decode at all), where cropdetect works fine on the immediate
path. So a VM E2E leg can never be the defect's red; see Guard.

## Fix: Vela-owned companion shim; upstream stays pristine
Owner ruling 2026-07-10: "that script comes from mpv's repo. if we mod
it, we own that fork" — so the vendored `autocrop.lua` is NOT patched.
It stays byte-identical to upstream (refreshable against mpv's repo any
time); the behavior change lives in a new, small, clearly-Vela file.

1. **New resource `src-tauri/resources/mpv-scripts/vela-autocrop.lua`**
   (~35 lines, Vela-owned): on every `file-loaded`, start a settle
   timer (`delay`, default ~5s); when it fires, if `video-crop` is
   still empty, invoke the stock script's own detection through its
   public binding — `mp.commandv("script-binding",
   "autocrop/toggle_crop")` — the same entry point Shift+C hits. One
   code path for fresh plays and resumes, and detection always runs
   with hwdec settled, so the stock hwdec guard works (the confirmed
   mechanism above).
   - Options are read with an EXPLICIT identifier:
     `read_options(options, "vela-autocrop")` — without it, mpv derives
     the identifier from the filename as `vela_autocrop` and
     `--script-opts-append=vela-autocrop-delay=N` would be silently
     ignored (plan-review r1, finding 1). Override stays available via
     the existing `mpv_extra_args` passthrough.
   - **Manual activity cancels the shim** (plan-review r1, finding 3):
     the shim observes `video-crop`; if it EVER becomes non-empty
     before the timer fires (only a manual Shift+C can do that — stock
     auto is off), the timer is killed. This mirrors the stock
     script's own kill-my-timer-on-toggle semantics: a user who crops
     and then UN-crops during the delay window stays un-cropped — the
     empty-at-fire check alone would re-crop over their explicit undo.
   - Timer killed on `end-file`; observer stays passive otherwise.
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
3. **Packaging (plan-review r1, finding 2):** `packaging/arch/PKGBUILD`
   hand-installs the mpv-scripts resources (`build:arch` runs
   `--no-bundle`, so Tauri does not stage them) — add an install line
   for `vela-autocrop.lua` next to `autocrop.lua` (PKGBUILD:34), or
   the Arch build (the owner's primary platform) silently takes the
   missing-shim fallback and keeps the broken resume behavior.
   `PROVENANCE.md` gains a line stating `vela-autocrop.lua` is
   Vela-authored (repo license), not an upstream file. The Tauri
   bundles (dmg/AppImage/deb/rpm) stage the whole
   `resources/mpv-scripts/` dir via `tauri.conf.json:36` — no change.
4. No change to the config surface (`mpv_autocrop` off/manual/auto),
   Shift+C, or manual mode. Existing `autocrop_args` unit tests extend
   for the new auto shape + degradation case.

Trade-offs accepted: fresh plays now detect on the shim's fixed settle
delay (~5s wall time after load) instead of upstream's
position-conditional delay — same practical timing; and detection near
dark scenes at the resume point stays as imperfect as upstream's design
(Shift+C remains the escape hatch).

## Guard: two-part, honestly scoped (reworked after the probe +
plan-review r1 findings 4-6)

**Part A — the defect's red→green: the mac-host probe.** The VM E2E
runs `--vo=null`, where hwdec falls back to copy-back and the failure
CANNOT reproduce (probe table above; r1 finding 6 reached the same
conclusion from the script code). The genuine guard for the defect is
the recorded probe on the owner's mac host with Vela's real video args
and `--vo=gpu`: PRE-FIX output is recorded above (gpu-resume never
crops); POST-FIX the same probe with the shim injection (stock script +
`autocrop-auto=no` + shim, `vela-autocrop-delay=2`) must show
gpu-resume cropping `320x140+0+20`. Both runs recorded in this plan.
The owner playtest on a real HDR/letterbox title is the final check.

POST-FIX probe result (2026-07-10, same host/args as the pre-fix
table — **red→green COMPLETE**):

| leg | vo | hwdec-current seen | video-crop |
|---|---|---|---|
| shim fresh | gpu | videotoolbox → no (guard fired) | `320x140+0+20` ✓ |
| **shim resume `--start=10`** | **gpu** | **videotoolbox → no (guard fired)** | **`320x140+0+20` ✓ (was `''` pre-fix)** |

**Part B — VM E2E scenario `autocrop.mjs`: a functional regression net
for the shim wiring, explicitly NOT the hwdec-race guard.** It proves
the injection + shim trigger + binding invocation end-to-end through
the real app (a broken shim path, wrong script-binding name, or lost
injection fails BOTH legs); it cannot catch the hwdec race (no hardware
decode on the VM).
1. Seed a mock-JF movie whose clip has LETTERBOX BARS (ffmpeg `pad`:
   content 320x140 padded to 320x180, ~30s duration so
   `playtime-remaining` clears the stock script's time check).
   Seed config (r1 finding 4 — `seedConfig` writes only what it is
   given) with **`mpv_autocrop: "auto"`** and `mpv_extra_args`
   including `--script-opts-append=vela-autocrop-delay=1` (short waits;
   the value is not the behavior under test).
2. Leg 1 (fresh): play from the grid, connect IPC, **assert the shim's
   load marker** (`user-data/vela-autocrop/loaded`, published by the
   shim at startup — ac-r2: cropping alone cannot prove the shim
   resolved, because a lost shim degrades to the stock trigger which
   ALSO crops under `--vo=null`; the marker fails red in exactly that
   masked case), then poll `video-crop` → non-empty (≈ `320x140+0+20`)
   within delay+detect+margin; quit at ~6s to store a resume point.
3. Leg 2 (resume): play again; **assert the session actually resumed**
   (r1 finding 5 — first `time-pos` sample ≥ ~4s, the `resume.mjs`
   pattern; otherwise stale progress silently turns this into a second
   fresh leg); poll `video-crop` → non-empty within the window.
4. Red→green honesty: leg 2 is NOT claimed red against stock code on
   the VM (it is not — the probe proved stock passes there). Two red
   proofs cover the two ways the shim can be lost:
   - Trigger lost entirely (mis-wired injection): sed-delete the shim
     `--script` line while `autocrop-auto=no` stays → detection never
     triggers → the crop assertion fails. **RUN 2026-07-10: RED
     confirmed on the VM** ("timed out waiting for fresh: video-crop").
   - Shim lost but masked by degradation (ac-r2): the degradation path
     re-enables the stock trigger, which still crops under `--vo=null`
     — the load-marker assertion is the discriminator and fails red
     with no shim loaded.

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
Plan-review loop (playbook `reviewloop`, reviewer `codex exec --json
--sandbox read-only` 0.144.1, mac host).

**r1 — 2026-07-10 — verdict `reopened`, 6 findings, all ADMITTED.**
Base `9f3b930`, head `1a31548`, `guard_confirmed:false`. In parallel
with the review, a live probe (mac host + Linux VM) upgraded the
diagnosis from two labeled candidate mechanisms to ONE confirmed
mechanism (hwdec-guard race; `playtime-remaining` candidate
eliminated) — converging independently with finding 6.
1. `--script-opts-append=vela-autocrop-delay=` would be silently
   ignored: mpv derives the options identifier `vela_autocrop` from
   the filename. Fixed: shim reads options with an explicit
   `read_options(options, "vela-autocrop")`.
2. The Arch package (`build:arch` = `--no-bundle`) hand-installs
   `autocrop.lua` only — the shim would be missing on the owner's
   primary platform, taking the degradation path forever. Fixed:
   PKGBUILD install line + PROVENANCE note added to the slice.
3. The empty-at-fire guard would re-crop over a user's explicit
   crop-then-undo during the delay window (stock kills its own timer
   on toggle; the shim didn't). Fixed: the shim observes `video-crop`
   and cancels its timer the moment any crop appears.
4. The E2E seed never set `mpv_autocrop: "auto"` — neither script
   would be injected and leg 1 would time out. Fixed in the seed spec.
5. Leg 2 never asserted it actually resumed — stale progress would
   silently turn it into a second fresh leg. Fixed: assert first
   `time-pos` ≥ ~4s (the `resume.mjs` pattern).
6. The claimed VM red→green was vacuous: stock code already crops the
   resumed leg under `--vo=null` (probe-confirmed: hwdec falls back to
   copy-back). Fixed: guard restructured into Part A (mac-host probe =
   the defect's true red→green, pre-fix output recorded) and Part B
   (VM E2E = shim-wiring regression net with an honest broken-shim red
   proof).

**r2 — 2026-07-10 — verdict `reopened`, 1 finding, ADMITTED.** Base
`9f3b930`, head `f8c616f`, `guard_confirmed:false`. Part B could not
detect a MISSING/misresolved shim: the deliberate degradation path
falls back to the stock trigger, which also crops under `--vo=null`,
leaving the E2E green while real GPU resumes stay broken. Fixed: the
shim publishes an observable load marker
(`user-data/vela-autocrop/loaded`) at startup and the scenario asserts
it before the crop assertions — a lost shim now fails red regardless
of the degradation masking. (The sed-red for the trigger-lost case had
already been run and recorded; the marker covers the degradation-masked
case.)

**r3 — 2026-07-10 — verdict `accepted`, 0 comments** (reviewed_sha
`7c419d0`, base `9f3b930`; read-only pass, implementation read as
feasibility evidence). **Plan-review loop CLOSED.** Healthy converging
loop: r1 (6) → r2 (1) → r3 (clean). Post-close verification: the
marker-asserting scenario PASSES on the VM (rebuilt, resources
restaged) and the full suite is 12/12.
