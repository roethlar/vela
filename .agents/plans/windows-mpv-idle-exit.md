# Plan: mpv must exit at end of file on Windows (`--idle=no`)

## Status

**IMPLEMENTED 2026-11-12 at `bf1b1c3` on `main` (1.0.62), unpushed.
Windows acceptance steps 1 and 3 are NOT done. Awaiting owner acceptance.**

Owner go, conditional, verbatim: "you can do a review with agy Gemini 4
Argon (High) to see if anything looks off. if that's good, then go." The
review returned `best_approach` with no material changes
(`.agents/review/openreview-2026-11-12-windows-mpv-idle-exit-plan.md`), so the
condition was met.

## Problem

Owner report, 2026-11-12: on a Windows install (Vela 1.0.59), a TV episode
played from the library does not advance to the next episode when it ends.
Continue Playing was at its default (`only-tv`), which is specified to
advance (`.agents/decisions.md` 2026-07-14).

### Evidence

Collected 2026-11-12 over SSH on `netwatch-01` (`.agents/machines.md`), with
a two-second lavfi test clip, `--no-config --vo=null --ao=null`, no server
and no Vela. mpv was started with no console and no redirected standard
handles, which is how Vela starts it (below).

| Launch | Exited by itself |
|---|---|
| no extra option | no — still running after 10 s (2/2 runs; 5/5 in an earlier 12 s variant) |
| `--idle=no` | yes, after about 2.1 s, exit code 0 (2/2) |
| `--player-operation-mode=cplayer` | yes, after about 2.1 s (1/1) |
| standard output and error redirected to files | yes, after about 2.1 s (1/1) |

- Same result for both builds on that host: `v0.42.0-5-g55d3feb45` and
  `v0.41.0-1421-ga687796d9`.
- IPC is healthy. Every connected named-pipe client (a passive one, and one
  that first sent `observe_property`) received
  `{"event":"end-file","reason":"eof","playlist_entry_id":1}`, then
  `{"event":"video-reconfig"}`, then `{"event":"idle"}`. mpv then stayed
  alive, idle.
- Linux control on `continuum` (mpv 0.41.0, Unix socket, passive client):
  `end-file`/`eof` delivered and mpv exited in 30 of 30 runs.
- The affected install's `config.json` held one recents entry (an episode,
  started 2026-11-08 23:58:34Z) with `ended_at_ms` 0 and no view offset, and
  the file was not written again after that start. `recents::finish_session`
  either removes an entry (past the watched threshold) or stamps
  `ended_at_ms` whenever a session ends, so for that play Vela never ran its
  session-end path. Its settings: `continue_playing` unset (default),
  `mpv_use_own_config` false, `mpv_extra_args` `--no-border`.

### Mechanism

- `src-tauri/src/playback.rs`, `play`: the playback `Command` sets no
  `stdin`/`stdout`/`stderr`, so mpv inherits Vela's. Vela is a Windows GUI
  process with no console and no standard handles.
- Assumption (from mpv's Windows entry point, not re-read for this plan; the
  table above is the evidence that matters): when `mpv.exe` starts with no
  console and no redirected standard I/O it treats itself as launched from
  the desktop and enables its pseudo-GUI operation mode, whose profile turns
  idle mode on. With idle on, mpv does not quit when its playlist ends.
- `src-tauri/src/lib.rs`, the playback sequence dispatcher: advancement and
  `continue-playing` are driven only by "the joined clean-EOF and
  final-tracker signal". The EOF half arrives (the event is delivered). The
  tracker half ends when mpv exits or its IPC connection drops, which never
  happens while mpv idles.
- NOT yet observed directly: Vela's real launch on Windows taking the
  no-extra-option row. It is inferred from the spawn code. Slice 1's Windows
  acceptance step observes it.

Expected visible symptom on Windows today: after the file ends, the mpv
window stays open and blank instead of closing.

### Blast radius (assumption until checked in Slice 1, step 1)

