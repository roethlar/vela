# pw-9: when mpv ends before Vela has connected to it, the play's end is held back five seconds, and the next play of a full-screen run opens windowed

**Severity**: MEDIUM — a play whose mpv ends within about a tenth of a second of starting (a stream the server refuses at once, say) is not seen to have ended for five seconds: its Stopped report, its recents stamp, the page's "playback ended" and the next play of a run all wait. And such a play hands on only its size, so the next play of a run that was full screen opens windowed.
**Status**: In progress (the first half is fixed; the second waits for the owner)
**Branch**: —
**Commit**: `0b313a2` (first half)

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

Two things, as the evidence says. They are fixed apart, because one is
within the approved player-window plan and the other is not.

## Approach

**First half, fixed at `0b313a2`: a play hands on how its window was
opened.** `WindowStateObservation::opened_at` (`src-tauri/src/playback.rs`)
now takes the whole state the play was opened with and seeds fullscreen and
maximized as well as the size; the launch in `src-tauri/src/commands.rs`
passes it. Each report from mpv replaces its seed at once and for good. A
state Vela did not set when it opened the window and mpv never reported
stays unknown, so the configured options still decide there. The screen is
not seeded: where a window was asked to open is not where the compositor
put it, and the Prefer Compatible ranking trusts an observed screen and
its size. The end of such a play, and Vela's exit, save the opening state
under the same rules as any other play's; a part that stayed unknown keeps
what the settings had. One side effect, wanted: with an opening state of
full screen or maximized, a size reported before the first report of that
state is no longer taken for the windowed size.

Written by codex (gpt-6-astra at max effort) as a coding task, at the
owner's word of 2026-11-14 that coding tasks be handed to codex and
reviewed by the agent. Codex left the change uncommitted in a worktree of
its own; the coordinating agent reviewed it and landed it as one commit.
Codex cannot be the reviewer of code it wrote, so this fix is not sent to
codex for verification: the review below stands in its place.

**Second half, not started: the reader giving up sooner.** As proposed
above the line in the evidence: let the reader stop trying once the play
is known to have ended, from a signal that the child process has exited,
wired through the player slot; not from "connection refused" alone. It
changes when tracking gives up for every play, in every player and for
every server, and is in no approved plan. It waits for the owner's word.

## Files changed

- `src-tauri/src/playback.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/config.rs` (a comment)
- `tests/player-window-ui.test.mjs`

## Guard proof

Codex's tests. In `playback::tests`:
`a_play_that_reports_nothing_hands_on_its_opening_flags` (all four
combinations of the two flags survive silence);
`an_mpv_fullscreen_report_replaces_only_its_seed` and
`an_mpv_maximized_report_replaces_only_its_seed`;
`flags_neither_set_at_opening_nor_reported_stay_unknown`;
`opening_window_state_is_not_a_display_measurement`;
`opening_flags_keep_screen_sizes_out_of_the_windowed_size`. In
`commands::tests`:
`the_next_play_stays_fullscreen_when_the_previous_reader_never_connected`
(through `publish_started_play` and `opening_window`, against a setting
that says otherwise); `a_never_connected_plays_end_saves_its_opening_flags`;
`quitting_saves_a_never_connected_plays_opening_flags`. In
`tests/player-window-ui.test.mjs`: "the real launch seeds and shares the
complete opening window state" (the launch path's text).

Codex's red-proofs, 24 runs, each restored from a copy and compared byte
for byte: each seed taken out; a report made to update both flags, or
refused, or the seed restored on a later tick; an unknown flag invented; a
screen name, a display width and a display height seeded; the launch
seeding the size only; the reader and the published play given fresh
observations. Each failed the test that names it.

The coordinating agent's review:

- Read the whole change. The product change is three lines in `opened_at`
  and one at the launch; the rest is tests and comments.
- Ran the full check in codex's worktree and again on `main` with the
  change applied: green apart from the two recorded audit exits (650 Rust
  tests, 488 Node tests).
- Took the fullscreen seed out, then the maximized seed:
  `the_next_play_stays_fullscreen_when_the_previous_reader_never_connected`
  and `a_play_that_reports_nothing_hands_on_its_opening_flags` failed in
  turn; the file was put back and compared.
- Ran end to end what codex's sandbox cannot: `windowsetting`,
  `playlistplay`, `playback`, `exitplay` and `playerfallback` in the
  external player (5 of 5), and `playback`, `embeddedplay`, `playlistplay`,
  `embeddedfullscreenend` and `exitplay` in the embedded one (5 of 5).

## Coder dispute (if any)

## Known gaps

- No end-to-end scenario makes mpv end before Vela has connected to it
  (the scenarios now wait for Vela, `pw-8`), so the first half is held by
  the Rust tests and the text test only.
- The second half is open.

## Reviewer comments

Raised by: helper agents (Claude Opus 5.5) while tracing `pw-8`,
2026-11-14 (`.agents/review/codereview-2026-11-14-player-window.md`,
"Raised after the intake"). Admitted by the coordinating agent on reading
the two places in the code. Not a codex finding.

First half: written by codex, reviewed and landed by the coordinating
agent, 2026-11-14 (`0b313a2`); not sent to codex for verification, since
codex wrote it.

### The second half, `c861796`: written by codex, reviewed by the coordinating agent. CLOSED.

The owner said on 2026-11-14 that what is obviously broken is fixed
without asking (`.agents/decisions.md`, that date), so the second half no
longer waits.

The reader now knows which player it follows (`PlayerExit` in
`src-tauri/src/playback.rs`: the slot and the serial of this launch's
handle) and, after a connection that fails, asks whether that player has
exited: the child process for mpv's own window, the session's thread for
the embedded player. If so, the tries stop and the play's end is dealt
with at once, where it took about five seconds. A refused connection
alone means nothing, nor does a busy slot, another player in the slot or
an error from the question; how long the reader waits for an mpv that is
still starting is unchanged. The reader's `done` is now set by a guard on
every way out of its thread, a panic included.

Guard: `a_play_ending_before_its_socket_exists_finishes_without_the_connect_timeout`
runs `play()` with a real process and a real embedded session thread and
no socket. Codex saw it fail with the one line taken out (the end timed
out) and pass with it.

**Not taken.** Codex had also built a cure for a neighbouring fault (the
known gap of `pw-8`: a play cancelled by a newer play stops its reader
with lines it had received unread, so the window it hands on can be one
change out of date). The cure replaced the read loop of every play on
every platform, with a polling read on Windows and non-blocking reads on
Unix, and nothing here can compile or run the Windows or macOS side. The
fault shows only when a play in mpv's own window is replaced by one that
is not in mpv's own window, which today means the hidden embedded player.
The coordinating agent judged the risk to progress tracking on the
shipped platforms greater than the gain and had codex take it out again.
It stays a known gap; codex's version is kept outside the repository, in
the session's scratch files, and may not be there later.

The coordinating agent read both versions, merged the kept half with the
certificate change (`bc51d01`), which touches the same function, and ran
the full check on the result (green: 694 Rust tests, 549 Node tests).
End to end on that content: `playback`, `replaceplay`, `playlistplay`, `sourcedeadend`, `surfaces`, `exitplay`, `continueon` and `multiplex` in the external player (8 of 8), and `embeddedplay`, `embeddedlatestart`, `playlistplay`, `replaceplay` and `exitplay` in the embedded one (5 of 5). It made no red-proofs of its own: the owner set work on tests
aside that day. Under the owner's decision of 2026-11-14 ("Coding tasks
may go to codex") this review closes the finding.

Not checked: Windows and macOS (not compiled here); a real server that
refuses a stream.
