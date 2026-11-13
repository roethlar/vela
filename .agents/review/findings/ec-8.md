# ec-8: a failed embedded start leaves the marker payload file behind

**Severity**: LOW — a small file of marker times (no credentials) stays in the runtime directory until a later play with markers clears it.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/host.rs:346-351 discards LaunchPlan.marker_payload during destructuring, and the Session::start error path never calls discard_marker_payload. An EGL failure before mpv initialization occurs before the marker script can consume and unlink the file."

## Predicted observable failure

Reviewer, verbatim: "The marker JSON survives the failed play and application exit, unless a later marker-bearing launch happens to prune it."

Reviewer's suggested direction, verbatim: "Keep a cleanup guard for the marker payload through startup and remove it on every failure before the script consumes it."

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
title: "Failed embedded starts abandon marker payload files". Reviewer's severity: LOW.
