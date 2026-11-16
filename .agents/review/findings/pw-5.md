# pw-5: an older play's window, or an earlier look at the current one, can be saved over a newer one

**Severity**: MEDIUM — whether a play is still the latest is checked, and its window is read, before the settings lock is taken; the write then uses what was read. A play whose end is slow can write after a newer play has saved, and the exit handler can write an earlier look at a window over the later one the play's own end has just saved. The next run then opens with the older fullscreen, screen or size.
**Status**: Verified
**Branch**: —
**Commit**: `cbd93ae`

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5740-5750 checks the latest session before acquiring config's write locks, then writes the captured tuple without rechecking. An interleaving probe captured play A, published and saved play B, then resumed A's update: A overwrote B. The exit path at commands.rs:4322-4338 likewise overwrote a newer final observation saved by that play's end callback."

## Predicted observable failure

Reviewer, verbatim: "Remember mode can persist an older play's window or an earlier snapshot of the current play. The next run consequently opens with stale fullscreen, screen or size values."

Reviewer's suggested direction, verbatim: "Check the session identity and sample its observation inside the serialized config mutation. Coordinate end and exit persistence so an earlier snapshot cannot replace a later one, without holding the observation lock across filesystem work."

## What

The defect is as the reviewer describes. Both the end of a play and the
exit hand-over checked "is this still the latest play" and took a copy of
the window before `config::update` took the settings lock, and then wrote
what they had copied. A play whose end waited for the lock could write
its window after a newer play had saved its own, and quitting Vela could
write an earlier look at a window over the later one the play's own end
had just saved. The next run then opened with the older fullscreen,
screen or size.

The helper who attacked the fix found nothing more to repair here.

## Approach

The decision is now made inside the settings write. The new
`remember_window_in(cfg, record, session)` runs inside the closure of
`config::update`: it takes the record lock (and the observation locks
inside `snapshot`) only to look, lets go of them, and then calls
`config::remember_mpv_window`. The new `write_play_end(update, record,
session_id, finish)` carries the recents stamp and the window in one
settings change, and the end callback calls it with `|change|
config::update(change)`. `remember_window_on_exit_with(record, load,
update)` keeps a first read only to skip a write that would change
nothing, then decides again inside the write. `SettingsChange<'a>` (a
boxed closure) lets the tests run the change against settings held in
memory and step in between the look and the write. A write that waited
for the lock therefore looks again and writes what is true then, or
nothing. Since the repair of pw-6 (`d753a0f`), "still the latest" reads
"still the last external play"; the decision is still made inside the
write.

Lock order: the settings lock (`CONFIG_LOCK`, then `config.lock`), then
the window record, then the observation. Nothing that holds the record or
an observation takes the settings lock: `publish_started_play` lets go of
the record lock before `on_success`'s `config::update`; `opening_window`,
the compatibility ranking and `get_playback_preferences` take the record
and observation locks and never the settings lock; the mpv reader
(`apply_ipc_event_at` in `playback.rs`) takes only the observation locks.
No lock is held across the file write.

What a user gets: the window saved for the next run is the newest one,
never an older play's or an earlier look at the same play.

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, confirmed by another on the merged tree, and read and landed by
the coordinating agent, one commit per helper commit. The attack found
nothing to repair here: the attacking helper checked the lock order at
every place that takes these locks and found no inversion, and its own
injections failed the tests (below).

## Files changed

- `src-tauri/src/commands.rs`
- `tests/player-selection.test.mjs`
- `tests/player-window-ui.test.mjs`

## Guard proof

In `commands::tests` (`src-tauri/src/commands.rs`), three tests drive the
interleavings deterministically, with settings held in memory and a hook
run inside the update before the change is applied:

- `an_older_plays_end_cannot_save_over_a_newer_plays_window`: an older
  play's end waits while a newer play starts and saves; the newer window
  stays, and the older play's recents stamp is still written.
