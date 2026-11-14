# pa-4: a timer started through Node's timer modules is not seen by the watch on importing a scenario

**Severity**: MEDIUM — the parity test says that importing a scenario does nothing, and that timers are among what it watches. It watches the global timer functions only. A scenario that imports `setTimeout` from `node:timers`, or awaits a delay from `node:timers/promises`, at import passes; a callback can then run later, in the runner or during another scenario.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "tests/e2e-parity.test.mjs:77 patches global timer functions only. A temporary scenario importing setTimeout from node:timers and scheduling an unreferenced timer passed all 13 tests; an awaited node:timers/promises delay also passed. The resource-count check at :94 does not expose the unreferenced timer."

## Predicted observable failure

Reviewer, verbatim: "Importing a scenario can schedule callbacks that later affect the runner or another scenario while the guard reports no import side effects. This contradicts the advertised timer coverage."

Reviewer's suggested direction, verbatim: "Instrument the timer-module APIs as well as globals, or track timer creation through an appropriate runtime hook. Add negative tests for both promise-based timers and unreferenced timers."

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
title: "The import watch misses timers from Node's timer modules". Reviewer's severity: MEDIUM.
