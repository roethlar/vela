# ec-2: a start that times out lets go of a worker that is still running

**Severity**: HIGH — the worker keeps its thread, its surface and its player with nothing left that can stop it or wait for it, and if it comes back it hands mpv the file and announces a play that was already reported as failed. It needs a start that takes longer than 20 seconds.
**Status**: In progress
**Branch**: —
**Commit**: `af4f142`, `d7d850a`

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/session.rs:358-366 and 433-439 ignore false from stop_and_wait. A setup operation that remains blocked, such as reading an Advanced --include pointing to an unwritten FIFO, survives the timeout. A review probe confirmed that dropping Session returns after five seconds while its worker remains alive; the existing teardown wiring test nevertheless passes."

## Predicted observable failure

Reviewer, verbatim: "Failed starts leave untracked threads, descriptors and graphics resources alive. If setup later resumes, it still loads the file and emits Opened before checking stop, despite the play already having failed; borrowed window lifetimes are no longer protected."

Reviewer's suggested direction, verbatim: "Retain ownership of timed-out workers until teardown actually completes, and cancel setup before loading or publishing lifecycle events. A timeout must not authorize releasing the worker's dependencies."

## What

`Session::start` waits 20 seconds for the play's thread to report that mpv has its file, and then reports that the play did not start. A thread cannot be stopped from outside, so the play's thread may still be running then. Two things followed. The caller waited five more seconds and dropped the thread's handle, after which nothing in Vela knew the thread existed while it still held a surface under the window, an OpenGL context and an mpv player. And a thread that came unstuck later carried on where it was: it gave mpv the file and announced the play to the page, for a play that had already been reported as failed, because it looked for a stop request only once it was in its drawing loop.

## Approach

Whether a starting play has started is now settled once, by whichever side gets there first (`StartRace`): the play's thread claims the start after mpv has taken the file, the caller gives up when its wait runs out, and a compare-and-swap lets exactly one of them win. The last step of a start is one function, `finish_start`: a play whose caller has already given up is not handed its file; one that loses the race in the moment after is torn down; in neither case is anything announced. Handing mpv the file moved out of `set_up` into that step. The caller's side reports "not in time" only if it won; if the play won in the same instant it is a started play like any other.

A thread that has not ended in the time it is given (five seconds after being asked to stop, on a failed start or when a running play is dropped) is no longer let go of. Its handle goes to `Leftovers`, a list kept for the run. While any play in it is still running, `Host::library` refuses to start another embedded play (the play goes to the external player); a play that has ended is taken off the list and joined the next time anyone asks; and the exit sweep gives the list two seconds before the process goes.

## Files changed

- `src-tauri/src/player/embedded/session.rs` — `StartRace`, `Started`, `finish_start`, `Leftovers`, `unfinished_plays`, `wait_for_unfinished_plays`, `Live::load`; the play's thread and `Session::start` use them; `Drop` keeps instead of dropping; eight tests.
- `src-tauri/src/player/embedded/host.rs` — `library` refuses while a kept play is running.
- `src-tauri/src/player/embedded/mod.rs`, `src-tauri/src/lib.rs` — the exit sweep's last wait.
- `tests/embedded-player.test.mjs` — one new wiring test; three pins follow the moved text.

## Guard proof

Run 2026-11-13 at `af4f142`, and the two marked (*) again at `d7d850a`, in a disposable worktree; `cargo +stable test --locked --lib embedded::session` and `node --test tests/embedded-player.test.mjs`.

| Injection | Rust | Node |
| --- | --- | --- |
| The original defect: a play that has not ended in time is let go of | FAIL `a_play_that_will_not_stop_is_kept_not_let_go_of` | FAIL `a play that outlasts the wait for it neither starts unannounced nor goes untracked` |
| The original defect: a play its caller gave up on is loaded and counts as started | FAIL `a_play_its_caller_gave_up_on_is_not_loaded`, `a_play_given_up_on_while_it_loads_does_not_count_as_started` | |
| A play already known to be given up on is still handed its file | FAIL, the first of those | |
| A play given up on during its load counts as started | FAIL, the second | |
| Both sides of a start can win | FAIL, four tests | |
| The caller gives up on a play that has already started | FAIL, three tests | |
| The caller reports a late start without settling it | | FAIL |
| A play that lost its start is announced and drawn anyway | | FAIL |
| Building the play hands mpv its file again, before the start is settled | | FAIL (two tests) |
| (*) A kept play is let go of, and waited for, while it still runs | FAIL `a_play_that_will_not_stop_is_kept_not_let_go_of` | |
| (*) A kept play that has ended is never let go of | FAIL, the same test | |
| A new embedded play starts next to one that would not end | | FAIL |
| Vela exits without a last wait for plays that would not end | | FAIL |
| Unchanged | PASS (11) | PASS |

(*) At `af4f142` the first of these two did not fail: the test waited for the stuck stand-in for ever. A guard that hangs CI is not a guard, so `d7d850a` makes the stand-in free itself after ten seconds and requires that asking about a kept play returns at once; both now fail cleanly. (The second was first run with an injection that did not compile.)

End to end, with the real player: the E2E scenario `embeddedlatestart` (added with finding `ec-6`, which gives a late start a visible outcome) stages a play that sits for eight seconds with the caller's wait cut to half a second. Vela's own log from that run: "an embedded play did not stop in time and is kept until it does", then the play starting in the external player; the scenario sees the embedded player still there without a file, then gone when its eight seconds are over, and the player view never opened.

## Coder dispute (if any)

## Known gaps

- A thread stuck for good stays stuck: it keeps its surface (which is under the window, behind the page, and shows nothing), its OpenGL context and its mpv player until Vela exits, and Vela exits with it still running after the two-second wait. Nothing can be done about a thread from outside it. What changed is that Vela knows, starts nothing next to it, and that it cannot come back as a play.
- After the caller has given up, a play can still be handed its file in the instant between the check and the claim. It is then torn down at once and never announced; mpv may have begun to open the stream.
- `loadfile` is still a waiting call into mpv on the play's own thread, made before any frame exists. (The drawing thread waiting on mpv during a play is finding `hd-3`.)

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "Timeout cleanup detaches a worker that still owns player resources". Reviewer's severity: HIGH.
