# ec-7: a stream-auth file whose removal fails once is never removed

**Severity**: MEDIUM — a file holding the stream's credentials stays on disk after the play, in Vela's private runtime directory. It needs the removal to fail at the one moment it is tried.
**Status**: In progress
**Branch**: —
**Commit**: `9026b4f`

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/session.rs:423-427 takes HeaderInclude out of Session and drops it when remove_now fails, unlike external.rs:47-57, which restores ownership. A review probe made the include directory temporarily non-writable, called removal, restored permissions, and dropped Session; the credential file remained."

## Predicted observable failure

Reviewer, verbatim: "A temporary deletion failure leaves the stream-auth include on disk permanently because reaping and exit no longer retain its path for retry."

Reviewer's suggested direction, verbatim: "Restore the include to Session when removal fails, matching ManagedChild, so reaping or exit can retry after the transient failure clears."

## What

Before a replacement launch the running player's stream-auth include is removed (`remove_consumed_header_include`). The embedded play's version took the include out of the play and, when the removal failed, returned the error with the include still in a local variable, which was then dropped. Dropping tries the removal once more; if that fails too, nothing owns the file any longer. The external player's handle puts the include back on failure, so that reaping that player tries again.

## Approach

One function, `HeaderInclude::remove_consumed` (`launch.rs`), now does this for both players: it takes the include out of its owner, removes the file, and puts the include back when the removal failed. `ManagedChild` and the embedded `Session` both call it and have no removal code of their own. An embedded play that could not remove its include therefore still owns it, and removes it when the play is reaped or Vela exits; the replacement launch fails closed, as it does for the external player.

## Files changed

- `src-tauri/src/player/launch.rs` — `HeaderInclude::remove_consumed`; one test.
- `src-tauri/src/player/external.rs` — `ManagedChild::remove_consumed_header_include` calls it (behaviour unchanged).
- `src-tauri/src/player/embedded/session.rs` — `Session::remove_consumed_header_include` calls it; test-only `for_test_holding`; two tests.
- `tests/embedded-player.test.mjs` — both handles use the one function and nothing else removes an include.

## Guard proof

Run 2026-11-13 at `e66a928` (the fix is `9026b4f`) in a disposable worktree; `cargo +stable test --locked --lib player::` and `node --test tests/embedded-player.test.mjs`. The tests make a removal fail by taking write permission from the file's directory, and let it succeed by giving it back; they return early when run as root, who can remove the file regardless.

| Injection | Rust | Node |
| --- | --- | --- |
| The original defect: the embedded play takes the include out and lets go of it on failure | FAIL `a_play_keeps_a_stream_auth_file_it_could_not_remove_and_removes_it_when_reaped` | FAIL `both players give up a consumed stream-auth file the same way` |
| The shared function drops the include after a failed removal | FAIL, that test and `an_include_whose_removal_fails_stays_with_its_owner_until_it_is_removed` | FAIL |
| A failed removal is reported as done | FAIL, the same two | FAIL |
| The file is handed back but never removed | FAIL `a_play_removes_a_consumed_stream_auth_file_at_once_when_it_can`, `consumed_include_is_removed_before_a_replacement_launch`, and the owner test | FAIL |
| The external player lets go of its include without the shared function | (not run) | FAIL |
| Unchanged | PASS (94) | PASS |

## Coder dispute (if any)

## Known gaps

- If the removal keeps failing for as long as Vela runs, the file is still there at exit, in Vela's private runtime directory (owner-only, under a random name). Nothing more can be done about a file that cannot be removed; the next start does not reuse that directory.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "Failed auth-file deletion loses its cleanup owner". Reviewer's severity: MEDIUM.
