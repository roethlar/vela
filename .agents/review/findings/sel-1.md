# sel-1: a settings file that cannot be read fails a play after its session is installed, and nothing is undone

**Severity**: MEDIUM — a failed play leaves the new session installed, the playlist position moved and, for a transcoded stream, an encoder running on the user's server with nothing left to stop it. It needs the settings file to become unreadable in the middle of a play start, which is rare.
**Status**: In progress
**Branch**: —
**Commit**: (this commit)

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5044 propagates a settings error after installing the session at :4939 and registering its transcode at :5022. Reproduced by invalidating settings during stream resolution with start_from_beginning=true: the command returned an error, but the new session remained active, the player record still named the old session, and no transcode-stop call occurred. Removing only the new read restored cleanup."

## Predicted observable failure

Reviewer, verbatim: "A failed play leaves inconsistent playback state and skips the established transcode teardown and cursor restoration. This is a MEDIUM failure-path defect."

## What

`play_by_key_locked` commits a play in stages: it resolves the stream,
installs the new session and playlist position, stops the previous player,
registers the transcode it may have started, and only then starts the
player. One arm at the end of the function undoes all of that when the
player does not start. Slice 2 added a settings read, with an early return
on failure, in the middle of those stages. That return skipped the arm.

## Approach

The read moved to where the function reads its other settings, before the
stream is resolved (next to `SkipPolicies::load()`). A failure there ends
the play while nothing has been resolved, installed or registered, which
is what already happens for the settings read beside it. The choice
between the players stays where it was; it has no way to fail.

The reviewer's other suggestion, sending the error through the cleanup
arm, would have kept a fallible step in the stretch where every fallible
step is a hazard.

## Files changed

- `src-tauri/src/commands.rs` — `play_by_key_locked`: `wanted_player` is
  read before `let resolved = ...`
- `tests/player-selection.test.mjs` — new test "once a play has installed
  its session, only the launch-failure path ends it early"

## Guard proof

(pending)

## Coder dispute (if any)

None.

## Known gaps

- The guard is a source-text test, like the other guards on this
  function's ordering: nothing in the repo can drive `play_by_key_locked`
  with a settings file that fails on demand. It lists every early return
  between the installation and the cleanup arm and requires the list to be
  exactly one known line.
- That one line is the same defect in older code (the settings read for
  Vela's own resume position). It is outside the reviewed range and the
  approved plan, so it is listed, not fixed: `.agents/state.md`, Blockers.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Read the player setting before stream resolution and provisional session installation, or route this error through the existing launch-failure cleanup."
