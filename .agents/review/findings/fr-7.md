# fr-7: a long wheel burst adds up to a seek the player refuses, and all of it is lost

**Severity**: MEDIUM — A long wheel burst adds up to a seek the player refuses, and all of it is lost.
**Status**: Verified — codex wrote the repair; reviewed by the working agent, 2026-11-16
**Branch**: —
**Commit**: `0cd99a2`

## Evidence

Reviewer, verbatim: "src/lib/playerControls.ts:454 and :466 generate and merge seek seconds without a bound. Holding a command unanswered and delivering 61 horizontal steps produces seek-relative [610]. src-tauri/src/player/control.rs:137 accepts only -600 through 600."

## Predicted observable failure

Reviewer, verbatim: "A sufficiently large wheel burst loses its entire queued seek and displays “Not a player command.” A single large wheel event can cause the same failure."

Reviewer's suggested direction, verbatim: "Dispatch accumulated seeks in chunks within the backend limit, retaining the remaining seconds until each reply arrives."

## What

Admitted on the reviewer's reproduction: wheel seeks queued while a command
is unanswered are added together without a bound, and the backend accepts a
relative seek of at most 600 seconds either way (`src-
tauri/src/player/control.rs`). Sixty-one steps make 610, which is refused
whole with "Not a player command."

## Approach

What landed (`0cd99a2`), in its commit's words:

Wheel seeks made while a command is unanswered are added together, and
the player accepts a relative seek of at most 600 seconds either way.
Sixty-one steps made 610, which was refused whole with "Not a player
command.": the whole burst was lost (review finding fr-7). One large
wheel event could do the same.

A seek larger than the player takes is now sent in pieces of at most 600
seconds, the rest kept queued for the next reply, where later wheel
input still adds to it or cancels it and a pause still goes first.

Written by codex; reviewed by the working agent: with playerControls.ts
put back, nine tests fail (61 steps in each direction, one large event
in each, the remainder kept, reversed and cancelled, Close and the
view's removal dropping it); with the repair, 91 of 91.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"The embedded player's view: in-page track lists, and the mouse". Reviewer's title: "Accumulated wheel seeks can exceed the backend's accepted range".
Reviewer's severity: MEDIUM.
