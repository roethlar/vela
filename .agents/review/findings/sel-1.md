# sel-1: a settings file that cannot be read fails a play after its session is installed, and nothing is undone

**Severity**: MEDIUM — a failed play leaves the new session installed, the playlist position moved and, for a transcoded stream, an encoder running on the user's server with nothing left to stop it. It needs the settings file to become unreadable in the middle of a play start, which is rare.
**Status**: Verified
**Branch**: —
**Commit**: `88c631b`, `51652d9`, `320b023`

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:5044 propagates a settings error after installing the session at :4939 and registering its transcode at :5022. Reproduced by invalidating settings during stream resolution with start_from_beginning=true: the command returned an error, but the new session remained active, the player record still named the old session, and no transcode-stop call occurred. Removing only the new read restored cleanup."

## Predicted observable failure

Reviewer, verbatim: "A failed play leaves inconsistent playback state and skips the established transcode teardown and cursor restoration. This is a MEDIUM failure-path defect."

## What

`play_by_key_locked` commits a play in stages: it resolves the stream,
installs the new session and playlist position, stops the previous player,
registers the transcode it may have started, and only then starts the
player. One arm at the end of the function undoes all of that when the
player does not start. Slice 2 added a settings read, with an early return
on failure, in the middle of those stages. That return skipped the arm.

## Approach

The read moved to where the function reads its other settings, before the
stream is resolved (next to `SkipPolicies::load()`). A failure there ends
the play while nothing has been resolved, installed or registered, which
is what already happens for the settings read beside it. The choice
between the players stays where it was; it has no way to fail.

The reviewer's other suggestion, sending the error through the cleanup
arm, would have kept a fallible step in the stretch where every fallible
step is a hazard.

## Files changed

- `src-tauri/src/commands.rs` — `play_by_key_locked`: `wanted_player` is
  read before `let resolved = ...`
- `tests/player-selection.test.mjs` — new test "once a play has installed
  its session, only the launch-failure path ends it early"

## Guard proof

Run 2026-11-13 at `db0b8b6` in a disposable worktree; each injection restored
with `git checkout` from the commit. `node --test tests/player-selection.test.mjs`, test "once a play
has installed its session, only the launch-failure path ends it early":

| Injection | Result |
| --- | --- |
| A second settings read with `?`, placed after the session is installed | FAIL (the list of early returns has two lines) |
| The original defect: the player-setting read back where Slice 2 put it | FAIL |
| A plain `return Err(..)` after the session is installed | FAIL |
| Unchanged | PASS |

## Coder dispute (if any)

None.

## Known gaps

- The guard is a source-text test, like the other guards on this
  function's ordering: nothing in the repo can drive `play_by_key_locked`
  with a settings file that fails on demand. It lists every early return
  between the installation and the cleanup arm and requires the list to be
  exactly one known line.
