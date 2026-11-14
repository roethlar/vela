# pa-5: the verdict takes a known failure that was skipped for one that failed

**Severity**: MEDIUM — for a scenario recorded as a known failure, the verdict compares the error text of any result that is not a pass. A result marked skipped that carries the recorded text is accepted, so the verdict can pass on a run in which the scenario never ran.
**Status**: In progress
**Branch**: —
**Commit**: `115cbaa`

## Evidence

Reviewer, verbatim: "tests/e2e/scenario-loader.mjs:201 checks the error text for every non-pass known-failure result without requiring status === 'fail'. With otherwise valid synthetic results, changing external embeddedownconfig to status 'skip' while retaining its error produced CLI exit 0, 'embeddedownconfig ... skip', and 'parity verdict: PASS'."

## Predicted observable failure

Reviewer, verbatim: "The verdict accepts evidence in which a required scenario was skipped instead of reproducing its recorded defect, undermining the full-run requirement."

Reviewer's suggested direction, verbatim: "Require an explicit fail status before matching a known failure's error. Reject skipped, missing, and unknown statuses independently of their error text."

## What

The defect is as the reviewer describes. For a scenario recorded as a known
failure, `verdictProblems` (`tests/e2e/scenario-loader.mjs`) compared the
error text of any result that was not a pass, so a result with status
`skip` that kept the recorded text was accepted. For any other scenario, a
status other than `pass` was reported as "failed" with its error, never in
its own words.

## Approach

Only `pass` and `fail` are results of a scenario that ran. Any other status
is a problem whatever text it carries: a skip reads "<name> was skipped in
the <player> run, so it has no result there"; anything else reads "<name>
has no result in the <player> run: its status is X, neither pass nor fail".
A known failure matches only when its status is exactly `fail` and its
error carries the recorded text. An ordinary scenario passes only on
`pass`.

Made, attacked, confirmed and landed as `pa-8` was.

## Files changed

- `tests/e2e/scenario-loader.mjs`
- `tests/e2e-parity.test.mjs`

## Guard proof

The test "the verdict of two full runs fails on a failure, a missing run,
and a row ..." in `tests/e2e-parity.test.mjs` now checks, each by its own
message: for a known failure carrying its text, the statuses `skip`, none,
`FAIL` and `failed`; for an ordinary scenario, `skip`, none and `passed`;
and that `fail` with the text is still accepted for the known failure,
`pass` passes and `fail` fails for the ordinary one.

Red-proofs, each restored from the commit:

- The status check skipped for known failures only (the reviewer's case):
  the test failed, expecting `b was skipped in the external run ...` and
  getting an empty list of problems.
- The status check skipped for ordinary scenarios only: failed on `a was
  skipped in the embedded run ...`.
- A skip let through: failed as the first did.
- The attacking helper changed the ordinary branch to `=== 'failed'`: the
  test failed.
- The confirming helper took the status check out on the merged content:
  the test failed on the skipped known failure.

`npm run check` was green at this commit. The verdict of the two full runs
made on the merged content passed with this rule in place, so no real
result is refused by it.

## Coder dispute (if any)

## Known gaps

None known. The runner writes no such result today (a skip carries its own
reason), so the old rule needed a results file that said so; the rule was
still wrong.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "A skipped known failure is accepted as an observed failure". Reviewer's severity: MEDIUM.
