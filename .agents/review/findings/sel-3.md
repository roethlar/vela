# sel-3: a play replaced within a second and a half uses up the fallback explanation without showing it

**Severity**: LOW — the user is never told why the embedded player was not used; playback is unaffected.
**Status**: In progress
**Branch**: —
**Commit**: `50e57e4` (which also carried four unrelated files by mistake; `fb64072` takes them out again), `db0b8b6` (a fourth test)

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5064 treats constructing the notice as telling the user, and :5213 records it immediately after launch succeeds. However, src-tauri/src/playback.rs:597 waits 1.5 seconds before sending and :598 cancels when the play is replaced. A command-path reproduction replaced the first play before that delay, then kept the replacement running: neither IPC connection received the fallback notice."

## Predicted observable failure

Reviewer, verbatim: "A quick replacement consumes the once-per-run explanation without displaying it; subsequent ordinary fallback plays silently suppress it. Playback itself remains functional."

## What

The explanation was recorded as given when the player started. It is
actually sent a second and a half later, so that it is not drawn over a
black frame, and it is dropped if the play has been replaced by then, or if
the player cannot be reached. A play that was replaced quickly used up the
once-per-run explanation without showing it.

## Approach

"Told" now means the line was handed to the player. `PlaySpec` carries an
optional callback next to the notice (`on_notice_shown`); the thread that
sends the notice (`spawn_osd_notice` in `src-tauri/src/playback.rs`) calls
it once, after its wait, after its check that the play has not been
replaced, and only if the write to the player succeeded. The play path
builds that callback only when the notice it launches IS the fallback
explanation, and the callback is what records the reason as told. The
success path after a launch now only forgets (a started play with nothing
to fall back from).

A replaced play therefore leaves the explanation due, and the play that
replaced it carries it. The thread's wait became a parameter so the tests
do not sleep for it.

## Files changed

- `src-tauri/src/playback.rs` — `PlaySpec::on_notice_shown`,
  `NoticeShown`, `OSD_NOTICE_DELAY`, `spawn_osd_notice` (delay and callback
  parameters); three tests named `a_notice_...`
- `src-tauri/src/commands.rs` — `play_by_key_locked` builds the callback
  and no longer records at launch
- `src-tauri/src/lib.rs` — the notice state is shared (`Arc`) so the
  callback can hold it
- `src-tauri/src/player/launch.rs`, `src-tauri/src/player/external.rs` —
  the new field in the two test helpers that build a `PlaySpec`
- `tests/player-selection.test.mjs` — the wiring test follows the new shape

## Guard proof

Run 2026-11-13 at `db0b8b6` in a disposable worktree; each injection restored
with `git checkout` from the commit. `cargo +stable test --locked --lib a_notice` (four tests) and
`node --test tests/player-selection.test.mjs`:

| Injection | Result |
| --- | --- |
| The original defect in the sender: reported shown before the wait | Rust FAIL: `a_notice_dropped_because_the_play_was_replaced_is_not_reported_shown` and `a_notice_the_player_did_not_take_is_not_reported_shown`; JS FAIL |
| Reported shown whether or not the write succeeded | Rust FAIL: `a_notice_the_player_did_not_take_is_not_reported_shown`; JS FAIL |
| The launch does not carry the callback | JS FAIL |
| `play` does not hand the callback to the sender | JS FAIL |
| The original defect in the play path: also recorded as told when the player starts | JS FAIL |
| Unchanged | PASS |

The fourth Rust test (`a_notice_the_player_did_not_take_...`) was added at
`db0b8b6`, after the first three: none of them made the write fail, and
the second row needs it.

## Coder dispute (if any)

None.

## Known gaps

- "Handed to the player" is a successful write of the `show-text` command
  to mpv's socket. Whether mpv then drew it is not observed.
- Two plays could both show the line if the second is planned in the
  instant between the first one's replaced-check and its record. The
  window is a few instructions wide; a session-scoped pending state, which
  the reviewer suggested, would close it and was judged not worth the
  state it adds for a repeated informational line.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Record successful notice delivery instead of successful process launch. Keep canceled or failed deliveries due, with session-scoped pending state to prevent duplicate delivery."
