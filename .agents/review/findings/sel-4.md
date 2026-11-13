# sel-4: the test that play refuses a player it cannot start passes with the refusal removed

**Severity**: LOW — the shipped refusal is correct; its test is not a test.
**Status**: Verified
**Branch**: —
**Commit**: `47eec9a`, `29445f2` (the guard, after the reopen)

## Evidence

Reviewer, verbatim: "tests/player-selection.test.mjs:110 checks only that the conditional exists before four calls. Removing the return Err at src-tauri/src/playback.rs:301 while retaining the conditional left all five player-selection tests—and all 19 targeted Node tests—passing."

## Predicted observable failure

Reviewer, verbatim: "The test named 'play refuses a player it cannot start before it starts anything' falsely passes when an Embedded request can proceed into external-player side effects. This is a LOW regression-test defect; the current production refusal is correct."

## What

The test read the source of `play` and checked that an `if` on the player
comes before four calls. It never checked what the `if` does, so emptying
its body changed nothing.

## Approach

A Rust test calls `playback::play` with a play for the embedded player and
requires three things: the specific refusal comes back, no player is in
the slot afterwards, and the end-of-play callback has not run. The
source-text test stays for the one thing it can say, that the check comes
first, and now requires the Rust test to exist.

If the refusal is lost, the Rust test starts a real external player on a
machine that has one; it stops whatever it finds in the slot before it
asserts anything.

## Files changed

- `src-tauri/src/playback.rs` — test
  `play_refuses_a_player_it_cannot_start_and_starts_nothing`
- `tests/player-selection.test.mjs` — "play refuses a player it cannot
  start before it starts anything" names that test

## Guard proof

Run 2026-11-13 at `db0b8b6` in a disposable worktree; each injection restored
with `git checkout` from the commit. `cargo +stable test --locked --lib play_refuses` and
`node --test tests/player-selection.test.mjs`:

| Injection | Result |
| --- | --- |
| The reviewer's mutation: the refusal's body removed | Rust FAIL: `play_refuses_a_player_it_cannot_start_and_starts_nothing` |
| The Rust test renamed away | JS FAIL: "play refuses a player it cannot start before it starts anything" |
| Unchanged | PASS |

## Coder dispute (if any)

None.

## Known gaps

- Slice 3 of the plan gives `play` an embedded launcher. The refusal then
  becomes "no embedded player is available here", and this test changes
  with it.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Exercise playback::play with an Embedded PlaySpec and assert the expected refusal and absence of side effects. Prove that removing the return makes that test fail."

### Verification round 1, recorded 2026-11-13 07:29 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly; the host's codex config
  resolves to gpt-6-astra @ max. Thread `01a1d3c5-c977-73d3-a8e6-8e954f2b7e24`, run
  in the disposable worktree `~/dev/vela-probe-wt` at the head SHA; the
  stream ends in `turn.completed`; dispatched 07:18 UTC, finished
  07:25 UTC. Routing: T1 no sensitive path in the diff, T2 severity
  LOW, T3 the recorded proof re-run by the orchestrator agrees.
- Reviewed head `47eec9a4064e4727ae7b0fbb9d310ee7458ab6e0`, base
  `fb64072a3336e3cfdd196dceedf201e28b910526` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/playback.rs:1046 — A weakened refusal still escapes both guards: leave the original conditional empty and place the guarded return after resolve_mpv(). Both verification commands pass, while an instrumented executable confirms mpv --version starts before refusal. The Rust assertions miss side effects before slot registration; the Node test checks only the conditional's position. Guard that refusal returns before startup calls. Removing the refusal failed as expected; restoring the pinned head passed. No mpv process remained.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. The comment is correct. The Rust test sees that a player was
not started; it cannot see that `mpv --version` was run on the way to
finding that out, and the source-text test pinned where the `if` stands,
not that the `return` is inside it.

### Repair (round 2)

The Rust test is unchanged: it proves the refusal refuses and that no
player is started, and it cannot see a program that was run and exited on
the way. What can see that is the source: the test in
`tests/player-selection.test.mjs` now requires the refusal WHOLE, the check
with its `return` inside it, to be the first statement of `play`. With the
`if` emptied, or the `return` anywhere else, or anything before it, that
is no longer the text.

File: `tests/player-selection.test.mjs` only.

Guard proof, run 2026-11-13 against `9b2c819` with the repaired test, in a
disposable worktree, each injection restored with `git checkout`:

| Injection | Source-text test | Rust test |
| --- | --- | --- |
| The reviewer's: the `if` emptied, the `return` moved below the search for the mpv program | FAIL | passes, as the reviewer found |
| The `if` emptied, no `return` anywhere | FAIL | FAILS |
| A step done before the refusal | FAIL | passes |
| The refusal weakened by a second condition | FAIL | FAILS |
| Unchanged | PASS | PASS |

### Verification round 2 (repair-delta redispatch), recorded 2026-11-13 07:45 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d3d6-88a1-7181-885a-6709563d546b`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 07:36 UTC, finished 07:41 UTC.
- Reviewed head `29445f240ce163e4c8df9fd88d18b4ee84090f76`, base
  `47eec9a4064e4727ae7b0fbb9d310ee7458ab6e0` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted; the finding is
closed.

Note for a later reader: Slice 3 of the plan (`0a63799`) changed how `play`
decides its player, so the refusal is now a `?` inside the function's
first statement and its text is different. The same test pins that
statement the same way.
