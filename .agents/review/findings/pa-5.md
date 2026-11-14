# pa-5: the verdict takes a known failure that was skipped for one that failed

**Severity**: MEDIUM — for a scenario recorded as a known failure, the verdict compares the error text of any result that is not a pass. A result marked skipped that carries the recorded text is accepted, so the verdict can pass on a run in which the scenario never ran.
**Status**: In progress
**Branch**: —
**Commit**: `115cbaa`, `dbb91b4` (repair), `0d8ccbd` (second repair)

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

### Verification round 2 (repair-delta redispatch), recorded 2026-11-14 10:42 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d999-f83a-7190-a22a-4064dd0fa2e6`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 10:28 UTC, finished 10:36 UTC.
- Reviewed head `dbb91b41bcae7ad042f86cada77a936f54dd0d70`, base
  `397850ad80512e18c1189d9354247001594e587a` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/scenario-loader.mjs:222 — Passing results bypass error validation. With otherwise valid full-run JSON fixtures, external markers with status 'pass' and error ['scenario failed'] returns no problems. run.mjs:411 always writes error: null for passes. Require that invariant and add a regression assertion.
  - tests/e2e/scenario-loader.mjs:243 — The array check only filters coverage; it does not reject malformed shown values. Setting external playback.shown to '13', 7, or {} in otherwise valid full-run fixtures still returns no problems because other scenarios cover rows 1 and 3. The runner always writes an array. Validate shown for each applicable result before aggregating coverage, and add a regression with overlapping row coverage.

Orchestrator: a valid verdict, reopened on two comments, and both are
right. The first repair checked a result's shape only where the verdict
happened to read it. It is the second reopen of this finding; a third goes
to the owner, as `pc-2` did.

### Repair (round 3), `0d8ccbd`

The first repair asked of a result only what the verdict was about to use.
This one judges every result whole, as the runner writes it
(`tests/e2e/run.mjs`), before it counts for anything:

- A scenario that does not run in a player must be in that player's
  results as `{ status: 'skip' }` and nothing else. A full run names every
  scenario, the skipped ones too, so one that is missing is reported.
- A scenario that runs must have status `pass` or `fail` (a skip or any
  other status is reported as before), and then exactly `status`, `error`,
  `shown` and `routes`. `shown` and `routes` are lists of text. A
  failure's error is text. A pass has no error.
- A pass must also carry a proof the runner itself would have passed: the
  runner's own rule (`proofProblems`) is applied to it, so a pass that does
  not show a row it declares, shows one it does not, or took a test-only
  route it does not declare is no result of a run.
- A result that fails any of this is reported as "<name> has a result in
  the <player> run that the runner does not write: <why>", and counts for
  nothing: not as a pass, not as a known failure, not as showing a row.
- The results must name a commit.

The first repair's rules stay (a result looked up among the file's own
names; results by scenario name; `dirty: false` and `full: true` said, not
assumed). Its sentence for a failure whose error is not text is replaced
by the general one above.

Two older assertions changed with it, both from this finding's first fix.
A made-up failure with no error (`error: null`) was expected to read "a
failed in the embedded player: null"; the runner never writes that, so the
case now gives the failure an error text, and a failure with none is
expected to be refused. The made-up failures in the test gained `routes:
[]`, as the runner writes them.

Files: `tests/e2e/scenario-loader.mjs`, `tests/e2e-parity.test.mjs`.

Guard: the verdict test gained the reviewer's two cases (a pass with an
error; a `shown` given as "13", 7, an object, null, a list of numbers or a
mixed list, while another scenario shows the same row) and a case for each
other rule: `routes` as text, a part missing, a part too many, a pass that
shows none of its rows, one that shows a row it does not declare, one that
took an undeclared route, a scenario of the other player given as a pass,
as a skip with more in it, as text, and left out, and results with no
commit. Red-proofs by the coordinating agent: fourteen rules, the first
repair's among them, were each taken out in turn, the file put back and
checked byte for byte after each. Every one made the verdict test fail,
and it passed once restored. `npm run check` was green before the commit.

The verdict on real results is unchanged: over the two results files of
the full runs made at `9bcdfbc` earlier the same day, the new rules report
one problem, the scenario that really failed there (`windowsetting`).

Not covered: the top of a results file is read for `player`, `full`,
`built`, `head`, `dirty` and `scenarios` only; what else the runner writes
there (`appBinary`, `finishedAt`, `changedDuringRun`) is not read by the
verdict and not checked.
