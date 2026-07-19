# chr-1: Clean-EOF refresh precedes server hub eligibility

**Severity**: MEDIUM — a newly eligible next episode can remain absent from
Continue Watching until the user manually refreshes Home
**Status**: Verified — accepted by in-session Claude review, 2026-07-19
**Branch**: `fix/chr-1-post-mark-refresh`
**Base**: `f1d70f5dc421ce52913d1af02ac4e8ffb41a957a`
**Implementation commit**: `6ec2ba65acd5ecf4fbf6c12d54bfaa44e7e6f3d7`
**Last dispatched head**: none — headless MCP dispatch waived by the owner
after the transport failure; reviewed in the interactive session at head
`5a7cabf6c94492482b182ca447f8434a9abf13e4`

## Evidence

The owner reported that finishing an episode from a new series did not add its
next episode until the Refresh icon was clicked. Before `6ec2ba6`, the joined
clean-EOF dispatcher in `src-tauri/src/lib.rs::run` emitted its authoritative
`playback-ended` before awaiting `commands::mark_clean_completion_played`.
Server hub eligibility could change only after that mutation settled, leaving
no later automatic refetch.

## Predicted observable failure

With Continue Playing Off and a server hub that exposes the follow-up episode
only after PlayedItems succeeds, both automatic Home reloads finish before the
server mutation. The follow-up remains absent after the mutation settles and
appears only after manual Refresh.

## What

The dispatcher now releases playlist/Continue Playing work first, awaits the
best-effort server played-state mutation, handles its error, and emits exactly
one unconditional authoritative `playback-ended` event. The earlier tracker
event remains unchanged for quit/error progress and the pre-mutation clean-EOF
repaint.

## Approach

`src-tauri/src/lib.rs:204-234` moves only the existing dispatcher refresh. A
review-worktree-safe source guard fixes its count and ordering, while the new
Jellyfin-mock transition exposes a follow-up Resume item only after a successful
PlayedItems response. The real-app scenario proves delayed success, delayed
401, exact refresh counts, local suppression/fallback, and no successor mpv.
Version surfaces advance together to 0.1.60.

## Files changed

- `src-tauri/src/lib.rs:204-234` — release sequence work before the server await,
  then emit one unconditional Home refresh after success or failure.
- `tests/clean-eof-refresh-order.test.mjs:159-201` — lexical dispatcher
  count/order/depth guard suitable for a detached review worktree.
- `tests/e2e/mockjf.mjs:135-147,561-587` — default-disabled successful
  PlayedItems-to-Resume eligibility transition; 401 neither applies nor consumes it.
- `tests/e2e/scenarios/completionhub.mjs:206-344` — delayed success/failure
  real-app proof with Continue Playing Off and exact post-EOF request counts.
- `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`,
  `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`,
  `packaging/arch/PKGBUILD` — register the source guard and advance Vela to 0.1.60.

## Guard proof

- Exact committed implementation: `6ec2ba65acd5ecf4fbf6c12d54bfaa44e7e6f3d7`.
- Moving the dispatcher refresh back before the server await made the source
  guard fail on ordering and `completionhub` fail waiting for the newly eligible
  episode without manual Refresh.
- Emitting only after a successful server write made the delayed-401 leg fail
  waiting for the unconditional post-failure Home response.
- Adding a duplicate pre-mark dispatcher emit made the source guard report two
  emits and made the success leg miss its exact two-response boundary.
- Moving sequence release below the server await made the source guard fail and
  `continuetv` prove E2 no longer launched before E1's delayed PlayedItems response.
- Removing the tracker emit left the dispatcher source guard green but made
  `completionhub` lose its pre-response repaint and `watchstate` time out waiting
  for quit progress without restart.
- Every mutation changed production only. After each proof the exact committed
  file was restored; final `git diff --exit-code` and the source guard were green.
- Exact-head local verification passed the Node/npm pin, clean install, zero npm
  vulnerabilities, 24 frontend/source tests, Svelte diagnostics, production
  frontend build, Rust 1.89 and stable checks, warning-denied clippy, all 146 Rust
  tests, and Cargo audit with zero vulnerabilities (17 existing allowed warnings).
- The checksum-matched Linux debug app passed the focused completion/continuation/
  playlist/watch-state set and the fresh-build full real-app suite 29/29.

## Coder dispute (if any)

None.

## Known gaps

Real Plex playtest remains deferred to the owner's final pre-release smoke.
The hermetic proof uses the shared Jellyfin mock to model a server-state-
dependent Home eligibility transition; production source APIs remain unchanged.

## Reviewer comments

**Verdict: Accepted — no material issues found.** Reviewed 2026-07-19 by the
interactive Claude Code 2.1.215 session at the owner's explicit direction
after the headless MCP transport was proven unavailable. Same-vendor,
same-session review without detached-worktree isolation — recorded as a
deliberate deviation from the codereview playbook, owner-approved.

Verified against the record at `6ec2ba6` (zero code drift to head `5a7cabf`):

- `src-tauri/src/lib.rs:204-234` is a pure reorder inside the joined
  dispatcher: sequence release (`advance_playlist` + `continue-playing`)
  stays before the server await; `mark_clean_completion_played` error is
  logged; exactly one unconditional `playback-ended` follows.
- The pre-mutation repaint claim holds: the only other `playback-ended`
  emitter is the tracker tail at `src-tauri/src/commands.rs:2609`, which
  stamps recents before emitting and is untouched by this commit.
- `tests/e2e/mockjf.mjs` one-shot `resumeAfterPlayed` applies only in the
  POST-success branch; the 401 early-return neither applies nor consumes it,
  and consumption is identity-guarded against re-armed transitions.
- The source guard masks non-code, isolates the dispatcher spawn block, and
  asserts emit count, ordering, and unconditional depth; it ran green at
  head during review. `completionhub` covers delayed success, delayed 401,
  exact Resume counts, fallback hero, and no successor mpv.
- Version surfaces agree at 0.1.60 across `package.json`, both lockfiles,
  `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and the Arch PKGBUILD.

Non-blocking observation: the authoritative refresh now waits on the server
write, so a hung (never-settling) `mark_played` would defer it and stall the
serial dispatcher loop — but the mark await already sat inside this locked
block before the change, so the exposure is pre-existing, and the tracker
emit covers the visible repaint regardless.
