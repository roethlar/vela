# fr-8: the volume can drift by one point near 0 and near 100

**Severity**: LOW — The volume can drift by one point near 0 and near 100.
**Status**: In progress
**Branch**: —
**Commit**: —

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

The repair is being written by codex as a coding task (worktree
`~/dev/vela-agent-keep/wt/cx-rvview` on `continuum`, not on `main` until the
working agent has reviewed and landed it), under the owner's decision of
2026-11-14 that coding tasks may go to codex and that a repair codex wrote
is closed by the working agent's review. This section is replaced by what
landed.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"The embedded player's view: in-page track lists, and the mouse". Reviewer's title: "Stale volume state can discard the latest wheel target".
Reviewer's severity: LOW.
