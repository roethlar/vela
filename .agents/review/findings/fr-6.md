# fr-6: a right-click, or a track chosen, while a wheel command is unanswered is dropped with "The player is busy"

**Severity**: MEDIUM — A right-click, or a track chosen, while a wheel command is unanswered is dropped with "The player is busy".
**Status**: Verified — codex wrote the repair; reviewed by the working agent, 2026-11-16
**Branch**: —
**Commit**: `e453ad1`

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

What landed (`e453ad1`), in its commit's words:

The embedded player takes one command at a time and refuses a second
with "The player is busy." The wheel's steps and the click's pause were
queued for that reason, but a right-click's pause and a choice in the
Audio or Subtitles list were sent directly, whatever was unanswered: made
while a wheel command was in flight, they were refused and lost (review
finding fr-6). So were the keys, the sliders and the buttons.

Every command of the view but Close now goes through the one queue. A
pause still goes first; the volume keeps its latest target and wheel
seeks add up, as before; any other command (a track, an absolute seek, a
crop) keeps its place, and wheel input after it is not merged across it,
so a relative seek cannot overtake a seek to a place. Close stops at
once and drops what was queued. A command the player refuses releases
the next one.

A user sees one difference: an action made while another is being
answered happens a moment later, where it used to do nothing.

docs/usage.md no longer says a right-click pauses "at once, as in mpv's
own window": it pauses without the click's quarter-second wait.

Written by codex; reviewed by the working agent. Its tests drive the
view's own script with command replies held open (a right-click, each
kind of track choice, keys and sliders and buttons, the order kept
around an absolute seek, Close and the view's removal dropping the
queue): with the two product files put back, nine tests fail; with the
repair, 82 of 82. The reviewer took one paragraph out of the guide that
described the queue; a user has no use for it.

Checked end to end with the three repairs after it: embeddedcontrols,
embeddedtracks, embeddedplay, embeddedfullscreenend, embeddedcrop and
playback in the embedded player, 6 of 6.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"The embedded player's view: in-page track lists, and the mouse". Reviewer's title: "Right-clicks and track choices bypass the command queue".
Reviewer's severity: MEDIUM.
