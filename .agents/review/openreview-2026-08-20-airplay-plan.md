# Openreview 2026-08-20 — macOS AirPlay synchronization plan

## Status

Two owner-directed Claude sessions completed naturally over the same pinned
plan range. Neither process was terminated. Both verdicts are retained here:
the first failed only the workflow's capability proof and remains substantive
evidence; the second passed the proof and is the schema-valid openreview
outcome. No recommendation is adopted and implementation is not approved.

Owner process constraint, stated 2026-08-20 after the two sessions: never start
more than one Claude session without explicit owner approval; never impose a
low timeout, end a Claude process, or discard any verdict for any reason without
explicit owner approval. A transport/schema problem is reported with its
verdict retained; it does not authorize another session.

Reviewed range:

- base: `32ef827f8fd0198acc3d1305421fede7fcb11d6a`
- head: `9a79bb391581882430388f8cd55f12eab34cec10`

## Session 1 — retained capability-failed verdict

`openreview claude` (Claude Code 2.1.237 / `claude-opus-5` / owner-specified
`xhigh` / owner-directed frontier choice, competitive) over the pinned range:
`acceptable_with_changes`, `capability_ok:false`.

- Result UUID: `04f61625-87b3-4d9d-966f-49f0a7eaba26`.
- 28 turns, 309.638 API seconds, USD 3.2497895.
- Exact base/head pins matched.
- Claude inspected the repository and returned five evidence-backed findings.
- The required `git diff --check` was denied because the launch grant allowed
  `Bash(git:*)` while repo guidance caused Claude to try `rtk git`, ptk MCP,
  PowerShell, and a plugin shell. This was an orchestrator permission error,
  not a reviewer conclusion.
- The original ptk output handle expired after the verdict had been returned.
  The transport had already represented three long fields and one material
  change as these exact markers: `<<ccr:7f179127aa41,string,269B>>`,
  `<<ccr:22894fa5a141,string,1.8KB>>`,
  `<<ccr:b485db9466ac,string,1.4KB>>`, and
  `<<ccr:6b91c91ddf9f,string,259B>>`. They are preserved rather than silently
  reconstructed. All visible substantive conclusions follow.

### Verdict 1 material changes

1. "Before building the live monitor, verify and record whether mpv's
   coreaudio AO actually follows a mid-play macOS default-output change; the
   whole of design section 4 is load-bearing on it and no evidence is cited."
2. "Re-slice: launch-time compensation (config, validation, policy,
   expert-arg classification, query-only detector, launch wiring, wiring guard)
   ships alone first; live route tracking second; Settings UI and docs third."
3. Transport-compressed field preserved above. Its corresponding finding calls
   for bounded per-play CoreAudio property polling instead of a C callback and
   for explicitly recording that current CI is Ubuntu-only.
4. "Close the pinned-device hole: when `mpv_use_own_config` is enabled Vela
   cannot prove mpv follows the system default route, so default that path to
   Unmanaged, and raise it as a fourth owner decision alongside the three
   already flagged."

### Verdict 1 findings

#### V1-F1 — live monitor assumes mpv follows the default route mid-play

**HIGH.** Evidence: the plan establishes only mpv's selected AirPlay device and
reported latency, while acceptance requires mpv to move output during a live
play. Predicted failure: injected route/IPC tests pass, but packaged playback
remains on the old output and Vela applies a delay for a device mpv is not
using. Recommendation: perform and record a direct mid-play output-switch probe
before building the monitor; launch-time compensation remains useful regardless.

#### V1-F2 — own mpv config can pin a different device

**HIGH.** Evidence: the plan disables management for an explicit non-auto
`--audio-device`, then keeps it enabled for "Use my own mpv config" even though
Vela does not inspect that config. Predicted failure: an mpv.conf-pinned wired
DAC plays with a two-second video delay merely because the system default is
AirPlay. Recommendation: treat `mpv_use_own_config == true` as Unmanaged unless
the owner chooses otherwise.

#### V1-F3 — unsafe listener lifetime is untested by current CI

**MEDIUM.** Evidence: the plan adds a C callback, callback state, and unregister
paths; current CI uses `ubuntu-latest`, so macOS-gated code is not compiled
there. Predicted failure: later exit-path refactoring leaks a callback or
invalidates its state while canonical CI passes. Recommendation: bounded
CoreAudio property polling in a per-play thread shaped like
`spawn_health_sampler`, plus explicit disclosure of the CI gap.

#### V1-F4 — first delivery slice is too broad

**MEDIUM.** Evidence: one slice combines config, policy, CoreAudio adapter and
listener, launch wiring, live IPC monitor, and six separate regression
injections. Predicted failure: a long single-commit loop repeats the repo's
recorded vacuous-guard failure mode and delays independently useful launch
compensation. Recommendation: launch-time backend first, live tracking second,
UI/docs third.

#### V1-F5 — exact objc2 CoreAudio feature set is unproved

**LOW.** Evidence: the plan pins only `AudioHardware`, while required address or
transport declarations may have additional crate feature gates. Predicted
failure: macOS compilation forces an ad hoc dependency change and leaves the
plan stale. Recommendation: resolve exact version/features at implementation
time while preserving target-only/minimum-feature intent.

## Session 2 — schema-valid verdict

`openreview claude` (Claude Code 2.1.237 / `claude-opus-5` / owner-specified
`xhigh` / owner-directed frontier choice, competitive) over the pinned range:
`acceptable_with_changes`, `capability_ok:true`.

- Session: `577631ae-4139-45ec-b3a9-209264575813`.
- Result UUID: `a9935e96-2d7d-48d4-944c-7e6433abed26`.
- 21 turns, 268.715 API seconds, USD 1.499069.
- Model resolved exactly to `claude-opus-5`; 18,680 output tokens, including
  11,328 thinking tokens.
