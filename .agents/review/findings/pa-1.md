# pa-1: a play left running reaches the end of the marker by itself, so the automatic-skip test passes without a skip

**Severity**: MEDIUM — `markers`, the only scenario that stands for automatic skipping (row 8a) in either player, waits up to 14 and 16 seconds for a running play to be past second 12. Ordinary playback gets there in that time, so the row is counted as shown, and the parity verdict passes, with automatic skipping broken.
**Status**: In progress
**Branch**: —
**Commit**: `25287b6`, `17d415f`, `343707b`, `da7ef27`, `716abc6` (repair), `9dfad5e` (second repair)

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

### Verification round 2 (repair-delta redispatch), recorded 2026-11-14 13:24 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1da33-5930-74c2-85ac-acbd60524ed4`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 13:16 UTC, finished 13:23 UTC.
- Reviewed head `716abc6c1f633736466b5a60634b953eeb89fb90`, base
  `26660a9547f0003923fae99d831fe7dda32d97a2` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/scenarios/markers.mjs:117 — The clock starts after the resume reply. With ordinary playback, a 9400 ms resume-reply delay followed by an 1800 ms position read certified 12.22 s after 11219 real milliseconds, reporting only 1809 ms. The only seek was setup to 1 s. Start timing before issuing the resume so its response latency counts.
  - tests/e2e/judge.mjs:43 — Observation gaps still hide short landings. A stand-in that skips to 9 s at 1000 ms, then plays normally, passed at 4220 ms and 12.22 s when a read begun around 900 ms took 3300 ms. Failed reads spanning the same interval also passed. Lua consumes the marker before seeking, so this continuation accurately models a short landing without another skip. Require sufficient observation coverage or independent landing evidence, with regression tests.
  - tests/e2e/scenarios/markers.mjs:131 — Date.now permits clock adjustments to defeat the deadline. With ordinary playback, a 9400 ms read delay and a simulated seven-second clock rollback, the scenario certified 12.21 s after 11213 real milliseconds while reporting 4212 ms. Use a monotonic elapsed clock.
  - tests/e2e-judge.test.mjs:238 — The slow-read fixture jumps from approximately 2.04 s to 12.3 s at 5200 ms; it models a late jump, not ordinary playback. Use an honest playback function for the no-skip regression. The existing deadline guards do bite: reverting the repair failed both named tests, and restoring it passed all 29 tests. Restoration was verified byte for byte.
  - tests/e2e/judge.mjs:34 — A correct jump around one second can now fail if observation is delayed roughly four additional seconds. A 4300 ms read begun around 900 ms reproduced rejection at 5225 ms. This is conservative timeout behavior, not a separate reopening reason. The E2E scenario and Lua were read; the E2E suite was not run.

Orchestrator: a valid verdict, reopened on four comments (a fifth says
the proof is now conservative, which the reviewer does not hold against
it). All four are right: they are about how the proof counts time and what
it does not see between two readings. It is the second reopen of this
finding.

### Repair (round 3), `9dfad5e`

- **Where time starts.** The clock started when mpv had answered the
  resume, so a slow answer was not counted. It now starts before the
  resume is asked for (`proveSkippedPast` in
  `tests/e2e/scenarios/markers.mjs`).
- **Which clock.** The wall clock can be set back. The proof now reads the
  process's monotonic clock (`performance.now()`).
- **When a reading counts.** A reading's time is taken when its read has
  answered, so a slow read counts against the window too.
- **What lies between two readings.** A crossing counts only if at most
  500 ms lie between the position seen before it and the one past the
  range's end (`skipJumpVerdict` in `tests/e2e/judge.mjs`, `maxGapMs`).
  In a longer gap, whether from one slow read or from reads that failed,
  a skip that landed short could have played the rest unseen, and the leg
  fails: "the play was not watched across the jump: N ms lie between ...".
- **An honest test.** The test added in round 2 made the position creep
  and then jump, which is not ordinary playback. The scenario's loop now
  takes its clock as a parameter, and the tests run it in made-up time
  against a play that goes at one second per second from the moment mpv is
  told to resume, where a skip is a jump of the position and an answer can
  be slow. That test is replaced by these.

Files: `tests/e2e/scenarios/markers.mjs`, `tests/e2e/judge.mjs`,
`tests/e2e-judge.test.mjs`.

Guard, in `tests/e2e-judge.test.mjs`:

- "the time a slow answer to the resume takes counts against the window"
  (the reviewer's first trace: 9400 ms for the resume, then 1800 ms for a
  read, no skip).
- "a skip that lands short and plays the rest behind a slow read, or
  behind reads that fail, fails" (the second: a landing at 9 s, a read of
  3300 ms; and the same with failing reads).
- "ordinary playback seen past the range late, through one slow read,
  fails".
- "a real skip passes in made-up time, and a watched one a little slow
  still does" (reads of 300 ms each).
- "a crossing counts only if the play was watched across it" (the rule
  itself: 500 ms passes, 501 fails; failed reads are not watching; a first
  reading that is the jump is measured from the resume).
- "the scenario counts time on a clock that cannot go back" (the text of
  the scenario, and a real run).

Red-proofs by the coordinating agent, the file put back and checked byte
for byte after each: the clock started after the resume's answer again;
the gap rule taken out; failed reads counted as watching; the wall clock
again; the gap allowed made a millisecond more. Each failed its test.

