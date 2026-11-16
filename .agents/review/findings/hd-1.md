# hd-1: an old description of the output can replace a newer one

**Severity**: MEDIUM — a change of the display during a play (HDR switched off, the brightness moved) is then missed until the next change, and the picture stays drawn for the display as it was. It needs a compositor that answers an earlier question after acknowledging a later one; KWin does not.
**Status**: Verified
**Branch**: —
**Commit**: `499a0d3`

## Evidence

Reviewer, verbatim: "surface.rs:202-208 replaces the shared `arriving` accumulator, while surface.rs:222-251 ignores which information object sent each event. Destroying the previous description at surface.rs:583-586 does not destroy its information child. A socket-pair test using the real bindings confirmed that old information arriving after the replacement's Ready consumed the accumulator: the result remained reference/max 265/465, and the subsequent 203/203 reply was discarded."

## Predicted observable failure

Reviewer, verbatim: "An output change is lost until another notification. HDR can remain selected after it is disabled, or playback can retain the previous brightness and tone-mapping target."

Reviewer's suggested direction, verbatim: "Give each information object a query generation and its own accumulator. Publish only the current generation's completed description; ignore obsolete replies."

## What

When the display changes, the compositor says so and Vela asks for its new description. The description comes in two steps: an object that becomes "ready", and then, on a second object Vela asks for, its contents as several events ending in "done". If the display changes again before the contents of the first answer have all arrived, two answers are in flight. Every piece went into one slot (`State::arriving`), whichever answer it belonged to. With KDE the pieces of the older answer arrive before the newer question is acknowledged, and all is well. The protocol does not promise that order. In the other order the older pieces filled the slot opened for the newer answer and completed it; the newer pieces then found no slot and were thrown away. The play kept the description of the display as it had been, until the display changed again.

## Approach

Every question now has a number. `OutputAnswers` (new, in `hdr.rs`, where it can be tested without a compositor) hands out the number when a question is asked, is told the number with every "ready", every piece and every "done", and puts together only the answer to the latest question. The surface gives the number to the protocol objects themselves: the description object is created with `Described::Output(question)`, and the object that delivers its contents is created with that same number as its data, so a late piece arrives labelled with the question it answers and is ignored. Asking a new question also drops a complete answer that nobody collected, since it describes a display that has changed since.

## Files changed

- `src-tauri/src/player/embedded/hdr.rs` — `OutputAnswers`, `Piece`; five tests.
- `src-tauri/src/player/embedded/surface.rs` — `Described::Output(u32)`, the contents handler keyed by question, `ask_about_output` and `output_described` go through `OutputAnswers`; the shared slot is gone.
- `tests/embedded-player.test.mjs` — one new wiring test; one pin follows the moved text.

## Guard proof

Run 2026-11-13 at `499a0d3` in a disposable worktree; `cargo +stable test --locked --lib hdr` and `node --test tests/embedded-player.test.mjs`.

| Injection | Rust | Node |
| --- | --- | --- |
| The original defect: pieces and ends go into the one slot whichever question they answer | FAIL `the_late_tail_of_an_old_answer_does_not_replace_the_new_one`, `two_answers_arriving_in_turns_do_not_mix` | |
| A piece is taken whichever question it answers (the end is still checked) | FAIL `two_answers_arriving_in_turns_do_not_mix` | |
| Any answer's end completes the latest answer | FAIL, both | |
| The contents of an answer to a replaced question are asked for | FAIL `only_the_latest_question_has_its_contents_asked_for` | |
| An answer nobody collected survives a new question | FAIL `asking_again_drops_an_answer_nobody_collected` | |
| Every question has the same number | FAIL, three tests | |
| A complete answer is handed on again and again | FAIL `an_answer_is_handed_on_once_it_is_complete` | |
| The object that delivers the contents is not told which question it is for | | FAIL `every piece of the output's description is matched to the question it answers` |
| Every question is put to the compositor as question one | | FAIL |
| Every end is taken for the end of question one | | FAIL |
| Unchanged | PASS (28) | PASS |

Also run: on `continuum`'s desktop (KDE, HDR on) the HDR clip plays as before: HDR target 1137 before the first frame, no dropped frames.

## Coder dispute (if any)

## Known gaps

- The out-of-order arrival itself is not reproduced in the repo against a compositor: KDE does not do it, and the test venue's compositor has no colour management. The reviewer reproduced it with a socket pair. The repo holds the logic with a stand-in for the sequence of events, and the hand-over of the numbers by source text.
- A description of the surface (the one Vela asks the compositor to make) has no such number. There only one object is ever current: asking for a new one destroys the old, and libwayland discards events for a destroyed object.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `74f44b6..e8db400`, 2026-11-13 08:38 UTC
(`.agents/review/codereview-2026-11-13-embedded-hdr.md`). Reviewer's
title: "Associate information replies with their output query". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-13 18:46 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d609-0948-73b3-a55e-fc7f6c5309d2`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 17:51 UTC, finished 18:03 UTC.
- Reviewed head `499a0d380e5b968675c7e6565fc00fde63c60191`, base
  `ddf475f49ec57df2a13a5f54b94016e350a34a42` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
