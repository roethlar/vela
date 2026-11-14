# pw-8: the end-to-end scenario for the player window fails about one external run in four, and with it the parity verdict

**Severity**: MEDIUM — `windowsetting`, the scenario that holds the player-window work end to end, failed about one external run in four at its fullscreen checks, and four of the last five full runs. It is not a recorded known failure, so every such run failed the parity verdict. The cause turned out to be in the scenario, not in Vela.
**Status**: In progress
**Branch**: —
**Commit**: `26660a9`

## Evidence

Raised by the coordinating agent from end-to-end runs on 2026-11-14, not
by the reviewer. The scenario failed in a helper's full external run of a
merged copy, then in the full external runs at `9bcdfbc`, `ac25240` and
`8cf3a05`, always in one of three places, each just after the scenario had
changed fullscreen over its own connection to mpv and at once gone on:

- after leaving fullscreen and ending the play, `config.json` still said
  fullscreen ("the end of the play remembers how it left the window":
  expected false, got true);
- after entering fullscreen and at once quitting Vela, `config.json` never
  got fullscreen ("timed out waiting for the window of the play still
  showing when Vela quit");
- the play after one that was quit at once opened windowed.

Three helpers measured it before the seven fixes of this review landed and
after: 3 failures in 16 runs at the review's base, and similar rates on
the fix branches.

## Predicted observable failure

A full external run fails on `windowsetting`, and `npm run e2e:verdict`
fails with it, about one time in four, with nothing wrong in Vela.

## What

Settled by a helper agent with a local build that printed the order of
events (never committed), and confirmed by a second helper.

Nothing is lost inside Vela. Vela learns how a window is left only over
its own connections to mpv's socket: a reader of the window state and a
watcher for the end of the file. It opens them by trying every 100 ms, and
the first try usually comes before mpv has made the socket, so Vela is
connected up to about 100 ms after mpv starts listening. The scenario
found the socket, connected, paused, changed fullscreen and quit mpv (or
asked Vela to exit) within 1 to 4 ms of the socket appearing. About one
play in fifteen, all of that fell between two of Vela's tries, and mpv
ended with only the scenario's connection open (mpv's own log shows one
client). It never told Vela anything.

In 34 traced runs there were 6 failures, and in every failing play Vela's
reader had never connected. In every play where it had, the traced order
was: mpv's fullscreen event read, applied, the end of the file read, the
reader's end of file, then the end callback and its look at the window,
then the settings written. With a delay of 60 ms or more between the
change and the end, no changed play failed in 32.

A person cannot do what the scenario did: mpv's window is not there yet a
few milliseconds after its socket appears. So this is the scenario's
fault at all three places. (The helpers who first measured the failures
had two readings, "mpv delivers the change in under a millisecond" and
"Vela's observation lags"; both were right: delivery is fast, and Vela was
not yet connected.)

## Approach

The scenario's `play()` now waits, after the play's recorded start, until
mpv has accepted three connections on its socket: the scenario's own and
Vela's two (the only ones Vela opens for a play like this one, with no
notice to show and no Automatic quality). The count is read from
`/proc/net/unix`. Nothing a user sees says that Vela is connected, so no
wait on something visible was possible; this is a wait on the sockets, not
a sleep, and nothing asserted afterwards changed.

The comment at the end of a play in `src-tauri/src/commands.rs` said "The
IPC connection has closed by now, so this is the final state". It now says
what is true in each case: when the reader read mpv's connection to its
end, this is all the play reported; when mpv ended before the reader had
connected, only the size the play opened at is handed on; when the play
was stopped by a newer play or by Vela quitting, the reader may have
stopped with lines unread. No behaviour changes.

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, repaired by a third (the comment, and corrected figures), and read
and landed as one commit by the coordinating agent, which moved the
counting into a function that plain `node --test` holds.

What the helpers found in Vela while tracing this is another finding,
`pw-9`.

## Files changed

- `tests/e2e/scenarios/windowsetting.mjs`
- `tests/e2e/judge.mjs`
- `tests/e2e-judge.test.mjs`
- `src-tauri/src/commands.rs` (a comment)

## Guard proof

In `tests/e2e-judge.test.mjs`:

- "pw-8: the connections a socket has accepted are counted from
  /proc/net/unix": the count on a made-up text of that file (the listening
  socket, three accepted ends, one still connecting, a connecting end with
  no path, another socket whose path only begins the same).
- "pw-8: windowsetting hands a play back only once Vela is connected to
  its mpv": the scenario's text, where the wait for three is the last step
  before `play()` returns, by its one way out, and the count is asked
  nowhere else.

Red-proofs by the coordinating agent, the file put back and checked byte
for byte after each: the count taking a socket still connecting; the count
taking a path that only begins the same; the scenario waiting for one
connection; the wait there but never run. Each failed its test.

End to end (the helpers'): with Vela's reader held back by a local change
to the product, the scenario passed with the wait and failed without it.
On the fixed scenario `windowsetting` passed 76 runs of 76 across the
three helpers (25, 31 and 20 with 5 in pairs with `playlistplay`), ten of
them with sixteen busy loops running; `playback`, `resume`, `playlistplay`
and `watchstate` passed twice and `--embedded playback embeddedplay` once.

`npm run check` and `cargo +stable check --locked` were green before the
commit. Both full suites and the parity verdict are being run at
`26660a9`; the plan's log has the result.

## Coder dispute (if any)

## Known gaps

- The wait counts connections, at least three, not Vela's reader itself.
  If Vela one day opens another connection for such a play, the wait could
  be met early and the old failure would come back. It would come back as
  a failure, not as a false pass. The scenario's comment says so.
- `/proc/net/unix` makes the wait Linux-only, as the end-to-end suite is.
- At Vela's exit nothing waits for the reader to have read what is already
  in the socket. With the reader connected the margin was about 2 ms in 40
  traced exits and no change was lost; no person can reach it.
- Other scenarios were not gone through for the same race; only
  `playlistplay` was run alongside.
- Nothing in `npm run check` shows that the wait cures the failure; that
  rests on the traced cause and on the end-to-end runs.

## Reviewer comments

Raised by: the coordinating agent, from end-to-end runs, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`, "Raised after
the intake"). Not a codex finding; codex verifies the fix like any other.
