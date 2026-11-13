# ec-7: a stream-auth file whose removal fails once is never removed

**Severity**: MEDIUM — a file holding the stream's credentials stays on disk after the play, in Vela's private runtime directory. It needs the removal to fail at the one moment it is tried.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/session.rs:423-427 takes HeaderInclude out of Session and drops it when remove_now fails, unlike external.rs:47-57, which restores ownership. A review probe made the include directory temporarily non-writable, called removal, restored permissions, and dropped Session; the credential file remained."

## Predicted observable failure

Reviewer, verbatim: "A temporary deletion failure leaves the stream-auth include on disk permanently because reaping and exit no longer retain its path for retry."

Reviewer's suggested direction, verbatim: "Restore the include to Session when removal fails, matching ManagedChild, so reaping or exit can retry after the transient failure clears."

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
title: "Failed auth-file deletion loses its cleanup owner". Reviewer's severity: MEDIUM.
