# pw-6: after a play in the embedded player, the next mpv window is treated as the first of the run again

**Severity**: MEDIUM — there is one record of the last play, and a play in the embedded player replaces it with one that carries no window. A play in mpv's window that follows it finds no window to inherit and gets what the setting says for the FIRST window of a run: with "Full screen" chosen, a window the user had left windowed earlier in the same run opens fullscreen again. The embedded player is hidden, so this needs it switched on by hand.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:7104 replaces the sole window record for every successful play, including embedded plays. opening_window at commands.rs:4349-4355 treats that embedded record as no previous external window. A probe using fullscreen mode, an external play left windowed, then an embedded play returned fullscreen=true for the next external play."

## Predicted observable failure

Reviewer, verbatim: "An external–embedded–external sequence discards the earlier external window and reapplies the setting to a window that is not the run's first external window."

Reviewer's suggested direction, verbatim: "Preserve the last external window separately from the latest-play metadata. Consult first_window_of_run only when no external window has been published during this run."

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
title: "An embedded play makes the first-window setting apply again". Reviewer's severity: MEDIUM.
