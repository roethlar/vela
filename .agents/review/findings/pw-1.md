# pw-1: the copy is chosen for one screen and the window can open on another

**Severity**: MEDIUM — with the Prefer Compatible source policy, the screen a copy is ranked for is read before the stream is resolved, and the screen the window opens on is read again after it. If the previous mpv window changes screen, or a late report of its screen arrives, in between, the play opens on the new screen with a copy chosen for the old one: the wrong resolution or HDR choice for that play.
**Status**: In progress
**Branch**: —
**Commit**: `68d6211`, `7b34af2`

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:4912 samples the display for ranking at :5189, before stream resolution. Window placement samples it again at :5446. If the previous mpv moves screens or delivers a late display event during resolution, these snapshots disagree. A temporary Rust probe reproduced selection of 2160p for DP-1 followed by placement on 1080p HDMI-1."

## Predicted observable failure

Reviewer, verbatim: "A manual replacement opens on the newly observed screen with a copy ranked for the old screen, producing an inappropriate resolution or HDR choice and potentially unnecessary bandwidth use."

Reviewer's suggested direction, verbatim: "Compare the ranking target with the post-resolution display snapshot and reselect/re-resolve when it changes. Keep window inheritance sampling after resolution."

## What

The defect is as the reviewer describes. With the Prefer Compatible source
policy, `playback_compatibility_target` looked at the record's screen
before the stream was resolved, and the launch looked at it again after
resolution (`opening_window`, `inherited_screen_name`). If a late report
of the previous mpv window's screen arrived in between, the window opened
on the new screen with a copy ranked for the old one: for example a 2160p
copy chosen for DP-1 played on a 1080p HDMI-1.

The helper who attacked the first fix found that the screen carried
forward and the screen the ranking ranks for were each picked in code of
their own: `RankedScreen::of` and `display::detect_profile` both took the
first of an observation's display names. Changing one of them to the last
name left every test green, because every test observation named one
screen. A window across two screens, for which mpv reports several
display names, would then open on a screen its copy was not ranked for,
and no test would notice.

## Approach

The screen is carried forward from the ranking. When the ranking is made
from a play's observation, `playback_compatibility_target` also returns a
`RankedScreen`: `Named` (the observed screen), or `VelasScreen` when the
observation names no screen (it was ranked for the screen holding Vela's
window). It is taken once, under the same record lock as the observation
the ranking uses, and travels in `PlaybackSelection.ranked_screen`. After
resolution the launch opens on `screen_to_open_on(ranked, inherited)`.
Fullscreen, maximized and the size are still sampled after resolution, as
the plan requires, so the previous player's last reports still count.
With no ranking from an observation (another policy, or nothing observed
yet), the launch opens on the screen sampled after resolution, as before.

`ObservedDisplay::screen_name()` in `display.rs` (the first display name)
is now the one rule for naming an observation's screen: `detect_profile`
(what the ranking ranks for), `RankedScreen::of` (what is carried),
`inherited_screen_name` and `window_to_remember` (the screen inherited and
remembered) all use it.

This departs from the reviewer's suggestion to rank again after
resolution and re-resolve when the screen changed. Ranking again means
resolving again and undoing what the first resolution started (a
transcode session), which is the riskier code, and no case was found
where carrying the screen forward is wrong.

What a user gets: a play opens on the screen its copy was chosen for,
even when a report of another screen arrives while the stream is
resolved.

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, repaired by a third, confirmed by a fourth on the merged tree, and
read and landed by the coordinating agent, one commit per helper commit.

## Files changed

- `src-tauri/src/commands.rs`
- `src-tauri/src/display.rs` (`screen_name` and its use in
  `detect_profile`, repair commit)
- `tests/player-window-ui.test.mjs`

## Guard proof

In `commands::tests` (`src-tauri/src/commands.rs`),
`a_play_opens_on_the_screen_its_copy_was_ranked_for` runs the reviewer's
probe in the play path's order: ranked for DP-1, a report of HDMI-1 while
the stream is resolved, and the play must open on DP-1 while the window
state is still the late one. It also covers the case with no ranking and
the case of an observation with no screen name, and, since the repair, a
window across DP-1 and HDMI-1, which must carry `Named("DP-1")`.

In `tests/player-window-ui.test.mjs`, "a play opens on the screen its
Compatible copy was ranked for" holds, by source text, that the screen the
ranking looks at is the one the selection carries and the launch opens
on, and since the repair that `RankedScreen::of` and `detect_profile` name
the screen through `screen_name()` and through no direct `.names` access.

