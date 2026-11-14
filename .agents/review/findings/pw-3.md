# pw-3: a width and a height that were never reported together can be carried as the window's size

**Severity**: MEDIUM — width and height arrive as two reports. Each new one is paired with the other's previous value, and when the pair that results is out of bounds the earlier, mixed pair is kept. After 800x450, a report of width 1024 followed by a height that is refused leaves 1024x450, which the window never was; the next play opens at that size, and in the default choice it is saved for the next run.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/playback.rs:160-180 combines each dimension with the previous complementary dimension, then leaves the candidate untouched when validation fails. After confirming 800x450, reporting width=1024 followed by height=89 makes snapshot() return 1024x450. A second Rust reproduction using width=1024, height=0, width=0 also returned 1024x450. Both reproductions failed their assertions."

## Predicted observable failure

Reviewer, verbatim: "The next play receives --geometry=1024x450 even though that size was never reported as a complete pair. An invalid resize or shutdown sequence overwrites the valid carried size and can persist across restarts in Remember mode. Playback continues, but window-size continuity is corrupted."

Reviewer's suggested direction, verbatim: "Track coherent dimension pairs, for example through osd-dimensions, and retain the last valid pair when an invalid report arrives. Add regression tests starting from an existing size, with valid and invalid dimensions arriving in either order."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 2 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "Invalid dimension reports preserve an invented window size". Reviewer's severity: MEDIUM.
