# wsp-1: Successful watch edit loses browse depth and position

**Severity**: MEDIUM — a routine successful edit tears down a large library,
returns it to page one, and forces the user to find their prior location again.
**Status**: Pending review
**Branch**: `fix/wsp-1-preserve-watch-edit-position`
**Base**: `dd67c069af50ae0b6dfdb0092ac0fa1321e7d6b8`
**Implementation commit**: `28f4a2d`
**Guard commit**: `2ad5b0d`
**Reviewed head**: pending evidence commit

## Evidence

`src/routes/+page.svelte::setWatched` calls `refreshWatchState()` after the
server edit succeeds. Its ordinary browse branch calls
`resetAndLoad({ preserve: true })`; on success that resets offset to zero,
assigns `items = []`, recreates the grid, and loads only the first page. The
owner reproduced the full-page refresh and lost scroll position live on
2026-07-18.

## Predicted observable failure

With at least two loaded pages, a successful Mark watched temporarily removes
the cards, leaves only page one, and resets grid scroll to zero. Replacing the
refetch with only the clicked card's local state creates a second failure: a
merged title can display unwatched while another backing remains watched.

## What

Preserve the mounted grid, loaded depth, pagination capability, and scroll
position while retaining a fresh server-authoritative listing after a
successful manual watched-state edit.

## Approach

Build the loaded page range into a private buffer from offset zero, then
atomically publish it under exact listing/navigation/generation ownership and
restore scroll after the DOM update. Keep the old grid on listing failure.
Home and one-shot query roots retain their live refreshes; explicit Refresh and
playback-ended behavior are unchanged.

## Files changed

- `.agents/plans/watch-edit-position.md` — approved binding design and proofs.
- `src/routes/+page.svelte` — stable listing descriptor, buffered revalidation,
  edit routing, and scroll restoration.
- `tests/watch-edit-position.test.mjs`, `package.json` — local structural guard
  in the canonical frontend check.
- `tests/e2e/scenarios/watchposition.mjs` — depth, position, failure, and stale
  publication behavior.
- `tests/e2e/scenarios/markwatched.mjs`, `mergedview.mjs` — continuous-grid,
  server-authority, and merged-backing guards.
- version surfaces maintained by `scripts/bump.sh` — Vela 0.1.58.

## Guard proof

Every production mutation below was made only after the implementation and
guard commits, failed for the stated reason, then was restored with
`apply_patch` to the committed source. The focused source guard returned 5/5
after every restoration, and the restored production file remained identical
to `HEAD`.

1. Route success back through the old general refresh: the source guard lost
   the dedicated call; Linux `markwatched` reported that the card did not
   remain mounted, and `watchposition` lost the confirmed badge during the
   delayed reload. Restored runs passed.
2. Cap the buffered target at one page: the source guard rejected the lost
   prior depth; Linux `watchposition` timed out waiting for offsets 0 and 60.
   Restored `watchposition` passed.
3. Restore scroll to zero instead of the captured value: the source guard
   rejected the argument; Linux `watchposition` observed `scrollTop` change
   from 777 to 0 during the continuity hold. Restored `watchposition` passed.
4. Blank the buffer path or clear listing state in its failure handler: the
   source guard rejected the second `items` publication; Linux
   `watchposition` reported that the browse grid was removed or replaced in
   the failed-revalidation leg. Restored `watchposition` passed.
5. Remove the ownership check immediately after a page fetch: the source guard
   saw two ownership boundaries instead of three; Linux `watchposition`
   observed stale old-root requests at offsets `[0, 60]` instead of stopping
   at `[0]`. Removing both pre-publication ownership boundaries then changed
   the destination's exact mounted card set when the old-root buffer settled.
   Restored `watchposition` passed after both mutations.
6. Skip the authoritative listing revalidation: the source guard lost its
   exact dispatch; Linux `markwatched` timed out waiting for the server Items
   refetch and `mergedview` timed out waiting for both fresh merged requests.
   Restored runs passed 2/2.
7. Remove the confirmed local `item.played` publication: the source guard
   rejected the missing assignment; Linux `markwatched` observed the watched
   badge disappear during the delayed authoritative refetch. The restored run
   passed.

The exact twelve changed implementation/test/version files were SHA-256
matched onto the Linux venue before the final run. Local final frontend gates
on Node 26.5.0/npm 12.0.1 passed: 23/23 source tests, Svelte 0 errors/0
warnings, and the production build. The earlier full cross-language run on the
implementation commit passed `npm ci`, both audits, MSRV/stable checks, clippy
with warnings denied, and 140 Rust tests. The final exact-source fresh-binary
Linux real-app suite passed 28/28 before review dispatch; the focused
`watchposition` restoration after the final ownership mutation also passed.

## Coder dispute (if any)

None.

## Known gaps

No owner playtest is required during this autonomous queue run. The deferred
final real-Plex smoke remains the pre-release manual gate. This finding does not
change general playback-ended refresh.

## Reviewer comments

Pending Claude Opus review.
