# wsp-1: Successful watch edit loses browse depth and position

**Severity**: MEDIUM — a routine successful edit tears down a large library,
returns it to page one, and forces the user to find their prior location again.
**Status**: VERIFIED — Claude accepted with an independent guard proof
**Branch**: `fix/wsp-1-preserve-watch-edit-position`
**Base**: `dd67c069af50ae0b6dfdb0092ac0fa1321e7d6b8`
**Implementation commit**: `28f4a2d`
**Guard commit**: `2ad5b0d`
**Last dispatched head**: `32b077742febfefa610597e0d21e9d401e46f7af`
**Accepted reviewed head**: `32b077742febfefa610597e0d21e9d401e46f7af`

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

Reviewer: claude / claude-opus-4-8 / high / standard

The owner authorized a one-finding CLI transport fallback after Claude Code
2.1.214's MCP Workflow proved unable to inherit the server's `--allowedTools`
grant. Claude was dispatched headlessly on 2026-07-18 in an exact detached
worktree against base `dd67c069af50ae0b6dfdb0092ac0fa1321e7d6b8` and head
`32b077742febfefa610597e0d21e9d401e46f7af`. The CLI transcript records session
`e778b03d-5301-4af5-ab15-ceab328f50fa`, result
`34cf0b62-9dc1-4b20-8e5d-ca0cdd5884f1`, Claude Code 2.1.214, resolved model
`claude-opus-4-8`, effort `high`, no permission denials, and no web use. Its
structured verdict was `accepted` with `guard_confirmed:true`.

Claude independently changed production `setWatched` to call the old general
`refreshWatchState()` path instead of `refreshAfterWatchEdit(browseOrigin)`.
The focused guard failed for the intended reason: the dedicated
preserved-position dispatch was absent (`0 !== 1`). Claude restored the file,
observed 5/5 passing, and recorded the exact head, an empty status, and an empty
diff. The orchestrator then independently repeated the exact-head, clean-tree,
and 5/5 green checks.

Claude found no material defect. It judged the implementation the best
available way to meet the goal: origin capture precedes the server await;
publication remains server-authoritative for merged backings; buffering begins
at zero and refills through prior depth; publication and scroll restoration are
atomic and ownership-gated; and failure retains the confirmed card and complete
old grid. The structural focused guard is intentionally paired with the
behavioral Linux scenarios rather than treated as their replacement.

Historical MCP Workflow attempts remain non-verdict evidence: they found no
code defect but could not execute the independent guard because the nested
reviewer did not inherit command grants. The accepted CLI round supersedes that
transport blocker for this finding only.
