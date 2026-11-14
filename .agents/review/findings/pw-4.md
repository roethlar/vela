# pw-4: quitting Vela waits, without limit, for the settings file to be written

**Severity**: MEDIUM — when Vela exits with a play's window to save, the exit handler reads and writes the settings file on the spot. That waits for the settings lock (which another Vela process can hold) and for the disk, with no deadline, on the thread that is closing the app. A stalled writer or filesystem keeps Vela from exiting.
**Status**: Verified
**Branch**: —
**Commit**: `5de1305`

## Evidence

Reviewer, verbatim: "src-tauri/src/lib.rs:514 calls remember_window_on_exit directly from the Tauri exit callback. src-tauri/src/commands.rs:4307-4311 synchronously loads and updates config; storage.rs:396-414 waits on both locks and performs filesystem writes without a deadline. A probe holding config.lock confirmed that this call returns only after the lock is released."

## Predicted observable failure

Reviewer, verbatim: "Quitting with a changed window freezes the event loop until config persistence finishes. A stalled config writer or filesystem can prevent Vela from exiting indefinitely."

Reviewer's suggested direction, verbatim: "Perform persistence on a blocking worker with a bounded shutdown protocol. Keep the event loop responsive and allow exit when the persistence deadline expires."

## What

The defect is as the reviewer describes. The Tauri exit handler called
`remember_window_on_exit` directly, and that function ran
`config::load_config` and `config::update` on the thread closing the app,
with no deadline for the settings lock (another Vela process can hold it)
or for the disk. A stalled writer or filesystem kept Vela from exiting:
quitting hung.

The helper who attacked the fix found nothing more to repair here.

## Approach

`remember_window_on_exit` takes the `Arc` of the window record and runs
the hand-over through the new `hand_over_within(EXIT_WINDOW_HANDOVER_LIMIT,
...)`. That starts a named thread and waits for it with `recv_timeout` for
at most two seconds, like the wait for embedded plays beside it, and then
returns so the process can go. It returns false at once if the thread
cannot be started. In `lib.rs` the call is unchanged; its comment was
updated.

A write cut off by the process ending leaves the old settings file whole.
The helpers confirmed this by reading the code: `config::update` goes
through `update_at` and `storage::update_json_before_save` to
`storage::save_json`, which writes a complete temporary file with
`storage::private_temp` (`create_new`, `write_all`, `sync_all`) and then
renames it over `config.json`. The history copy
(`durable::preserve_valid_history`) is written with
`storage::write_private_new`, a temporary file and a rename that does not
replace. At most a stray temporary file is left beside the intact file.

What a user gets: quitting is never held up longer than about two seconds
by the window save (about four in the worst case; see the gaps).

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, confirmed by another on the merged tree, and read and landed by
the coordinating agent, one commit per helper commit. The attack found
nothing to repair here: the attacking helper found no other unbounded
wait added on the thread that runs the event loop, and confirmed the
atomic write by reading `storage.rs`.

## Files changed

- `src-tauri/src/commands.rs`
- `src-tauri/src/lib.rs` (comment only)
- `tests/player-window-ui.test.mjs`

## Guard proof

In `commands::tests` (`src-tauri/src/commands.rs`):

- `quitting_waits_for_the_window_hand_over_only_so_long`: real
  `config.json` and `config.lock` files in a folder of its own under the
  system's temporary directory. It holds the file lock as another Vela
  would; `hand_over_within` with a 200 ms limit must return false within
  the limit and two seconds, without having written; once the lock is let
  go of, the hand-over writes and the saved file holds the window. A
  watchdog keeps a regression from hanging the suite.
- `a_hand_over_that_finishes_in_time_is_waited_for`.

In `tests/player-window-ui.test.mjs`, a source-text check with the
message "the exit waits for the hand-over with a limit" pins the exit's
use of the thread, the limit and its two seconds.

Red-proofs, each restored from the commit afterwards:

- A wait without limit (`recv` instead of `recv_timeout`): the test failed
  with "quitting must not wait for a settings lock somebody else holds:
  Timeout"; the Node pin failed too.
- The hand-over run inline: the same failure.
- The thread skips the write: the test failed with "the hand-over still
  writes once the lock is free: Disconnected".
- `remember_window_on_exit` bypasses `hand_over_within`: the Node pin
  failed.
- The limit set to 600 s: the Node pin failed on the two seconds.
- The attacking helper: `hand_over_within` always sleeps the full limit:
  the Node pin "the exit waits for the hand-over with a limit" failed; the
  Rust tests alone stayed green (a 10 s run).
- The confirming helper, on the merged tree: `recv()` with no limit:
  `quitting_waits_for_the_window_hand_over_only_so_long` failed (it
  panicked at `commands.rs:8603`, the in-test timeout), and the Node pin
  failed.

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

What rests on source-text checks only: that the real exit goes through
`hand_over_within`, and the value of the limit. That a cut-off write
leaves `config.json` whole rests on reading `storage.rs`; no test cuts a
write off.

At landing (2026-11-14) the ten commits of this review's fixes were put on
a branch cut from `main` in the helpers' order. Each passed `npm run check`
and `cargo +stable test --locked` on its own before the next was added,
and the full check was green on the last (641 Rust tests, 424 Node tests;
the two audit commands exit as recorded). The product code and the two
player-window test files there were byte for byte the tree the confirming
helper had run end to end.

## Coder dispute (if any)

## Known gaps

- The exit closure runs for both `RunEvent::ExitRequested` and
  `RunEvent::Exit`, so while the settings lock stays held the two-second
  wait can happen twice, about four seconds at worst. That is bounded, not
  deduplicated.
- A write cut off between creating the temporary file and the rename can
  leave a stray `config.json.tmp.<pid>.<uuid>` beside the intact
  `config.json`.
- The test writes under `std::env::temp_dir()`, like the existing
  `config.rs` tests, and removes its folder afterwards.
- `docs/usage.md` says the window is saved when you quit with a video
  playing. That is true unless the save cannot finish within two seconds;
  the sentence was left as it is.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 3 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "Exit performs an unbounded synchronous config write". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 11:34 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d9cc-31d7-7fe0-a74e-d73f7a7f7ab8`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 11:23 UTC, finished 11:33 UTC.
- Reviewed head `0e5ea592532c476cafe6a1142dc6de68d2f57ef1`, base
  `530e9a544cf0b7f2f3f4837aec093b1d5b3f0492` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
