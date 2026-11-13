# ec-8: a failed embedded start leaves the marker payload file behind

**Severity**: LOW — a small file of marker times (no credentials) stays in the runtime directory until a later play with markers clears it.
**Status**: In progress
**Branch**: —
**Commit**: `e66a928`

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/host.rs:346-351 discards LaunchPlan.marker_payload during destructuring, and the Session::start error path never calls discard_marker_payload. An EGL failure before mpv initialization occurs before the marker script can consume and unlink the file."

## Predicted observable failure

Reviewer, verbatim: "The marker JSON survives the failed play and application exit, unless a later marker-bearing launch happens to prune it."

Reviewer's suggested direction, verbatim: "Keep a cleanup guard for the marker payload through startup and remove it on every failure before the script consumes it."

## What

A launch with skip markers writes the marker ranges to a small file, names it in a script option, and relies on mpv's script to read it once and delete it. The external launcher deletes the file itself when it cannot start mpv. The embedded launcher (`Prepared::start`) took the launch plan apart with `..`, which dropped the file's path, and no failure path of an embedded start deleted the file. A start that failed before mpv existed (no usable OpenGL format, say) left it behind.

## Approach

The path now travels with the play's start request as `MarkerPayload`, an owner that deletes the file when it is dropped. The request is owned by the play's thread, so the file goes when the start has failed or when the play has ended, whichever happens, and whether or not the script ever ran; deleting a file the script already deleted is harmless. The plan is taken apart by naming every part, so a part added to `LaunchPlan` later has to be given an owner before `host.rs` compiles.

## Files changed

- `src-tauri/src/player/embedded/session.rs` — `MarkerPayload`, `Start::_marker_payload`; one test.
- `src-tauri/src/player/embedded/host.rs` — `Prepared::start` names every part of the plan and hands the payload on.
- `tests/embedded-player.test.mjs` — one wiring test.

## Guard proof

Run 2026-11-13 at `e66a928` in a disposable worktree; `cargo +stable test --locked --lib embedded::session` and `node --test tests/embedded-player.test.mjs`.

| Injection | Rust | Node |
| --- | --- | --- |
| The original defect: the plan taken apart with `..`, the payload's path dropped | (not run) | FAIL `everything a launch wrote to disk has an owner in an embedded play` |
| The payload is not given to the start request | | FAIL |
| The owner does not delete the file | FAIL `a_marker_payload_goes_when_what_holds_it_goes` | FAIL |
| The owner is made and then forgotten | | FAIL |
| Unchanged | PASS | PASS |

Also run: the `markers`, `markerpayload` and `embeddedplay` E2E scenarios under the headless compositor in the embedded player: PASS (the script still gets its file).

## Coder dispute (if any)

## Known gaps

- No test starts a real embedded play that fails; a play cannot be started in a unit test. That the request is dropped on every failure path is by construction (it is moved into the play's thread and has no other owner), and is held by source text.
- A start that never returns (finding ec-2) keeps its request, and so its payload, for as long as it is stuck.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "Failed embedded starts abandon marker payload files". Reviewer's severity: LOW.
