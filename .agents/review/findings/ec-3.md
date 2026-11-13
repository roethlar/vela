# ec-3: the settings read at start-up waits forever on a settings path that is not a regular file

**Severity**: MEDIUM — Vela never opens its window, not even its recovery screen, and this is on every Linux start whether or not the embedded player was chosen. It needs a settings path that is a pipe, a device or a link to one.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/startup.rs:138-146 unconditionally calls read_to_string before GTK or durable::load. With config.json replaced by a FIFO, a review subprocess blocked here; an ordinary external-settings file returned immediately. The previous storage path rejects nonregular files in storage.rs:62-72."

## Predicted observable failure

Reviewer, verbatim: "Vela never opens its window or recovery UI for this invalid settings path, even when embedded playback was never selected. A symlink to an unbounded device also bypasses the previous rejection."

Reviewer's suggested direction, verbatim: "Preserve the storage loader's regular-file and symlink checks before reading the startup setting. Bound this preliminary read and treat rejected input as ordinary window mode."

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
title: "The new startup read bypasses nonregular-file protection". Reviewer's severity: MEDIUM.
