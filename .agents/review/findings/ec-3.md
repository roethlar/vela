# ec-3: the settings read at start-up waits forever on a settings path that is not a regular file

**Severity**: MEDIUM — Vela never opens its window, not even its recovery screen, and this is on every Linux start whether or not the embedded player was chosen. It needs a settings path that is a pipe, a device or a link to one.
**Status**: In progress
**Branch**: —
**Commit**: `d804fc8`, `867a694`, `d66bb7b`

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/startup.rs:138-146 unconditionally calls read_to_string before GTK or durable::load. With config.json replaced by a FIFO, a review subprocess blocked here; an ordinary external-settings file returned immediately. The previous storage path rejects nonregular files in storage.rs:62-72."

## Predicted observable failure

Reviewer, verbatim: "Vela never opens its window or recovery UI for this invalid settings path, even when embedded playback was never selected. A symlink to an unbounded device also bypasses the previous rejection."

Reviewer's suggested direction, verbatim: "Preserve the storage loader's regular-file and symlink checks before reading the startup setting. Bound this preliminary read and treat rejected input as ordinary window mode."

## What

To decide how the window is created, `startup::prepare` reads the `player` setting straight from the settings file, first thing in `main`, on every Linux start. It did so with `std::fs::read_to_string`. A settings path that is a pipe makes that call wait until something writes to the pipe; a link to a device such as `/dev/zero` makes it read without end. Either way the process never reaches the window, so the recovery screen the settings loader shows for such a path never appears. The loader itself has always refused anything that is not a regular file. This is the one finding of the review that reaches users who never chose the embedded player, because the read is what finds out whether they did.

## Approach

`settings_text` (new, `startup.rs`) opens the path with `O_NONBLOCK | O_NOFOLLOW`, asks the open file what it is, and gives up unless it is a regular file: the open cannot wait on a pipe, cannot follow a link, and the check is made on the file that was opened, not on the path, so nothing can be swapped in between. What it opened is read by `bounded_text`, which takes at most the limit (8 MiB) plus one byte from its source and returns nothing when there was more. Every refusal is "no setting", which is today's start; the loader then meets the same path and handles it as it always has. Nothing is changed on disk (the loader tightens permissions when it reads; this does not).

## Files changed

- `src-tauri/src/player/embedded/startup.rs` — `SETTINGS_READ_LIMIT`, `settings_text`, `bounded_text`; `saved_player_setting` uses them; four tests.
- `tests/embedded-player.test.mjs` — the first test also holds that start-up reads the file one way only.

## Guard proof

Run 2026-11-13 at `867a694` in a disposable worktree, each injection restored with `git checkout`; `cargo +stable test --locked --lib startup` and `node --test tests/embedded-player.test.mjs`.

| Injection | Rust | Node |
| --- | --- | --- |
| The original defect: the setting is read with `std::fs::read_to_string` again | (not run) | FAIL `every start is today's unless embedded window mode was decided` |
| The open waits (`O_NONBLOCK` removed) | FAIL `a_settings_path_that_is_not_a_regular_file_is_not_read_and_not_waited_for` (the two-second limit) | FAIL |
| The open follows a link (`O_NOFOLLOW` removed) | FAIL, same test | FAIL |
| What was opened is not checked to be a regular file | FAIL, same test | FAIL |
| The read has no limit | FAIL `no_more_than_one_byte_past_the_limit_is_ever_taken` | FAIL |
| The file is read without the bounded reader | FAIL `a_settings_file_is_read_up_to_the_limit_and_not_beyond` | FAIL |
| An over-long file is returned cut short | FAIL, both limit tests | |
| A file exactly at the limit is refused | FAIL `a_settings_file_is_read_up_to_the_limit_and_not_beyond` | |
| A second read appears in the start-up module | | FAIL |
| Unchanged | PASS (12) | PASS |

The first version of the fix (`d804fc8`) had one guard that did not bite: with the limit removed every test still passed, because an over-long regular file is refused either way, only after being read whole. `867a694` separates the bounded read and tests how much it takes from a source that never ends.

