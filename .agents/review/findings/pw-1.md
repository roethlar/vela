# pw-1: the copy is chosen for one screen and the window can open on another

**Severity**: MEDIUM — with the Prefer Compatible source policy, the screen a copy is ranked for is read before the stream is resolved, and the screen the window opens on is read again after it. If the previous mpv window changes screen, or a late report of its screen arrives, in between, the play opens on the new screen with a copy chosen for the old one: the wrong resolution or HDR choice for that play.
**Status**: Verified
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

Orchestrator: a valid verdict, reopened on four comments (the fifth
confirms the guard proof). All four are right. Comments 1, 3 and 4 are
repaired in full. Comment 2 is repaired where that does not restart a
video, and its other half is disputed below. It is the first reopen of
this finding. `pw-2` was reopened on the same subject, and one repair
answers both.

### Repair (round 2): `52138d0`, `8526d28`, `eb2f1c2` (with `pw-2`'s `2eb5626`, `fec6128`, `f9acc1f`)

One rule for `pw-1` and `pw-2` together, built as one piece of work and
landed as six commits, each for one of the two findings.

- **The screen is decided once, by name, before the copy is chosen**, for
  every source policy: `select_playback_version` calls `decide_play_screen`
  once, after the player is chosen and the probes have answered, and that
  calls `screen_for_play` (`src-tauri/src/commands.rs`), the one place the
  screen is decided. For a play in mpv's own window: the screen the last
  external play's window was observed on; while the run has had none, the
  screen the player window setting saved for the first window. For a play
  in the embedded player: the screen holding Vela's window (the exact
  successor of an embedded play keeps that play's observed screen).
- **A screen that is not connected is passed over**, ending at the screen
  holding Vela's window, by name. What is connected is looked at once per
  play (`display::look_at_screens`, on a blocking thread): the monitors
  Tauri lists, each under Tauri's name and mpv's, and on Wayland the
  compositor's own outputs by connector and model. When the outputs cannot
  be listed, or the list names none, a name is taken unchecked, as before
  the check existed.
- **A Compatible copy is ranked for that output's profile**:
  `display::profile_basis` (pure: it is told the output and never picks
  one) and `profile_of` (the HDR state, asked for that output). The
  observation's size counts when the name came from that observation;
  otherwise the output's own.
- **The launch asks for that screen and looks at no screen again**:
  `PlaybackSelection.screen` carries the name, and `play_by_key_locked`
  does `let screen_name = selection.screen.clone();`. A play the embedded
  player cannot start after all is described again for mpv's own window
  from the same play, so its window asks for the same screen.
- **On Wayland the screen holding Vela's window is named by its
  connector** ("DP-3"), not its model, when the compositor's list tells
  which output it is (`display::connector_of`): two screens of one model
  share the model's name.

`RankedScreen`, `screen_to_open_on`, `compatibility_display`,
`playback_compatibility_target`, `inherited_screen_name` and
`display::current_screen_name` are gone; `opening_window` gives the window
state only.

By the reviewer's comment:

1. *Vela's screen has no identity; moving Vela between the ranking and the
   launch.* Closed: the screen holding Vela's window is a name decided
   once, and the launch does not look again. Guard:
   `commands::tests::the_screen_holding_velas_window_is_carried_by_name`
   (ranked 1080p SDR for HDMI-1, Vela's window then moves to DP-1, the
   destination stays HDMI-1), and in `tests/player-window-ui.test.mjs`
   "a play is shown on the one screen decided before its copy is chosen".
   Red-proofs: the name dropped; a second look put into the launch in
   three forms.
2. *A ranked output that disappears; placement is only a request.* The
   output is checked as connected when it is decided:
   `a_screen_that_is_not_connected_is_passed_over`,
   `display::tests::a_name_is_connected_when_an_output_has_it`,
   `what_was_seen_of_the_screens_is_put_together`, and end to end
   `windowsetting`, which now requires that the stored `no-such-screen` is
   NOT asked for and that a screen is asked for by name. Red-proofs: the
   connected check made always true; every name accepted; an unreadable
   list rejecting every name; the compositor's list dropped; Vela's screen
   given no name, end to end on a fresh build. The rest is the
   disagreement below.
