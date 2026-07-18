# pws-1: Automatic playback loses mpv window state

**Severity**: MEDIUM — every automatically continued item opens with configured
defaults instead of the window state the user just chose, forcing repeated
fullscreen/maximize actions through a playlist or episode run.
**Status**: In progress — implementation and coder guard proof complete; Claude
code review pending
**Branch**: `fix/pws-1-playback-window-state`
**Base**: `f40da74a31be1ab4554b142ce0c1d5ee8b594e9d`
**Implementation commit**: `97091e624aeb1376a692c74a9b79d8b674efd30a`
**Last dispatched head**: pending

## Evidence

The owner reported that the next item in a playlist opens windowed and must be
maximized again. `src-tauri/src/commands.rs::play_by_key` starts a new mpv
process for every item, while `src-tauri/src/playback.rs::play` previously
captured neither `fullscreen` nor `window-maximized`. The automatic/manual
boundary already exists: exact automatic successors supply the completed
session id; manual plays supply no replacement session.

## Predicted observable failure

After changing fullscreen or maximized state in one mpv process, a clean-EOF
playlist or Continue Playing successor reports its configured/default state
instead of the predecessor's actual state. A manual replacement must still use
the configured defaults and must not inherit the process it replaces.

## What

Carry independently known fullscreen and maximized booleans from one
successfully launched session into only its exact automatic successor. Preserve
manual launch behavior, configured mpv options, failed-launch behavior, and all
window geometry outside those two booleans.

## Approach

Each playback launch receives a fresh thread-safe observation handle populated
by the existing shared mpv IPC reader. `AppState` publishes the handle with its
session id only after the full launch succeeds. `play_by_key` snapshots it only
when the supplied replacement id matches exactly, then appends explicit known
state after user/autocrop options so observed runtime state wins under mpv's
last-value-wins rules. Unknown values remain omitted.

## Files changed

- `src-tauri/src/playback.rs` — tri-state value and per-launch observation,
  strict IPC parsing, property subscriptions, inherited launch flags, and unit
  guards.
- `src-tauri/src/commands.rs` — exact-session snapshot selection, fresh handle
  creation, and success-only publication.
- `src-tauri/src/lib.rs` — current window-session record in application state.
- `tests/e2e/scenarios/playlistplay.mjs` — hostile configured baseline, manual
  non-inheritance, false/false inheritance, and fresh true/false rebinding.
- `tests/e2e/scenarios/continuetv.mjs` — frontend `expectedSession` fullscreen
  inheritance outside a persistent playlist.
- version surfaces maintained by `scripts/bump.sh` — Vela 0.1.59.

## Guard proof

Every mutation below changed production only after implementation commit
`97091e6`, failed for the stated reason, then was restored with `apply_patch`.
The named focused guard passed after each restoration, and the local tree
returned byte-identical to the committed head.

1. Removed the `fullscreen` IPC subscription. Fresh Linux `continuetv` timed
   out waiting for E2 to inherit fullscreen; restored `continuetv` passed.
2. Removed the `window-maximized` IPC subscription. Fresh Linux `playlistplay`
   timed out waiting for the automatic successor to override the configured
   maximized `yes` with observed `false`; restored `playlistplay` passed.
3. Moved inherited flags before user options. Fresh Linux `playlistplay`
   failed the same hostile-baseline state assertion because configured `yes`
   won; restored `playlistplay` passed.
4. Allowed manual `replace_session: None` to sample the current observation.
   `automatic_window_state_requires_the_exact_replaced_session` failed with
   observed true/false instead of unknown; restored test passed.
5. Removed the exact session-id comparison. The same Rust guard failed its
   stale-continuation assertion; restored test passed.
6. Removed success-only publication of the new window-session record. Fresh
   Linux `playlistplay` failed at the first automatic inherited-state
   assertion; restored `playlistplay` passed.
7. Coerced unknown properties to false launch flags.
   `inherited_window_args_emit_known_values_in_override_order` failed because
   an unknown state emitted flags; restored test passed.
8. Dropped frontend `play_item(expectedSession)` from `play_by_key`. Fresh Linux
   `continuetv` timed out waiting for E2 to inherit fullscreen; restored
   `continuetv` passed.

The exact eleven implementation/test/version files were SHA-256 matched onto
the Linux venue after restoration. Final local verification passed the pinned
Node/npm assertion, clean npm install, zero-vulnerability npm audit, 23/23
frontend source tests, Svelte with zero errors/warnings, production build,
Rust 1.89 and stable checks, clippy with warnings denied, all 146 Rust tests,
and cargo audit with no vulnerabilities (17 allowed upstream
unmaintained/unsoundness warnings). The final exact-source fresh-build Linux
real-app suite passed 28/28.

## Coder dispute (if any)

None.

## Known gaps

The macOS owner playtest was not run during this autonomous queue slice.
Window size, position, monitor/workspace placement, and state across unrelated
manual plays or app restarts remain intentionally out of scope.

## Reviewer comments

Pending Claude code review.
