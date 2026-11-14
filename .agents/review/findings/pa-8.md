# pa-8: a name every object has, such as toString, is accepted as a test-only route

**Severity**: LOW — the loader and the recorder ask whether a route's name is `in` the table of test-only routes, which is also true of names every object inherits. A scenario declaring `via: 'toString'` is accepted, and `testOnlyRoute('toString')` is recorded as a known route. Nothing does this today; the check promises to refuse it.
**Status**: Verified
**Branch**: —
**Commit**: `ade0278`

## Evidence

Reviewer, verbatim: "tests/e2e/scenario-loader.mjs:86 uses 'in' against the ordinary TEST_ONLY_ROUTES object; tests/e2e/proof.mjs uses the same membership check. A declaration with via: 'toString' passed scenarioProblems, and testOnlyRoute('toString') followed by proofProblems also returned no problems."

## Predicted observable failure

Reviewer, verbatim: "The loader and runtime recorder accept a nonexistent route inherited from Object.prototype, despite promising to reject routes outside the three registered helpers."

Reviewer's suggested direction, verbatim: "Check own-property membership, such as Object.hasOwn, in both places. Add rejection cases for inherited names such as toString and constructor."

## What

The defect is as the reviewer describes. The loader (`scenarioProblems` in
`tests/e2e/scenario-loader.mjs`) and the recorder (`testOnlyRoute` in
`tests/e2e/proof.mjs`) both asked whether a route's name was `in` the table
`TEST_ONLY_ROUTES`. `in` is also true of every name an object inherits, so
`via: 'toString'` passed the loader and `testOnlyRoute('toString')` was
recorded as a route taken.

## Approach

Both places now look the name up among the table's own names only, and take
nothing that is not a string: `typeof name === 'string' &&
Object.hasOwn(TEST_ONLY_ROUTES, name)`. Nothing else changes.

How it was made: a helper agent (Claude Opus 5.5) wrote the fix on a local
branch, a second helper attacked it, a third confirmed it on the merged
content, and the coordinating agent read the diff and landed it as one
commit.

## Files changed

- `tests/e2e/scenario-loader.mjs`
- `tests/e2e/proof.mjs`
- `tests/e2e-parity.test.mjs`

## Guard proof

Two tests in `tests/e2e-parity.test.mjs`:

- "the loader refuses a scenario it does not understand" now also refuses
  `toString`, `constructor`, `__proto__` and `hasOwnProperty` as `via`.
- New: "the recorder refuses a test-only route it does not name, an
  inherited name included". The recorder must throw on each of those names
  and record exactly the three real routes.

Red-proofs, each made after the fix was committed and restored from the
commit:

- Loader back to `!(route.via in TEST_ONLY_ROUTES)`: the loader's test
  failed with `[] should match /"via" is "toString", not one of the
  test-only routes/`.
- Recorder back to `!(id in TEST_ONLY_ROUTES)`: the recorder's test failed,
  the recorded routes holding the inherited names.
- The attacking helper's other form, `!TEST_ONLY_ROUTES[id]`, in each
  place: the same two tests failed.
- The confirming helper repeated the first two on the merged content: both
  failed.

`npm run check` was green at this commit (412 tests; one skipped, a
packaging test that needs `mksquashfs`, which this machine does not have).

## Coder dispute (if any)

## Known gaps

None known. The attacking helper looked for other lookups by name on a
plain object in `tests/e2e/*.mjs` and found only `ran[name]` in the
verdict, where a value that is no result is refused by the fix of `pa-5`.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "Inherited property names are accepted as test-only routes". Reviewer's severity: LOW.

### Verification round 1, recorded 2026-11-14 10:01 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d97d-445e-7702-9b25-7637eeb0112c`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 09:57 UTC, finished 10:01 UTC.
- Reviewed head `ade0278604e39994346c4983f923316e5a36bfc1`, base
  `efbf30cedf74a39508a10f432d9e5019f7ec5d56` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