3. *The guard missed the reversed names.* The profile is now chosen by a
   pure function that can be run:
   `display::tests::the_profile_is_the_named_outputs_among_several` (two
   and three outputs, either order, an observation naming the output
   second), `an_output_the_list_lacks_gets_the_values_of_velas_screen`,
   `commands::tests::an_observation_across_several_screens_is_shown_on_its_first`,
   `display::tests::two_screens_of_one_model_are_told_apart`. The
   reviewer's own mutation (the names reversed) now fails, where the name
   is taken and inside `profile_basis`.
4. *The first play of a run.* Closed:
   `the_first_play_of_a_run_is_ranked_for_the_saved_screen_it_opens_on`
   (saved DP-1 connected: 4K HDR on DP-1; unplugged: 1080p SDR on HDMI-1;
   in full screen mode the saved screen is not used).

Files: `src-tauri/src/commands.rs`, `src-tauri/src/display.rs`,
`src-tauri/src/playback.rs` (a doc comment, and the test stand-ins handed
to the command layer's tests), `tests/player-window-ui.test.mjs`,
`tests/player-selection.test.mjs`,
`tests/e2e/scenarios/windowsetting.mjs`.

**How it was made and checked.** Three helper agents on Opus 5.5 in turn:
an author; a second that attacked the result (thirteen mutations of the
deciding code, one of which survived, and four more findings); a third
that fixed those, each with a guard and a red-proof. Their red-proofs are
per comment below. The coordinating agent then replayed the six commits
onto a `main` that had moved on (two conflicts with the first half of
`pw-9` and one in the Node pins, and that half's test adapted to the new
rule), read the product diff, and on the merged content ran:

- the full check: green (661 Rust tests, 493 Node tests; the two audit
  commands exit as recorded);
- end to end, external player: `mergedview`, `mpvargs`, `multiplex`,
  `exitplay`, `playback`, `playerfallback`, `playlistplay`, `replaceplay`,
  `sourcepolicy`, `windowsetting`, 10 of 10; embedded player:
  `embeddedplay`, `playlistplay`, `embeddedfullscreenend`,
  `embeddedcontrols`, 4 of 4;
- eight red-proofs of its own on the merged content, each restored from
  the committed state and each failing the test named: a name that is not
  connected taken anyway; the saved first screen not used; any replaced
  play lending an embedded play its screen; Vela's screen looked up by
  name among the outputs; the connector never taken; the real ranking
  ranking Best whatever the policy; a play of this run handing on nothing
  (the adapted `pw-9` test); an observation that names another screen
  lending its size;
- the compositor's list on a real desktop (the development machine, one
  screen): one output, connector `DP-3`, model `XG27UQDMS`, 3840x2160, and
  the model's name is turned into `DP-3`.

The venue has one screen. A second screen, an unplugged one and two of
one model rest on unit tests.

**What changes for a user**, also one who changed no setting:

- A saved or observed screen that is no longer connected is not asked for
  any more; the video goes to the screen holding Vela's window. Before,
  mpv was asked for the gone screen and chose one itself. The plan's log
  had this under "Left as they are", and its entry of 2026-11-14 now says
  so.
- The first video of a run, under Prefer Compatible, is ranked for the
  saved screen it opens on. Also "Left as they are" before.
- The screen is fixed before the stream is resolved, for every policy. A
  report of another screen that arrives from the play before while the
  stream is being resolved no longer moves the next window. How the window
  opens (full screen, maximized, size) is still read after resolution.
- An observation that names no screen no longer lends its size to the
  ranking; the size of the screen holding Vela's window is used.
- On Wayland a video sent to the screen holding Vela's window asks mpv for
  the connector's name where it asked for the model's name. With one
  screen it opens on that screen either way.
