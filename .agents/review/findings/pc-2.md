# pc-2: a play that ends in an error leaves Vela's window in fullscreen for 50 seconds

**Severity**: MEDIUM — after an embedded play in fullscreen ends in an error, the player view goes and the library stays fullscreen until a 50-second backstop, with no successor coming.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src/routes/+page.svelte:112 requires two playback-ended events before leaving fullscreen. An error-ending play produces only one: playback.rs:681 excludes errors from the clean-completion dispatcher that emits the second event. Executing the page helpers for fullscreen → player-closed → playback-ended produced no fullscreen-off request, only the 50,000 ms timer from +page.svelte:94."

## Predicted observable failure

Reviewer, verbatim: "When an embedded play ends with a playback error, its view disappears but the library remains in player-owned fullscreen for 50 seconds despite there being no successor."

Reviewer's suggested direction, verbatim: "Report an explicit session-scoped terminal outcome, including errors and whether a successor is pending, and leave player-owned fullscreen on that outcome. Reserve the backstop for missing events."

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
title: "Playback errors leave Vela fullscreen for 50 seconds". Reviewer's severity: MEDIUM.
