# fr-6: a right-click, or a track chosen, while a wheel command is unanswered is dropped with "The player is busy"

**Severity**: MEDIUM — A right-click, or a track chosen, while a wheel command is unanswered is dropped with "The player is busy".
**Status**: In progress
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src/lib/PlayerView.svelte:138, :282 and :391: send() increments commandsPending without checking it. Running the actual handlers with a wheel command unanswered sends a subsequent right-click toggle or track choice immediately. The backend rejects overlapping commands at src-tauri/src/player/control.rs:362."

## Predicted observable failure

Reviewer, verbatim: "The pause toggle or track choice is dropped with “The player is busy.” It is never retried."

Reviewer's suggested direction, verbatim: "Route all non-stop player commands through one dispatcher that waits for the outstanding reply. Preserve pause priority and wheel coalescing."

## What

Admitted on the reviewer's run of the handlers: the player takes one command
at a time and the wheel's steps and the click's pause go through the queue
built for that, but the right-click's pause and a choice in the Audio or
Subtitles list are sent directly, whatever is unanswered. The backend
refuses the second command and the user's action is lost. The mouse work's
own rule (one queue) was not applied to these two.

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
"The embedded player's view: in-page track lists, and the mouse". Reviewer's title: "Right-clicks and track choices bypass the command queue".
Reviewer's severity: MEDIUM.