End to end, on the repair's content: `markers` passed 3 runs of 3 in the
external player and 2 of 2 in the embedded one. In all ten legs the last
reading before the jump was 1.88 to 1.92 s and the first after it 12.00 s,
1004 to 1043 ms after the resume was asked for. `npm run check` was green
before the commit.

Not covered: a skip that lands up to half a second short of the range's
end can pass (it was "about a tenth of a second" before; the rule now says
what it guarantees, not what usually happens). The proof is conservative
the other way: a correct skip fails if the machine is so slow that half a
second passes between two readings around the jump.

### Verification round 3 (repair-delta redispatch), recorded 2026-11-14 14:05 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1da54-74e7-7092-be08-e9bfc81b290e`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 13:52 UTC, finished 14:02 UTC.
- Reviewed head `9dfad5e2aa5cb758a987ee9db338597d2fdfbee2`, base
  `2d9012b67e5a6c965655773c2fe735e988da700d` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/scenarios/markers.mjs:147 — Buffered replies can still hide short landings. Using the unchanged helper, real MpvIpc and real clock, a socket stand-in answered 1.91 s at 905 ms, then the response waited through a 3300 ms event-loop stall. Playback skipped to 9 s at 1000 ms and continued normally. The helper timestamped the buffered sample around 4205 ms and accepted 12.31 s at 4306 ms: its apparent observation gap was only 101 ms, despite a landing three seconds short. Keep completion timestamps for deadlines, but conservatively bound coverage using request/response intervals or independent observation timestamps. Add a buffered-response regression; madeUpTime currently computes the position after its delay and misses this case. Guard mutations failed as expected, and all 37 tests passed after byte-for-byte restoration.

Orchestrator: a valid verdict, reopened on one comment, and it is right.
It is the third reopen. The rounds have narrowed (four comments, then
one), the comment names its own cure, and the cure is a rule that covers
the kind and not the one trace, so one more repair is made. If it is
reopened again, the finding goes to the owner: the finding as raised (a
play with no skip passing the proof) has been closed since the first fix,
and what is left is how far a proof by sampling can be pushed.

### Repair (round 4), `8ed9acb`

- **The fault.** The proof timed a sample by when its answer was READ. An
  answer can wait before it is read (the test's process busy, a reply held
  in a buffer), and the position in it is then older than its time says.
  The reviewer's trace: asked at 900 ms, answered at once with 1.91 s, read
  3300 ms later; the skip landed at 9 s at 1000 ms and the play ran on;
  the next sample, 101 ms after that read, was past the end. A landing
  three seconds short passed.
- **The rule now.** Each sample keeps when its read was asked for
  (`askedMs`) and when it was answered (`ms`). The position in it was true
  no earlier than the first and no later than the second. The gap across
  the jump therefore runs from when the position before it was ASKED for
  to when the one past the end was ANSWERED, and must be at most 500 ms
  (`skipJumpVerdict` in `tests/e2e/judge.mjs`). Whatever waits, on either
  side, only makes that span longer, never shorter. The window is still
  counted by the answers, which is the strict side for it.
- **In the scenario** (`proveSkippedPast` in
  `tests/e2e/scenarios/markers.mjs`): the asking time is taken before the
  read is sent, rounded down; the answer's time after it, rounded up.
- **No way round it.** `skipJumpVerdict` throws on a sample without both
  times, or with them out of order.

It is stricter on purpose: reads that each take 300 ms to answer no longer
count as watching the jump (700 ms lie across it), though it is a real
skip. At 150 ms each they do.

Files: `tests/e2e/judge.mjs`, `tests/e2e/scenarios/markers.mjs`,
`tests/e2e-judge.test.mjs`.

Guard, in `tests/e2e-judge.test.mjs`:

- "a skip that lands short behind an answer that waits to be read fails":
  the reviewer's trace through the scenario's own loop, in made-up time.
  The made-up world (`madeUpTime`) can now hold an answer back before the
  scenario reads it, the position in it staying what it was when mpv
  answered; the reviewer had noted that it could not. Also: every answer
  waiting 150 ms still passes (400 ms across the jump); 250 ms each fails
  (600 ms).
- "the gap across the jump runs from when the position before it was asked
  for": the rule on samples written out, at 500 and 501 ms, with the same
  two readings asked for early (fails, 3406 ms) and late (passes), and
  with a failed read in between (its times are not the position's).
- "a sample without the time it was asked for, or asked for after it was
  answered, is refused".
- "a real skip passes in made-up time, and a watched one a little slow
  still does" changed with the rule: its slow reads are 150 ms now, and it
  also requires that 300 ms each fails.

Red-proofs by the coordinating agent, the file restored and checked byte
for byte after each: the gap counted from when the position before the
jump was read (three tests fail); the scenario noting the asking time only
once the read has answered (two); a sample without its asking time taken
as asked for when answered; the order of the two times not checked; the
gap allowed made a millisecond more. Each failed the test meant for it.

End to end, on the repair's content: `markers` passed 3 runs of 3 in the
external player and 2 of 2 in the embedded one; in all ten legs the last
reading before the jump was 1.88 to 1.96 s and the first after it 12.00 to
12.04 s, 1013 to 1057 ms after the resume was asked for. `npm run check`
was green before the commit.

Not covered: a skip that lands up to half a second short of the range's
end can pass, as before. The proof fails a correct skip when the machine
is so slow that half a second passes between asking for one reading and
having the next.
