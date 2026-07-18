# wsp-1: Successful watch edit loses browse depth and position

**Severity**: MEDIUM — a routine successful edit tears down a large library,
returns it to page one, and forces the user to find their prior location again.
**Status**: Review blocked — Claude MCP Workflow cannot inherit command grants
**Branch**: `fix/wsp-1-preserve-watch-edit-position`
**Base**: `dd67c069af50ae0b6dfdb0092ac0fa1321e7d6b8`
**Implementation commit**: `28f4a2d`
**Guard commit**: `2ad5b0d`
**Last dispatched head**: `557865c1c66c2d19d3c2cae0a378ab6dcdd5dc2b`
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

Claude Code 2.1.214 was most recently dispatched through MCP Workflow run
`wf_d26d3564-551` on 2026-07-18 against base
`dd67c069af50ae0b6dfdb0092ac0fa1321e7d6b8` and exact head
`557865c1c66c2d19d3c2cae0a378ab6dcdd5dc2b`. Its transcript records model
`claude-opus-4-8`, effort `high`, entrypoint `mcp`, the exact worktree cwd, and
Claude Code version 2.1.214. The structured result was `invalid` with
`guard_confirmed:false`, explicitly because command execution was unavailable;
it reported no material code defect. No accepted verdict exists.

The owner rejected settings-file permission rules and directed the correct
server launch mechanism: `--allowedTools`. That path was implemented and
proved rather than assumed:

1. The MCP registration now launches
   `claude --allowedTools=Read,Glob,Grep,Edit,Bash(*) mcp serve`.
2. The server's direct Bash tool ran
   `node --test tests/watch-edit-position.test.mjs` successfully, 5/5, in the
   exact detached review worktree without an approval prompt.
3. The Workflow-launched Opus reviewer still received an approval refusal for
   every code-executing Node form. Workflow agents do not inherit the server's
   individual allowed-tool grants and do not receive the configured ptk MCP
   tool.
4. A server-level `bypassPermissions` probe did not propagate to Workflow.
   Command-line and user custom-agent permission modes were not visible in the
   Workflow registry, and the MCP `Agent` endpoint exposed no runnable agent
   type. These bounded probes left the exact worktree clean and were removed.

Opus independently performed another full static review. It found the
implementation coherent: the edit origin is captured before the server await;
confirmed local state publishes only after success; buffered revalidation
starts at zero, refills through prior depth, publishes once, restores scroll
after `tick()`, and gates fetch/publication/restoration on exact ownership; the
failure path retains the old grid. This is useful review evidence but does not
replace the playbook's reviewer-executed red/green proof.

The earlier `reopened` and current `invalid` payloads all describe this same
environment-only failure, not a code objection. Review therefore remains
failed closed. Do not merge this branch or begin the next code slice.

## Required owner ruling

Recommended: authorize this finding's independent review through the headless
Claude CLI using Opus/high, the same allowed tools, the same exact disposable
worktree, and the same structured verdict. Claude remains the reviewer; only
the transport changes from MCP to CLI, because the installed MCP Workflow
cannot pass command authority to its reviewer process.

If MCP-only review remains mandatory, the executable reviewer guard cannot be
satisfied on Claude Code 2.1.214. The remaining alternative is an explicit
one-finding waiver accepting the two clean Opus static reviews plus the coder's
separate production-mutation proofs. Until the owner chooses one of those two
paths, review, merge, and the next code slice remain blocked.
