# Plan: preserve mpv window state across automatic continuation

Status: **APPROVED 2026-07-18.** The owner approved carrying actual fullscreen
and maximized state into exact automatic successors. Manual plays retain their
configured defaults; window size and position remain untouched.

## Goal

When clean completion advances a Vela playlist, a server playlist, or Continue
Playing, the new mpv process retains the completed process's actual fullscreen
and maximized state. The change must preserve exact-session replacement,
configured mpv options, backend-neutral playback, and failed-launch behavior.

## Existing boundary

- Every item starts a new mpv process, normally with `--no-config`; runtime
  window state cannot survive without explicit transfer.
- `spawn_position_reader` already observes `time-pos` through every playback
  target's mpv IPC connection.
- `play_by_key` receives `replace_session: Some(ended_session)` only for an
  exact automatic successor. Backend playlist advancement and frontend
  `play_item(expectedSession)` both use it. Manual starts pass `None`.
- mpv applies repeated options last-wins. Inherited state must follow user
  `mpv_extra_args` and autocrop arguments so observed state wins, while staying
  before Vela's protected IPC/title/auth/start/URL block.

## State model

Add a backend-only `PlaybackWindowState` containing independently optional
`fullscreen` and `maximized` booleans. Unknown is distinct from false.

Each launch gets a fresh thread-safe observation handle. `AppState` publishes
at most one `{ session_id, observation }` record after a successful launch.
An old IPC reader can therefore update only its old handle; it cannot overwrite
the observation published for a newer session.

## Playback flow

### Observe

Extend the shared IPC reader to observe `fullscreen` and `window-maximized`
with distinct mpv observation IDs alongside `time-pos`.

- Position parsing stays numeric and unchanged.
- Window parsing accepts only boolean `property-change` events carrying the
  exact owned property name.
- Null, wrong-type, response-only, or unknown-property payloads do not invent
  state and never block playback.

### Inherit

While the existing `play_lock` is held, `play_by_key` validates the expected
session, resolves the new stream, then snapshots window state only when
`replace_session` matches the published window record's exact session ID.
Manual `None`, stale IDs, missing observations, and unknown properties inherit
nothing.

Unavailable playlist entries leave the old record intact while the existing
loop tries the next entry. Snapshot after stream resolution so any final IPC
updates from the completed process can land before replacement.

### Launch and publish

Pass the inherited snapshot and a fresh observation handle into
`playback::play`. Append known flags after user/autocrop arguments in this
order:

1. `--window-maximized=yes|no`
2. `--fullscreen=yes|no`

Emit explicit `no` so a runtime exit from fullscreen or maximized overrides a
configured `yes`. Omit unknown properties so normal configuration remains
authoritative.

Publish `{ new_session_id, fresh_observation }` only after mpv and its tracker
launch successfully, before the existing recents/start gate is released.
Resolve, spawn, and tracker failures cannot publish attempted state. Existing
session validation prevents any retained old record from authorizing later
inheritance after a failed replacement.

## Concurrency and failure rules

- No new lock spans network resolution or process lifetime; `play_lock`
  remains the replacement authority.
- A stale delayed continuation is rejected before snapshot or launch.
- Manual playback publishes its own fresh observation after success but
  inherits no old state.
- Old IPC tails mutate only their per-session handles.
- One known property may inherit while the other stays unknown.
- User close remains non-continuing; only existing clean-EOF paths transfer
  state.
- Failed resolution leaves the current process and record intact. Failed
  spawn/tracker setup follows the existing terminated-old failure behavior and
  does not publish the attempted observation.

## Implementation slice

One code slice and one version bump:

- `src-tauri/src/playback.rs`: window-state and observation types, strict IPC
  event parsing, observation registration, inherited flags, focused unit tests.
- `src-tauri/src/commands.rs`: exact-session snapshot selection, fresh-handle
  creation, success-only publication, focused session/isolation tests.
- `src-tauri/src/lib.rs`: initialize the current window-session record.
- `tests/e2e/scenarios/playlistplay.mjs`: manual non-inheritance, automatic
  true/false inheritance, configured-option precedence, successor rebinding.
- `tests/e2e/scenarios/continuetv.mjs`: frontend `expectedSession`
  continuation inherits fullscreen outside a persistent playlist.
- Version surfaces maintained by `scripts/bump.sh`: Vela 0.1.59.

## Guard design

Focused Rust tests must prove:

- strict boolean property-event parsing and wrong-payload rejection;
- independent true, false, and unknown values;
- maximized-then-fullscreen flag generation and unknown omission;
- exact-session-only snapshots; manual and stale requests inherit nothing;
- old and successor observation handles remain isolated;
- publication uses the existing success-only boundary.

The real-app `playlistplay` scenario uses configured
`--fullscreen=yes --window-maximized=yes` as a hostile baseline:

1. set the first process false/false;
2. manually replace it and require configured true/true, proving manual starts
   do not inherit;
3. set that process false/false, finish cleanly, and require its automatic
   successor false/false, proving observed `no` overrides configured `yes`;
4. set the successor true/false, finish cleanly, and require the next process
   true/false, proving both properties and fresh-handle rebinding.

The real-app `continuetv` scenario sets fullscreen true on a naturally
completed episode and requires the frontend-selected next episode to report
true. Every property mutation waits for mpv readback before EOF.

## Production mutation proofs

After implementation, mutate production only and restore exact head after each
red result:

1. stop observing fullscreen;
2. stop observing maximized;
3. append inherited flags before user options;
4. let manual `None` inherit;
5. drop exact session matching;
6. reuse the old handle or omit successor publication;
7. coerce unknown to false;
8. bypass frontend `play_item(expectedSession)` inheritance.

Each mutation must fail the focused Rust guard or its exact real-app leg and
pass after restoration.

## Verification

- canonical frontend and Rust verification from `.agents/repo-guidance.md`;
- focused Rust window-state tests;
- syntax-check changed `.mjs` scenarios;
- fresh-build Linux real-app `playlistplay` and `continuetv`;
- full fresh-build Linux real-app suite;
- Claude `codereview` over pinned base/head with an independent
  production-only mutation, red/restored-green proof, and clean exact head.

## Non-goals

- State across app restarts or unrelated manual plays.
- Window size, position, monitor, workspace, border, or always-on-top.
- Reusing one mpv process between items.
- Changes to EOF detection, playlist ordering, unavailable-entry skipping,
  Continue Playing policy, resume, autocrop, or user mpv settings.
- Treating missing observations as false or making observation a playback
  prerequisite.
