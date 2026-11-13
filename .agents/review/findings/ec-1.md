# ec-1: a window closed while a play is starting leaves that play a pointer to a surface that is gone

**Severity**: HIGH — a use of freed memory in Vela's own process, which can crash it. It needs the window to be closed in the short time between a play being asked for and its video surface being created.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "host.rs:262-268 retains borrowed WindowHandles, consumed by session.rs:179-189 and surface.rs:208-211. The exit sweep in src-tauri/src/lib.rs:409 handles only ExitRequested/Exit. Pinned tauri-runtime-wry 2.11.4 delivers ExitRequested after destroying the last window. Closing after playback.rs:365 acquires the slot, but before the worker consumes the parent pointer, therefore destroys the parent before the sweep can acquire that lock."

## Predicted observable failure

Reviewer, verbatim: "The worker can pass a freed wl_surface pointer to ObjectId::from_ptr, which calls wl_proxy_get_class on it: a use-after-free capable of crashing Vela."

Reviewer's suggested direction, verbatim: "In embedded mode, intercept CloseRequested and prevent native destruction until all starting and running workers have released their window references. Coordinate completion asynchronously without blocking the window thread on the player-slot lock."

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
title: "Native window close can invalidate a starting player's Wayland pointer". Reviewer's severity: HIGH.
