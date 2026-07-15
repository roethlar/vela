# dlr-s8-3: live Jellyfin mistakes Home for a loaded server library

**Severity**: MEDIUM — the required live integration gate fails before testing
Jellyfin even though the authenticated server has libraries and is healthy.
**Status**: In progress
**Branch**: `main` (approved dependency-refresh Slice 8)
**Commit**: pending

## Evidence

At versioned Slice 8 head `39b233a`, two consecutive live Jellyfin runs on the
new matching WebKit driver failed at `tests/e2e/live/jellyfin.mjs:138` because
the selected non-Home sidebar item was null. The wait immediately above used
`querySelectorAll('button.sideitem').length > 0`; the static Home button
satisfies that condition before asynchronous server libraries arrive. During
the failure the VM reached Jellyfin's public endpoint, and a token-safe direct
`/Users/{id}/Views` probe returned HTTP 200 with two libraries.

## Predicted observable failure

When WebDriver reaches the ready DOM before the library request renders, the
wait returns on Home alone and the next statement fails `the real server must
offer at least one library`. None of the intended browse/offline/edit/restart
behavior runs, producing a false red release gate.

## What

Wait for a non-Home sidebar entry rather than any sidebar entry before selecting
the real Jellyfin library.

## Approach

Keep the existing document-ready condition, but require that at least one
`.sideitem` has trimmed text other than `Home`. The subsequent selection and
assertion remain unchanged, so this fixes only readiness and does not weaken
the proof that the real server supplies a library.

## Files changed

- `tests/e2e/live/jellyfin.mjs` — wait for an actual server library.

## Guard proof

Pending: land the corrected predicate, restore the old any-item predicate in a
disposable committed worktree and require the exact null-library failure, then
restore the fix and require the complete live Jellyfin scenario to pass.

## Coder dispute (if any)

None. The repeated failure and direct API proof admit the harness race.

## Known gaps

The scenario is intentionally non-hermetic and owner-server dependent; it is
not promoted into the normal gating suite.

## Reviewer comments

Pending external Grok review.
