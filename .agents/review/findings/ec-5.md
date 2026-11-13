# ec-5: Close can stop the play that replaced the one it was for

**Severity**: MEDIUM — a play the user just started is stopped by a Close that belonged to the play before it. It needs a Close and a replacement play within the same instant.
**Status**: Verified
**Branch**: —
**Commit**: `2a04402`

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5361-5377 releases active_playback_session before acquiring current_child and never acquires play_lock. After Close validates session A, a concurrent replacement can install and publish B before Close obtains the player-slot lock."

## Predicted observable failure

Reviewer, verbatim: "A delayed Close belonging to A kills B, contradicting the command's session-isolation promise."

Reviewer's suggested direction, verbatim: "Serialize the session check and stop operation with the same play_lock used by replacement plays, or store and validate the session identity atomically alongside the player handle."

## What

`player_close` decided in two steps under two locks: it compared the command's session with `active_playback_session`, released that lock, then locked the player slot and stopped whatever embedded player was in it. A replacement play installed between the two steps, with its own session and its own player, was stopped by a Close that had been checked against the play before it.

## Approach

The identity that matters is the player's, so the player carries it. `embedded::Session` now holds the session id it was started for (`Start::session_id`, from `Prepared`) and answers `is_play`. `PlayerHandle::close_embedded_play` stops the player only when it is the embedded play of that session, and never an external player. `player_close` locks the slot and calls that; it reads nothing else, so there is no check in one place and act in another. The active-session comparison is gone rather than kept beside it: a second, weaker condition would only invite the same split again.

## Files changed

- `src-tauri/src/commands.rs` — `player_close`.
- `src-tauri/src/player/mod.rs` — `PlayerHandle::close_embedded_play`; two tests.
- `src-tauri/src/player/embedded/session.rs` — `Session::session_id`, `is_play`, `Shared::new`, and test-only `for_test`, `stop_requested`.
- `src-tauri/src/player/embedded/host.rs` — passes the session id to the play.
- `tests/player-view.test.mjs` — the command's exact body, and the wiring of the id.

## Guard proof

Run 2026-11-13 at `867a694` (the fix is `2a04402`) in a disposable worktree; `cargo +stable test --locked --lib player::tests` and `node --test tests/player-view.test.mjs`.

| Injection | Rust | Node |
| --- | --- | --- |
| The original defect: the active session is checked, then whatever player is current is stopped | (not run) | FAIL `Close asks the player itself whose play it is, under the player's own lock` |
| Any Close stops the embedded play in the slot | FAIL `a_close_stops_the_embedded_play_it_names_and_no_other` | FAIL |
| A Close stops an external player | FAIL `a_close_never_stops_an_external_player` | FAIL |
| Every play answers to every session | FAIL `a_close_stops_the_embedded_play_it_names_and_no_other` | FAIL |
| A play answers to a session id that only begins like its own | FAIL, same test | FAIL |
| A play is started without its session id | | FAIL |
| The named play is found and not stopped | FAIL, same test | FAIL |
| A Close that is not for this play stops it anyway, in the command | | FAIL |
| The session forgets whose play it is when it starts | | FAIL |
| Unchanged | PASS | PASS |

Also run: the `embeddedplay` E2E scenario under the headless compositor, which closes a play from the page and waits for the player to stop: PASS.

## Coder dispute (if any)

## Known gaps

- That the id reaches the play from `Prepared` through `Start` into `Session` is held by source text, not by a running play: a play cannot be started in a unit test.
- The frontend's side (a view only ever sends its own session's id) was already covered and is unchanged.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "Close can stop a successor after validating the predecessor". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-13 08:57 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d415-f889-7322-8cb7-e8a57418725b`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 08:46 UTC, finished 08:52 UTC.
- Reviewed head `2a044023e09184f8a36bceb2be8a25cfbbdff6a4`, base
  `d804fc8a4d08500fb07d7922daa9fe158a29ba09` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
