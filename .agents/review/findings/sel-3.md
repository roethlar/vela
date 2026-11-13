# sel-3: a play replaced within a second and a half uses up the fallback explanation without showing it

**Severity**: LOW — the user is never told why the embedded player was not used; playback is unaffected.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5064 treats constructing the notice as telling the user, and :5213 records it immediately after launch succeeds. However, src-tauri/src/playback.rs:597 waits 1.5 seconds before sending and :598 cancels when the play is replaced. A command-path reproduction replaced the first play before that delay, then kept the replacement running: neither IPC connection received the fallback notice."

## Predicted observable failure

Reviewer, verbatim: "A quick replacement consumes the once-per-run explanation without displaying it; subsequent ordinary fallback plays silently suppress it. Playback itself remains functional."

## What

(pending)

## Approach

(pending)

## Files changed

(pending)

## Guard proof

(pending)

## Coder dispute (if any)

None.

## Known gaps

(pending)

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Record successful notice delivery instead of successful process launch. Keep canceled or failed deliveries due, with session-scoped pending state to prevent duplicate delivery."