Everything that waits for the session to end on Windows after a natural end
of file: Continue Playing, Vela playlist and server playlist advancement,
the final played-state write to the server, the recents/Continue Watching
update, and `playback-ended` refreshes. Closing mpv by hand ends the process
and is unaffected.

## Goal

mpv started by Vela exits when its file ends, on every platform and under
every Vela setting, so the existing clean-completion path runs on Windows as
it does on Linux and macOS. No change to that path itself.

## Non-goals

- No change to how completion is detected or joined (`lib.rs` dispatcher,
  `PlaybackAdvance`, trackers).
- Do NOT fix this by redirecting mpv's standard handles or by passing
  `--player-operation-mode=cplayer`. Both also switch off the rest of the
  pseudo-GUI profile on Windows (assumption: it includes `force-window` and
  terminal handling), which would change window behaviour Vela users
  already have. `--idle=no` changes only the defect.
- `--keep-open`: an expert who sets `keep-open=yes` (Advanced arguments or
  their own `mpv.conf`) also prevents mpv from ending the file. That is a
  deliberate expert setting, not this defect. Leave it alone; mention it in
  `docs/usage.md` only if that file already documents Advanced arguments'
  interaction with Continue Playing.
- mpv window size and position carry-over: separate request, separate plan.

## Slice 1 — pass `--idle=no` in the protected tail of the mpv command

File: `src-tauri/src/playback.rs`.

1. Before editing, confirm the blast-radius paragraph by reading where the
   position tracker's `done` flag and the `mark_eof` signal are joined
   (`PlaybackAdvance` in `commands.rs`, and the tracker functions that call
   `spawn_position_reader`). Record in the implementation log what actually
   blocks while mpv idles. If the join turns out NOT to depend on mpv
   exiting, stop and report: the mechanism above is then wrong.

2. Add next to `window_state_args`:

   ```rust
   /// Options Vela's session lifecycle depends on. Asserted after every
   /// user-controlled option so neither Advanced arguments nor the expert's
   /// own mpv.conf can leave mpv running after its file ends.
   ///
   /// Windows mpv started without a console or redirected standard handles
   /// (which is how a GUI app starts it) turns idle mode on and then never
   /// exits at end of file; Vela's completion path waits for that exit.
   fn lifecycle_args() -> [&'static str; 1] {
       ["--idle=no"]
   }
   ```

3. In `play`, apply it in the protected tail: after the user's Advanced
   arguments, the autocrop arguments, the marker arguments,
   `window_state_args` and the screen argument, and immediately before
   `cmd.arg(format!("--input-ipc-server={}", ipc_path))`:

   ```rust
   for arg in lifecycle_args() {
       cmd.arg(arg);
   }
   ```

   It applies on all platforms and in own-config mode
   (`mpv_use_own_config = true`) too: this is a lifecycle requirement, not a
   playback preference (contrast the 2026-08-20 audio-output ruling, which
   yields to own-config).

4. Check the two other mpv spawns in this file (`mpv_runs`, near line 340,
   and the one near line 2469). They set `Stdio::null()` and are probes, not
   playback; confirm and leave them unchanged.

Tests:

- U1 (Rust unit, `playback.rs` tests module): `lifecycle_args()` contains
  exactly `--idle=no`.
- W1 (wiring; node source-text guard in the style of
  `tests/transcoding-ui.test.mjs`, which already matches on Rust source —
  add to an existing test file that is already in the `package.json`
  `check` list, or add a new file to that list): within the body of
  `pub fn play(` in `src-tauri/src/playback.rs`, the text
  `for arg in lifecycle_args()` occurs exactly once, AFTER the last
  occurrence of each of `mpv_extra_args`/the Advanced-arguments loop,
  `window_state_args(` and `screen_name_arg(`, and BEFORE
  `--input-ipc-server=`. Locate the Advanced-arguments application inside
  `play` first and name its real identifier in the test; if Advanced
  arguments are applied inside `base_mpv_command` instead, assert that the
  `lifecycle_args` loop comes after the `base_mpv_command(` call.

