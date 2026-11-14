# pw-9: when mpv ends before Vela has connected to it, the play's end is held back five seconds, and the next play of a full-screen run opens windowed

**Severity**: MEDIUM — a play whose mpv ends within about a tenth of a second of starting (a stream the server refuses at once, say) is not seen to have ended for five seconds: its Stopped report, its recents stamp, the page's "playback ended" and the next play of a run all wait. And such a play hands on only its size, so the next play of a run that was full screen opens windowed.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Raised by the helper agents that traced `pw-8`, 2026-11-14, from their
traces and the code; not by the reviewer.

- The delay. `spawn_position_reader` in `src-tauri/src/playback.rs` tries
  to connect to mpv's socket every 100 ms, up to 50 times, and has no way
  to know that the play has already ended (the end-of-file watcher beside
  it has). When mpv has gone before the reader connected, it keeps trying
  and sets `done` only after about 5 s; the tracker's end callback waits on
  `done`. Seen in a trace: a play quit with only the test's connection
  open, 50 refused tries, and the end callback 5.0 s after mpv had gone.
- The window. `WindowStateObservation::opened_at` seeds only the windowed
  size. A play whose reader never connects has observed nothing, so it
  hands the next play no fullscreen, no maximized and no screen, although
  Vela itself opened it full screen. Seen in a trace: the first play quit
  before the reader connected, and the second play read `fullscreen:false`
  as its starting value.

Both are older than the fixes of this review; the second is in the
player-window work of 1.0.64, the first in the tracking of every play.

## Predicted observable failure

A user starts something that mpv gives up on at once, in a run where the
player window is full screen. For five seconds Vela still treats the play
as running (the next item of a playlist does not start, the page does not
return). Then the next play opens windowed.

## What

## Approach

Not started. What the helpers propose, and where it stands:

- Seed the observation's fullscreen and maximized (and perhaps the screen)
  with the state the play was opened with, so that a play that reported
  nothing hands on how it was opened. This is within the approved
  player-window plan (every play opens as the previous one left it).
- Let the reader give up once the play is known to have ended, from a
  signal that the child process has exited, wired through the player slot;
  not from "connection refused" alone, which can also happen in the moment
  between mpv binding its socket and listening on it, and would then drop
  the tracking of a live play. This changes when tracking gives up for
  every play, in every player and for every server, and is not part of any
  approved plan: it needs the owner's word before it is built.

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: helper agents (Claude Opus 5.5) while tracing `pw-8`,
2026-11-14 (`.agents/review/codereview-2026-11-14-player-window.md`,
"Raised after the intake"). Admitted by the coordinating agent on reading
the two places in the code. Not a codex finding.
