# ec-6: an embedded start that fails after the checks passed fails the play instead of falling back

**Severity**: MEDIUM — the user gets an error instead of a film, once; decision D3 says a play is never failed over the embedded player.
**Status**: Verified
**Branch**: —
**Commit**: `31eacad`, `ff147c2`

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/egl.rs:166-173 checks available formats without creating the required OpenGL 3.3 context. If context or renderer creation subsequently fails, host.rs:375-381 only marks future starts unavailable, and playback.rs:376 propagates the error. No external launch is attempted for that play."

## Predicted observable failure

Reviewer, verbatim: "On a machine that passes the preliminary probe but cannot create the renderer, the first Play fails outright; only a later Play falls back. This contradicts D3 and the new development documentation's fallback promise."

Reviewer's suggested direction, verbatim: "After confirming complete cleanup of the failed embedded start, rebuild the launch for the external player and complete the original play with its fallback notice."

## What

Decision D3 of the plan says a play is never failed over the embedded player: when it cannot run, the play goes to the external player with one line saying why. That held when the embedded player was known to be unavailable before the play began. It did not hold when the embedded player had answered "available" and then could not start the play: no usable OpenGL context after the format check had passed, mpv refusing its options, a start that took too long. `playback::play` returned that error, the play failed, and only the next play went to the external player. This was a shortcut of the agent's, recorded as one in the plan's log; the review did not let it stand.

## Approach

`playback::play` now starts a play's player through `start_player`: the embedded one when the play was given it, and if that fails (in preparing, which asks the window, or in starting), the external one, inside the same launch and under the same lock on the player slot. The external launch is described afresh for the external player (`launch::build_launch_for`, so it carries the external player's options and none of the embedded one's) and on a socket of its own (`another_ipc_path`: the embedded player that failed may still own the first). Everything after that point in `play` (the watchers, the trackers, the notice) follows the player that started. `play` returns which player that was (`PlayStarted`), and the command layer records it: the play counts as the external player's from then on (a relaunch stays there), it does not count as proof that nothing needs explaining, and the one line "Embedded player: could not start" is planned under the same once-per-run rule as every other fallback and marked as told only when it has been shown. The failed start has by then put back what it did: findings `ec-8` (the marker payload) and `ec-2` (a thread that will not end is kept and cannot come back as a play). A play given the external player takes exactly the path it took before: the same calls in the same order.

## Files changed

- `src-tauri/src/playback.rs` — `start_player`, `another_ipc_path`, `PlayStarted`, `EmbeddedFallback`, `PlaySpec::if_embedded_cannot_start`; `play` uses them; five tests.
- `src-tauri/src/player/launch.rs` — `build_launch_for(spec, player, ...)`; `build_launch` is that for the play's own player.
- `src-tauri/src/commands.rs` — plans what a fallen-back play says; records the player that started.
- `src-tauri/src/player/embedded/host.rs`, `session.rs` — a development-only way to shorten the wait for a start (`VELA_EMBEDDED_START_LIMIT_MS`; a release build ignores it), for the E2E scenario.
- `tests/e2e/scenarios/embeddedlatestart.mjs` (new), `tests/e2e/README.md`, `docs/development.md`.
- `tests/player-selection.test.mjs`, `tests/mpv-lifecycle.test.mjs`, `tests/embedded-player.test.mjs` — one new wiring test; pins follow the moved text.

## Guard proof

Run 2026-11-13 at `499a0d3` and `7fa009b` (the code of this finding is the same at both) in a disposable worktree; `cargo +stable test --locked --lib playback::tests`, `node --test tests/player-selection.test.mjs tests/mpv-lifecycle.test.mjs`, and the E2E scenario `embeddedlatestart` under the headless compositor with the real players.

| Injection | Rust | Node | E2E |
| --- | --- | --- | --- |
| The original defect: a play the embedded player cannot start fails | FAIL `a_play_the_embedded_player_cannot_start_runs_in_the_external_one` and one more | | FAIL: "timed out waiting for the play to start in the external player" |
| A play the embedded player started is recorded as the external player's | FAIL, two tests | | |
| The external launch after a failure uses the socket of the player that failed | | FAIL `a play the embedded player cannot start goes on in the external player` | |
| A second socket path may be the first again | FAIL `a_second_launch_of_a_play_gets_a_socket_of_its_own` | | |
| The external launch after a failure is described for the embedded player | | FAIL | |
| The trackers stay on the socket of the player that failed | | FAIL | |
| A play that fell back says nothing | | FAIL | FAIL: "timed out waiting for the notice on the external player" |
| When neither player starts, only the external one is blamed | FAIL `a_play_neither_player_can_start_fails_and_says_why_for_both` | | |
| The play is still recorded as the embedded player's | | FAIL | |
| A play that fell back counts as proof that nothing needs explaining | | FAIL `a fallback is explained once, and only by a play that started` | |
| The fallback from a failed start is planned as no fallback | | FAIL | |
| Finding `ec-2`'s defect, for comparison: the late play loads and announces itself | | | FAIL: "the player view never opened for a play that was given up on" |
| Unchanged | PASS (22) | PASS | PASS |

Also run before the commit: the full E2E suite in the ordinary venue (39 pass; `pagefail` fails as recorded in `.agents/state.md`; `continuetv` failed once and is a race in the scenario, recorded there too, seen on a build from before this change as well), and seven scenarios in the embedded venue, all passing.

## Coder dispute (if any)

