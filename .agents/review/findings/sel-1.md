# sel-1: a settings file that cannot be read fails a play after its session is installed, and nothing is undone

**Severity**: MEDIUM — a failed play leaves the new session installed, the playlist position moved and, for a transcoded stream, an encoder running on the user's server with nothing left to stop it. It needs the settings file to become unreadable in the middle of a play start, which is rare.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5044 propagates a settings error after installing the session at :4939 and registering its transcode at :5022. Reproduced by invalidating settings during stream resolution with start_from_beginning=true: the command returned an error, but the new session remained active, the player record still named the old session, and no transcode-stop call occurred. Removing only the new read restored cleanup."

## Predicted observable failure

Reviewer, verbatim: "A failed play leaves inconsistent playback state and skips the established transcode teardown and cursor restoration. This is a MEDIUM failure-path defect."

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
Reviewer's suggested approach, verbatim: "Read the player setting before stream resolution and provisional session installation, or route this error through the existing launch-failure cleanup."
