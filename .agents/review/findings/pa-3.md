# pa-3: half of a split row can be deleted from the list and the guard still says the plan's list is all there

**Severity**: MEDIUM — the check that every row of the plan is in the parity list accepts any one part of a split row. Deleting row 8b (the skip button) together with its two scenarios leaves every parity test passing, and the verdict then has no row 8b to ask for.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "tests/e2e/scenario-loader.mjs:285 accepts any nonempty collection of suffixed IDs for a plan row. Removing inventory entry 8b and both markerbutton.mjs and embeddedskip.mjs left all 13 parity tests passing in a local injection, with the plan unchanged. The 'half-split' test at tests/e2e-parity.test.mjs:212 tests whole-plus-split duplication instead."

## Predicted observable failure

Reviewer, verbatim: "The entire skip-button requirement can disappear while the inventory guard still reports that the plan's inventory is represented. The verdict subsequently has no 8b entry to enforce."

Reviewer's suggested direction, verbatim: "Validate complete approved split groups, including every required subrow. Add a regression test that deletes one half and its scenarios."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "The inventory guard accepts removal of a split half". Reviewer's severity: MEDIUM.
