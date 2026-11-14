# pa-1: a play left running reaches the end of the marker by itself, so the automatic-skip test passes without a skip

**Severity**: MEDIUM — `markers`, the only scenario that stands for automatic skipping (row 8a) in either player, waits up to 14 and 16 seconds for a running play to be past second 12. Ordinary playback gets there in that time, so the row is counted as shown, and the parity verdict passes, with automatic skipping broken.
**Status**: In progress
**Branch**: —
**Commit**: `25287b6`, `17d415f`, `343707b`, `da7ef27`, `716abc6` (repair)

## Evidence

Reviewer, verbatim: "tests/e2e/scenarios/markers.mjs:90 credits row 8a, but the assertions at :125 and :148 allow 14 and 16 seconds respectively for playback to reach 12 seconds. Neither leg pauses playback. A clock-controlled probe using the actual pollUntil implementation passed both predicates at 12 seconds without any skip."

## Predicted observable failure

Reviewer, verbatim: "If marker loading still works but automatic seeking stops working, both legs pass through natural playback and shown('8a') is reached. This is the only scenario credited for automatic skipping in either player, so the verdict can falsely certify that behaviour."

Reviewer's suggested direction, verbatim: "Establish a known position inside the marker and require a jump across its end within a period too short for ordinary playback to cross it. Prove the assertion fails with automatic seeking disabled."

## What

The defect is as the reviewer describes. Both automatic-skip legs of
`markers` (intro and commercial; row 8a; both players) waited 14 and 16
seconds for `time-pos >= 12` on a running play whose range is 2 to 12 s.
Ordinary playback gets there in about 12 seconds, so the legs passed with
nothing skipped. The scenario's own comment ("never crossed by ordinary
playback inside an assertion window") was false.

The helper who attacked the first fix found a hole in it. It accepted the
first sample past the range's end at any moment of a 5 s window, so a skip
that landed inside the range (at 9 s) let playback cover the last 3 s
inside the window: with the built script's seek sent to 3 s short of the
end, the scenario passed in both players, its log showing 11.96 s then
12.08 s, no jump across the end at all.

## Approach

`proveSkippedPast(mpv, what, rangeStart, rangeEnd)` in
`tests/e2e/scenarios/markers.mjs`: the play is paused and put at 1 s, just
ahead of the range (put back if a late skip moves it), and waited for
there. Then it plays on, sampled every 100 ms. The position must be past
the range's end within 5 s; from 1 s, playback alone reaches about 6 s in
that time. And the crossing must be one jump: a sample seen inside the
range, from 1 s past its start on, fails the leg at once, naming the
sample and the one before it. The 1 s margin allows for the sample that
sees the play just past the start before the script's seek lands; in 20
passing legs the last sample before the jump was 1.88 to 1.92 s and the
first after it 12.00 s, about a second after resuming.

Both legs use it, and the commercial leg now also waits for the script to
have loaded with its payload, as the intro leg did.

Made by a helper agent (Claude Opus 5.5), attacked by a second, repaired by
a third, confirmed by a fourth on the merged content, and read and landed
as one commit by the coordinating agent.

## Files changed

- `tests/e2e/scenarios/markers.mjs`
- `tests/e2e/judge.mjs` (new, the follow-up)
- `tests/e2e-judge.test.mjs` (new, the follow-up)
- `package.json` (the new test file in the `check` script)

## Guard proof

The scenario is the guard, and what proves it is a run with the skip
broken. Each of these was run in the external and in the embedded player,
and restored afterwards:

- The policy changed from `autoskip` to `button`, so that the script loads
  and does not seek: each leg failed with "the position must be past the
  range's end (12 s) within 5000 ms of playing on from 1.00 s, which only
  a skip can do; it is 6.04". The old checks passed under the same change
  (12.2 s after 12 s of playing).
- The seek removed from the built copy of the script (the attacking
  helper, and again the confirming helper on the merged content): failed
  in each player.
- The built script's seek sent to 3 s short of the range's end: "the play
  must jump from the range's start (2 s) past its end (12 s) in one step;
  it was seen inside the range at 9 s (the sample before was 1.92 s)".
- Only the commercial range's seek shortened: the intro leg passed and the
  commercial leg failed with the same sentence.

On the committed content: 5 of 5 passing runs in each player by the
repairing helper, 5 of 5 in each by the attacking one, and both full
suites on the merged content.

The follow-up (`17d415f`, `343707b`, `da7ef27`). As first landed, nothing
that plain `node --test` runs failed when this fix was taken out: a
stricter end-to-end check, removed, makes nothing fail, and the reviewer's
sandbox cannot run the end-to-end suite. Now:

- The judgement is a function of its own, `skipJumpVerdict` in the new
  `tests/e2e/judge.mjs`, which only judges samples. `proveSkippedPast`
  keeps the observing exactly as it was (pause, put the play at 1 s,
  resume, sample every 100 ms) and asks the function after each sample.
  The thresholds, the comparisons and the failure sentences are unchanged.
