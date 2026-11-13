# sel-2: an Automatic quality step-down makes the fallback explanation appear again

**Severity**: LOW — one extra line on screen; playback is unaffected.
**Status**: In progress
**Branch**: —
**Commit**: (this commit)

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5208 records None for a successful step-down: apply_step_down supplies the actual External player, so choose reports no fallback. FallbackNotices::record at src-tauri/src/player/select.rs:107 then clears the previous notice. Exercising the command path with unchanged player="embedded" produced IPC notices in this order: fallback, quality change, fallback on the next play."

## Predicted observable failure

Reviewer, verbatim: "The same fallback explanation appears twice within one Vela run despite unchanged settings and availability. Playback succeeds; the defect is repeated informational output."

## What

The notice state knew two things about a started play: it fell back for a
reason, or it had nothing to fall back from. The second clears what the
user has been told, so that a later fallback is explained again. An
Automatic step-down relaunch is handed the player it replaces, so its own
choice reports no fallback, and it was taken for the second kind.

## Approach

A relaunch is a third kind: it continues a play that was explained, or
not, when it started. `FallbackNotices::plan(chooses_player, fallback)`
now makes the whole decision in one pure function
(`src-tauri/src/player/select.rs`): a play that does not choose its player
explains nothing and forgets nothing; a play that does either explains a
reason not yet told, stays quiet about one already told, or (having
nothing to fall back from) forgets once it has started. `due` and `record`
are gone; the play path calls `plan` before the launch and `explained` or
`forget` after it succeeded.

## Files changed

- `src-tauri/src/player/select.rs` — `NoticePlan`,
  `FallbackNotices::{plan, explained, forget}`; tests
  `a_fallback_is_explained_once_until_it_goes_away` and
  `an_automatic_relaunch_neither_explains_nor_forgets`
- `src-tauri/src/commands.rs` — `play_by_key_locked` uses them
- `tests/player-selection.test.mjs` — "a fallback is explained once, and
  only by a play that started" re-anchored to the new calls

## Guard proof

(pending)

## Coder dispute (if any)

None.

## Known gaps

- No end-to-end run of fallback, step-down, next play: a step-down needs
  a server that transcodes, which the E2E mock is not. The rule is
  unit-tested and its one call site is pinned by the wiring test.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Preserve fallback notice history across automatic relaunches of the same logical play. Add coverage for fallback followed by step-down followed by another play."
