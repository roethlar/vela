# pa-1: a play left running reaches the end of the marker by itself, so the automatic-skip test passes without a skip

**Severity**: MEDIUM — `markers`, the only scenario that stands for automatic skipping (row 8a) in either player, waits up to 14 and 16 seconds for a running play to be past second 12. Ordinary playback gets there in that time, so the row is counted as shown, and the parity verdict passes, with automatic skipping broken.
**Status**: In progress
**Branch**: —
**Commit**: `25287b6`

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

No check that plain `node --test` can run fails when this fix is taken
out: a stricter end-to-end check, removed, makes nothing fail. That part
is being added as a follow-up commit before the fix goes to the reviewer,
whose sandbox cannot run the end-to-end suite.

## Coder dispute (if any)

## Known gaps

- The skip is proved while the play crosses into the range from 1 s, not
  from where a fresh play starts. The first leg still covers the chain from
  a fresh play: the request, the payload, the injection and the script
  loading.
- A skip that lands less than a sampling gap (about 0.1 s) short of the end
  can slip between two samples. Any landing 0.2 s or more short of the end,
  and at least 1 s past the start, is caught.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "Ordinary playback satisfies the autoskip proof". Reviewer's severity: MEDIUM.
