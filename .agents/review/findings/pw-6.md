# pw-6: after a play in the embedded player, the next mpv window is treated as the first of the run again

**Severity**: MEDIUM — there is one record of the last play, and a play in the embedded player replaces it with one that carries no window. A play in mpv's window that follows it finds no window to inherit and gets what the setting says for the FIRST window of a run: with "Full screen" chosen, a window the user had left windowed earlier in the same run opens fullscreen again. The embedded player is hidden, so this needs it switched on by hand.
**Status**: Verified
**Branch**: —
**Commit**: `55be8fc`, `0e5ea59`

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:7104 replaces the sole window record for every successful play, including embedded plays. opening_window at commands.rs:4349-4355 treats that embedded record as no previous external window. A probe using fullscreen mode, an external play left windowed, then an embedded play returned fullscreen=true for the next external play."

## Predicted observable failure

Reviewer, verbatim: "An external–embedded–external sequence discards the earlier external window and reapplies the setting to a window that is not the run's first external window."

Reviewer's suggested direction, verbatim: "Preserve the last external window separately from the latest-play metadata. Consult first_window_of_run only when no external window has been published during this run."

## What

The defect is as the reviewer describes. `AppState.playback_window_session`
held one record: the last play that started. `publish_started_play`
replaced it for every play, embedded plays included, and `opening_window`
treated an embedded record as "no external window yet", so it fell back to
`first_window_of_run`. With the setting on "Full screen", the sequence
"external play left windowed, then an embedded play, then an external
play" opened the third play fullscreen again: a window the user had left
windowed earlier in the same run.

The helper who attacked the first fix found that it opened a second
disagreement. After the first fix, `window_to_remember` and the exit still
read the latest play. When an external play A ended only after an embedded
play B had started, A's end saved nothing, and quitting saved nothing,
because the latest play was embedded. The next mpv window of this run
still opened as A left it, but the next run opened as some older saved
window. That is likely when the user starts an embedded title while A
runs, because A's end waits on its final check-in with the server. Losing
A's window was possible before the fix too; what was new is that this run
and the next disagreed. The attacking helper showed it with a test of its
own: `in-run opens: fs=Some(false) screen=Some("HDMI-1"); saved for next
run: None; exit wrote: false`.

## Approach

