# fr-7: a long wheel burst adds up to a seek the player refuses, and all of it is lost

**Severity**: MEDIUM — A long wheel burst adds up to a seek the player refuses, and all of it is lost.
**Status**: In progress
**Branch**: —
**Commit**: —

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

The repair is being written by codex as a coding task (worktree
`~/dev/vela-agent-keep/wt/cx-rvview` on `continuum`, not on `main` until the
working agent has reviewed and landed it), under the owner's decision of
2026-11-14 that coding tasks may go to codex and that a repair codex wrote
is closed by the working agent's review. This section is replaced by what
landed.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"The embedded player's view: in-page track lists, and the mouse". Reviewer's title: "Accumulated wheel seeks can exceed the backend's accepted range".
Reviewer's severity: MEDIUM.