- A player window setting that cannot be read fails the choice of copy
  (and the title menu's "which version would play") where it failed the
  launch before. The play does not start either way.
- Each play, and each "which version would play" query, now looks at the
  screens once: one question to Vela's window on a blocking thread, and on
  Wayland one round trip to the compositor, with no limit of its own, as
  the HDR query that was already made there for a Compatible choice.

**Not built, and put to the reviewer as a disagreement** (comment 2's
second half): nothing ranks or resolves again once resolution has started,
and nothing checks after the window has opened that the compositor put it
where it was asked to, to relaunch if not. An output unplugged in the
second or so that resolution takes is still asked for, and mpv places the
window where it can; on Wayland the screen is a request. Either would end
a transcode the server has just started and restart a video the user has
just started, on evidence (a report from mpv about its window) that
arrives after the video is already showing. The cheap half is done: the
output is checked as connected when it is decided. If the reviewer holds
that the rest must be built, it is the owner's to decide, because it
changes what a user sees.

Known gaps, besides the disagreement:

- macOS and Windows were not compiled. The new code there is Tauri's own
  calls and pure functions. Assumption, not checked: on Windows the names
  mpv reports for a screen are Tauri's names for it; if they differ, an
  observed screen counts as not connected and the video goes to the screen
  holding Vela's window (the same screen when there is one).
- On X11 the connected check relies on GDK naming a monitor as mpv does
  (the RandR name). The venue (Xvfb) showed the check working.
- A Wayland compositor that does not name its outputs (before `wl_output`
  version 4) lists models only. Assumption from mpv's source as the
  coordinating agent remembers it, not checked here: mpv then reports the
  model too, so what was saved or observed is still in the list.
- Two screens of one model AND one size cannot be told apart: the name
  stays the model's; the size ranked for is still that of Vela's own
  monitor.
- The Settings display (`detect_profile`) still asks for Vela's monitor on
  an async worker, as before this work. It is not a play.
- The link from the choice of copy to the launch is held by source text
  (`select_playback_version` needs a running app); the late fallback's
  test builds the choice of copy by hand and runs the real
  `playback::play`.

### Verification round 2 (repair-delta redispatch), recorded 2026-11-14 14:18 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1da5d-a2e6-7560-87ca-2c8d8bebb381`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 14:02 UTC, finished 14:18 UTC.
- Reviewed head `eb2f1c274b240a5dc9abc26e6c7b90657af9e74c`, base
  `9a28219dbabd0a92b941eb34497e62f0c2f0f0b2` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/display.rs:212 — Any named output makes the list authoritative, even when connector aliases could not be read. On Wayland, platform_outputs converts errors to an empty list while Tauri still supplies model names. A connected, remembered DP-1 is then rejected and playback moves to Vela's screen; subsequent observations can replace the remembered destination. A temporary probe reproduced this. Preserve unknown connectivity when alias enumeration is incomplete.
  - src-tauri/src/display.rs:368 — The new connectivity check exposes a macOS translation failure. Locked tao 0.35.3 supplies names such as 'Monitor #123' and scales CoreGraphics dimensions/positions; mac_screen_matches compares localized names and unscaled CoreGraphics values. On HiDPI monitors the translation can therefore return None, leaving remembered mpv names absent from the supposedly complete connected list. Correct the translation before rejecting those names. This conclusion is from source inspection and a missing-alias probe; macOS was not run.
  - src-tauri/src/display.rs:419 — connector_of compares Tauri/GDK geometry against raw Wayland mode dimensions, which need not share units under fractional scaling. With same-model outputs, this can identify the other connector rather than remain ambiguous. A temporary probe representing 3840x2160 at 150% as Tauri's 5120x2880 selected the other, native 5120x2880 output. Use a reliable native identity or normalize geometry before disambiguating.
  - src-tauri/src/display.rs:982 — The newly ranked output dimensions ignore wl_output's transform. For a saved portrait output other than Vela's monitor, profile_basis takes the platform entry first and ranks for its unrotated mode. A temporary probe returned 1920x1080 for a 1080x1920 destination. Account for rotation before using these dimensions for compatibility selection.
  - src-tauri/src/commands.rs:5348 — Freezing the screen for Best, Fastest and Ask introduces a window-continuity regression: a screen report arriving during resolution is now ignored even though these policies have no display-dependent ranking to preserve. Slice 1 explicitly requires sampling after resolution. Retain that behavior for these policies, or obtain an explicit owner decision changing the promise.
  - tests/player-window-ui.test.mjs:218 — The production ranking connection remains insufficiently guarded. I inserted a shadow PlayScreen in compatibility_target using screens.velas while leaving the selected launch destination untouched. This restores ranking-for-Vela/launching-on-another-screen, yet all 67 prescribed Rust tests and 20 Node tests passed. Exercise the actual target-construction path with injectable display data and require this mutation to fail.
  - src-tauri/src/commands.rs:5693 — The carried-name approach repairs the ordinary moving-Vela and saved-screen cases, and reversing observation names now fails the behavioral guard. I do not require post-resolution reranking or automatic post-open relaunch to close pw-1; the documented hot-unplug/compositor limitation is an acceptable boundary. The pre-resolution identity and connectivity defects above still require repair.
  - src-tauri/src/commands.rs:9172 — Guard proof confirmed: dropping Vela's name, disabling connectivity filtering, reversing observation names, ignoring the saved screen, and dropping the launch screen each failed the corresponding guard. After restoration, all 67 Rust and 20 Node tests passed. Tracked files are restored; embedded playback and E2E were inspected, not run.

Orchestrator: a valid verdict, reopened on six comments, and all six are
right; the seventh ACCEPTS that nothing ranks, resolves or relaunches
again after resolution has started, so that disagreement is settled. It is
the second reopen. `pw-2` was reopened on the same code, on one guard, and
one repair answers both.

### Repair (round 3): `c9e7245` to `79384d9` (fourteen commits, `pw-2`'s among them)

By the reviewer's comment:

1. and 2. *A connected screen judged not connected when the list of names
   is incomplete (Wayland when the compositor's list cannot be read; macOS
   when a monitor's mpv name cannot be worked out).* What is connected is
   now known only from a list that is COMPLETE in the names mpv may use:
   Tauri's monitors listed, every one with its mpv name, and in a Wayland
   session the compositor's list read with a connector name for every
   output. Otherwise nothing is known and every name is taken as it is, as
   before the check existed (`c9e7245`; `PlatformOutputs`, `screens_from`).
   The macOS translation itself is not changed: nothing here can run
   macOS. An incomplete list still gives a decided screen its own size for
   the ranking (`3d19f97`, found when the repair was attacked).
3. *Two screens of one model told apart by size, wrongly under fractional
   scaling.* The connector is taken only when exactly one output carries
   the model's name; otherwise the name stays the model's, as before round
   2 (`a1f48ab`).
4. *A rotated output ranked for its unrotated size.* The compositor's
   outputs are given as they are shown, turned by their transform
   (`5708b4d`).
5. *The screen frozen before resolution for every policy, against the
   plan's Slice 1.* Only a Prefer Compatible choice fixes the screen
   before the stream is resolved, because only it ranks for a screen. For
   every other policy the screen is decided in the launch, after
   resolution, by the same function, as for a play in mpv's own window; a
   play the embedded player cannot start then opens where the last mpv
   window was, as before round 2. The two cases are two values of one
   type (`PlayDestination`), and the choice of copy looks at no screen
   unless the policy is Compatible (`6cb2af8`).
6. *The ranking's link to the decided screen not held by a test.* The
   target of a Compatible choice is built by one function a test runs with
   made-up display data (`compatibility_target_from`, `6d95bc0`); the
   reviewer's mutation fails four tests.

Also `f0aefb7`: an empty `WAYLAND_DISPLAY` is not a Wayland display for
the list of outputs. The end-to-end venue runs X11 with that variable
empty and the session type inherited from the desktop, and rule 1 would
otherwise have made everything unknown there (`windowsetting` failed until
this commit).

Files: `src-tauri/src/display.rs`, `src-tauri/src/commands.rs`,
`tests/player-window-ui.test.mjs`, `tests/player-selection.test.mjs`,
`tests/e2e/scenarios/windowsetting.mjs`.

**How it was made and checked.** Three helper agents on Opus 5.5: an
author, an attacker (one defect and four mutations that survived), a
repairer who fixed all five, each with a test it red-proved. On the final
content the helpers ran the end-to-end sets: external 10 of 10
(`playback`, `playlistplay`, `sourcepolicy`, `mergedview`, `multiplex`,
`windowsetting`, `playerfallback`, `replaceplay`, `exitplay`, `mpvargs`),
embedded 3 of 3 (`embeddedplay`, `playlistplay`, `embeddedfullscreenend`).
The coordinating agent read the reports, ran the full check on the
content that landed (green: 674 Rust tests, 533 Node tests; the two audit
commands exit as recorded) and landed the commits. It made no red-proofs
of its own this time: the owner set work on tests aside that day
(`.agents/decisions.md`, 2026-11-14).

**What changes for a user, against `main` before this repair:**

- Under Best, Fastest and Ask the screen is again read after the stream is
  resolved, so a late report of where the last mpv window was moves the
  next window, as the plan says. The title menu's "which version would
  play" looks at no screen under those policies.
- A remembered screen is no longer dropped when Vela cannot tell what is
  connected; it is asked for, and mpv decides, as before round 2.
- With two screens of one model, Vela's screen is asked for by its model
  again, so the window may open on the other one, as before round 2.
- A rotated screen is ranked as it is shown.

Known gaps: macOS and Windows were not compiled or run; no real desktop
with two screens, a rotated screen or fractional scaling was at hand; on
macOS with HiDPI screens the connected check may never apply; the HDR
query's fallback by size still uses the unrotated mode (it predates this
work); for a play decided at the launch the window's state and its screen
are read a moment apart; each launch on Wayland makes one round trip to
the compositor with no limit of its own.

### Verification round 3 (repair-delta redispatch), recorded 2026-11-14 19:05 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1db6c-2777-7be2-ba3b-463680259c11`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 18:57 UTC, finished 19:04 UTC.
- Reviewed head `79384d9a6d7a5567aacc59b9f0281d098272d9e3`, base
  `719db900d361bf7fea36e0e60e8d393f47c78727` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.

### Mutation check of round 3, 2026-11-16 (the release's test pass)

Round 3 (`c9e7245` to `79384d9`) landed on 2026-11-14 without red-proofs of
its own: the owner had set work on tests aside that day. With a release in
view they were made. A helper agent (Opus 5.5, under the multi-agent workflow the owner switched on
for this session, in a git worktree of its own) injected regressions into
the product code one at a time, restored the file from git after each, and
ran the guards against each.
The guards: 52 Rust tests (`cargo +stable test --locked --lib`, the sixteen
named guards and the modules around them) and the Node test "a Compatible
play is shown on the screen its copy was ranked for, any other on the one
its launch decides".

Result: every regression made its guard fail on the intended assertion
(the helper counts 28 injections; its table lists 26, each "fails for the
right reason"). No guard of round 3 is vacuous. `pw-2`'s part of round 3 is
covered by the same run.

Held only by a pin on the source text (`tests/player-window-ui.test.mjs`),
with the Rust tests green under the mutation: the transform read from the
Geometry event reaching the list; the empty-`WAYLAND_DISPLAY` check being
wired into `platform_outputs`; `platform_outputs` treating an unreadable
compositor list as incomplete; the launch deciding the screen for mpv's own
window; the session not being shadowed; the launch playing `resolved.url`.
Each pin fails on its mutation, but a rewrite with the same wrong effect in
other words would pass. This is the gap already recorded above: the play
path needs a running app. Not claimed as guarded and not tested: the HDR
query's fallback by size still matches the unrotated mode, and the macOS
name translation (Known gaps).
