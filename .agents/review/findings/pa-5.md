# pa-5: the verdict takes a known failure that was skipped for one that failed

**Severity**: MEDIUM — for a scenario recorded as a known failure, the verdict compares the error text of any result that is not a pass. A result marked skipped that carries the recorded text is accepted, so the verdict can pass on a run in which the scenario never ran.
**Status**: In progress
**Branch**: —
**Commit**: `115cbaa`, `dbb91b4` (repair)

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

### Verification round 1, recorded 2026-11-14 10:04 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d980-fb9e-7d81-8c71-591f9b5697a1`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 10:01 UTC, finished 10:04 UTC.
- Reviewed head `115cbaad15c2fcca76b69239265a29733be12f30`, base
  `ade0278604e39994346c4983f923316e5a36bfc1` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/scenario-loader.mjs:213 — String(result.error) still accepts malformed failure evidence. With otherwise valid full-run fixtures, external embeddedownconfig with status 'fail' and error [KNOWN_FAILURES[0].failsWith] returns no problems; nested arrays also pass. The runner writes string errors, so require typeof result.error === 'string' before matching and add a regression assertion. The recorded skip guard failed when the fix was reverted and passed after restoration.

Orchestrator: a valid verdict, reopened on one comment, and the comment is
right. The match on the error text took anything that turns into text
holding the recorded words. The runner only ever writes text there
(`String(err?.message ?? err)` in `tests/e2e/run.mjs`).

### Repair (round 2), `dbb91b4`

The reviewer's point is one case of something wider in the same function:
`verdictProblems` gave a results file the benefit of the doubt wherever it
read one. The repair takes from the file only what the runner writes:

- A known failure's error must be text. Anything else reads "<name> failed
  in the <player> player, but its error is not text (...): the runner
  writes text, so this is not its known failure seen".
- A row counts as shown only from a list. `shown` given as text matched one
  row's id inside another's ("12" holds "1" and "2").
- A scenario's result is looked up among the file's own names only, so a
  scenario whose name every object answers to has no result unless the
  file holds one.
- Results that are not an object by scenario name hold none ("the <player>
  results hold no scenarios by name").
- A file that does not say `dirty: false` is not taken as clean, and one
  that does not say `full: true` is not taken as a full run (`built` was
  already read this way).

What the runner writes is unchanged, and so is the verdict on any file it
wrote: the parity test's own fixtures and the rest of `npm run check` pass
as before.

Files: `tests/e2e/scenario-loader.mjs`, `tests/e2e-parity.test.mjs`.

Guard: the verdict test gained a case for each of the six rules: a known
failure whose error is a list with the text, a nested list, an object, null,
nothing, and a number; `shown` as the text "12"; a scenario named
`constructor`; results given as a list and as null; `dirty` missing and 0;
`full` given as "yes". Red-proofs by the coordinating agent, one rule taken
out at a time, the file put back and checked byte for byte after each: all
six made the verdict test fail, and it passed once restored. `npm run
check` was green before the commit (422 tests).

Not covered: `routes` in a result is not read by the verdict, so its shape
is not checked. An ordinary scenario's failure is a problem whatever its
error is, so that branch asks nothing of the error.
