# ps-1: two tests that write a marker payload delete each other's file

**Severity**: MEDIUM — the Rust suite, the gate every slice passes, fails
at random; a gate that fails at random gets re-run until it is green, and
that is how a real failure gets waved through. Nothing the app ships
changes.
**Status**: Verified
**Branch**: —
**Commit**: `4fbff87` (the lock and the refusal), `1d7c405` (a test that the lock excludes)

## Evidence

`src-tauri/src/player/launch.rs` (at `b719a65`, line 565):
`try_write_marker_payload` calls `prune_marker_payloads` (line 576) before
it writes, and that removes every payload the current process wrote
earlier. Slice 1 added a second test that reaches it,
`the_full_external_command_line_is_exactly_the_recorded_one`
(`src-tauri/src/player/external.rs:388`, through `build_launch`), next to
`marker_payload_is_written_owner_only_and_absent_for_no_markers`
(`launch.rs:1030`). Cargo runs both in one process, on separate threads.

The reviewer ran the two with `--test-threads=2` and got a failure on the
third repetition (`launch.rs:1044`, `NotFound`). Reproduced here at
`90db1a6` before any fix: 60 repetitions, 2 failed, the first at
`external.rs:459` ("payload is readable: ... NotFound").

## Predicted observable failure

`cargo +stable test --locked` fails now and then, in either test, at the
line that reads the payload back or checks its permissions, with nothing
wrong in the code under test.

## What

In the app the sweep is right: plays start one at a time, so anything it
finds is left over from a play that is finished. Two tests in one process
break that assumption, and neither knew about the other.

## Approach

A lock for tests that write a payload, and a refusal for tests that write
one without it.

`player::launch::payload_test_lock` (test builds only) is one process-wide
mutex. `hold()` takes it and marks the calling thread; the mark is cleared
when the guard is dropped. Both tests take it before their payload is
written and keep it until they have finished with the file.

A lock on its own leaves the next person to add such a test exactly where
this slice was: nothing says the lock exists, and forgetting it fails one
run in thirty. So, in test builds, `try_write_marker_payload` panics when
the calling thread does not hold the lock ("a test that writes a marker
payload must hold payload_test_lock::hold()"). Forgetting it now fails
every run, in the test that forgot, with a message that says what to do.
The app's build contains none of this.

Independent runtime directories per test, the reviewer's other suggestion,
would have meant a test-only way to redirect a directory whose whole point
is that the app chooses it; the lock changes nothing the app runs.

## Files changed

- `src-tauri/src/player/launch.rs` — `payload_test_lock` (test builds
  only); the refusal at the top of `try_write_marker_payload` (test builds
  only); `marker_payload_is_written_owner_only_and_absent_for_no_markers`
  holds the lock; two new tests,
  `a_test_that_writes_a_marker_payload_without_the_lock_is_refused` and
  `the_payload_test_lock_is_held_only_while_its_guard_lives`
- `src-tauri/src/player/external.rs` —
  `the_full_external_command_line_is_exactly_the_recorded_one` holds the
  lock from before `build_launch` to the end of the test

## Guard proof

Run 2026-11-13 at `1d7c405` in a disposable worktree; each injection
restored with `git checkout` from the commit, then
`cargo +stable test --locked --lib`.

Each behaviour the fix claims, separately:

| Injection | Result |
| --- | --- |
| The recorded-command test does not take the lock | FAIL, every run: that test, "a test that writes a marker payload must hold payload_test_lock::hold()" |
| The payload test does not take the lock | FAIL, every run: that test, same message |
| The writer refuses nothing | FAIL: `a_test_that_writes_a_marker_payload_without_the_lock_is_refused` ("did not panic as expected") |
| Dropping the guard leaves the thread marked | FAIL: `the_payload_test_lock_is_held_only_while_its_guard_lives` |
| `hold()` marks the thread and excludes nobody | FAIL: `the_payload_test_lock_keeps_a_second_test_out_until_the_first_is_done` |
| No refusal and no lock in one test (the state the finding reported) | the two tests side by side, 150 runs: 27 failed, first at `external.rs:461`, the payload read, `NotFound` |
| Restored | PASS, 378 tests; the two tests side by side, 300 runs: 0 failed |

Before the fix (`90db1a6`), the same side-by-side run failed 2 of 60.

## Coder dispute (if any)

None.

## Known gaps

- The refusal covers `try_write_marker_payload`, the one function that
  sweeps. A test that removed payload files some other way would not be
  caught; none does.
- The auth include has no such sweep (each launch removes only its own
  file), so its tests need no lock. Checked by reading
  `HeaderInclude::remove_now` and `write_header_include`, and by the
  300 clean runs above, which include a test that writes both.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-seam.md`.
Verification pass, recorded 2026-11-13 06:29 UTC (this record is committed):

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly; the host's codex config
  resolves to gpt-6-astra @ max. Thread
  `01a1d38e-147b-74a2-92f2-ab085da09a43`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; 32 tool calls; dispatched 06:17 UTC, finished 06:26
  UTC. Routing: T1 no sensitive path in the diff, T2 severity MEDIUM, T3
  the recorded proof re-run by the orchestrator agrees (378 pass at the
  head).
- Reviewed head `1d7c405ae2ef88d5b46ef497e27375e005e3e96b`, base
  `46d4b6b083e7b143c5721c1eb49ff9eabeb57101` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted; the finding is
closed.
