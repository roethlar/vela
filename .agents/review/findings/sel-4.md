# sel-4: the test that play refuses a player it cannot start passes with the refusal removed

**Severity**: LOW — the shipped refusal is correct; its test is not a test.
**Status**: In progress
**Branch**: —
**Commit**: (this commit)

## Evidence

Reviewer, verbatim: "tests/player-selection.test.mjs:110 checks only that the conditional exists before four calls. Removing the return Err at src-tauri/src/playback.rs:301 while retaining the conditional left all five player-selection tests—and all 19 targeted Node tests—passing."

## Predicted observable failure

Reviewer, verbatim: "The test named 'play refuses a player it cannot start before it starts anything' falsely passes when an Embedded request can proceed into external-player side effects. This is a LOW regression-test defect; the current production refusal is correct."

## What

The test read the source of `play` and checked that an `if` on the player
comes before four calls. It never checked what the `if` does, so emptying
its body changed nothing.

## Approach

A Rust test calls `playback::play` with a play for the embedded player and
requires three things: the specific refusal comes back, no player is in
the slot afterwards, and the end-of-play callback has not run. The
source-text test stays for the one thing it can say, that the check comes
first, and now requires the Rust test to exist.

If the refusal is lost, the Rust test starts a real external player on a
machine that has one; it stops whatever it finds in the slot before it
asserts anything.

## Files changed

- `src-tauri/src/playback.rs` — test
  `play_refuses_a_player_it_cannot_start_and_starts_nothing`
- `tests/player-selection.test.mjs` — "play refuses a player it cannot
  start before it starts anything" names that test

## Guard proof

(pending)

## Coder dispute (if any)

None.

## Known gaps

- Slice 3 of the plan gives `play` an embedded launcher. The refusal then
  becomes "no embedded player is available here", and this test changes
  with it.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Exercise playback::play with an Embedded PlaySpec and assert the expected refusal and absence of side effects. Prove that removing the return makes that test fail."
