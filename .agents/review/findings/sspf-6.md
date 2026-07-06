# sspf-6: registry eviction frees an SMB context while holding the registry lock

**Severity**: MEDIUM — a blocking `smbc_free_context` teardown runs under the
process-wide proxy registry mutex, stalling every proxy register / lookup /
release for the duration of a network teardown (up to the SMB op timeout).
**Status**: Verified
**Commit**: `<pending>`

## Evidence
`src-tauri/src/stream_proxy.rs`, `register` eviction (sub-slice 3, head
`05ed86b`): `if reg.len() >= REGISTRY_CAP { reg.pop_front(); }` drops the evicted
`Entry` — and its cached `Session` — while the `reg` mutex guard is held. If that
`Session` owns the last `Arc<SmbConnection>`, `SmbConnection::drop` runs
`smbc_free_context(ctx, 1)` (a blocking network teardown under `ctx_lifecycle_lock`)
inline, so the registry mutex is held across blocking SMB I/O. This is the same
lock-across-blocking class already avoided in `release_session` and
`get_or_create_session`, missed at the eviction site.

## Predicted observable failure
When an eviction frees the last reference to a cached session, the session's drop
must run with the registry lock NOT held. A drop-time probe
(`registry().try_lock()`) succeeds only if the lock is free at drop. Before the
fix the probe sees the lock held; after the fix it sees it free.

## What
Eviction dropped the evicted entry (hence its context-owning session) inline while
the registry lock was held, violating the repo's no-lock-across-blocking invariant.

## Approach
In `register`, capture the evicted entry (`reg.pop_front()`) into a local, finish
the registry mutation, explicitly `drop(reg)` to release the lock, then
`drop(evicted)` so the context teardown runs off-lock — mirroring
`release_session`'s off-lock drop.

## Files changed
- `src-tauri/src/stream_proxy.rs` — `register` holds the evicted entry and drops
  it after releasing the registry lock.

## Guard proof
- `stream_proxy::tests::eviction_frees_the_session_off_the_registry_lock` — seeds a
  drop-probing session at the front of a full registry, registers one more to evict
  it, and asserts the probe observed the registry lock FREE at drop (and that it
  was dropped at all). Reverting to an inline `reg.pop_front();` makes it FAIL (the
  probe sees the lock held); restoring makes it PASS.

## Reviewer comments
- **r1** 2026-07-05 `codex` (codex-cli 0.142.5), `codex exec --json`, reviewed head
  `05ed86b3` base `21cd8909`, `guard_confirmed:false`, verdict **reopened**:
  "Evicting an entry now drops its cached Session while the proxy registry mutex is
  held; if the evicted Session owns the last Arc<SmbConnection>, SmbConnection::drop
  runs smbc_free_context under that mutex, stalling all proxy register/lookup/release
  paths during blocking SMB teardown — MEDIUM". Admitted. Fix below; re-review
  pending.