The slot now holds a `PlaybackWindowRecord` with two parts: `latest` (the
latest play that started, in either player) and `last_external` (the
latest play that started in mpv's own window). Both are set only by
`PlaybackWindowRecord::publish`, which `publish_started_play` calls. How
the next mpv window opens (`opening_window`, `inherited_window_state`,
`inherited_screen_name`) reads `last_external`, so the setting is
consulted only while no external play has started in this run. The
detected display in Settings (`get_playback_preferences`) and the embedded
exact-successor case of `compatibility_display` read `latest`.

What a play's end and Vela's exit save follows `last_external` too:
`window_to_remember` uses `last_external` filtered by the session id, and
the exit (`remember_window_in` with no session) saves `last_external`'s
window. This departs on purpose from the earlier decision that the end
and the exit use the latest play. The reason is that the window saved for
the next run is then the same window the next mpv window of this run
would open from, as Goal 2 of the plan asks ("the first window of a run
opens as the last run left it"). An embedded play's end still saves
nothing, and an older external play's end still saves nothing once a
newer external play has started. For a run with only external plays
nothing changes, because `latest` and `last_external` are then the same
play. Doc comments in `commands.rs` and `lib.rs` now say "the last
external play".

What a user gets: an embedded play no longer makes the next mpv window
open as the first of the run, and an mpv window left just before an
embedded play is still remembered for the next run.

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, repaired by a third, confirmed by a fourth on the merged tree, and
read and landed by the coordinating agent, one commit per helper commit.

## Files changed

- `src-tauri/src/commands.rs`
- `src-tauri/src/lib.rs` (the slot's type, its initial value, doc
  comments)
- `tests/player-selection.test.mjs` (first commit only)
- `tests/player-window-ui.test.mjs`

## Guard proof

In `commands::tests` (`src-tauri/src/commands.rs`):

- `an_embedded_play_does_not_make_the_setting_apply_again`: the reviewer's
  sequence through `publish_started_play`, with the setting on "Full
  screen": an external play left windowed, an embedded play, then the
  next external play must open windowed on the first one's screen. Since
  the repair it also requires that the external play's
  `window_to_remember` is that play's window (it was `None`).
- `an_external_plays_late_end_after_an_embedded_play_saves_its_window`:
  the attacking helper's sequence through `publish_started_play`,
  `write_play_end` (the end callback's function) and
  `remember_window_on_exit_with` (the exit's function), with settings
  held in memory. Its last block covers an older external play's end
  after a newer external play.
- `only_the_latest_play_hands_its_window_over_when_it_ends`: rewritten in
  the repair, so that an embedded play hands nothing over (it used to hand
  over its player for the settings to refuse; `config::remember_mpv_window`
  still refuses any player but mpv's own window).
- The existing tests rewritten to the record's new shape.

In `tests/player-selection.test.mjs` and `tests/player-window-ui.test.mjs`,
source-text pins of the record's readers and its one writer, brought up to
the new shape.

Red-proofs, each restored from the commit afterwards:

- `publish` sets `last_external` for every play: 4 tests failed, the new
  one with left (fullscreen `Some(true)`, DP-1), right (`Some(false)`,
  HDMI-1).
- `opening_window` falls back to the old rule: the new test failed, left
  (fullscreen `Some(true)`, `None`).
- An embedded publish clears `last_external`: the new test and the
  compatible-choice test failed.
- `publish_started_play` writes only `.latest`, bypassing `publish` (the
  wiring): the new test failed.
- The play path stops using `opening_window`: the Node pins failed.
- Repair, `window_to_remember` back to the latest play: 3 tests failed,
  the new one with "the next run opens as this run would", left `None`,
  right `Some(MpvWindow{fullscreen false, HDMI-1})`.
- Repair, the exit's no-session arm back to the latest play: the new test
  failed with "quitting saves it too", left `None`.
- Repair, the session filter disabled: 3 tests failed, the new one because
  the embedded play's end saved the external window (left
  `Some(..HDMI-1..)`, right `None`).
- The attacking helper, on the first fix: `inherited_screen_name` back to
  "latest if external" failed
  `an_embedded_play_does_not_make_the_setting_apply_again`;
  `compatibility_display`'s external arm back to "latest if external"
  failed `a_compatible_choice_follows_the_screen_the_play_opens_on`.
- The confirming helper, on the merged tree: `publish` sets
  `last_external` for every play, embedded included: 6 tests failed,
  `an_embedded_play_does_not_make_the_setting_apply_again` with left
  (fullscreen `Some(true)`, DP-1), right (`Some(false)`, HDMI-1), the
  reviewer's predicted failure, and
  `an_external_plays_late_end_after_an_embedded_play_saves_its_window`
  with left DP-1, right HDMI-1.

On the merged tree (the confirming helper's trial merge of both branches
onto `main` at `efbf30c`; `main` has since moved by commits that touch
none of these files), the full check was green apart from the two
recorded audit exits (`npm audit` exits 1 on known advisories, `cargo
audit` exits 101 because it is not installed): 641 Rust tests passed (10
ignored), and of 413 Node tests 412 passed and 1 was skipped, an older
skip. Both end-to-end suites were run with a fresh build: external 46/48
and embedded 49/50, the only failures being the recorded ones
(`embeddedownconfig` in the external suite, `pagefail` in both), and `npm
run e2e:verdict` gave "parity verdict: PASS". `playlistplay` and
`windowsetting` passed. The behaviour rests on Rust unit tests; only the
wiring of the play path to `opening_window` and of the record's one writer
rests on source-text checks.

At landing (2026-11-14) the ten commits of this review's fixes were put on
a branch cut from `main` in the helpers' order. Each passed `npm run check`
and `cargo +stable test --locked` on its own before the next was added,
and the full check was green on the last (641 Rust tests, 424 Node tests;
the two audit commands exit as recorded). The product code and the two
player-window test files there were byte for byte the tree the confirming
helper had run end to end.

## Coder dispute (if any)

## Known gaps

- The block of the new test that covers an older external play's end
  after a newer external play was not proved by an injection of its own:
  the session-filter injection failed first on the embedded-end assertion.
  That case is also held by
  `an_older_plays_end_cannot_save_over_a_newer_plays_window`.
- The end-to-end scenario `windowsetting` fails now and then in the
  external player (at `windowsetting.mjs` lines 152, 160 and 183), and
  did so before this work: the helper who attacked the other branch saw
  3 failures in 16 runs at the base `663fe52`. The repairing helper saw 4
  failures in 6 runs at `d753a0f` and 1 in 4 at `cc2fad6`; it found no
  cause in the code and treats the difference as small-sample noise, but
  it is not explained. In its failing runs Vela's own observation of the
  previous mpv had not yet recorded the fullscreen change when the next
  play opened or the save ran. The helper who attacked the other branch
  measured mpv delivering the flag in under 1 ms, so the delay is not in
  mpv's delivery. The merged tree passed the pair 3 times out of 3.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 3 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "An embedded play makes the first-window setting apply again". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 11:00 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d9ad-f7a1-7763-ad98-1b9d2f5a543e`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 10:50 UTC, finished 11:00 UTC.
- Reviewed head `0e5ea592532c476cafe6a1142dc6de68d2f57ef1`, base
  `530e9a544cf0b7f2f3f4837aec093b1d5b3f0492` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. Comments, verbatim:
  - src-tauri/src/commands.rs:3796 — publish is the only writer after default initialization: last_external at 3798 follows successful external plays; latest at 3800 follows every successful play. Accessors at 3804/3808 only read. External-only runs keep identical sessions and shared live observations in both slots, so these two commits preserve their opening, saving and quitting behavior.
  - src-tauri/src/commands.rs:1221 — get_playback_preferences correctly uses latest: Settings reports the most recent play's observed display, including embedded playback.
  - src-tauri/src/commands.rs:4289 — inherited_window_state correctly uses last_external; embedded observations must not replace an external window's state.
  - src-tauri/src/commands.rs:4296 — inherited_screen_name correctly uses last_external for the next external window's screen.
  - src-tauri/src/commands.rs:4320 — window_to_remember correctly uses last_external with an exact session filter. Embedded ends save nothing; an external A's end after embedded B and external C cannot save a window.
  - src-tauri/src/commands.rs:4361 — Exit correctly selects last_external, including while embedded playback remains active. Variables named latest in the exit helpers refer to the whole record. Persistence still respects Remember mode and never saves embedded window observations.
  - src-tauri/src/commands.rs:4458 — opening_window correctly checks last_external before applying first_of_run; intervening embedded plays cannot restart the first-window rule.
  - src-tauri/src/commands.rs:5044 — compatibility_display correctly uses last_external for upcoming external playback; its embedded exact-successor arm at 5045 correctly uses latest, filtered by player and session.
  - src-tauri/src/commands.rs:8346 — The remaining latest reads, here and at 8938/8954, correctly test latest-play publication and failed-launch preservation. The rewritten assertions are meaningful: reverting window_to_remember to latest failed both rewritten tests and the late-end persistence test.
  - src-tauri/src/commands.rs:8330 — Clearing last_external on embedded publish reproduced the predicted fullscreen reset and failed the named guard. Separate mutations proved end persistence, exit persistence, and the previously unproved older-external-end assertion at 8391. After restoring the pinned source, all 76 commands::tests passed.
  - tests/player-selection.test.mjs:423 — Bypassing publish failed the writer guard; bypassing opening_window failed the launch wiring guards. Restored verification passed all 13 Node tests. Both prescribed commands ran; neither had a socket-related failure. Tracked files were restored unchanged.
  - tests/e2e/scenarios/windowsetting.mjs:152 — The recorded intermittent failures still warrant separate investigation; their cause is not established here. They predate this fix, and external-only record behavior remains equivalent. Relevant E2E and embedded paths were read, but neither runtime was executed.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
