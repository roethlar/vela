# pa-5: the verdict takes a known failure that was skipped for one that failed

**Severity**: MEDIUM — for a scenario recorded as a known failure, the verdict compares the error text of any result that is not a pass. A result marked skipped that carries the recorded text is accepted, so the verdict can pass on a run in which the scenario never ran.
**Status**: In progress
**Branch**: —
**Commit**: `115cbaa`, `dbb91b4` (repair), `0d8ccbd` (second repair), `bf86854` (third repair)

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

### Verification round 3 (repair-delta redispatch), recorded 2026-11-14 10:51 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d9a6-a75f-7fe3-931d-a4655a154b03`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 10:42 UTC, finished 10:50 UTC.
- Reviewed head `0d8ccbd231ecda7b39051e79d7cbc560f4e1f6ee`, base
  `ac2524072c877789edf96dbdedf8dbeb2affcc27` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/scenario-loader.mjs:158 — Impossible proof lists still count. In otherwise valid full-run JSON fixtures, external markers.shown=['8a','8a'], autocrop.routes=['mpvInput','mpvInput'], and embeddedownconfig.routes=['not-a-route'] retaining its fail status and known error each independently return no problems. proof.mjs serializes Sets and rejects unregistered routes, so the runner cannot emit these results. Validate uniqueness in both lists and registered route membership before either passes or known failures count, and add regression assertions.

Orchestrator: a valid verdict, reopened on one comment, and it is right: a
list with the same entry twice, and a route nobody registered, are things
the runner cannot write (`proof.mjs` keeps sets and refuses an unknown
route), and the verdict still took them.

This is the third reopen. After the second, this record said a third would
go to the owner, as `pc-2` did. The coordinating agent does not send it,
and says why here so that the owner can overrule it:

- The rule for stopping is about work that has stalled: rounds with no
  delta that can be checked. Each round here closed what was raised and
  was narrower than the one before (a skipped known failure; an error that
  is not text; a pass with an error and a malformed list; a list with a
  duplicate and an unregistered route). `pc-2` was different: each repair
  there opened a new race in what a user sees.
- It is a matter of how strictly a test tool reads its own results file.
  Nothing a user of Vela sees depends on it, and the owner asked to be
  brought only what does.

What three rounds do show is that checking one more field each time does
not end. The next repair is therefore not another field. It is one
description of the whole results file that the runner writes, used by the
runner to write it and by the verdict to refuse anything else: every key
at the top and in each result, its type, the lists without duplicates, the
routes among the registered ones, and the combinations the runner cannot
produce. It waits for the repair of `pa-6`, which changes what the runner
writes at the top of that file, so that the description is written once.

The stop that holds from here: if the reviewer reopens that repair for
anything other than a mistake in it (that is, by asking the verdict to
tell a hand-made file that is exactly like the runner's from a real one,
which no check of the file can do), the finding goes to the owner as
contested.

### Repair (round 4), `bf86854`

The repair this record announced after the third reopen, made once the
repair of `pa-6` (`8cf3a05`) had settled what the runner writes at the top
of the file. It is one description of the whole results file, in
`tests/e2e/scenario-loader.mjs`, in three parts:

- `resultsTopProblems`: the top of the file. Exactly its ten names
  (`player`, `full`, `built`, `appBinary`, `appBinaryFrom`, `head`,
  `dirty`, `changedDuringRun`, `finishedAt`, `scenarios`); the four flags
  true or false and nothing that reads like it; a commit; the app binary
  by its full path; where it came from, which must be the sentence a run
  with that `built` writes; the time of finishing as the runner writes it;
  no "changed during the run" on a run that says it was clean; the results
  by scenario name.
