# sel-4: the test that play refuses a player it cannot start passes with the refusal removed

**Severity**: LOW — the shipped refusal is correct; its test is not a test.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "tests/player-selection.test.mjs:110 checks only that the conditional exists before four calls. Removing the return Err at src-tauri/src/playback.rs:301 while retaining the conditional left all five player-selection tests—and all 19 targeted Node tests—passing."

## Predicted observable failure

Reviewer, verbatim: "The test named 'play refuses a player it cannot start before it starts anything' falsely passes when an Embedded request can proceed into external-player side effects. This is a LOW regression-test defect; the current production refusal is correct."

## What

(pending)

## Approach

(pending)

## Files changed

(pending)

## Guard proof

(pending)

## Coder dispute (if any)

None.

## Known gaps

(pending)

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Exercise playback::play with an Embedded PlaySpec and assert the expected refusal and absence of side effects. Prove that removing the return makes that test fail."
