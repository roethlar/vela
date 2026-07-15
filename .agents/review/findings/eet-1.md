# eet-1: Failed edit error persists indefinitely

**Severity**: LOW — a handled edit failure leaves a permanent red status until
another edit, making resolved attention state look continuously active.
**Status**: In progress
**Branch**: `fix/eet-1-edit-error-auto-dismiss`
**Commit**: pending

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

Pending the four separate injected regressions and restored green runs required
by `.agents/plans/edit-error-auto-dismiss.md`.

## Coder dispute (if any)

None.

## Known gaps

The mock cannot force `removeFromContinue` failure, and source teardown/unmount
have no safe E2E control surface. Both use the centralized helpers and remain
inspection-covered.

## Reviewer comments

Pending Grok headless review after commit and coder guard proofs.
