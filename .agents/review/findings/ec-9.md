# ec-9: every embedded play adds a registry the compositor keeps for the life of Vela

**Severity**: LOW — memory in the compositor grows by a small amount with every embedded play and is only given back when Vela exits.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/surface.rs:118 requests a new registry each time join runs, including every VideoSurface::create. The guest connection deliberately never disconnects. The installed Wayland protocol specifies that get_registry server resources survive destruction of the client proxy and are released only when the connection disconnects."

## Predicted observable failure

Reviewer, verbatim: "Repeated plays accumulate compositor-side registry resources for the lifetime of Vela, despite dropping each VideoSurface."

Reviewer's suggested direction, verbatim: "Reuse a registry and global bindings owned for the lifetime of the shared display, while keeping per-play surfaces and event handling separately owned."

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
title: "Every embedded play leaks another server-side Wayland registry". Reviewer's severity: LOW.
