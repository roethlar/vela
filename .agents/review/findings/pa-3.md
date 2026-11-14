# pa-3: half of a split row can be deleted from the list and the guard still says the plan's list is all there

**Severity**: MEDIUM — the check that every row of the plan is in the parity list accepts any one part of a split row. Deleting row 8b (the skip button) together with its two scenarios leaves every parity test passing, and the verdict then has no row 8b to ask for.
**Status**: In progress
**Branch**: —
**Commit**: `ddbb743`

## Evidence

Reviewer, verbatim: "tests/e2e/scenario-loader.mjs:285 accepts any nonempty collection of suffixed IDs for a plan row. Removing inventory entry 8b and both markerbutton.mjs and embeddedskip.mjs left all 13 parity tests passing in a local injection, with the plan unchanged. The 'half-split' test at tests/e2e-parity.test.mjs:212 tests whole-plus-split duplication instead."

## Predicted observable failure

Reviewer, verbatim: "The entire skip-button requirement can disappear while the inventory guard still reports that the plan's inventory is represented. The verdict subsequently has no 8b entry to enforce."

Reviewer's suggested direction, verbatim: "Validate complete approved split groups, including every required subrow. Add a regression test that deletes one half and its scenarios."

## What

The defect is as the reviewer describes. `inventoryProblems`
(`tests/e2e/scenario-loader.mjs`) accepted any non-empty set of ids of the
form `<n>` or `<n><letter>` for a plan row. Deleting row 8b together with
`markerbutton` and `embeddedskip`, the two scenarios that claim it, left
every parity check passing: `parityProblems` on what remained really is
empty.

## Approach

The approved splits are now data: `SPLITS` in `tests/e2e/parity.mjs` says
that rows 8, 12 and 13 are each split into their `a` and `b` parts.
`inventoryProblems(inventory, planNumbers, splits = SPLITS)` checks both
directions. For a split row every approved part must be in the inventory
and the row must not also be there whole; an unsplit row must be there
whole; an id that is no approved part is refused; so is a split recorded
for a row the plan does not have.

The parity test also spells out the seventeen row ids and the splits, so a
row or a part can only come or go with a change to the test as well as to
`parity.mjs`.

Made, attacked, confirmed and landed as `pa-8` was. The coordinating agent
put one line break back in the test file (two statements had been joined on
one line).

## Files changed

- `tests/e2e/parity.mjs`
- `tests/e2e/scenario-loader.mjs`
- `tests/e2e-parity.test.mjs`
- `tests/e2e/README.md`

## Guard proof

In `tests/e2e-parity.test.mjs`:

- New: "half of a split row deleted with its scenarios is refused". It
  makes the reviewer's injection on the real data (8b gone, with the
  scenarios that claim it), shows that the parity check itself still
  passes on that, and requires the inventory check to answer exactly `part
  8b of split plan row 8 is not in the inventory`.
- New: "the inventory is exactly these rows, split exactly so" (the
  literals).
- "the inventory check refuses a dropped, a made-up and a half-split row"
  gained cases: each missing part, both missing, an unapproved part of an
  unsplit row, a third part of a split row, an unsplit row in parts, and a
  split of a row the plan does not have.

Red-proofs, each restored from the commit:

- The old body of `inventoryProblems`: the 8b test and the unit test
  failed.
- The missing-part check removed: the same two failed.
- The unapproved-part check removed: the unit test failed.
- 8b and its two scenarios really deleted, with the fix in place: the plan
  test, the literal test and the 8b test failed.
- A made-up row `8c` added: the literal test and the plan test failed.
- The attacking helper disabled the "both whole and split" check and the
  "no row of the plan" check: each failed the unit test.
- The confirming helper removed the missing-part check on the merged
  content: both tests failed.

`npm run check` was green at this commit (414 tests).

## Coder dispute (if any)

## Known gaps

Nothing reads the splits out of the plan's text. A split is approved by the
two lists in code (`SPLITS`, and the literal in the test); the plan's log
says in words that rows 8, 12 and 13 are split.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "The inventory guard accepts removal of a split half". Reviewer's severity: MEDIUM.