Red-proofs, each restored from the commit afterwards:

- The `Named` arm returns the inherited screen: the Rust test failed, left
  `Some(HDMI-1)`, right `Some(DP-1)`.
- The `VelasScreen` arm returns the inherited screen: the Rust test
  failed.
- The launch passes `None` instead of `selection.ranked_screen`; the
  target returns no ranked screen; the ranked selection drops it: each
  failed the Node test.
- Repair, the attacking helper's own injection (`RankedScreen::of` uses
  `names.last()`): the Rust test failed, left `Named("HDMI-1")`, right
  `Named("DP-1")`; the Node test failed with "the screen carried is named
  by screen_name". Before the repair the same injection left every test
  green.
- Repair, `screen_name` returns the last name: the Rust test failed, left
  `Some("HDMI-1")`, right `Some("DP-1")`.
- Repair, `detect_profile` uses `names.last()` directly: the Rust tests
  stayed green (`detect_profile` needs an `AppHandle` and has no unit
  test); the Node test failed with "the ranking names the observed screen
  by screen_name".
- The attacking helper, on the first fix: `screen_to_open_on` prefers an
  inherited name: `a_play_opens_on_the_screen_its_copy_was_ranked_for`
  failed.
- The confirming helper, on the merged tree: the `Named` arm lets a later
  inherited screen win: the Rust test failed with left `Some(HDMI-1)`,
  right `Some(DP-1)`, the reviewer's probe; the launch passes `None`: the
  Node test failed.

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

What rests on source-text checks only: the wiring from the ranking
through `PlaybackSelection.ranked_screen` to the launch, and
`detect_profile`'s use of `screen_name`.

At landing (2026-11-14) the ten commits of this review's fixes were put on
a branch cut from `main` in the helpers' order. Each passed `npm run check`
and `cargo +stable test --locked` on its own before the next was added,
and the full check was green on the last (641 Rust tests, 424 Node tests;
the two audit commands exit as recorded). The product code and the two
player-window test files there were byte for byte the tree the confirming
helper had run end to end.

## Coder dispute (if any)

## Known gaps

- When the ranking is not made from an observation (the first play of a
  run, or nothing observed yet), behaviour is unchanged: the copy is
  ranked for Vela's screen while the window may open on the saved screen.
  The plan's log lists this as left for the owner.
- A play whose ranked observation named no screen opens on Vela's current
  screen even if a name for another screen arrives during resolution. The
  helpers record this as intended.
- `detect_profile`'s use of `screen_name` is guarded by source text only.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 1 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "Revalidate the compatibility display after stream resolution". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 11:18 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d9be-4cb1-7c20-a866-0def9af47313`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 11:08 UTC, finished 11:17 UTC.
- Reviewed head `0e5ea592532c476cafe6a1142dc6de68d2f57ef1`, base
  `530e9a544cf0b7f2f3f4837aec093b1d5b3f0492` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/commands.rs:4938 — VelasScreen does not capture a screen identity. With an empty observation, detect_profile ranks for Vela's monitor before resolution, but launch queries current_screen_name again at line 5648. Moving Vela between differently capable monitors still produces the recorded mismatch. Carry the concrete detected output and test this transition.
  - src-tauri/src/commands.rs:4937 — Named unconditionally overrides the later observation, even if that output disappears during resolution. Unplugging ranked DP-1 while the previous player moves to HDMI-1 leaves launch requesting nonexistent DP-1; mpv falls back without reranking. Placement is also only a request on Wayland. Validate the effective output and reconcile selection when placement cannot honor the ranked target.
  - tests/player-window-ui.test.mjs:176 — The detect_profile guard misses the same multi-screen regression expressed differently: I reversed observed.names inside detect_profile before screen_name() was called. Ranking then names HDMI-1 while RankedScreen carries DP-1, yet both prescribed suites passed. Exercise the profile-selection behavior with multiple screens rather than checking only selected source expressions.
  - src-tauri/src/commands.rs:5076 — The documented first-play gap remains a real compatibility defect: an empty record ranks for Vela's display, carries no ranked screen, and opens on the saved screen. This predates these commits, but leaving it for owner preference does not resolve the incorrect resolution/HDR selection.
  - src-tauri/src/commands.rs:8811 — Guard proof confirmed: restoring inherited-screen precedence failed the Rust guard with HDMI-1 versus DP-1; dropping the carried screen failed the Node guard. After restoration, all 76 Rust and 13 Node tests passed. Tracked files are restored. Embedded playback and E2E were inspected, not run.
