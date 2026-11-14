# pa-7: nothing checks that Plex is told the play has stopped

**Severity**: MEDIUM — row 2 includes the final stop. For Plex, `multiplex` waits for any timeline request and any progress request. With the final "stopped" timeline never sent, the periodic "playing" ones still satisfy it; the other scenario of row 2, `watchstate`, is Jellyfin's and cannot see it.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "tests/e2e/scenarios/multiplex.mjs:72 claims row 2, but :243 accepts any /:/timeline request and :247 separately checks /:/progress. Neither assertion checks timeline state. If the final update_timeline('stopped') in src-tauri/src/playback.rs:1150 is omitted while periodic 'playing' reports and final progress remain, these assertions still pass."

## Predicted observable failure

Reviewer, verbatim: "The verdict can credit Plex's final-stop behaviour even though Plex receives no stopped timeline report. The other row-2 scenario, watchstate, exercises Jellyfin and cannot detect that omission."

Reviewer's suggested direction, verbatim: "Assert a final Plex timeline request with state=stopped, the correct item, and the final position. Distinguish it from reports sent while playback was running."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "The Plex coverage does not prove the final stopped timeline". Reviewer's severity: MEDIUM.
