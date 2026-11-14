# pa-7: nothing checks that Plex is told the play has stopped

**Severity**: MEDIUM — row 2 includes the final stop. For Plex, `multiplex` waits for any timeline request and any progress request. With the final "stopped" timeline never sent, the periodic "playing" ones still satisfy it; the other scenario of row 2, `watchstate`, is Jellyfin's and cannot see it.
**Status**: In progress
**Branch**: —
**Commit**: `17d9a81`

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

No check that plain `node --test` can run fails when this fix is taken
out. That part is being added as a follow-up commit before the fix goes to
the reviewer, whose sandbox cannot run the end-to-end suite.

## Coder dispute (if any)

## Known gaps

- A stopped report between 0.5 and 2 s late would pass.
- `key` was not broken apart from `ratingKey`; "exactly one stopped report"
  and "the stopped report comes last" were not each broken on their own
  (each would take a product change of its own).
- The other scenarios that call `playAndQuit` were not run one by one after
  the helper changed; they ran in both full suites on the merged content.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "The Plex coverage does not prove the final stopped timeline". Reviewer's severity: MEDIUM.
