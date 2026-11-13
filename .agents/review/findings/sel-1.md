# sel-1: a settings file that cannot be read fails a play after its session is installed, and nothing is undone

**Severity**: MEDIUM — a failed play leaves the new session installed, the playlist position moved and, for a transcoded stream, an encoder running on the user's server with nothing left to stop it. It needs the settings file to become unreadable in the middle of a play start, which is rare.
**Status**: In progress
**Branch**: —
**Commit**: `88c631b`

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

Run 2026-11-13 at `db0b8b6` in a disposable worktree; each injection restored
with `git checkout` from the commit. `node --test tests/player-selection.test.mjs`, test "once a play
has installed its session, only the launch-failure path ends it early":

| Injection | Result |
| --- | --- |
| A second settings read with `?`, placed after the session is installed | FAIL (the list of early returns has two lines) |
| The original defect: the player-setting read back where Slice 2 put it | FAIL |
| A plain `return Err(..)` after the session is installed | FAIL |
| Unchanged | PASS |

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

### Verification round 1, recorded 2026-11-13 07:14 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly; the host's codex config
  resolves to gpt-6-astra @ max. Thread `01a1d3b6-a811-7470-b193-4cdbbf92144f`, run
  in the disposable worktree `~/dev/vela-probe-wt` at the head SHA; the
  stream ends in `turn.completed`; dispatched 07:02 UTC, finished
  07:07 UTC. Routing: T1 no sensitive path in the diff, T2 severity
  below HIGH, T3 the recorded proof re-run by the orchestrator agrees.
- Reviewed head `88c631b719e108032decbe5aa7c1a403f1555271`, base
  `e3b288eaddb72c43cba55ee96c4160f341c6e199` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/player-selection.test.mjs:65 — The guard misses `?` followed by a comma. Immediately before `let player_choice`, I injected a settings reread using `drop(config::load_config().map_err(...)?,);`. This reproduces the cleanup-skipping return after session/transcode installation, yet all 6 Node tests and 382 Rust tests passed. Cover this equivalent regression. Reverting the original fix did fail the named guard; restoring the pinned head passed. All mutations were restored.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. The comment is correct: the fix stands, and the test that guards
it recognises an early return only when `?` is followed by `;`, a line end
or `.`.
