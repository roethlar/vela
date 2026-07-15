# eet-1: Failed edit error persists indefinitely

**Severity**: LOW — a handled edit failure leaves a permanent red status until
another edit, making resolved attention state look continuously active.
**Status**: In progress — coder verification complete; awaiting Grok r1
**Branch**: `fix/eet-1-edit-error-auto-dismiss`
**Commit**: `01e30cf`

## Evidence

At base `26a48ca`, `src/routes/+page.svelte` publishes `editStatus` in the two
watch-state catch paths and clears it only when another edit begins or the
source list changes. The markup explicitly rejects timer cleanup. The owner's
successful 0.1.49 stopped-Plex playtest confirmed the grid fix and reported
that this red line remains too long.

## Predicted observable failure

After any failed Mark watched/unwatched or Remove from Continue action, the red
action line remains for the rest of the session unless another edit occurs.

## What

Failed watch-state edit errors auto-dismiss eight seconds after publication,
while retaining immediate next-edit/source teardown clearing and attempt-owned
race safety.

## Approach

Centralize edit failure publication and clearing around one tracked 8,000 ms
timer. The captured `editAttempt` owns expiry so a canceled-but-queued older
callback cannot erase a newer failure. Use a hermetic two-timer browser probe
to exercise exact duration, immediate clearing, stale callback ownership, and
the current timer's dismissal without a production test hook.

## Files changed

- `src/routes/+page.svelte` — attempt-owned edit failure timer.
- `tests/e2e/scenarios/pagefail.mjs` — deterministic timer/race guard.
- `tests/e2e/live/plex.mjs`, `tests/e2e/live/jellyfin.mjs` — align recovery
  assertions with timer-owned dismissal.
- version metadata — bump 0.1.49 to 0.1.50 with `scripts/bump.sh`.

## Guard proof

- `tests/e2e/scenarios/pagefail.mjs`, case 4b — four separate temporary
  regressions were proven red on the Linux real-app runner, restoring the exact
  committed `+page.svelte` blob after each:
  1. changing `EDIT_STATUS_TTL_MS` from 8,000 to 9,000 failed `failure A
     requests the exact 8s promise` (`actual: []`, `expected: [8000]`);
  2. leaving the exact schedule but making the callback a no-op timed out
     waiting for failure B to auto-dismiss on its own accelerated callback;
  3. removing the callback's attempt check let A's deliberately queued stale
     callback erase B (`actual: null`, expected B's exact failure);
  4. removing `setWatched`'s immediate `clearEditStatus()` left failure A on
     screen while delayed edit B was in flight (`actual`: A's exact failure,
     `expected: null`).
- The restored committed tree passed the targeted `pagefail` scenario, then the
  full Linux real-app suite passed 18/18 at `01e30cf`.
- `tests/e2e/live/plex.mjs` passed the real stopped-Plex/restart path; the named
  failure auto-dismissed independently, the item remained unwatched/actionable,
  and both the Plex service and watchdog were confirmed active afterward.
- `tests/e2e/live/jellyfin.mjs` passed unchanged once the existing Jellyfin.app
  was running and warm. The first setup attempt found the server stopped; one
  startup-race attempt saw only Home before the authenticated Views response.
  A direct authenticated health check confirmed both real video libraries, and
  the final unchanged scenario exercised proxy outage, recovery, and dismissal.
- Local verification at `01e30cf`: all three changed `.mjs` files pass
  `node --check`; `npm run check` reports 0 errors and 0 warnings; `npm run
  build`, `cargo check --locked`, and clippy with `-D warnings` pass; `cargo
  test --locked` passes all 95 tests. The coder tree remained clean, all ten
  implementation blobs matched the Linux VM after verification, temporary live
  credentials and E2E processes were absent, and both real servers were healthy.

## Coder dispute (if any)

None.

## Known gaps

The mock cannot force `removeFromContinue` failure, and source teardown/unmount
have no safe E2E control surface. Both use the centralized helpers and remain
inspection-covered.

## Reviewer comments

Pending Grok headless review of exact base `26a48ca` and code head `01e30cf`.