- Unit tests in the new `tests/e2e-judge.test.mjs` (part of `npm run
  check`) hold each rule: ordinary playback from 1 s fails at the first
  sample at 3 s; the trace the first fix let through (a landing at 9 s,
  later 11.96 s then 12.08 s) fails at the 9 s sample; one jump (1.92 s
  then 12.0 s) passes and names both samples; a sample inside the margin
  followed by the jump passes; exactly the range's end passes and exactly
  start plus margin fails; null samples change nothing; a play stuck at
  1 s, and a range that playback cannot reach, fail at the first sample
  past 5000 ms and not before.
- The scenario's use of the function is held too: `proveSkippedPast` is
  exported, and tests drive it with a stand-in for mpv. A skip that lands
  inside the range must reject with the in-range sentence; one jump must
  resolve and log its line; a play held at 1 s must reject with "which
  only a skip can do"; a skip landing at 7.5 s must reject.

Red-proofs of the follow-up, each restored from the commit: the in-range
check out (four tests failed, the 9 s trace among them, accepted as a
pass); the window out; `>=` at the range's end and at the margin each made
`>`; the scenario made to throw only the window's failure (the stand-in
test failed; before `343707b` that injection hung the test file, which is
why that commit exists); and the fix itself taken out, the scenario put
back to the first fix's rule: "the markers scenario fails a skip that
lands inside the range" failed with "Missing expected rejection". The
attacking helper's sweep of mutations at the last commit: 39 caught, none
survived. End to end, with the built script's seek sent to 3 s short of
the end, `markers` failed in the external player with the in-range
sentence thrown from the verdict. On the follow-up's content `markers`
passed 3 of 3 in each player.

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, repaired by a third, read by the coordinating agent, and landed
one commit per helper commit, each with `npm run check` green.

## Coder dispute (if any)

## Known gaps

- The skip is proved while the play crosses into the range from 1 s, not
  from where a fresh play starts. The first leg still covers the chain from
  a fresh play: the request, the payload, the injection and the script
  loading.
- A skip that lands less than a sampling gap (about 0.1 s) short of the end
  can slip between two samples. Any landing 0.2 s or more short of the end,
  and at least 1 s past the start, is caught.
- Still held only by end-to-end runs: the pause, the seek and the resume
  against a real mpv, and that `run()` calls `proveSkippedPast` at all.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "Ordinary playback satisfies the autoskip proof". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 12:24 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d9f8-3b8c-7112-8395-08712227b34a`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 12:11 UTC, finished 12:21 UTC.
- Reviewed head `2a42345091b17ed54fb89ab281882e7e85f7f505`, base
  `f00cdc472623d796e115b474461e81249f738922` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/judge.mjs:30 — Success bypasses the deadline check. With unchanged proveSkippedPast, ordinary playback and one 9400 ms delayed observation (below IPC's 10000 ms timeout), my stand-in passed at 11256 ms and position 12.26 s. Its only seek was the setup seek to 1 s; no automatic skip occurred. Enforce the deadline before accepting a crossing and add a delayed-observation regression test. The existing tests pass after restoration but permit this false certification.

Orchestrator: a valid verdict, reopened on one comment, and it is right.
The judgement took a position past the range's end as the skip whenever it
was seen, also after the window; one slow read was enough to certify a play
in which nothing was skipped.

### Repair (round 2), `716abc6`

In `skipJumpVerdict` (`tests/e2e/judge.mjs`) a position at or past the
range's end now counts only if it is seen inside the window (5000 ms from
the resume). Seen later, the leg fails: "the position was first seen past
the range's end (12 s) N ms after playing on from 1.00 s, after the 5000 ms
in which only a skip can get it there; it is T (the sample before was B
s)". At exactly the window's end it still counts. Nothing else in the
judgement or in the scenario changes; in real runs the jump is seen about
one second after the resume.

One assertion of the follow-up changed, and it is this finding's own: the
unit test "the end is judged before the window" required a crossing first
seen at 5050 ms to pass. That was the rule the reviewer showed to be wrong;
the test, now "a crossing counts only inside the window, and the first
sample may be the jump", requires it to fail.

Files: `tests/e2e/judge.mjs`, `tests/e2e-judge.test.mjs`.

Guard, in `tests/e2e-judge.test.mjs`: that unit test, with a crossing at
5050 ms (fails, with the whole sentence), at exactly 5000 ms (passes), and
the reviewer's trace (ordinary playback, then a sample past the end at
11256 ms); and a new test of the scenario's own loop, "the markers scenario
fails a play first seen past the range after the window, through one slow
read", against a stand-in for mpv whose one read of the position takes
4.9 s. Red-proofs by the coordinating agent, the file put back and checked
byte for byte after each: the late-crossing rule taken out (both tests
failed); the window's own end made too late (the unit test failed). `npm
run check` was green before the commit (480 tests), and the test file
passes inside `codex sandbox` in workspace-write mode.

Not covered: as listed above.
