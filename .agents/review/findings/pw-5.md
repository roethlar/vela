# pw-5: an older play's window, or an earlier look at the current one, can be saved over a newer one

**Severity**: MEDIUM — whether a play is still the latest is checked, and its window is read, before the settings lock is taken; the write then uses what was read. A play whose end is slow can write after a newer play has saved, and the exit handler can write an earlier look at a window over the later one the play's own end has just saved. The next run then opens with the older fullscreen, screen or size.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5740-5750 checks the latest session before acquiring config's write locks, then writes the captured tuple without rechecking. An interleaving probe captured play A, published and saved play B, then resumed A's update: A overwrote B. The exit path at commands.rs:4322-4338 likewise overwrote a newer final observation saved by that play's end callback."

## Predicted observable failure

Reviewer, verbatim: "Remember mode can persist an older play's window or an earlier snapshot of the current play. The next run consequently opens with stale fullscreen, screen or size values."

Reviewer's suggested direction, verbatim: "Check the session identity and sample its observation inside the serialized config mutation. Coordinate end and exit persistence so an earlier snapshot cannot replace a later one, without holding the observation lock across filesystem work."

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
title: "Captured window snapshots can overwrite newer saved state". Reviewer's severity: MEDIUM.
