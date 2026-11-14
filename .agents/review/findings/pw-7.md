# pw-7: when saving the Player window setting fails, the control goes on showing the choice that was not saved

**Severity**: LOW — the control is bound to the value being saved, and a failed save only shows the error. The select keeps showing a choice the backend does not hold, and playback follows the old one until a save succeeds.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src/lib/Settings.svelte:664 binds the selection directly to playerWindowMode, while the rejection handler at lines 428-431 only sets err. Executing the save function with a rejected setter left 'fullscreen' displayed although the backend still held 'remember'."

## Predicted observable failure

Reviewer, verbatim: "After a failed save, the control continues showing a mode the backend does not hold. Playback follows the previous mode until a successful save; an error is shown, so this is a UI consistency defect rather than data corruption."

Reviewer's suggested direction, verbatim: "Restore or reload the last confirmed mode after rejection. Alternatively, explicitly distinguish the unsaved selection from the active backend value."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 3 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "A failed setting save leaves the unsaved mode selected". Reviewer's severity: LOW.