Red-proofs, each separately, restoring with `git checkout -- <file>`:

- R1: change the constant to `--idle=yes` → U1 fails.
- R2: delete the loop from `play` → W1 fails (count 0). This is the wiring
  guard: U1 alone stays green.
- R3: move the loop above `window_state_args(` → W1 fails on ordering.
- R4: move the loop below `--input-ipc-server=` → W1 fails on ordering.

Version and notes: run `scripts/bump.sh` (next patch) and add a bullet under
`# Unreleased` / `## Fixes` in `RELEASE_NOTES.md`: on Windows, playback now
ends properly when a file finishes, so Continue Playing and playlists
advance and the watched state and resume position are saved.
`.agents/plans/server-identity-sidebar.md` also plans a bump; whichever
lands second takes the following patch number.

Commit: `fix: make mpv exit at end of file on Windows`.

## Verification before claiming completion

Backend change, so the full set from `.agents/repo-guidance.md`
(Verification): toolchain check, `npm ci`,
`npm audit --audit-level=moderate`, `npm run check`, `npm run build`, and
from `src-tauri/` the 1.89.0 MSRV check, stable check, clippy
`-D warnings`, `cargo +stable test --locked`, `cargo audit`. `continuum`
has no 1.89.0 toolchain as of 2026-11-12: run the MSRV check elsewhere or
rely on GitHub CI for it and say so.

Windows acceptance (the only real proof; nothing above exercises it):

1. On `netwatch-01`, build the installer (`pwsh scripts/build.ps1`; the C:
   install has the build tools) and run `cargo +stable test --locked` there
   so the Windows `cfg` paths compile and pass.
2. Re-run the launch probe with Vela's real argument tail to confirm the
   option survives the full command: start `mpv.exe` with no console and no
   redirected handles, `--no-config --vo=null --ao=null --idle=yes
   --idle=no` plus a two-second lavfi clip, and confirm it exits (last
   option wins even against an explicit `--idle=yes`).
3. Owner, on the affected Windows install: install the build, play an
   episode's last minute to its natural end with Continue Playing at "TV
   only". Expected: the mpv window closes, the next episode starts, the
   finished episode shows as watched, and the recents entry no longer has
   `ended_at_ms` 0. Then confirm that closing mpv by hand mid-episode still
   stops playback and saves the position.

Linux/macOS regression: `npm run e2e` scenarios `continueon`, `playverbs`,
`serverplaylists`, `playlistplay` cover completion and advancement and must
still pass. No E2E venue is ready as of 2026-11-12 (`.agents/machines.md`:
the VM is suspended with Node below the `engines` floor; `continuum` lacks
`tauri-driver` and `Xvfb`). If E2E cannot run, say so; do not claim it.
`--idle=no` is mpv's documented default, so no behaviour change is expected
off Windows.

## Paperwork on landing

- `.agents/state.md` "Now": one bullet with the commit id and exactly which
  verification ran where.
- `.agents/machines.md`: the Windows acceptance results.
- `ISSUES.md`: only if the owner wants the user-facing workaround recorded
  for releases before the fix (set Advanced mpv arguments to include
  `--idle=no`).
- This file: Status with the owner's go verbatim, and an implementation log
  with each red-proof result.

## Implementation log

2026-11-12, host `continuum` (CachyOS; Node 26.13.1, npm 12.2.2, Rust
stable 1.99.1).

- Slice 1 step 1 (mechanism check): confirmed by reading. Each tracker in
  `playback.rs` calls `on_end` only after its loop sees `done` or the stop
  flag; `done` is set by `spawn_position_reader` when its IPC read returns
  EOF or an error, i.e. when mpv exits. `on_end` (built in `commands.rs`
  near the `EndNotify` closure) is the only caller of
  `recents::finish_session`, the tracker-side `playback-ended` emit, and
  `PlaybackAdvance::mark_ended`. While mpv idles the pipe stays open, so
  none of them run. The blast-radius paragraph holds as written.
