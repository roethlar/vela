# sspf-8: a play() failure after a same-file replay leaks the reactivated session

**Severity**: MEDIUM — on a same-file replay, `register` reactivates the token and
keeps the cached `SmbConnection`; if `playback::play` then fails before installing
the new `on_end` owner, nothing releases the new generation, so the session stays
active/servable until eviction/app exit.
**Status**: Verified
**Commit**: `<pending>`

## Evidence
`src-tauri/src/commands.rs`, `play_by_key` (after the sspf-7 fix, head `dec0121`).
A same-file replay: `resolve_stream` → `register_smb` reuses the token, bumps the
generation to g1, sets `active = true`, and KEEPS the cached session S1. The prior
player is then killed; its `on_end` fires `release_session(token, g0)`, which is a
no-op (g0 ≠ g1). If `playback::play(...)` now returns `Err` (e.g. `shutting_down`,
mpv missing, or tracker-thread exhaustion) it returns before installing the new
`on_end` owner that would fire `release_session(token, g1)`. So g1 is never
released: S1 stays `active = true` with the session cached until eviction.

## Predicted observable failure
When `play()` fails, the play's reserved proxy session must be released; when it
succeeds, it must NOT be released here (the `on_end` owner will). Before the fix a
failed play released nothing.

## What
`play_by_key` freed the proxy session only via the `on_end` owner, which `play()`
installs only on success. A failed play after a replay reactivation therefore left
the kept session ownerless and active.

## Approach
Route `play()`'s result through a small generic helper, `finish_play(result,
release_on_failure)`, that runs the release action iff the result is `Err`. In
`play_by_key` the release action frees the play's snapshotted proxy `session_key`
(the same `(token, generation)` the `on_end` owner would have released). On success
the release does not run, so the `on_end` owner remains the sole releaser (no
double-free). This also clears the stale `active` flag on a failed non-replay play
(where the session was never cached).

## Files changed
- `src-tauri/src/commands.rs` — `finish_play` helper; `play_by_key` routes the
  `play()` result through it, releasing `session_key` on failure.

## Guard proof
Originally guarded by `commands::tests::finish_play_releases_the_session_only_on_failure`
(asserted the release ran on `Err`, not `Ok`). **Refined by sspf-9**: that release
had to move onto the blocking pool (its drop runs a blocking `smbc_free_context`),
so the sync `finish_play` helper was removed and the on-failure release is now an
inlined awaited `spawn_blocking` in `play_by_key` (that test was removed with it —
see sspf-9). The release *mechanism* — that `release_session` frees a
replay-reactivated session — remains covered by
`stream_proxy::tests::a_replays_late_end_does_not_free_the_new_plays_session`. The
end-to-end `play_by_key` failure path is not unit-driven: `AppState` is built only
in `lib.rs` and there is no async-command/mpv test harness; the e2e harness is
where an integration test would live.

## Reviewer comments
- **r3** 2026-07-05 `codex` (codex-cli 0.142.5), `codex exec --json`, reviewed head
  `dec0121` base `21cd8909`, `guard_confirmed:false`, verdict **reopened**:
  "A same-file replay reactivates the token and keeps the cached session before the
  old player is killed; if `playback::play` then returns `Err` before installing
  the new `on_end` owner, the old `on_end` is generation-mismatched and the new
  generation is never released, leaving the cached `SmbConnection` active/servable
  until eviction — MEDIUM". Admitted. Fix below; re-review pending.
