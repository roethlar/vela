# ec-2: a start that times out lets go of a worker that is still running

**Severity**: HIGH — the worker keeps its thread, its surface and its player with nothing left that can stop it or wait for it, and if it comes back it hands mpv the file and announces a play that was already reported as failed. It needs a start that takes longer than 20 seconds.
**Status**: Verified
**Branch**: —
**Commit**: `af4f142`, `d7d850a`, `7fa009b`

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

### Verification round 1, recorded 2026-11-13 17:46 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d447-6996-7883-aeb2-4ff51f7b0b38`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 09:40 UTC, finished 09:48 UTC.
- Reviewed head `d7d850abc7866ab5af3f42f0fa62ea2c2def858b`, base
  `f237b2984235428c5823374ae5658a9dbb173b00` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/player/embedded/session.rs:645 — The second receive can time out after claim_started() succeeds but before ready.send(). The caller then reports failure while the race remains STARTED, allowing the worker to resume and emit Opened. A deterministic probe of this branch with shortened deadlines reproduced that sequence. An already-won start must return success, or publication must be atomically cancelled.
  - src-tauri/src/player/embedded/session.rs:208 — shared.ended is set before the thread drops Start, including MarkerPayload's filesystem cleanup. Treating it as thread completion permits an unbounded join in availability checks and the supposedly bounded exit wait. Concurrent queries also see zero after the entry is removed. A delayed-destructor probe made wait(5ms) block over 100ms and return success while another query reported zero. Check actual JoinHandle completion before removing and joining workers.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. Both comments are correct.

The first: when the caller's wait runs out and the play has already won
the start, the fix waited a second time for the play's word, and reported
failure if that did not come in time, with the start still settled as
"started". That is the original defect again in a narrower window. Once
the play has won, the caller knows everything it needs: the play started.

The second: "has this thread ended" was answered from a flag the thread
sets as its last statement, which is before it has let go of what it
holds (dropping its start request removes files). Joining a thread on the
strength of that flag can wait, in a check that is meant not to; and
another caller asking in the meantime is told there is nothing left. The
thread itself knows when it has finished; the repair asks it.

### Repair (round 2), `7fa009b`

For the first comment: `Session::start` waits once. When that wait runs
out, `waited_out` reads the outcome off the race: if the caller's giving
up settles it, the start has failed; if the play had already won, it is a
started play and is returned as one, without waiting for the play's word
on the channel. Nothing is waited for a second time, so there is no
second deadline to miss.

For the second: whether a play's thread has finished is asked of the
thread (`JoinHandle::is_finished`), in `Session::has_ended` and in
`Leftovers::still_running` alike, and a thread is joined only after it has
said so, which returns at once. The `ended` flag is gone. A kept thread
stays in the list until it has really finished, so a second caller asking
meanwhile is told the truth.

Files: `src-tauri/src/player/embedded/session.rs`,
`tests/embedded-player.test.mjs`.

Guard proof for round 2, run 2026-11-13 at `7fa009b` in a disposable
worktree; `cargo +stable test --locked --lib embedded::session` and
`node --test tests/embedded-player.test.mjs`:

| Injection | Rust | Node |
| --- | --- | --- |
| The first repair, which was reopened: a second wait for the word of a play that has won its start | (not run) | FAIL `a play that outlasts the wait for it neither starts unannounced nor goes untracked` |
| A start that was won is reported as failed when the wait runs out | FAIL `a_start_that_was_won_before_the_wait_ran_out_is_a_started_play` | FAIL |
| The wait running out no longer settles the start | FAIL, the same test | FAIL |
| The first repair's other half: a play's thread is joined without asking whether it has finished | FAIL `a_play_has_ended_only_once_its_thread_has_finished_and_asking_does_not_wait` and two more | FAIL |
| Kept threads are joined whether or not they have finished | FAIL `asking_whether_a_play_has_finished_never_waits_for_it`, `a_play_that_will_not_stop_is_kept_not_let_go_of` | FAIL |
| A kept thread that has finished is never let go of | FAIL, the same two | FAIL |
| Unchanged | PASS (14) | PASS |

The two new timing tests use a stand-in that takes 0.6 s to let go of what
it holds after its work is done, and require the question to be answered
within 0.3 s.

Also run: `embeddedlatestart`, `embeddedplay`, `embeddedownconfig` and
`playback` in the embedded player under the headless compositor: PASS.

End to end, with the real players (run 2026-11-13 at `7fa009b` for finding
`ec-6`'s guard proof): with `finish_start` made to load and claim the start
whatever the caller has done, the scenario `embeddedlatestart` FAILS with
"the player view never opened for a play that was given up on"; unchanged
it passes.

### Verification round 2 (repair-delta redispatch), recorded 2026-11-13 18:46 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d614-48a9-72c3-9aab-5e5ba8b3f50f`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 18:03 UTC, finished 18:11 UTC.
- Reviewed head `7fa009b1a2e9e6d92db9b06a36d71f2168d2091b`, base
  `270857aac02f69e35ecd850b7837bc9bca595c5c` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
