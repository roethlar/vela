# sel-2: an Automatic quality step-down makes the fallback explanation appear again

**Severity**: LOW — one extra line on screen; playback is unaffected.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5208 records None for a successful step-down: apply_step_down supplies the actual External player, so choose reports no fallback. FallbackNotices::record at src-tauri/src/player/select.rs:107 then clears the previous notice. Exercising the command path with unchanged player="embedded" produced IPC notices in this order: fallback, quality change, fallback on the next play."

## Predicted observable failure

Reviewer, verbatim: "The same fallback explanation appears twice within one Vela run despite unchanged settings and availability. Playback succeeds; the defect is repeated informational output."

## What

(pending)

## Approach

(pending)

## Files changed

(pending)

## Guard proof

(pending)

## Coder dispute (if any)

None.

## Known gaps

(pending)

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Preserve fallback notice history across automatic relaunches of the same logical play. Add coverage for fallback followed by step-down followed by another play."
