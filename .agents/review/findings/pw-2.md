# pw-2: a play in the embedded player is ranked for the previous mpv window's screen

**Severity**: MEDIUM — the screen for Prefer Compatible is taken from the last external play's record whatever player the coming play will use. A play that will run in the embedded player is drawn in Vela's window, on Vela's screen, and gets a copy chosen for the screen mpv's window was on. Before this work such a play was ranked for Vela's screen. The embedded player is hidden, so only someone who has switched it on by hand can meet this.
**Status**: In progress
**Branch**: —
**Commit**: `6bb2db6`, `577fe5b`

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:4893 accepts every previous external-player record without considering the upcoming player, which is resolved later at :5389. In a run initialized for embedding, switch the player setting from external playback on DP-1 back to embedded playback with Vela on HDMI-1. Ranking retains DP-1, while src-tauri/src/player/launch.rs:913 correctly omits external placement options. A temporary Rust probe confirmed this mismatch."

## Predicted observable failure

Reviewer, verbatim: "The embedded play renders on Vela's screen but Prefer Compatible selects resolution/HDR for the previous external window's screen. This regresses manual embedded launches, which previously used Vela's display."

Reviewer's suggested direction, verbatim: "Use Vela's display for an upcoming embedded play and inherit the external display only for an upcoming external play. Reconsider the target if fallback changes the effective player."

## What

The defect is as the reviewer describes. `compatibility_display` took the
screen to rank for from the record of the last play, whatever player the
coming play would use. The coming player was resolved only later, in
`play_by_key_locked`, from a second read of the settings. So a play about
to run in the embedded player, which is drawn in Vela's window, got a
Prefer Compatible copy chosen for the screen the last mpv window had been
on: the wrong resolution or HDR choice for the screen it is shown on.

The helper who attacked the first fix found that it ranked for the player
the play asked for, not the player that would run it. When the setting
asks for the embedded player but it cannot run (Vela needs a restart after
the setting changed in this run, an X11 session, the NVIDIA driver, the
mpv library missing, `StartFailed`, and on every non-Linux system), the
copy was ranked for Vela's screen, and `player::choose` in the launch then
sent the play to mpv's own window, which opened on the last external
play's screen. That is pw-1's mismatch again, every time, and a regression
from `663fe52`, where such a play was ranked for the mpv screen. The
attacking helper showed it with a test of its own: `ranked_for=None
runs_in=External opens_on=Some("DP-1")`.

## Approach

`select_playback_version` takes `player: Option<PlayerKind>` (the
replaced play's player for a relaunch, `None` otherwise) and reads the
setting from the settings it already loads, so the settings file is read
once fewer per play. Before it ranks, it decides the player that will
really run the play: it asks `player::embedded_availability` only when the
play wants the embedded player (that call is async, and its blocking part
runs in `spawn_blocking`, as before) and calls `player::choose` once. The
ranking gets `upcoming_player.kind`. `PlaybackSelection.player` is now the
whole `PlayerChoice` (the player and the reason for a fallback), and the
launch uses it (`let player_choice = selection.player.clone();`): it no
longer asks the window or chooses a second time, and the fallback notice
still comes from `player_choice.fallback`. `PlayerChoice` is re-exported
from `player/mod.rs`.

`compatibility_display(record, replace_session, upcoming)` returns the
last external play's display for a play in mpv's own window. For an
embedded play it returns nothing, which means the display holding Vela's
window, except for the exact automatic successor of an embedded play,
which uses that play's observed display. `selected_version_for` (the
title menu's "which version would play") passes `None`, so it ranks for
the player a play started now would run in.

The check whether the embedded player can run now happens before the
stream is resolved instead of just before the launch. The repairing
helper judged that safe: the library's answers are fixed for the run, or
are checked again by `prepare()` at the start (the `StartFailed` fallback).

What a user gets: an embedded play gets a copy chosen for Vela's screen,
and a play that cannot run embedded gets a copy chosen for the mpv window
it opens in.

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, repaired by a third, confirmed by a fourth on the merged tree, and
read and landed by the coordinating agent, one commit per helper commit.

## Files changed

- `src-tauri/src/commands.rs`
- `src-tauri/src/player/mod.rs` (re-export of `PlayerChoice`, repair
  commit)
- `tests/player-selection.test.mjs`

## Guard proof

In `commands::tests` (`src-tauri/src/commands.rs`):

- `an_embedded_play_is_ranked_for_the_screen_holding_velas_window`: the
  reviewer's case: an external play on DP-1, then an embedded play, which
  must be ranked for Vela's display.
- `a_compatible_choice_follows_the_screen_the_play_opens_on`: rewritten
  per player and per record.
- `a_play_that_cannot_run_embedded_is_ranked_for_the_mpv_window_it_opens`:
  runs the play path's steps (`choose` with `Err(NeedsRestart)`, rank,
  open) and requires that the ranked screen and the opened screen are both
  DP-1; it also requires that a play that can run embedded is ranked for
  Vela's screen.