- Exact base/head pins matched.
- Claude read repository files and ptk ran required `git diff --check`, which
  exited zero. Two initial `Bash(rtk git ...)` calls were denied, but the
  launch-granted ptk path completed the capability proof.

### Verdict 2 goal and recommended approach

Goal: make Vela automatically compensate the roughly two-second
AirPlay/HomePod buffer on macOS, applying/removing mpv video delay as the system
route changes without per-route user action.

Claude independently chose the same core mechanism: CoreAudio default-output
device plus transport type, no device-name parsing, macOS-only framework
bindings, and negative mpv `audio-delay` at launch and over dedicated JSON IPC.
It judged the plan's evidence, config boundary, argument ordering, and separate
IPC-client pattern sound, subject to three material changes.

### Verdict 2 material changes

1. "Define 0ms setting as fully unmanaged — emit no `--audio-delay` argument,
   register no listener, and start no monitor — so it is a real off switch
   instead of a no-op that still clobbers `mpv.conf` and still spends a thread
   and CoreAudio listener per play."
2. "Replace the hardcoded reset-to-zero with a baseline captured from mpv: the
   monitor should read `audio-delay` once at startup and restore that value on
   AirPlay -> non-AirPlay, and emit no launch-time argument when the route is
   already non-AirPlay. This preserves 'Use my own mpv config' instead of
   silently zeroing a deliberate `mpv.conf` delay."
3. "Front-load a short manual spike proving that mpv hot-swaps its CoreAudio AO
   when the macOS default output changes mid-play, and record the result before
   building the live monitor — the monitor is dead weight if mpv stays on the
   old device."

### Verdict 2 findings

#### V2-F1 — zero milliseconds is not a real disable switch

**MEDIUM.** Evidence: the plan calls zero disabled while retaining route
detection, emitting zero, and starting the monitor. Predicted failure: setting
0.00 still installs every mechanism and overwrites an own-config audio delay.
Recommendation: route zero through `Unmanaged`.

#### V2-F2 — unconditional zero overrides opted-in mpv.conf

**MEDIUM.** Evidence: the plan deliberately forces zero on non-AirPlay while
the product has an explicit "Use my own mpv config" option. Predicted failure:
a deliberate non-AirPlay correction in mpv.conf silently stops working in Vela.
Recommendation: read mpv's current delay through IPC as the baseline and
restore it after AirPlay rather than assuming zero.

#### V2-F3 — mid-play output following is unverified

**MEDIUM.** Evidence: the plan builds live route management without recording
that mpv actually moves to a changed macOS default, deferring the only real
check to packaged acceptance. Predicted failure: implementation changes delay
for a route mpv is not using. Recommendation: spike the existing binary first;
let the result confirm the monitor or redirect toward relaunch/pinned output.

#### V2-F4 — per-play CoreAudio listener creates avoidable cleanup surface

**LOW.** Evidence: separate pre-spawn query, per-play registration,
post-registration re-query, and unregister-on-every-exit exist only because the
listener lifetime is per play. Predicted failure: later code misses an
unregister path and leaks callbacks. Recommendation: one app-lifetime listener
publishes classification into an atomic; each play owns only an IPC writer.

#### V2-F5 — UI platform-gating mechanism is unspecified

**LOW.** Evidence: the plan requires macOS-only rendering/tests but does not
name the mechanism; Settings has only a local `navigatorIsWindows()` helper.
Predicted failure: implementation introduces an unnecessary dependency or an
inconsistent UA check. Recommendation: explicitly extend the existing
`navigator.platform` helper unless intentionally adding Tauri's OS plugin.

## Intake and consolidation

All findings have repository evidence, a predicted observable failure, and a
proportionate severity. They are **ADMITTED as plan findings**, consolidated as:

- **AR-1 HIGH** — prove mpv's mid-play output behavior before designing live
  tracking (`V1-F1`, `V2-F3`).
- **AR-2 HIGH** — do not infer mpv's actual device/delay from the system default
  when own config can pin or offset it (`V1-F2`, `V2-F2`).
- **AR-3 MEDIUM** — replace per-play listener lifetime surface; reviewers
  conflict between bounded polling and one app-lifetime listener
  (`V1-F3`, `V2-F4`).
- **AR-4 MEDIUM** — split launch-time and live tracking into independently
  guarded slices (`V1-F4`).
- **AR-5 MEDIUM** — make 0ms genuinely unmanaged (`V2-F1`).
- **AR-6 LOW** — verify exact CoreAudio crate features (`V1-F5`).
- **AR-7 LOW** — name and guard frontend platform detection (`V2-F5`).

No finding is fixed and no material change is adopted until the owner rules on
the design choices one at a time.

## Shared conclusions and contested choices

Both verdicts agree that CoreAudio transport plus mpv delay is the right base
approach, that a real mid-play switch spike must precede the live monitor, and
that the plan must not blindly force zero over user configuration.

First owner-routed conflict — live route observation:

- Verdict 1: bounded CoreAudio polling during a play, avoiding callback
  lifetime and registration risk.
- Verdict 2: one app-lifetime CoreAudio listener publishing route state,
  avoiding polling and per-play registration.
- Draft plan: a per-play listener; neither reviewer recommends retaining it.

Next conflict after that ruling — own-config behavior:

- Verdict 1: `mpv_use_own_config` makes the play fully unmanaged.
- Verdict 2: manage AirPlay but capture and restore mpv's actual baseline delay.

Other non-conflicting material changes wait behind those rulings.
