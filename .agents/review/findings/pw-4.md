# pw-4: quitting Vela waits, without limit, for the settings file to be written

**Severity**: MEDIUM — when Vela exits with a play's window to save, the exit handler reads and writes the settings file on the spot. That waits for the settings lock (which another Vela process can hold) and for the disk, with no deadline, on the thread that is closing the app. A stalled writer or filesystem keeps Vela from exiting.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/lib.rs:514 calls remember_window_on_exit directly from the Tauri exit callback. src-tauri/src/commands.rs:4307-4311 synchronously loads and updates config; storage.rs:396-414 waits on both locks and performs filesystem writes without a deadline. A probe holding config.lock confirmed that this call returns only after the lock is released."

## Predicted observable failure

Reviewer, verbatim: "Quitting with a changed window freezes the event loop until config persistence finishes. A stalled config writer or filesystem can prevent Vela from exiting indefinitely."

Reviewer's suggested direction, verbatim: "Perform persistence on a blocking worker with a bounded shutdown protocol. Keep the event loop responsive and allow exit when the persistence deadline expires."

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
title: "Exit performs an unbounded synchronous config write". Reviewer's severity: MEDIUM.