- `an_exit_that_waited_cannot_save_an_earlier_look_over_the_plays_end`:
  an exit waits while the play's end saves a later report.
- `a_plays_end_that_waited_cannot_save_an_earlier_look_over_an_exit`: a
  play's end waits while an exit saves a later report.
- `quitting_writes_the_window_only_when_the_stored_one_changes`: the
  older exit test, rewritten to the new update signature, with the same
  assertions plus the stored window's value.

In `tests/player-selection.test.mjs` and `tests/player-window-ui.test.mjs`,
source-text pins that the end callback goes through `write_play_end` and
the exit through `remember_window_in`.

Red-proofs, each restored from the commit afterwards:

- `write_play_end` takes its copy before the update: two tests failed
  (left (fullscreen true, DP-1), right (false, HDMI-1); left `Some(true)`,
  right `Some(false)`).
- The exit takes its copy before the update: the exit-waited test failed.
- The end callback goes back to "copy, then `config::update`" (the
  wiring): the Node pin failed.
- The attacking helper: the exit writes what it decided at its first read
  failed `an_exit_that_waited_cannot_save_an_earlier_look_over_the_plays_end`
  and a Node pin; the exit writes even when nothing changed failed
  `quitting_writes_the_window_only_when_the_stored_one_changes` and its
  Node pin.
- The confirming helper, on the merged tree: `write_play_end` decides its
  window before the update: `an_older_plays_end_cannot_save_over_a_newer_plays_window`
  failed with left (fullscreen true, DP-1), right (false, HDMI-1), the
  reviewer's interleaving, and
  `a_plays_end_that_waited_cannot_save_an_earlier_look_over_an_exit`
  failed with left `Some(true)`, right `Some(false)`; the Node pin of
  `write_play_end` failed too.

All three interleaving tests still pass after the repair commits of the
other findings.

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

What rests on source-text checks only: that the real end callback and the
real exit go through `write_play_end` and `remember_window_in`. The lock
order rests on the helpers' reading of the code; the interleaving tests
use settings held in memory, not the real settings lock.

At landing (2026-11-14) the ten commits of this review's fixes were put on
a branch cut from `main` in the helpers' order. Each passed `npm run check`
and `cargo +stable test --locked` on its own before the next was added,
and the full check was green on the last (641 Rust tests, 424 Node tests;
the two audit commands exit as recorded). The product code and the two
player-window test files there were byte for byte the tree the confirming
helper had run end to end.

## Coder dispute (if any)

## Known gaps

- The exit's first read is not authoritative: in a race it can lead to
  writing an unchanged file again, which is harmless.
- The end-to-end scenario `windowsetting` fails now and then in the
  external player (at `windowsetting.mjs` lines 152, 160 and 183), and did
  so before this work (3 failures in 16 runs at the base `663fe52`). All
  its failures have one shape: a fullscreen value is missing from what
  the end of the play or Vela's exit saves. The helper who attacked the
  other branch suggested looking into it next to this finding; the
  repairing helper traced its failing runs to Vela's own observation of
  the previous mpv lagging, not to the record logic. The merged tree
  passed it 3 times out of 3.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 3 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "Captured window snapshots can overwrite newer saved state". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 11:25 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d9c7-4c0a-79a1-9340-bd3b9d78401c`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 11:17 UTC, finished 11:23 UTC.
- Reviewed head `0e5ea592532c476cafe6a1142dc6de68d2f57ef1`, base
  `530e9a544cf0b7f2f3f4837aec093b1d5b3f0492` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. Comments, verbatim:
  - src-tauri/src/commands.rs:8650 — Both snapshot regressions reproduced the expected failures across all three interleaving guards. Reverting callback wiring also failed both Node pins. After restoration, all 76 Rust and 13 Node tests passed.
  - tests/e2e/scenarios/windowsetting.mjs:115 — Embedded playback and E2E were reviewed without execution. Separate-client fullscreen readback does not synchronize Vela's observer; the recorded flakes do not establish a remaining pw-5 write-ordering defect.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
