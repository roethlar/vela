# fwer-1: Failed watch edit reloads and can lose the browse grid

**Severity**: HIGH — one failed card edit can blank or permanently truncate the
entire loaded library while leaving the item unchanged on the server.
**Status**: Awaiting Grok review
**Branch**: `fix/fwer-1-failed-watch-edit-recovery`
**Commit**: `b5c170a`

## Evidence

At base `012a031`, `src/routes/+page.svelte:1765` routes a same-root failed
`setWatched` catch through `refreshWatchState()`. That reaches
`resetAndLoad({ preserve: true })`, which publishes `items = []` at
`src/routes/+page.svelte:1106` while a dead server is rediscovered. The owner's
0.1.48 Plex playtest lost the entire Movies grid and then omitted **12 Years a
Slave**, although Plex still returned that exact unwatched item at index 5 of
the first page.

## Predicted observable failure

With Plex unavailable, Mark watched can make every loaded card disappear for
the timeout window; an overlapping generation can prevent the preserved pages
from restoring. The action also creates a separate view failure even though a
failed edit produced no new listing truth.

## What

Failed watch-state edits must repair only Home's potentially transient
recents/tombstones. Browse, search, person, drill, and detail listings must keep
their exact loaded identity, pagination, and scroll, with only the named edit
failure appearing.

## Approach

`repairFailedWatchEdit` invalidates hidden Home hubs and reloads Home only when
Home is the active underlying root. `setWatched` calls that narrow repair after
backend rollback; the obsolete `rootSig` recovery gate is deleted. The
hermetic case asserts exact poster identity continuously and proves that no
Items request occurs. The opt-in Plex case targets **12 Years a Slave** and
checks the same identity and unwatched action through the real outage/restart.

## Files changed

- `src/routes/+page.svelte` — narrow failed-edit repair; delete browse recovery gate.
- `tests/e2e/scenarios/pagefail.mjs` — hermetic no-request/exact-identity guard.
- `tests/e2e/live/plex.mjs` — exact real-Plex regression path.
- version metadata — bump 0.1.48 to 0.1.49 with `scripts/bump.sh`.

## Guard proof

- `tests/e2e/scenarios/pagefail.mjs`, case 4 — four separate temporary
  regressions were proven red on the Linux real-app runner, restoring the
  committed tree after each:
  1. the old delayed `refreshWatchState()` catch emptied the grid: `poster
     cardinality changed`, `0 !== 60`;
  2. the old undelayed catch made an unnecessary listing request: `a failed
     edit has no new browse truth and must not request the listing`, `7 !== 6`;
  3. a synthetic failed-edit view error violated the independent surface guard:
     `the failed edit must not manufacture a view failure`;
  4. a same-length in-memory substitution kept 60 cards but failed exact
     identity: poster 59 changed from `Movie 059 — 2019` to `Movie 059
     replacement — 2019`.
- The restored committed tree then passed `pagefail` on the Linux real-app
  runner. The full hermetic Linux suite passed 18/18 at `b5c170a` before the
  temporary proof mutations.
- `tests/e2e/live/plex.mjs` — passed against the exact owner report: the target
  stayed present continuously, no view failure appeared, and Mark watched
  remained offered after Plex restart plus Refresh.
- Local verification at `b5c170a`: both changed `.mjs` files pass
  `node --check`; `npm run check` reports 0 errors and 0 warnings; `npm run
  build`, `cargo check --locked`, and clippy with `-D warnings` pass; `cargo
  test --locked` passes all 95 tests; `git diff --check` passes. The Plex
  service and watchdog timer were both confirmed active after the live run.

## Coder dispute (if any)

None.

## Known gaps

The owner must repeat the stopped-Plex playtest in the installed app after the
review loop closes. The live test is opt-in and non-gating by design.

## Reviewer comments

Pending Grok headless review of exact base `012a031` and code head `b5c170a`.
Acceptance requires Grok's independent guard proof and
`guard_confirmed: true`.
