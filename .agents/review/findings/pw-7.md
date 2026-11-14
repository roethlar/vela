# pw-7: when saving the Player window setting fails, the control goes on showing the choice that was not saved

**Severity**: LOW — the control is bound to the value being saved, and a failed save only shows the error. The select keeps showing a choice the backend does not hold, and playback follows the old one until a save succeeds.
**Status**: In progress
**Branch**: —
**Commit**: `ac25240`, `0ee2e0d` (repair)

## Evidence

Reviewer, verbatim: "src/lib/Settings.svelte:664 binds the selection directly to playerWindowMode, while the rejection handler at lines 428-431 only sets err. Executing the save function with a rejected setter left 'fullscreen' displayed although the backend still held 'remember'."

## Predicted observable failure

Reviewer, verbatim: "After a failed save, the control continues showing a mode the backend does not hold. Playback follows the previous mode until a successful save; an error is shown, so this is a UI consistency defect rather than data corruption."

Reviewer's suggested direction, verbatim: "Restore or reload the last confirmed mode after rejection. Alternatively, explicitly distinguish the unsaved selection from the active backend value."

## What

The defect is as the reviewer describes. In `src/lib/Settings.svelte` the
Player window select (`select#player-window`) was bound to
`playerWindowMode`, the value being saved, and the catch of
`savePlayerWindowMode` only set `err`. When the backend refused
`set_mpv_window_mode`, the select went on showing the choice that was not
saved, with the error, while the backend held the old mode and playback
followed it.

The helper who attacked the fix found nothing against it.

## Approach

A plain (not reactive) `playerWindowSaved` holds the mode the backend
last said it holds. `load()` sets it with the mode it reads
(`playerWindowMode = playerWindowSaved = windowMode`), and a save that
succeeds sets it with the mode the backend returns. The catch puts
`playerWindowMode` back to it before it shows the error, as before. If
loading itself failed, the value stays `"remember"`, which is the
backend's default for a missing field and what the control showed before.
A load that has been overtaken (`seq !== loadSeq`) changes nothing.

What a user gets: after a failed save the select shows the mode that is
really saved, together with the error.

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, confirmed by another on the merged tree, and read and landed by
the coordinating agent, one commit per helper commit. The attack found
nothing to repair here: the attacking helper checked the overtaken load
and the failed load by reading the code, and its own injections failed
the test (below).

## Files changed

- `src/lib/Settings.svelte`
- `tests/player-window-ui.test.mjs` (new test)

## Guard proof

In `tests/player-window-ui.test.mjs`, "a failed save of the player window
setting puts the control back to the saved mode" takes the component's own
`load()` and `savePlayerWindowMode()` and the `playerWindow*` declarations
out of `Settings.svelte`, strips their types with the repository's
TypeScript compiler (`ts.transpileModule`), and runs them against a fake
backend whose `set_mpv_window_mode` can refuse. The mode comes from
loading and a refused save goes back to it; a save succeeds and a later
refused one goes back to that. The test also checks, by source text, that
the select binds `playerWindowMode` and that its button calls
`savePlayerWindowMode`. The test returns early, and passes, when
`Settings.svelte` has no `id="player-window"` (the control withheld). The
confirming helper checked that this does not fire: `Settings.svelte:671`
has the select.

Red-proofs, each restored from the commit afterwards:

- The revert removed (the original defect): failed with "a refused save
  must show what the backend holds", actual `'fullscreen'`, expected
  `'default'`.
- `load()` does not record the mode: failed, actual `'remember'`, expected
  `'default'`.
- A successful save does not record the mode: failed, actual `'default'`,
  expected `'fullscreen'`.
- The select bound to another variable: failed the `bind:value` check.
- The button calls another function: failed the `onclick` check.
- The attacking helper, with injections of its own: a revert to the
  constant `"remember"` failed (actual `'remember'`, expected
  `'default'`); recording the attempted mode as saved before the backend
  answers failed (actual `'fullscreen'`); not recording a successful save
  failed (actual `'default'`, expected `'fullscreen'`); dropping the error
  failed "and say why".
- The confirming helper, on the merged tree: the catch's revert removed:
  failed with "a refused save must show what the backend holds", actual
  `'fullscreen'`, expected `'default'`.

On the merged tree (the confirming helper's trial merge of both branches
onto `main` at `efbf30c`; `main` has since moved by commits that touch
none of these files), the full check was green apart from the two
recorded audit exits (`npm audit` exits 1 on known advisories, `cargo
audit` exits 101 because it is not installed): 641 Rust tests passed (10
ignored), and of 413 Node tests 412 passed and 1 was skipped, an older
skip. Both end-to-end suites were run with a fresh build: external 46/48
and embedded 49/50, the only failures being the recorded ones
(`embeddedownconfig` in the external suite, `pagefail` in both), and `npm
run e2e:verdict` gave "parity verdict: PASS". The merge had one conflict,
in `tests/player-window-ui.test.mjs`, where this test and pw-1's new test
were added at the same place; both were kept unchanged.

What rests on source-text checks only: that the rendered select shows
`playerWindowMode` and that its button calls `savePlayerWindowMode`. The
behaviour test runs the functions taken out of the file, not the mounted
Svelte component.

At landing (2026-11-14) the ten commits of this review's fixes were put on
a branch cut from `main` in the helpers' order. Each passed `npm run check`
and `cargo +stable test --locked` on its own before the next was added,
and the full check was green on the last (641 Rust tests, 424 Node tests;
the two audit commands exit as recorded). The product code and the two
player-window test files there were byte for byte the tree the confirming
helper had run end to end.

