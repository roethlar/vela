# pa-7: nothing checks that Plex is told the play has stopped

**Severity**: MEDIUM — row 2 includes the final stop. For Plex, `multiplex` waits for any timeline request and any progress request. With the final "stopped" timeline never sent, the periodic "playing" ones still satisfy it; the other scenario of row 2, `watchstate`, is Jellyfin's and cannot see it.
**Status**: In progress
**Branch**: —
**Commit**: `17d9a81`, `7e1dda5`, `f07fd8c`, `4906831`

## Evidence

Reviewer, verbatim: "tests/e2e/scenarios/multiplex.mjs:72 claims row 2, but :243 accepts any /:/timeline request and :247 separately checks /:/progress. Neither assertion checks timeline state. If the final update_timeline('stopped') in src-tauri/src/playback.rs:1150 is omitted while periodic 'playing' reports and final progress remain, these assertions still pass."

## Predicted observable failure

Reviewer, verbatim: "The verdict can credit Plex's final-stop behaviour even though Plex receives no stopped timeline report. The other row-2 scenario, watchstate, exercises Jellyfin and cannot detect that omission."

Reviewer's suggested direction, verbatim: "Assert a final Plex timeline request with state=stopped, the correct item, and the final position. Distinguish it from reports sent while playback was running."

## What

The defect is as the reviewer describes. For Plex, `multiplex` (row 2, both
players) waited for any `/:/timeline` request and any `/:/progress`
request. The periodic "playing" timeline reports satisfied the first wait,
so a play that never sent its final `update_timeline("stopped")`
(`start_tracking_plex` in `src-tauri/src/playback.rs`) still passed.
Nothing checked that a "playing" report arrives while the play runs either.

The helper who attacked the first fix found a hole in it. Its check on the
stopped report's time was `>= 6000 && <= 10000`, and the mock's item lasts
exactly 10000 ms. With the stopped call's time and duration arguments
swapped, a report at the item's full length, which tells Plex the item was
watched to the end, passed in both players.

## Approach

In `tests/e2e/scenarios/multiplex.mjs`:

- While the play runs, the scenario waits for a timeline report with
  `state=playing` to reach Plex A.
- After the stop it waits for one with `state=stopped`, and requires: it is
  the only stopped report and the last timeline report; only "playing"
  reports come before it, at least one; it names the item (`ratingKey` 1,
  `key` `/library/metadata/1`); it carries the item's duration, 10000; its
  time is between 6000 and 10000; no report carries a greater time; and
  its time is where the play stopped: no earlier than mpv's position read
  just before the stop less 500 ms, at most 2 s after it, and below 10000.

For that reading `playAndQuit` in `tests/e2e/helpers.mjs` takes an optional
`beforeStop(mpv)` step, run after the seek has been observed and just
before the stop. It defaults to nothing, so other callers are unchanged.
In passing runs mpv was at about 7458 to 7500 ms and the report said 7458
(external) and 7825 (embedded).

Made, attacked, repaired, confirmed and landed as `pa-1` was.

## Files changed

- `tests/e2e/scenarios/multiplex.mjs`
- `tests/e2e/helpers.mjs`
- `tests/e2e/judge.mjs` (the follow-up)
- `tests/e2e-judge.test.mjs` (the follow-up)

## Guard proof

The scenario is the guard, and what proves it is a run with the product
broken. Each of these was a local change to `start_tracking_plex`, built,
run in the external and in the embedded player, and restored (the app was
rebuilt and passed afterwards):

- The stopped call skipped: "timed out waiting for the final "stopped"
  timeline report to reach Plex A".
- No "playing" reports: "timed out waiting for a "playing" timeline report
  ...".
- The stopped time sent as 1000: "the stopped report carries the final
  position ...".
- `ratingKey` 2: "the stopped report names the item played".
- Time and duration swapped: "the stopped report's position (10000 ms) is
  where the play stopped: mpv was at 7458 ms just before the stop, and the
  clip ends at 10000 ms". The older 6000-to-10000 check passed in that run.
- The time sent 1000 ms early: the same sentence, 6458 against 7458. The
  older check passed here too.
- The duration sent as the position: "the stopped report carries the item's
  duration (10000 ms in the mock)".
- The final report sent as "playing" (the confirming helper, on the merged
  content): the first sentence above, in each player.

On the committed content: 5 of 5 passing runs in each player by the
repairing helper, 5 of 5 in each by the attacking one, and both full
suites on the merged content.

The follow-up (`7e1dda5`, `f07fd8c`, `4906831`). As first landed, nothing
that plain `node --test` runs failed when this fix was taken out, and the
reviewer's sandbox cannot run the end-to-end suite. Now:

- The rules are a function of their own, `plexStopProblems` in
  `tests/e2e/judge.mjs`: one sentence per broken rule, none when all is
  well. The rules are those listed above, unchanged.
- What the scenario hands the rules is a function too, `assertPlexStopped`
  in `multiplex.mjs` (exported): of all the requests Plex A received it
  takes the `/:/timeline` ones, prints them, and requires no problem for
  item 1, a 10000 ms duration, the 6000 ms seek and mpv's position just
  before the stop. `run()` waits for the stopped report as before and
  calls it.
- Unit tests in `tests/e2e-judge.test.mjs` compare exact lists of
  sentences: the list seen in passing runs gives none; the argument swap
  the first fix let through (time 10000, duration 7458); a report 1 s
  early, just over 500 ms early, and exactly 500 ms early (which passes);
  more than 2 s late, and exactly 2 s late (passes); no stopped report,
  two, one that is not last; a report that is not "playing" before it; a
  wrong `ratingKey`; a wrong `key`; a "playing" report further on; a
  position that was never read; a time that is no number; a stop at
  exactly the seek; a duration written "10000.0". `assertPlexStopped` is
  run on recorded requests, a passing set and the swap, and with a stop
  short of the seek.

Red-proofs of the follow-up, each restored from the commit: every rule of
`plexStopProblems` taken out in turn (fourteen injections; each failed its
own test, showing the missing sentence); the fix itself taken out (the
bounds against mpv's position removed: the swap test failed); in
`assertPlexStopped`, the position fixed at a constant, the filter widened,
the print removed, the duration, the `ratingKey` and the `key` changed
(each failed); the seek set to 0 at first SURVIVED, a hole in the guard
that `4906831` closed (it then failed). End to end, with the stopped
report's time and duration swapped in `start_tracking_plex` and the app
built, `multiplex` failed at `assertPlexStopped` with the swap's two
sentences; restored and rebuilt, it passed 3 of 3 in each player.

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, repaired by a third, read by the coordinating agent, and landed
one commit per helper commit, each with `npm run check` green.

## Coder dispute (if any)

## Known gaps

- A stopped report between 0.5 and 2 s late would pass.
- `key` was not broken apart from `ratingKey`; "exactly one stopped report"
  and "the stopped report comes last" were not each broken on their own
  (each would take a product change of its own).
- The other scenarios that call `playAndQuit` were not run one by one after
  the helper changed; they ran in both full suites on the merged content.
- Still held only by end-to-end runs: that `run()` calls
  `assertPlexStopped`, that the position is read from mpv in `beforeStop`,
  and the waits for the "playing" and the "stopped" report.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "The Plex coverage does not prove the final stopped timeline". Reviewer's severity: MEDIUM.