## Coder dispute (if any)

## Known gaps

- The marker file (`embedded-window-starting`) is only tested for existence with `Path::exists`, which does not open it; a marker path that is a pipe or a link cannot make a start wait. Not changed.
- A settings file larger than 8 MiB now means "no setting" for this one read. Vela's own are a few kilobytes.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "The new startup read bypasses nonregular-file protection". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-13 08:49 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d40e-a3bc-7f42-b99b-6047d5fbb660`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 08:38 UTC, finished 08:46 UTC.
- Reviewed head `867a694837b09f5ffcbc708bc8acd916b65752a8`, base
  `5ef4b5366a9346667930be77b123a2ef59196cfe` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/player/embedded/startup.rs:158 — Nonregular paths are rejected only after opening them. Probing the exact reader against a terminal device in a fresh session acquired a controlling terminal; its subsequent hangup terminated the process with SIGHUP. Checking the type before opening prevented both effects. A FIFO with a waiting writer also produced EPIPE when this reader opened and immediately closed it. These effects occur before player selection, including for external-player users. Reject nonregular objects without a data open, retain protection against path replacement, and guard these cases. The recorded guards failed under injection and passed after restoration, but do not cover this remaining defect.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. The comment is correct. The fix opened the path for reading and
only then asked what it had opened; for a regular file that is free, but
for other things the open itself does something: a terminal becomes the
process's controlling terminal, a writer waiting on a pipe is woken and
then finds nobody reading. The repair must find out what is at the path
without opening it for reading, and still read the file it looked at
rather than whatever is at the path a moment later.

### Repair (round 2), `d66bb7b`

`settings_text` is now two steps, each a function. `regular_file_at` takes
a handle on whatever is at the path without opening it (`O_PATH`) and
without following a link, asks the handle what it is, and returns it only
for a regular file. `text_of` is given that handle and no path: it opens
the same file through the handle (`/proc/self/fd/N`) and reads it with the
limit as before. Nothing but a regular file is ever opened for reading, so
nothing at the other end of a pipe or a terminal is touched; and what is
read is the file that was looked at, whatever has been put at the path in
between. If `/proc` is not there the reopen fails, which is "no setting":
today's start.

Files: `src-tauri/src/player/embedded/startup.rs`,
`tests/embedded-player.test.mjs` (the finding's declared two).

Guard proof for round 2, run 2026-11-13 at `d66bb7b` in a disposable
worktree; `cargo +stable test --locked --lib startup` and
`node --test tests/embedded-player.test.mjs`:

| Injection | Rust | Node |
| --- | --- | --- |
| The original defect: a plain read of the path | FAIL, three tests | FAIL |
| The first repair, which the reviewer reopened: the path opened for reading, then checked | FAIL `looking_at_a_settings_path_does_not_wake_a_writer_waiting_on_a_pipe` | FAIL |
| The first open is a real open (waits on a pipe) | FAIL, that test and the two-second limit | FAIL |
| The first open is a real open that does not wait (wakes the writer) | FAIL, that test | FAIL |
| The handle follows a link | FAIL `a_settings_path_that_is_not_a_regular_file_is_not_read_and_not_waited_for` | FAIL |
| The handle is not asked what it is | FAIL `the_file_that_was_checked_is_the_file_that_is_read` and the writer test | FAIL |
| What was opened through the handle is not checked a second time | PASS: it is a second check of something the first already established; only the source-text test holds it | FAIL |
| The read has no limit | FAIL `no_more_than_one_byte_past_the_limit_is_ever_taken` | FAIL |
| An over-long file is returned cut short | FAIL, both limit tests | |
| A file exactly at the limit is refused | FAIL | |
| A second read appears in the start-up module | | FAIL |
| Unchanged | PASS (14) | PASS |

Not proven by a test: the terminal case the reviewer demonstrated (a
terminal opened by a process with none becomes its controlling terminal).
It needs a process in a session of its own. It is closed by the same
means as the pipe case, which is tested: nothing but a regular file is
opened for reading. The reopen also passes `O_NOCTTY`.

