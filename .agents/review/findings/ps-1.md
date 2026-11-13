# ps-1: two tests that write a marker payload delete each other's file

**Severity**: MEDIUM — the Rust suite, the gate every slice passes, fails
at random; a gate that fails at random gets re-run until it is green, and
that is how a real failure gets waved through. Nothing the app ships
changes.
**Status**: In progress
**Branch**: —
**Commit**: (this commit)

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

(pending)

## Coder dispute (if any)

None.

## Known gaps

(pending)

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-seam.md`.