- That one line is the same defect in older code (the settings read for
  Vela's own resume position). It is outside the reviewed range and the
  approved plan, so it is listed, not fixed: `.agents/state.md`, Blockers.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-selection.md`.
Reviewer's suggested approach, verbatim: "Read the player setting before stream resolution and provisional session installation, or route this error through the existing launch-failure cleanup."

### Verification round 1, recorded 2026-11-13 07:14 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly; the host's codex config
  resolves to gpt-6-astra @ max. Thread `01a1d3b6-a811-7470-b193-4cdbbf92144f`, run
  in the disposable worktree `~/dev/vela-probe-wt` at the head SHA; the
  stream ends in `turn.completed`; dispatched 07:02 UTC, finished
  07:07 UTC. Routing: T1 no sensitive path in the diff, T2 severity
  below HIGH, T3 the recorded proof re-run by the orchestrator agrees.
- Reviewed head `88c631b719e108032decbe5aa7c1a403f1555271`, base
  `e3b288eaddb72c43cba55ee96c4160f341c6e199` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/player-selection.test.mjs:65 — The guard misses `?` followed by a comma. Immediately before `let player_choice`, I injected a settings reread using `drop(config::load_config().map_err(...)?,);`. This reproduces the cleanup-skipping return after session/transcode installation, yet all 6 Node tests and 382 Rust tests passed. Cover this equivalent regression. Reverting the original fix did fail the named guard; restoring the pinned head passed. All mutations were restored.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. The comment is correct: the fix stands, and the test that guards
it recognises an early return only when `?` is followed by `;`, a line end
or `.`.

### Repair (round 2)

The fix is unchanged. The test now lists every line between the
installation and the cleanup arm that can leave early, however it is
written: the `?` operator wherever it stands, a `return`, `.unwrap()`,
`.expect(`, and the panicking macros. String and character literals are
emptied first, so a `?` in a message is not counted. The list must be
exactly two lines: the known older settings read, and a `return;` inside
the end-of-play callback, which leaves that callback on the tracker's
thread and cannot leave this function.

File: `tests/player-selection.test.mjs` only.

Guard proof, run 2026-11-13 against `4459b40` with the repaired test, in a
disposable worktree; each line injected after the session is installed and
restored with `git checkout`:

| Injection | Result |
| --- | --- |
| The reviewer's: `drop(x?,);` | FAIL |
| `?` inside a call's arguments | FAIL |
| `?.` before a field | FAIL |
| `?` at a line end | FAIL |
| `.unwrap()` | FAIL |
| `.expect(..)` | FAIL |
| `panic!` | FAIL |
| `unreachable!` | FAIL |
| `return Err(..)` | FAIL |
| The original defect: the player-setting read back after the installation | FAIL |
| A question mark inside a string only | PASS, rightly: it is not code |
| A `'?'` character literal only | PASS, rightly |
| Unchanged | PASS |

### Verification round 2 (repair-delta redispatch), recorded 2026-11-13 07:41 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d3d1-01d2-7ba0-bb30-7e6564499012`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 07:30 UTC, finished 07:36 UTC.
- Reviewed head `51652d9e287d90f066ab13eb267ec5dd179162b3`, base
  `88c631b719e108032decbe5aa7c1a403f1555271` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/player-selection.test.mjs:67 — Literal stripping hides a real propagation: injecting `drop(('"', config::load_config().map_err(|e| PlayFailure::fatal(e.to_string()))?, '"'));` before `let player_choice` passes all six tests and compiles offline. The regex treats the double quotes inside character literals as string delimiters, erasing the intervening `?`. A failed read still skips cleanup.
  - tests/player-selection.test.mjs:68 — The matcher also misses error propagation through `r#try!(...)`, `.expect (...)` with whitespace, and `assert!(config::load_config().is_ok(), "guard probe")`. Injected before player selection, each passes all six tests; offline compilation also succeeds. These permit returning or panicking after transcode registration without cleanup.
  - tests/player-selection.test.mjs:73 — The exemption checks text without its context. Moving the unchanged `local_resume_ms` and `resume_ms` calculations after transcode registration passes all six tests and compiles, although the exempt settings failure now strands a registered transcode. Bind the exemption to its original location and branch. The original comma-form mutation did fail the named guard; restoration passed. All mutations were restored.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. All three comments are correct, and together they say one
thing: a test that reads the source cannot list every way Rust code can
leave a function early, and an exemption written as text cannot know
where its line stands. Patching the list again would invite a fourth
round on the same ground (as happened with ps-2). The next repair changes
the code instead, so that leaving early past the cleanup cannot be
written.

### Repair (round 3)

The code changed this time, not the test's list.

`play_by_key_locked` commits a play in stages and has one arm that undoes
them. Everything between the two used to be plain statements of the
function, so anything among them that could fail could also leave the
function, past the arm. Now the steps that start the play (stop the
previous player, register the transcode, describe and start the launch)
are written as ONE block, `launch`, placed before the installation and
run by a single line after it:

```rust
let launch = async { /* ...the steps... */ played };
// ...
let previous_cursor = { /* swap the playlist position in */ };
install_playback_session(state, session_id, item, run_kind, next_affinity).await;
let launched: Result<Arc<AtomicBool>, String> = launch.await;
let stop = match launched { Ok(stop) => stop, Err(message) => { /* undo */ } };
```

Inside the block a `?` or a `return` leaves the block, with a message the
arm receives. It cannot leave the function: that is how the language
works, not something a test has to notice. Between the installation and
the arm there is one line, and it cannot fail.

This answers the three comments in kind. (1) and (2): there is no list of
early exits any more, so nothing to slip past it; `?`, `r#try!` and
`return` in any spelling end in the arm. `.expect`, `assert!` and the
like panic, and a panic is not an early return here: release builds are
built with `panic = "abort"` (`src-tauri/Cargo.toml`), so the process is
gone, not left half-started. (3): there is no exempt line. The older
settings read that used to leave the function from this stretch (noted
under Blockers in `.agents/state.md` when round 1 found it) is inside the
block like everything else, so its failure now reaches the arm too. That
closes that older defect as a consequence; the Blockers entry is removed.
What it changes for a user: a play that fails because the settings file
cannot be read no longer leaves the previous play's session and playlist
position replaced by one that never started.

The test (`tests/player-selection.test.mjs`) holds the shape with exact
text and nothing that reads Rust: the statements from the playlist-position
swap to the arm are exactly six lines; `launch` is written once, before
them, and run once, by the last of them; and the arm is exactly its
sixteen statements.

Files: `src-tauri/src/commands.rs`, `tests/player-selection.test.mjs`
(the finding's declared two).

Verified before the guard proof: clippy on stable and the 1.89 check, 418
Rust tests, 127 node tests, and the full E2E suite in both venues, since
every play goes through this function.

Guard proof for round 3, run 2026-11-13 at `320b023` in a disposable
worktree, each injection restored with `git checkout`;
`node --test tests/player-selection.test.mjs`, and `cargo check` where
noted, to show the injected code is real Rust that builds:

| Injection | Test | Builds |
| --- | --- | --- |
| The original defect: a settings read with `?` between the installation and the launch | FAIL | yes |
| The reviewer's round-2 form, a `?` between two quote characters, same place | FAIL | yes |
| The reviewer's `assert!(...)`, same place | FAIL | |
| The reviewer's `.expect (` with spaces, same place | FAIL | |
| A fallible step directly before the launch is run | FAIL | yes |
| The arm no longer clears the session | FAIL | |
| A `?` inside the arm, before the undoing | FAIL | yes |
| The launch run before the installation | FAIL | |
| Another block run before the installation | PASS, rightly: nothing is installed yet | |
| A `?` inside the block | PASS, rightly, and builds: it can only reach the arm | yes |
| Unchanged | PASS | yes |

### Verification round 3 (repair-delta redispatch), recorded 2026-11-13 08:20 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d3f4-9f96-7651-befe-d5e6ba116f72`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 08:09 UTC, finished 08:14 UTC.
- Reviewed head `320b023364d0d9b5a117a57613068fa26cf76c14`, base
  `51652d9e287d90f066ab13eb267ec5dd179162b3` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
