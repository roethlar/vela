# pc-1: a line that arrives in pieces is read past the limit of whoever is waiting for it

**Severity**: MEDIUM — a command to the embedded player can wait longer than its two seconds, holding the play's one-command gate, and a stopped play's state thread can stay alive, for as long as mpv's socket keeps sending pieces of a line or lines too long to keep. mpv does not ordinarily do either.
**Status**: In progress
**Branch**: —
**Commit**: `a909c20`

## Evidence

Reviewer, verbatim: "src-tauri/src/player/report.rs:313 loops internally until a complete accepted line or socket timeout; oversized lines restart that loop at line 341. Consequently, control.rs:430 cannot recheck its deadline, and report.rs:485 cannot recheck the stop flag. Socket probes showed a 100 ms command budget taking 602 ms with partial data and 446 ms with consecutive oversized events. A reporter also retained its notifier 500 ms after stop while partial data continued."

## Predicted observable failure

Reviewer, verbatim: "Continuous input can hold a command worker and its gate indefinitely, making subsequent controls return busy. The reporter can likewise stop emitting state and retain its thread and connection despite cancellation."

Reviewer's suggested direction, verbatim: "Check elapsed time and cancellation between read chunks, including while discarding oversized lines. Preserve partial data across bounded returns so callers can enforce their deadlines and stop flags."

## What

Two things read lines from mpv's IPC socket for the embedded player's view: a command waiting for its answer (`exchange` in `control.rs`, with a two-second limit for the whole exchange and a gate that lets one command through at a time), and the per-play thread that reports state to the page (`spawn_reporter` in `report.rs`, which looks at the play's stop flag and at its clock between reads). Both read through `Lines::next`, and that function did not come back until it had a whole line. While pieces of a line kept arriving, or while a line too long to keep was being thrown away, the caller was not asked anything: a command stayed past its limit and held the gate, and a stopped play's reporter stayed alive and sent no state.

Reproduced before anything was changed, with a stand-in player on a real socket that answers with a line that never ends, four bytes every few milliseconds: a command with a 300 ms limit got no answer at all in the test's window, the reporter sent no state for the whole 2.5 s, and a reporter whose play had been stopped lived on until the stand-in stopped sending.

mpv does not ordinarily send lines in dribbles, so this is not reached in an ordinary play. The limit and the stop flag are promises of the slice all the same.

## Approach

- `Lines::next` reads from the socket at most once per call. With no whole line yet it returns a new answer, `Read1::Piece`, and keeps the piece it has: a line that arrives in pieces is still read whole, as before. It returns `Piece` between the pieces of an over-long line it is dropping too, and at that line's end.
- `exchange` goes back to the top of its loop on a piece, where the one deadline set before the connect is checked and the read's own limit is set from what is left of it. So the limit covers the connect, the write and every read.
- The reporter treats a piece like a wait that ran out: it falls through to its clock (the at-most-four-a-second, at-least-one-a-second rule) and then to the play's stop flag.
- Nothing changes in what either does with a complete line. No other code reads through `Lines`.

Written by one helper agent, attacked by another (which found the finding closed at its root, and one thing outside it: see Known gaps), then verified by the coordinating agent.

## Files changed

- `src-tauri/src/player/report.rs` — `Read1::Piece`; `Lines::next` returns after each read; the reporter's loop; a test-only stand-in that sends a line without end; two socket tests; the two scripted reader tests follow the new answer.
- `src-tauri/src/player/control.rs` — `exchange` continues on a piece; two socket tests.

## Guard proof

Run 2026-11-14 by the coordinating agent in a disposable worktree at the fix's content; `cargo +stable test --locked --lib player::`.

| Injection | Rust |
| --- | --- |
| Unchanged | PASS (207) |
| A piece of a line is taken for the answer (the command gives up at the first piece instead of reading on within its limit) | FAIL, four tests, among them `a_line_that_comes_in_pieces_without_end_does_not_stretch_the_limit`, `a_line_past_the_cap_without_end_does_not_stretch_the_limit` and `each_command_reaches_the_player_as_its_fixed_mpv_command` |
| The state thread does not look at its clock or its stop flag after a piece | FAIL `a_line_that_comes_in_pieces_without_end_holds_up_neither_state_nor_stop`, `a_line_past_the_cap_without_end_holds_up_neither_state_nor_stop` |

Also run by the coordinating agent at the fix's content: the full check (571 Rust tests; `npm audit` fails on `main` for a reason of its own, see `.agents/state.md`, Blockers), and four scenarios in the embedded venue, among them `embeddedcontrols` (4 of 4).

The helper that wrote the fix ran nine injections, each failing its named test, and the attacking helper re-ran the first. Each alone: the inner loop put back (all four new tests and the scripted reader test fail); the same with the test's state bound widened so that the stop assertion is reached (a stopped reporter lives on for 1.5 s); control handed back for a normal line but not while dropping an over-long one (only the two over-long tests fail); `exchange` reading on in a loop of its own while pieces come; the reporter's stop check removed; the reporter not looking at its clock after a piece; the kept piece thrown away (a line in two pieces is no longer whole); the gate not released after an unanswered command; the quiet spell not started.

## Coder dispute (if any)

## Known gaps

- No scenario feeds real mpv a line in pieces; the socket tests hold it. `embeddedcontrols` and three more embedded scenarios pass on the fix, which shows commands and state still work against the real player.
- The tests measure time: a command must give up within its 300 ms limit plus 300 ms; states must come at most 1.2 s apart; a stopped reporter must end within 500 ms. They passed in every run here and could fail on a far more loaded machine.
- Outside this finding, found by the attack and left alone: when a line passes the reader's cap, the buffer is freed rather than only emptied, and no test would notice if it were only emptied. Memory is bounded either way (the cap is checked before anything is kept); the difference is up to a megabyte held for the life of one connection.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `0aeeff0..3d2fef6`, 2026-11-14 00:45 UTC
(`.agents/review/codereview-2026-11-14-embedded-controls.md`). Reviewer's
title: "Partial or oversized IPC lines bypass the read deadline". Reviewer's severity: MEDIUM.
