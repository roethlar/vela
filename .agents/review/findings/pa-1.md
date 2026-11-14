# pa-1: a play left running reaches the end of the marker by itself, so the automatic-skip test passes without a skip

**Severity**: MEDIUM — `markers`, the only scenario that stands for automatic skipping (row 8a) in either player, waits up to 14 and 16 seconds for a running play to be past second 12. Ordinary playback gets there in that time, so the row is counted as shown, and the parity verdict passes, with automatic skipping broken.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "tests/e2e/scenarios/markers.mjs:90 credits row 8a, but the assertions at :125 and :148 allow 14 and 16 seconds respectively for playback to reach 12 seconds. Neither leg pauses playback. A clock-controlled probe using the actual pollUntil implementation passed both predicates at 12 seconds without any skip."

## Predicted observable failure

Reviewer, verbatim: "If marker loading still works but automatic seeking stops working, both legs pass through natural playback and shown('8a') is reached. This is the only scenario credited for automatic skipping in either player, so the verdict can falsely certify that behaviour."

Reviewer's suggested direction, verbatim: "Establish a known position inside the marker and require a jump across its end within a period too short for ordinary playback to cross it. Prove the assertion fails with automatic seeking disabled."

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
title: "Ordinary playback satisfies the autoskip proof". Reviewer's severity: MEDIUM.
