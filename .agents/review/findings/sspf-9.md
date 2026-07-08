# sspf-9: the on-failure session release ran a blocking free on an async worker

**Severity**: MEDIUM — the sspf-8 release ran on the `play_by_key` async task; when
the cached session is the registry's last `Arc`, its drop runs a blocking
`smbc_free_context` on a Tokio worker, so a failed SMB play can stall unrelated
async commands / UI backend work until the SMB teardown returns (up to the SMB op
timeout).
**Status**: Verified
**Commit**: `ab3f74c`

## Evidence
`src-tauri/src/commands.rs`, `play_by_key` (after the sspf-8 fix, head `ada9f65`).
The sspf-8 fix called `release_proxy_session(session_key)` directly in
`finish_play`'s closure, which runs on the async `play_by_key` task. If the freed
session is the last `Arc<SmbConnection>`, `SmbConnection::drop` runs the blocking
`smbc_free_context(ctx, 1)` inline on that async worker — violating the repo
invariant "do not hold async runtime workers across blocking OS/network work". The
proxy internals already drop freed/evicted sessions off the *registry lock*, but
sspf-8 reintroduced the block on the *async worker*.

## Predicted observable failure
On a failed SMB replay whose cached session is the last reference, the freeing
`smbc_free_context` must not run on an async runtime worker — it belongs on the
blocking pool. Before the fix it ran on the async worker, so a slow teardown
stalled other async commands.

## What
The on-failure release was correct in *what* it freed but wrong in *where*: it ran
the blocking context teardown on a Tauri/Tokio worker instead of the blocking pool.

## Approach
Perform the on-failure release on the blocking pool. `play_by_key` now inlines the
failure handling and wraps the release in `tauri::async_runtime::spawn_blocking(…)
.await` — the repo's standard pattern for blocking OS/network work (used at ~10
other call sites in `commands.rs`). The `finish_play` helper added by sspf-8 (a
sync closure that could not `await`) was removed. On success the `on_end` owner
still frees the session; on failure this awaited blocking release does, and it is
generation-guarded so it is a no-op if a newer play has since reused the token.

## Files changed
- `src-tauri/src/commands.rs` — removed `finish_play`; `play_by_key` inlines the
  on-failure release inside an awaited `spawn_blocking`.

## Guard proof
Structural fix (relocating blocking work to the blocking pool), verified by
inspection — the same way the repo's other ~10 `spawn_blocking` call sites and the
slice-7 async-worker fix (`e7c5231`) are verified; thread placement is not
hermetically unit-testable without runtime instrumentation. The release's
functional behavior is unchanged and remains covered: `release_session` frees a
reactivated session (`a_replays_late_end_does_not_free_the_new_plays_session`) and
frees off the registry lock (`eviction_frees_the_session_off_the_registry_lock`,
same off-lock drop discipline). Full suite green (98) + clippy `-D warnings` clean.

## Reviewer comments
- **r4** 2026-07-05 `codex` (codex-cli 0.142.5), `codex exec --json`, reviewed head
  `ada9f65` base `21cd8909`, `guard_confirmed:true`, verdict **reopened**:
  "On a failed SMB replay where the cached SmbConnection is the registry's last
  Arc, finish_play releases it on the async play_by_key task; that inline drop runs
  blocking smbc_free_context on a Tauri/Tokio worker, so the failed play can stall
  unrelated async commands/UI backend work until SMB teardown returns — MEDIUM".
  Admitted. Fix below; re-review pending.
