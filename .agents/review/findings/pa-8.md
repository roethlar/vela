# pa-8: a name every object has, such as toString, is accepted as a test-only route

**Severity**: LOW — the loader and the recorder ask whether a route's name is `in` the table of test-only routes, which is also true of names every object inherits. A scenario declaring `via: 'toString'` is accepted, and `testOnlyRoute('toString')` is recorded as a known route. Nothing does this today; the check promises to refuse it.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "tests/e2e/scenario-loader.mjs:86 uses 'in' against the ordinary TEST_ONLY_ROUTES object; tests/e2e/proof.mjs uses the same membership check. A declaration with via: 'toString' passed scenarioProblems, and testOnlyRoute('toString') followed by proofProblems also returned no problems."

## Predicted observable failure

Reviewer, verbatim: "The loader and runtime recorder accept a nonexistent route inherited from Object.prototype, despite promising to reject routes outside the three registered helpers."

Reviewer's suggested direction, verbatim: "Check own-property membership, such as Object.hasOwn, in both places. Add rejection cases for inherited names such as toString and constructor."

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
title: "Inherited property names are accepted as test-only routes". Reviewer's severity: LOW.
