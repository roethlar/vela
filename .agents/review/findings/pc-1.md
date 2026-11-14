# pc-1: a line that arrives in pieces is read past the limit of whoever is waiting for it

**Severity**: MEDIUM — a command to the embedded player can wait longer than its two seconds, holding the play's one-command gate, and a stopped play's state thread can stay alive, for as long as mpv's socket keeps sending pieces of a line or lines too long to keep. mpv does not ordinarily do either.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/player/report.rs:313 loops internally until a complete accepted line or socket timeout; oversized lines restart that loop at line 341. Consequently, control.rs:430 cannot recheck its deadline, and report.rs:485 cannot recheck the stop flag. Socket probes showed a 100 ms command budget taking 602 ms with partial data and 446 ms with consecutive oversized events. A reporter also retained its notifier 500 ms after stop while partial data continued."

## Predicted observable failure

Reviewer, verbatim: "Continuous input can hold a command worker and its gate indefinitely, making subsequent controls return busy. The reporter can likewise stop emitting state and retain its thread and connection despite cancellation."

Reviewer's suggested direction, verbatim: "Check elapsed time and cancellation between read chunks, including while discarding oversized lines. Preserve partial data across bounded returns so callers can enforce their deadlines and stop flags."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `0aeeff0..3d2fef6`, 2026-11-14 00:45 UTC
(`.agents/review/codereview-2026-11-14-embedded-controls.md`). Reviewer's
title: "Partial or oversized IPC lines bypass the read deadline". Reviewer's severity: MEDIUM.
