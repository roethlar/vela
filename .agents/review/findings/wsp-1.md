# wsp-1: Successful watch edit loses browse depth and position

**Severity**: MEDIUM — a routine successful edit tears down a large library,
returns it to page one, and forces the user to find their prior location again.
**Status**: Open
**Branch**: `fix/wsp-1-preserve-watch-edit-position`
**Commit**: pending

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

Pending implementation. Record separate production-mutation
red/restored-green evidence for buffered depth, scroll restoration, continuous
grid, stale-root ownership, offset-zero authority, failure preservation, and
confirmed local badge state. Claude independently executes the focused
macOS-capable source guard in its disposable worktree.

## Coder dispute (if any)

None.

## Known gaps

No owner playtest is required during this autonomous queue run. The deferred
final real-Plex smoke remains the pre-release manual gate. This finding does not
change general playback-ended refresh.

## Reviewer comments

Pending.

