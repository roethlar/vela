# ec-2: a start that times out lets go of a worker that is still running

**Severity**: HIGH — the worker keeps its thread, its surface and its player with nothing left that can stop it or wait for it, and if it comes back it hands mpv the file and announces a play that was already reported as failed. It needs a start that takes longer than 20 seconds.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/session.rs:358-366 and 433-439 ignore false from stop_and_wait. A setup operation that remains blocked, such as reading an Advanced --include pointing to an unwritten FIFO, survives the timeout. A review probe confirmed that dropping Session returns after five seconds while its worker remains alive; the existing teardown wiring test nevertheless passes."

## Predicted observable failure

Reviewer, verbatim: "Failed starts leave untracked threads, descriptors and graphics resources alive. If setup later resumes, it still loads the file and emits Opened before checking stop, despite the play already having failed; borrowed window lifetimes are no longer protected."

Reviewer's suggested direction, verbatim: "Retain ownership of timed-out workers until teardown actually completes, and cancel setup before loading or publishing lifecycle events. A timeout must not authorize releasing the worker's dependencies."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "Timeout cleanup detaches a worker that still owns player resources". Reviewer's severity: HIGH.
