# fr-8: the volume can drift by one point near 0 and near 100

**Severity**: LOW — The volume can drift by one point near 0 and near 100.
**Status**: Verified — codex wrote the repair; reviewed by the working agent, 2026-11-16
**Branch**: —
**Commit**: `1f35c2b`

## Evidence

Reviewer, verbatim: "src/lib/PlayerView.svelte:317 calculates wheel targets from volumeBar, which :506 updates through nextSeekBar. That reducer accepts values within one unit as arrived (src/lib/playerControls.ts:600). Reproduced: start at 99, wheel up to 100, receive stale state 99, then wheel down; the queued target becomes 97 instead of 98."

## Predicted observable failure

Reviewer, verbatim: "Volume drifts by one point near either boundary, depending on state-event timing."

Reviewer's suggested direction, verbatim: "Use volume-specific acknowledgement logic with exact target matching, rather than the seek bar's one-second arrival tolerance."

## What

Admitted on the reviewer's reproduction: the wheel's next volume target is
worked out from a value that the seek bar's rule for "arrived" (within one
unit) has already let an older state event overwrite, so a step down from
100 after a stale 99 gives 97, not 98.

## Approach

What landed (`1f35c2b`), in its commit's words:

The volume control held its target with the seek bar's rule for
"arrived", which is "within one unit": right for a position in seconds,
wrong for a volume. An older state event one point away from the target
counted as the target having arrived and released it, and the next wheel
step was worked out from the stale value: from 99, a step up to 100, a
stale 99, a step down gave 97 where it should give 98 (review finding
fr-8).

The volume now holds its target until the player reports exactly that
value (or the hold times out, as before); the seek bar keeps its
tolerance.

Written by codex; reviewed by the working agent: with the two product
files put back, the five tests that drive the handlers near 100 and near
0, and the slider's, fail; with the repair, 98 of 98.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"The embedded player's view: in-page track lists, and the mouse". Reviewer's title: "Stale volume state can discard the latest wheel target".
Reviewer's severity: LOW.