- Slice 1 steps 2-3: `lifecycle_args()` added beside `window_state_args`;
  applied in `play` after the screen argument and immediately before
  `--input-ipc-server`. Advanced arguments are applied inside
  `base_mpv_command`, so W1 anchors on the `base_mpv_command(` call.
- Step 4: the other two spawns (`mpv_runs`, and the one near the end of the
  file) set `Stdio::null()` and are not playback; unchanged.
- Tests: U1 is `playback::tests::lifecycle_args_make_mpv_exit_when_its_file_ends`.
  W1 is `tests/mpv-lifecycle.test.mjs` (added to the `check` list), plus a
  second test there asserting Advanced arguments still live in
  `base_mpv_command`, since W1's ordering depends on it.
- Red-proofs, each injected into the committed file and restored with
  `git checkout --`:
  - R1 (`--idle=yes`): U1 FAILED; W1 stayed green (it guards wiring only).
  - R2 (loop deleted): W1 failed, "play must apply lifecycle_args()"; U1
    alone would have stayed green.
  - R3 (loop above `window_state_args`): W1 failed, "lifecycle_args() must
    follow window_state_args(".
  - R4 (loop below the IPC option): W1 failed, "lifecycle_args() opens the
    protected tail, before the IPC option".
  - R5 (extra, Advanced arguments renamed out of `base_mpv_command`): the
    second test failed on `/mpv_extra_args/`.
- Verification on `continuum` at `bf1b1c3`: toolchain check; `npm ci`;
  `npm audit --audit-level=moderate` (exit 0); `npm run check` (80 pass, 0
  fail, 0 skipped); `npm run build`; `node --test tests/` (80 pass);
  `cargo +stable check --locked`; clippy `-D warnings`;
  `cargo +stable test --locked` (365 passed); `git diff --check`.
- NOT run: the 1.89.0 MSRV check (toolchain absent on this host) and
  `cargo audit` (not installed on this host) — both left to GitHub CI, which
  has not run because nothing is pushed. E2E (no venue). Any Windows build
  or Windows `cargo test`.
- Windows acceptance step 2, done over SSH on `netwatch-01` with both mpv
  builds (`v0.42.0-5-g55d3feb45`, `v0.41.0-1421-ga687796d9`), no console,
  no redirected handles, two-second lavfi clip:
  - user `--idle=yes` followed by `--idle=no`: exited by itself after about
    2.1 s, code 0 (both builds).
  - user `--idle=once` followed by `--idle=no`: same (both builds).
  - control, `--idle=yes` only: still running after 10 s (both builds).
  So the last option wins against an explicit user override, as the
  protected-tail placement requires.
- Windows acceptance step 1 (build and `cargo test` on `netwatch-01`) and
  step 3 (owner plays an episode to its end on the affected install) are
  open. Step 1 was deliberately not started: an owner per-core
  overclock-test script was logging sensors on that host, and a release
  build would load every core.- Pushed 2026-11-12 (`8bfd4f8`, after the owner set the push policy to
  `always`). GitHub CI run 43301071621: Frontend and cargo audit passed, and
  in the Rust job the 1.89 MSRV check and stable check passed; clippy then
  FAILED and the test step was skipped. The failure was not in this change:
  CI's stable had rolled to 1.100.0 (released 2026-11-09), whose clippy
  errors on a `#[allow(dead_code)]` written twice on
  `SourceQualityOptions::has_choice` in `src/source/mod.rs` since
  `e0e5fc7` (2026-07-25). Reproduced on `continuum` with a side-by-side
  1.100.0 toolchain; fixed in its own commit by deleting the duplicate
  line. Under 1.100.0, clippy `-D warnings`, check, and all 365 tests then
  pass, and clippy still passes under 1.99.1.
- Owner-directed codereview of the implementation (codex, gpt-6-astra @
  max) over `5f90b2a..8a0abbe`: no material issue
  (`.agents/review/codereview-2026-11-12-windows-mpv-idle-exit.md`).
