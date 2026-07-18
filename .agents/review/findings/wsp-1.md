# wsp-1: Successful watch edit loses browse depth and position

**Severity**: MEDIUM — a routine successful edit tears down a large library,
returns it to page one, and forces the user to find their prior location again.
**Status**: Review blocked — Claude MCP cannot execute the independent guard
**Branch**: `fix/wsp-1-preserve-watch-edit-position`
**Base**: `dd67c069af50ae0b6dfdb0092ac0fa1321e7d6b8`
**Implementation commit**: `28f4a2d`
**Guard commit**: `2ad5b0d`
**Last dispatched head**: `3e6c23dc6dd6b5e03a852143cb8fbb41f733f32f`
**Accepted reviewed head**: pending

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

Claude Code 2.1.214 was dispatched through its MCP Workflow route on
2026-07-18 against base `dd67c069af50ae0b6dfdb0092ac0fa1321e7d6b8`
and head `3e6c23dc6dd6b5e03a852143cb8fbb41f733f32f`. Transcript provenance records
model `claude-opus-4-8`, effort `high`, entrypoint `mcp`, and the exact harness
version. **No accepted verdict exists:** every structured result carried
`guard_confirmed:false`, so the playbook's fail-closed acceptance check rejects
it.

Three bounded dispatch attempts established the blocker rather than a code
finding:

1. Workflow's automatic isolated worktree started at `3203a38`, nine commits
   behind the pinned head, and correctly refused review.
2. An orchestrator-created exact detached worktree under `/tmp` was outside
   the nested reviewer's allowed path.
3. An exact detached worktree under the project let Opus read the finding and
   implementation. Its static review found no material defect and concluded
   that the code appears correct and appropriately handles ownership,
   buffering, merged authority, failure retention, and scroll restoration.
   It still could not execute the required source guard: nested Workflow agents
   do not receive the configured ptk MCP tool, while Bash is redirected to ptk
   or requires interactive approval that a headless reviewer cannot answer.
   `dangerouslyDisableSandbox`, background execution, and the exact authorized
   command did not change that result.

The returned `reopened` strings explicitly describe an environment-only,
inconclusive result, not a code defect. The literal T5 rule nevertheless makes
any reopened payload a frontier redispatch; this machine has no owner-confirmed
Claude frontier mapping. Review therefore fails closed pending owner direction
and a capable runner. Do not merge this branch or begin the next code slice.

## Required owner ruling

Recommended: authorize a narrowly scoped machine-local permission for the
headless Claude reviewer to run read-only git inspection and this finding's
focused `node --test` command in disposable review worktrees, and rule these
environment-only `reopened` payloads failed dispatches eligible for a fresh
standard Opus retry. This restores the approved proof without weakening its
acceptance bar.

Alternatively, the owner can require literal T5, which first needs an
owner-confirmed Claude frontier mapping and the same command-execution repair;
or explicitly waive the reviewer-executed guard and accept Opus's clean static
review plus the coder's independent mutation proofs as a one-finding exception.
Until one option is chosen, review, merge, and the next code slice remain
blocked.