## Coder dispute (if any)

## Known gaps

- The reactive rendering itself is covered only by the `bind:value` and
  `onclick` source-text checks; the test does not mount the component.
- Neighbouring controls in Settings have the same slip and were not
  changed (outside this review): `saveContinuePlaying` (the
  `continuePlaying` select keeps a refused choice, exactly the same slip);
  `savePlaybackPreferences` (the `playbackSourcePolicy`, resolution and
  HDR selects); `saveMpvAdvanced` (extra arguments, own configuration,
  autocrop, the skip policies and playback quality); `saveMpvPath` and
  `clearMpvPath` (the path field keeps a typed or cleared path after a
  refused `set_mpv_path`), and `browseMpv` through `saveMpvPath`.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 3 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "A failed setting save leaves the unsaved mode selected". Reviewer's severity: LOW.

### Verification round 1, recorded 2026-11-14 11:56 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d9dc-b7e7-73f3-a0dc-29731c6e3940`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 11:41 UTC, finished 11:55 UTC.
- Reviewed head `ac2524072c877789edf96dbdedf8dbeb2affcc27`, base
  `2dd9eb70661d0771fa2439b3ec5b68e1bc898bf8` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src/lib/Settings.svelte:282 — A pending load can overwrite a newer successful save: load reads remember, waits for playback preferences, fullscreen saves successfully, then load restores remember into both variables. A refused retry of fullscreen now restores remember although the backend holds fullscreen. Reproduced against both pins: base displays fullscreen after that retry; head displays remember. Prevent stale loads from overwriting newer confirmed saves and guard this interleaving.
  - tests/player-window-ui.test.mjs:203 — The guard bites at this head, but demonstrated two false passes with rollback removed: single-quoting the select ID triggers the early return; retaining the previous saver in a comment before its broken replacement makes functionSource execute the commented copy. Both variants compile and retain the real control. Parse the actual element and function declarations. The pinned source was restored, and all five tests passed.

Orchestrator: a valid verdict, reopened on two comments, and both are
right.

### Repair (round 2), `0ee2e0d`

**A load that is slower than a save.** Settings loads all its values in
one go and applies them when the last has arrived. A save of the Player
window mode confirmed in between was then overwritten, in the control and
in the mode kept as "last confirmed", by the older value the load had read;
a later refused save put the control back to that older value although
Vela held the newer one. Now the component counts the saves of the mode
that Vela confirmed (`playerWindowSaves`). `load()` notes the count before
it asks, and applies the mode it read only if no save was confirmed since.
A refused save changes nothing in Vela and does not count.

How each order of events ends, the control and the confirmed mode on what
Vela holds in every one:

- A load pending while a save succeeds (the reviewer's case): the load
  leaves the mode alone; the control keeps the saved mode; a later refused
  save goes back to it.
- A load pending while a save is refused: nothing was confirmed, so the
  load applies what it read, which is what Vela holds.
- A save pending while a load completes: the control briefly shows what
  the load read, which is what Vela holds at that moment; the save's
  answer then sets it (or, refused, puts it back to that).
- A save started before a load and confirmed after it: whichever finishes
  second leaves the confirmed mode.
- Two loads: the older is dropped as before (`loadSeq`).

**The test.** It no longer searches the component's text. The select and
the button are found through the Svelte compiler's parser: exactly one
`select` whose id is `player-window`, however quoted, bound to
`playerWindowMode`; exactly one button in its section, which calls
`savePlayerWindowMode` and is disabled while a save is under way. `load`
and `savePlayerWindowMode` are found as function declarations through the
TypeScript compiler's syntax tree, exactly one of each, so a copy left in
a comment is never what runs. The control is required: the early return is
gone, here and in the file's first test, which had the same one. A new
test, "a load that read the mode before a save was confirmed does not put
the older mode back", drives the reviewer's order of events and its
mirrors against a made-up backend that holds answers back and lets them go
one at a time.

Files: `src/lib/Settings.svelte`, `tests/player-window-ui.test.mjs`.

How it was made: one helper agent (Claude Opus 5.5) on a local branch; the
coordinating agent read the change, made the first test of the file
require the control as well, and landed it as one commit. No second helper
attacked it: the change to the component is four lines.

Red-proofs, each restored from the commit: the order check taken out, and
the count not advanced on a confirmed save ("a load must not undo a save
confirmed after it read": `remember` where `fullscreen` was expected); a
refused save counted too; the rollback taken out; the rollback taken out
with the select's id single-quoted, and with the old save function kept in
a comment before a broken one (the reviewer's two false passes: both now
fail, and both were seen to pass against the old test); the select bound
to another variable; the button calling another save; the button never
disabled; the control's id changed (the file's first test failed). On the
code as it was, the new test fails as the reviewer described.

Checks: `npm run check` (474 tests, no svelte-check error) and `npm run
build` green before the commit.

Not covered:

- The other values Settings loads in the same go (Continue Playing, the
  mpv path and advanced options, the playback preferences) can be
  overwritten by a slow load in the same way, and their controls still
  keep a refused choice. They are outside this finding.
- The select itself stays usable while a save is under way; only the
  button is disabled.
- The test runs the component's functions and reads its parsed markup; it
  does not mount the component.
