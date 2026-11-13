# sel-2: an Automatic quality step-down makes the fallback explanation appear again

**Severity**: LOW — one extra line on screen; playback is unaffected.
**Status**: Verified
**Branch**: —
**Commit**: `12828ca`

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

Run 2026-11-13 at `db0b8b6` in a disposable worktree; each injection restored
with `git checkout` from the commit. `cargo +stable test --locked --lib select` and
`node --test tests/player-selection.test.mjs`:

| Injection | Result |
| --- | --- |
| The original defect: `plan` ignores whether the play chooses its player | Rust FAIL: `an_automatic_relaunch_neither_explains_nor_forgets` |
| A relaunch forgets what was told | Rust FAIL: the same test |
| The play path tells `plan` that every play chooses its player | JS FAIL: "a fallback is explained once, and only by a play that started" |
| Unchanged | PASS |

## Coder dispute (if any)

None.

## Known gaps

- No end-to-end run of fallback, step-down, next play: a step-down needs
  a server that transcodes, which the E2E mock is not. The rule is
  unit-tested and its one call site is pinned by the wiring test.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Preserve fallback notice history across automatic relaunches of the same logical play. Add coverage for fallback followed by step-down followed by another play."

### Verification round 1, recorded 2026-11-13 07:14 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly; the host's codex config
  resolves to gpt-6-astra @ max. Thread `01a1d3bb-4965-7a30-8c40-83ea2cb1c933`, run
  in the disposable worktree `~/dev/vela-probe-wt` at the head SHA; the
  stream ends in `turn.completed`; dispatched 07:07 UTC, finished
  07:11 UTC. Routing: T1 no sensitive path in the diff, T2 severity
  below HIGH, T3 the recorded proof re-run by the orchestrator agrees.
- Reviewed head `12828caaa3d4fc809ce8abc8cec014418ab11de8`, base
  `88c631b719e108032decbe5aa7c1a403f1555271` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. Comments, verbatim:
  (none)

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted; the finding is
closed.
