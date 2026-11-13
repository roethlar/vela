# ec-6: an embedded start that fails after the checks passed fails the play instead of falling back

**Severity**: MEDIUM — the user gets an error instead of a film, once; decision D3 says a play is never failed over the embedded player.
**Status**: In progress
**Branch**: —
**Commit**: `31eacad`

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
