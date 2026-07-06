# sspf-4: the write deadline breaks a normal long mpv pause

**Severity**: MEDIUM/HIGH — pausing playback longer than the deadline breaks the
stream on resume (premature EOF), where before the slice the same pause resumed
fine; pausing is a completely normal user action
**Status**: Verified
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: `8f41b90`

## Evidence
`src-tauri/src/stream_proxy.rs` `serve_target` now sets a 30s write deadline on
the proxy socket. When mpv pauses, its demuxer cache stops draining; once the
socket buffers fill, the deadline closes the connection mid-body. Vela targets
system mpv 0.38+ and does not enable ffmpeg HTTP reconnect, which defaults off,
so a resumed pause reads a closed connection as a premature EOF rather than
issuing a fresh Range. A pause is a sequential continuation, not a seek, so mpv
emits no new Range on its own. Filed by codex on review round 1 of sub-slice 2
(base `5c50044`, head `d45ffe3`).

## Predicted observable failure
Play an SMB file, pause for longer than (buffer-fill + 30s), resume: playback
errors / stops at a premature EOF instead of continuing. Before this slice the
connection stayed open (blocked write) and the same pause resumed normally.

## What
A wall-clock write deadline cannot tell a paused-but-healthy client from a dead
one, and dropping a paused connection is unsafe when the player will not
re-request on its own.

## Approach
Make a dropped proxy stream recoverable, then make the deadline generous:
- `playback::proxy_reconnect_args(url)` enables ffmpeg reconnect
  (`--stream-lavf-o-append=reconnect=1 / reconnect_streamed=1 /
  reconnect_delay_max=5`) for the loopback proxy URL only, asserted AFTER the
  user's extra args (load-bearing, like the IPC socket) so it can't be
  clobbered. A dropped proxy stream now transparently reopens with a `Range` at
  the current offset and continues. Scoped to `http://127.0.0.1:` — inert for
  anything but our own deliberately-closing proxy (loopback never spontaneously
  drops mid-stream).
- Raise the deadline default 30s → 300s (`DEFAULT_WRITE_TIMEOUT_MS`) so it sits
  past any normal pause and acts as a resource backstop, not a pause-killer.
  Dropping a paused stream early would also force a fresh SMB session on resume,
  re-incurring the very per-seek session cost Bug 1 removes; the generous
  backstop avoids that while reconnect keeps any drop safe.

## Files changed
- `src-tauri/src/playback.rs` — `proxy_reconnect_args` + injection after user
  args; unit test.
- `src-tauri/src/stream_proxy.rs` — `DEFAULT_WRITE_TIMEOUT_MS` = 300_000; test
  restore uses the const.

## Guard proof
`playback::tests::proxy_reconnect_only_for_the_loopback_proxy`: the loopback
proxy URL yields the three reconnect args; server/local/edl URLs yield none.
Executed 2026-07-05: red — with the loopback branch neutered the loopback URL
returns `[]` (assertion FAIL); green — restored. The end-to-end
pause>deadline-resumes behavior needs mpv + a live connection drop, so it is an
owner playtest; only arg emission is unit-tested (the repo's established
autocrop-args pattern).

## Coder dispute (if any)
None — admitted. It is the pause/reconnect tension the coder considered during
design and wrongly dismissed ("mpv reconnects like a seek"); codex correctly
distinguished a sequential pause from an explicit seek.

## Known gaps
Behavioral confirmation (pause past the deadline resumes seamlessly) is an owner
NAS playtest. Until sub-slice 3's session reuse lands, a drop-and-reconnect
rebuilds the SMB session; the 300s backstop keeps that rare.

## Reviewer comments
codex (codex-cli 0.142.5), `-s read-only`, JSON mode. Round 1 (2026-07-05):
**reopened** on this finding, reviewed head `d45ffe39`, base `5c50044`,
`guard_confirmed: true`. Round 2: **accepted**, reviewed head `8f41b907`, base
`5c50044`, `guard_confirmed: true`, no comments.