- `outcomeShapeProblem`: one result by itself. A skip and nothing else, or
  a pass or a failure with exactly `error`, `shown` and `routes`; the two
  lists are text, each entry once (the reviewer's first two cases), and a
  route only if it is a registered one (the third); a failure's error is
  text and a pass has none.
- In `verdictProblems`, as before: what a result means for its scenario in
  its player (a skip only where the scenario does not run; a pass only with
  a proof the runner would have passed; a known failure only as a failure
  with its text; a row shown only by a result that stood).

The verdict applies the first two to the files it is given. The runner
(`tests/e2e/run.mjs`) reads back the file it has just written, applies
them to it (`resultsFileProblems`), and fails the run if it breaks one. So
a rule the runner's own file does not meet fails every real run, and what
is written and what is read cannot drift apart unseen. The two sentences
for where the binary came from are one pair of constants in
`tests/e2e/app-binary.mjs`, written by the runner and read by the rules.

Files: `tests/e2e/scenario-loader.mjs`, `tests/e2e/run.mjs`,
`tests/e2e/app-binary.mjs`, `tests/e2e-parity.test.mjs`,
`tests/e2e/README.md`.

Guard: the verdict test gained the reviewer's three cases (`shown` with an
entry twice; `routes` with an entry twice; a known failure that keeps its
text and names a route nobody registered, and one named `toString`) and a
case for each rule of the top of the file; a new test, "the runner checks
the results file it wrote by the rules the verdict reads it with", holds
the runner's reading back of its file, its exit, and the shared sentences,
and runs the rules on a file as the runner writes one, built and not built.
Red-proofs by the coordinating agent, the file put back and checked byte
for byte after each: fourteen rules and wirings were each taken out in
turn (each list rule; each rule of the top; the top rules' call from the
verdict; the per-result part of the runner's check; the runner's failing of
the run; the skip's shape; a third sentence for where the binary came
from). The last one at first passed every test: nothing held the sentence
a `--skip-build` run records. A check of it was added, and it then failed.
`npm run check` was green before the commit (473 tests), and the parity
test passes inside `codex sandbox` in workspace-write mode.

Against real files: the results of the two full runs made at `8cf3a05`
are checked with these rules when those runs end; the plan's log has the
result. An older results file (from before `appBinaryFrom`, and from
before row 9 was split) is refused, as it should be: it is not a file this
runner writes.

Not covered, by nature: a file made by hand that is exactly like one of
the runner's. The commit named at the top is compared with the checkout by
the verdict, not checked for its form.

### Verification round 4 (repair-delta redispatch), recorded 2026-11-14 12:53 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1da18-3058-7483-b44a-494dde5713f5`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 12:46 UTC, finished 12:52 UTC.
- Reviewed head `bf86854ee5881558c7e14888abec6ef447d3584c`, base
  `204495db2f965bc94f77dae1ff232259518a78a3` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/scenario-loader.mjs:335 — Failure/proof consistency is still unchecked. In otherwise valid full-run JSON fixtures, external embeddedownconfig with status 'fail', error KNOWN_FAILURES[0].failsWith, shown ['13b','14'], and routes [] returns no problems from either resultsFileProblems or verdictProblems. The runner cannot write this combination: scenario exceptions retain empty proof lists; run.mjs:172–179 retains nonempty lists only after scenario.run returns, then either passes or fails with the generated proof-validation error. Require failures with nonempty proof lists to have proofProblems and the corresponding generated error before accepting them as known failures, and add a regression assertion.

Orchestrator: a valid verdict, reopened on one comment, and it is right:
a failure that carries proof lists, with nothing wrong in them, is a
combination the runner cannot write, and the description of round 4 took
it. That is a gap in the description by its own claim, not the case this
record set as the stop (a hand-made file exactly like the runner's), so it
is repaired again.

The repair is being written by codex. On 2026-11-14 the owner asked for
coding tasks to be handed to codex and reviewed by the coordinating agent;
this one suits it, since the rule to be completed is the reviewer's own.
The task asks for a derivation, not one more check: every combination the
runner can write, from its code, and a description that takes exactly
those. The coordinating agent reviews what comes back and lands it. Codex
cannot be the reviewer of a repair it wrote, so that review stands in
place of a fifth verification.