## Known gaps

- No unit test calls `play` itself down the fallback path: that would start the real `mpv` program on whatever machine runs the tests. The decision is tested as a function with stand-ins for both players; that `play` uses it, and what it does around it, is held by source text; the whole path is exercised by the E2E scenario, which CI does not run.
- When the embedded start fails by taking too long, the play carries on only after the wait for it (20 seconds) and the wait for its thread to stop (5 more). A user sees a long pause and then the film in mpv.
- The stream-auth include and marker payload are written twice for such a play (once per launch). The first pair is removed by the failed start.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "An embedded setup failure does not fall back for the current play". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-13 18:46 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d61b-b90c-7640-be07-19ecfeb21f1f`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 18:11 UTC, finished 18:18 UTC.
- Reviewed head `31eacad98f252a1bec7ca26df5e4a5e7cef668f7`, base
  `cdaccafe8b013d889ef8e84c8ef64edd18bb0aba` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/playback.rs:465 — Fallback calls find_mpv() while holding the player-slot mutex. resolve_mpv() waits for candidate executables' --version processes without a timeout, so a stalled probe can hold this lock indefinitely, blocking the main-thread exit handler before transcode cleanup. A headless play() probe confirmed that fallback holds the lock throughout discovery, whereas an ordinary external play leaves it available. Resolve mpv and build its launch outside the lock, then reacquire it and recheck shutdown before spawning and publishing.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. The comment is correct. Finding the `mpv` program runs candidate
programs (`mpv --version`) and waits for them, with no limit; a play given
the external player does that before it takes the lock on the player slot,
and the fallback did it inside the lock. The exit handler takes the same
lock on the window's thread. The repair looks for mpv and describes the
launch with the lock released, and takes it again only to check that Vela
is not exiting, start the player and publish it.

### Repair (round 2), `ff147c2`

`play` now takes the lock on the player slot once per attempt to start a
player, and does nothing under it but check that Vela is not exiting,
start the player and publish it. The embedded attempt is one such hold.
If it fails, the lock is released; the search for the `mpv` program, the
second socket path and the description of the external launch all happen
with the lock free; then the lock is taken a second time, the exit check
is made again, and the external player is started and published. A play
given the external player is as before: it has looked for mpv before it
takes the lock at all.

One case had to be told apart for this: a play that comes to start the
embedded player and finds Vela exiting. That is not the embedded player
failing, and the external player must not be started in its place.
`start_player`'s embedded side now says which it was (`NotStarted::CouldNot`
or `MustNot`).

Three new tests drive the whole of `play` down the fallback path, which
the first version could only pin by source text. They use stand-ins at
both ends: an embedded launcher whose start fails, and a small program in
place of mpv that writes down its arguments (a test-only hook replaces the
search for mpv on the calling thread, so no test depends on, or starts, a
real player).

Files: `src-tauri/src/playback.rs`, `tests/player-selection.test.mjs`,
`tests/mpv-lifecycle.test.mjs`.

Guard proof for round 2, run 2026-11-13 at `ff147c2` in a disposable
worktree; `cargo +stable test --locked --lib playback::tests` and
`node --test tests/player-selection.test.mjs tests/mpv-lifecycle.test.mjs`:

| Injection | Rust | Node |
| --- | --- | --- |
| The first repair, which was reopened: mpv is looked for with the player slot locked | FAIL `looking_for_mpv_after_a_failed_embedded_start_does_not_hold_the_player_slot` | FAIL `a play the embedded player cannot start goes on in the external player` |
| The fallback starts a player without checking again that Vela is not exiting | FAIL `a_fallback_is_not_started_once_vela_has_begun_to_exit_during_the_search` | FAIL |
| A play that must not start is tried in the external player all the same | FAIL `a_play_that_must_not_start_is_not_started_in_the_other_player_either` | |
| Vela exiting is taken for the embedded player failing | FAIL `an_embedded_play_is_not_started_once_vela_is_shutting_down` | FAIL |
| The embedded player failing is taken for a play that must not start (the original defect) | FAIL, three tests | FAIL |
| The fallback is described for the embedded player | FAIL `a_play_the_embedded_player_cannot_start_is_started_in_the_external_one_by_play` | FAIL |
| The fallback uses the socket of the player that failed | FAIL, the same test | FAIL |
| A play given the external player no longer checks that Vela is not exiting | (not run) | FAIL, two tests |
| Unchanged | PASS (26) | PASS |

Also run: `embeddedlatestart`, `embeddedplay` and `playback` in the embedded
venue, and seven scenarios in the ordinary venue (`playback`, `resume`,
`playerfallback`, `playverbs`, `markers`, `continueon`, `continueoff`): all
pass.

Still true, and not changed here: the embedded start itself runs under the
lock on the player slot, as every start does, and it can take as long as
the wait for a start (20 seconds) plus the wait for its thread to stop (5).
Quitting Vela in that time waits for it. That is deliberate: the exit
handler must not pass a player that is in the middle of starting, because
that player draws on the window's connection. It is slow only when an
embedded start is already going wrong.

### Verification round 2 (repair-delta redispatch), recorded 2026-11-13 19:18 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d642-cac8-7601-a66c-18e8aa4fc5b2`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 18:54 UTC, finished 19:01 UTC.
- Reviewed head `ff147c21cf860efc93bda5c8ab728db2ce7fe4a4`, base
  `544c9cd1e82bdeb02e0854f003d06ce8baecea9c` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