In `tests/player-selection.test.mjs`, "every play chooses its player from
the setting and what can run" holds, in `select_playback_version`'s text:
the wanted player is read from the settings already loaded;
`embedded_availability` is asked only for an embedded play; `let
upcoming_player = crate::player::choose(wanted_player,
&embedded_available);` comes exactly once, before
`playback_compatibility_target(state, replace_session,
upcoming_player.kind, &cfg)`; both ready selections carry it; and the play
path binds `let player_choice = selection.player.clone();` once and has no
`from_setting(`, `player::choose(` or `embedded_availability(`.

Red-proofs, each restored from the commit afterwards:

- `compatibility_display` ignores the coming player: 2 tests failed, the
  new one with left `Some(DP-1)`, right `None`.
- The ranking is passed a fixed external player; the play path reads the
  setting again; the coming player ignores the relaunch's player: each
  failed the Node pins.
- Repair, the ranking passed `wanted_player` instead of
  `upcoming_player.kind` (the attacking helper's regression): the Rust
  tests stayed green, as expected, because no Rust test reaches
  `select_playback_version` (it needs `AppState` and sources); the Node
  test failed with "the copy is ranked for the player that will run the
  play".
- Repair, the launch chooses again instead of using the carried choice:
  the Node test failed on the `/let player_choice =
  selection\.player\.clone\(\);/` pin.
- Repair, `choose`'s fallback arm keeps the embedded player: the new Rust
  test failed at its first assertion.
- The attacking helper, on the first fix: the single-copy path's
  `PlaybackSelection.player` read again from the setting, dropping the
  relaunch's player: the Node pin "both ready selections carry it" failed.
- The confirming helper, on the merged tree: the embedded arm returns the
  last external display (the reviewer's defect):
  `an_embedded_play_is_ranked_for_the_screen_holding_velas_window` failed
  with left `Some(DP-1)`, right `None`, and 2 more failed. The ranking
  given `wanted_player` again: Rust green, the Node test failed with "the
  copy is ranked for the player that will run the play".

On the merged tree (the confirming helper's trial merge of both branches
onto `main` at `efbf30c`; `main` has since moved by commits that touch
none of these files), the full check was green apart from the two
recorded audit exits (`npm audit` exits 1 on known advisories, `cargo
audit` exits 101 because it is not installed): 641 Rust tests passed (10
ignored), and of 413 Node tests 412 passed and 1 was skipped, an older
skip. Both end-to-end suites were run with a fresh build: external 46/48
and embedded 49/50, the only failures being the recorded ones
(`embeddedownconfig` in the external suite, `pagefail` in both), and `npm
run e2e:verdict` gave "parity verdict: PASS".

What rests on source-text checks only: that `select_playback_version`
decides the player before it ranks, ranks with the player that will run,
and hands that choice to the launch, which does not choose again. The
Rust tests prove `compatibility_display` and `choose` themselves, not
that the play path calls them this way.

At landing (2026-11-14) the ten commits of this review's fixes were put on
a branch cut from `main` in the helpers' order. Each passed `npm run check`
and `cargo +stable test --locked` on its own before the next was added,
and the full check was green on the last (641 Rust tests, 424 Node tests;
the two audit commands exit as recorded). The product code and the two
player-window test files there were byte for byte the tree the confirming
helper had run end to end.

## Coder dispute (if any)

## Known gaps

- An embedded play that could run and then fails to start (the
  `StartFailed` fallback at the launch, or a leftover embedded play counted
  by `session::unfinished_plays` that appears during resolution) runs in
  mpv's own window with a copy ranked for Vela's screen. Undoing that would
  need a second resolution, which was not built, as decided.
- The wiring from `select_playback_version` to the ranking is guarded by
  source text only (see the guard).
- A change of behaviour to note: the title menu's "which version would
  play" (`selected_version_for`) now ranks for the player a play started
  now would run in, and asks the window whether the embedded player can
  run when the setting wants it.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 1 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "Choose the compatibility display using the upcoming player". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 11:08 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d9b7-5fe3-72f2-8593-8a176768d068`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 11:00 UTC, finished 11:08 UTC.
- Reviewed head `0e5ea592532c476cafe6a1142dc6de68d2f57ef1`, base
  `530e9a544cf0b7f2f3f4837aec093b1d5b3f0492` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/commands.rs:5380 — Late StartFailed fallback leaves pw-2 incomplete. With the previous external window on DP-1 and Vela on HDMI-1, an available embedded choice ranks for HDMI-1. If preparation/start subsequently fails, including because an unfinished embedded play appears, playback::play launches the same resolved stream externally on DP-1. Carrying PlayerChoice does not prevent this later player change. Reconcile the selected version and destination when fallback occurs; documenting this as a known gap does not close the defect.
  - tests/player-selection.test.mjs:31 — Guard proof confirmed: restoring the original display mistake failed all three named Rust guards; passing wanted_player to ranking failed the Node guard. After restoration, 76 Rust and 13 Node tests passed. Coverage remains incomplete: forcing the actual rank_candidates call to Best also passed both suites. The Rust guards test helpers, and the text pins do not establish the selected version or its correctness after StartFailed. Add a behavioral selection-to-fallback guard. All mutations were restored; embedded/display and E2E behavior was inspected, not executed.
