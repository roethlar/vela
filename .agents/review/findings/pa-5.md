# pa-5: the verdict takes a known failure that was skipped for one that failed

**Severity**: MEDIUM — for a scenario recorded as a known failure, the verdict compares the error text of any result that is not a pass. A result marked skipped that carries the recorded text is accepted, so the verdict can pass on a run in which the scenario never ran.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "tests/e2e/scenario-loader.mjs:201 checks the error text for every non-pass known-failure result without requiring status === 'fail'. With otherwise valid synthetic results, changing external embeddedownconfig to status 'skip' while retaining its error produced CLI exit 0, 'embeddedownconfig ... skip', and 'parity verdict: PASS'."

## Predicted observable failure

Reviewer, verbatim: "The verdict accepts evidence in which a required scenario was skipped instead of reproducing its recorded defect, undermining the full-run requirement."

Reviewer's suggested direction, verbatim: "Require an explicit fail status before matching a known failure's error. Reject skipped, missing, and unknown statuses independently of their error text."

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
title: "A skipped known failure is accepted as an observed failure". Reviewer's severity: MEDIUM.
