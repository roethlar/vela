# pa-2: the external player's cropping is tested in automatic mode only, and the row says off, manual and automatic

**Severity**: MEDIUM — row 9 is "Autocrop off / manual / auto". In the external player `autocrop` is the only scenario that stands for it, and it sets automatic mode and nothing else. A manual toggle that does nothing, or an Off that still crops, leaves the row shown and the verdict passing; the embedded player's scenario does exercise manual cropping, so the parity claimed is one-sided.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "tests/e2e/scenarios/autocrop.mjs:117 claims row 9, whose inventory includes off/manual/auto. Its configuration at :131 always selects auto, and its two legs at :148 and :159 test only fresh and resumed automatic cropping. It is the sole external-player coverage for row 9."

## Predicted observable failure

Reviewer, verbatim: "A broken external manual-crop binding, or an Off setting that still enables cropping, can leave the scenarios and verdict green. Manual cropping is exercised in embeddedcrop, so the declared parity conceals a one-sided proof."

Reviewer's suggested direction, verbatim: "Add external manual and Off legs that verify both the absence of automatic cropping and the manual toggle's effect. Credit the complete row only after all its modes have been exercised."

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
title: "Row 9 credits external crop modes that are never exercised". Reviewer's severity: MEDIUM.
