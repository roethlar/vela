# pc-2: a play that ends in an error leaves Vela's window in fullscreen for 50 seconds

**Severity**: MEDIUM — after an embedded play in fullscreen ends in an error, the player view goes and the library stays fullscreen until a 50-second backstop, with no successor coming.
**Status**: In progress
**Branch**: —
**Commit**: `4118fea`

## Evidence

Reviewer, verbatim: "src/routes/+page.svelte:112 requires two playback-ended events before leaving fullscreen. An error-ending play produces only one: playback.rs:681 excludes errors from the clean-completion dispatcher that emits the second event. Executing the page helpers for fullscreen → player-closed → playback-ended produced no fullscreen-off request, only the 50,000 ms timer from +page.svelte:94."

## Predicted observable failure

Reviewer, verbatim: "When an embedded play ends with a playback error, its view disappears but the library remains in player-owned fullscreen for 50 seconds despite there being no successor."

Reviewer's suggested direction, verbatim: "Report an explicit session-scoped terminal outcome, including errors and whether a successor is pending, and leave player-owned fullscreen on that outcome. Reserve the backstop for missing events."

## What

While a play runs in the embedded player the page can put Vela's window in fullscreen, and it keeps the window there from one play of a run to the next. It leaves fullscreen when the run is over. It learnt that a run was over from the SECOND `playback-ended` of a play: the first comes from the play's tracker, before the backend has decided on a successor; the second comes from the completion dispatcher, which runs only after a clean end of file. A play that ends any other way (an error, a quit over mpv's socket that the page did not ask for, mpv dying) is reported once. The page then had nothing to go on, and left fullscreen only when its 50-second backstop fired: the view gone, the library fullscreen, nothing coming.

Reproduced in the app before anything was changed, in the embedded venue: a playlist's second play whose stream the server refuses, and a play quit over mpv's socket, each left the window fullscreen for the ten seconds the test watched.

The attack on the first fix found the same hole one step further on: a playlist that ends cleanly and whose NEXT entry cannot be started is also a run with nothing coming, and it too waited out the backstop. That is fixed here as well.

## Approach

The backend is the side that knows how a play ended and whether anything replaced it, so it says so, per session, as a flag:

- **The tracker's `playback-ended`** now also carries `sessionId` and `runOver`. `runOver` is true when the end was not a clean end of file AND no other play has become the active one (`end_is_run_over`). A replaced play is never the end of its run: a step-down, and any play the backend or the user starts, installs the new session before the old player is stopped.
- **The dispatcher's `playback-ended`** (after a clean end) carries the same two fields; `runOver` is true there when the run had a successor to start and could not start it. `advance_playlist` now answers with one of three outcomes (`AfterCleanEnd`: Continue Playing may apply; a successor started, is being chosen, or the play was replaced; the run is over) where it answered yes or no. What it does is unchanged; only the "no" is now told apart.
- **Whether the end was clean has to be known when the tracker reports.** mpv's "end of file" is read by a separate watcher thread, which could be a moment behind the tracker. For an embedded play the tracker's report therefore waits until that watcher has stopped (it stops as mpv closes its socket; a watcher that never connected gives up once the play has ended; one second at most). A play in mpv's own window reports exactly when it did before.
- **The page** (`endLeavesPlayerFullscreen` in `playerView.ts`, called from `playbackEnded`): a report that says the run is over leaves fullscreen at once, unless another play's view is showing by then. Any other report follows the old rule (no view showing, the second report, nothing pending on the page). The 50-second backstop stays for a report that never comes.
- `playback-ended` keeps its job, the refresh of watch state, and its payload only grows, by two ids-and-flag fields, for both players. Nothing reads the new fields for a play in mpv's own window.

The eight ways a play can end, and what the page now does (observed in the embedded venue unless marked):

| How it ends | What the page is told | Fullscreen |
| --- | --- | --- |
| Clean, successor started by the backend (a playlist) | tracker: not over; the successor's view opens; dispatcher: not over | kept |
| Clean, successor started by the page (Continue Playing) | tracker: not over; `continue-playing`; dispatcher: not over, with the page still deciding | kept, then left by the page's own end-of-run path if it finds nothing (by reading and unit test) |
| Clean, nothing follows | tracker: not over; dispatcher; the page's end-of-run path | left (as before) |
| Clean, the playlist's next entry cannot be started | tracker: not over; dispatcher: over | left at once |
| The user stops from the view | the view leaves fullscreen itself | left (as before) |
| An error (the server refuses the stream) | tracker: over | left at once |
| mpv quit over its socket, or dying | tracker: over | left at once (dying: by reading) |
| The quality step-down, or a play replaced by another | tracker: not over (another session is the active one) | kept (by reading and unit tests) |

Written by one helper agent, attacked by another (three findings: the playlist case above, a missing test, and the wait for the watcher being unbounded in a corner), repaired by a third; then read and verified by the coordinating agent.

## Files changed

- `src-tauri/src/commands.rs` — `end_is_run_over`, `PlaybackAdvance::saw_eof`, the tracker's report, `AfterCleanEnd`, `advance_playlist` and its two helpers; tests.
- `src-tauri/src/lib.rs` — the dispatcher uses the three outcomes and reports `runOver`.
- `src-tauri/src/playback.rs` — `eof_watch`, `after_eof_watcher`, the end-of-file watcher told when the play has ended; tests.
- `src/lib/playerView.ts`, `src/routes/+page.svelte` — the rule and its call.
- `tests/player-view.test.mjs` — one test per case, and the wiring on both sides.
- `tests/e2e/scenarios/embeddedfullscreenend.mjs` (new, six legs), `tests/e2e/mockjf.mjs` (a switch that makes the mock refuse a stream), `tests/e2e/README.md`.

## Guard proof

Run 2026-11-14 by the coordinating agent in a disposable worktree at the fix's content; `cargo +stable test --locked --lib commands::tests` and `node --test tests/player-view.test.mjs tests/player-controls.test.mjs`.

| Injection | Rust | Node |
| --- | --- | --- |
| Unchanged | PASS | PASS |
| No end is ever the end of its run (the original defect, on the backend's side) | FAIL `an_end_is_the_end_of_the_run_only_when_it_is_not_clean_and_nothing_replaced_the_play` | FAIL `the tracker's report of an end names its session and says whether the run is over` |
| A play that was replaced (a step-down) still ends its run | FAIL, the same test | FAIL, the same test |
| The page ignores what the report says (the original defect, on the page's side) | | FAIL, five tests, among them `an end in an error leaves fullscreen at its one report (case 5)` |
| An old play's error end leaves fullscreen although another play's view shows | | FAIL `a step-down keeps fullscreen: the old play is replaced, not over` |

Also run by the coordinating agent at the fix's content: the full check (577 Rust tests; `npm audit` fails on `main` for a reason of its own, see `.agents/state.md`, Blockers); six scenarios in the embedded venue, among them the new `embeddedfullscreenend` with its six legs (6 of 6; the error leg left fullscreen 6 ms after the view went); and the WHOLE suite in the external player, because the fix changes code both players use to go on to the next play: 41 of 42, the one failure being `pagefail`, the recorded product race. No panic in any app log of those runs.

The helpers ran, each alone and each failing its named test: the whole fix taken out (the scenario's error and quit legs sit in fullscreen until the test's ten seconds are up); a clean end counted as the end of a run (the playlist leg loses its fullscreen in the gap); the page ignoring the flag; the flag left out of the tracker's report; the page not handing the report to the rule; a replaced play ending its run; `saw_eof` never true; an embedded play's report not waiting for the watcher (a clean end read 400 ms late is taken for an error); an external play's report made to wait; the dispatcher never saying the run is over (the failed-successor leg sits in fullscreen); the flag made to wait for the page's own pending successor; the watcher's give-up removed, and its wait set to 30 seconds.

## Coder dispute (if any)

## Known gaps

- **A stream that fails in the middle of a play** was not staged: the test clips are small enough that mpv has them whole. The error in the scenario is a stream refused when it is opened. mpv may also report a stream cut short as a clean end; then the play counts as finished, as it does in mpv's own window.
- **The step-down, a replaced play, and a successor the page starts** are held by unit tests on both sides and by reading (the new session is installed before the old player is stopped), not end to end in fullscreen.
- **A replacement that is installed and then fails to start**: the old play's end may see the replacement still active and say "not over"; fullscreen is then left by the backstop. By reading.
- **An end that reports nothing at all** (the play's bookkeeping failed before the report) still relies on the backstop, as before.
- An embedded play's end report can be up to a second late when mpv is gone before its socket ever answered.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `0aeeff0..3d2fef6`, 2026-11-14 00:45 UTC
(`.agents/review/codereview-2026-11-14-embedded-controls.md`). Reviewer's
title: "Playback errors leave Vela fullscreen for 50 seconds". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 02:17 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d7ce-e93e-7842-b0bd-47dec7fd6aa6`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 02:07 UTC, finished 02:14 UTC.
- Reviewed head `4118fea55f8dddb27aac30cd2d18078c4aa41a3c`, base
  `a909c206eb53e21d5e8d6f1181fd0df69960b6be` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/commands.rs:4167 — A provisional replacement is treated as a guaranteed successor. During automatic step-down, the old tracker can report runOver=false before the replacement fails to launch. Cleanup at line 5391 clears the session without emitting a terminal outcome; apply_step_down only logs the failure. Replaying the actual page handlers confirms fullscreen remains until the 50-second backstop in either close/report order. Emit a terminal outcome when that replacement fails and nothing follows.
