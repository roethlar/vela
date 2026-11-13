# ec-5: Close can stop the play that replaced the one it was for

**Severity**: MEDIUM — a play the user just started is stopped by a Close that belonged to the play before it. It needs a Close and a replacement play within the same instant.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5361-5377 releases active_playback_session before acquiring current_child and never acquires play_lock. After Close validates session A, a concurrent replacement can install and publish B before Close obtains the player-slot lock."

## Predicted observable failure

Reviewer, verbatim: "A delayed Close belonging to A kills B, contradicting the command's session-isolation promise."

Reviewer's suggested direction, verbatim: "Serialize the session check and stop operation with the same play_lock used by replacement plays, or store and validate the session identity atomically alongside the player handle."

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
title: "Close can stop a successor after validating the predecessor". Reviewer's severity: MEDIUM.
