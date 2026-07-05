# smb-1: Native SMB client wrapper; share add/browse without OS mounts

**Severity**: — (planned slice 1 of `.agents/plans/smb-native-client.md`, not a defect)
**Status**: Verified (accepted by reviewer; awaiting owner-gated merge)
**Branch**: `smb-native` (stacked slices; this is commit 1)
**Commit**: `fde07aae0efeef6f1449ee0217a2be0987d072f9` (base `21e950cd297a92682563eba94a9e5b4234318c97`)

## Evidence
Approved plan slice 1. Owner-observed failure driving the plan: adding
`smb://10.1.10.206/media/movies` on Arch/KDE fails with the gvfs/kio-fuse
error because `smb.rs` only resolves desktop FUSE mounts.

## Predicted observable failure
Before: `mount_smb` on a Linux box with no gvfs and no KIO mapping always
errors; no share can be added. After: `mount_smb` verifies the share
natively over libsmbclient and `list_smb_directories` browses it with no
mount present.

## What
First slice of the native-SMB feature: an in-process libsmbclient client and
the add/browse flow switched to it on Linux-family targets.

## Approach
`src-tauri/src/smb_client.rs` wraps raw `pavao-sys` bindings with one
`SMBCCTX` per `SmbConnection`, each behind its own mutex; context
create/free additionally serializes on a process-wide lock. The safe
`pavao` crate was rejected after code reading: it holds one process-global
context — a second client silently reuses the first's credentials, and its
`Drop` frees the context under other live clients (use-after-free risk).
Credentials reach libsmbclient through the auth callback from a registry
keyed by context pointer; they never appear in URLs, logs, or errors
(`friendly_error` maps errno to fixed messages). `NoAutoAnonymousLogin` is
set when a username is present so bad passwords fail loudly instead of
downgrading to guest. `commands.rs`: `mount_smb` calls the new
`establish_smb_share` (Linux: native verify, `mountpoint` stays empty as
the native marker; macOS/Windows: unchanged OS mount);
`list_smb_directories` lists natively on Linux. Dead Linux
`prepare_mount`/`default_mountpoint` removed from `smb.rs` (clippy
`-D warnings` gate).

## Files changed
- `src-tauri/src/smb_client.rs` — new module (client, auth callback, URL
  builder, error mapping, unit + env-gated live tests)
- `src-tauri/src/commands.rs` — `establish_smb_share`; native
  `list_smb_directories` path; empty-mountpoint guards
- `src-tauri/src/smb.rs` — removed now-unused Linux mount helpers
- `src-tauri/src/lib.rs` — module registration
- `src-tauri/Cargo.toml`, `Cargo.lock` — `pavao` → `pavao-sys` + `libc`
  (Linux-family target deps), with rationale comment

## Guard proof
- `smb_client::tests::write_cstr_buf_truncates_and_nul_terminates` (+
  `_strips_interior_nuls`): mutating the truncation bound
  (`dst_len - 1` → `dst_len`) fails both; restored, all 63 pass.
- `smb_client::tests::smb_url_*` pin the URL join/normalization rules.
- Live FFI proof (network, not unit): `VELA_SMB_LIVE=10.1.10.206/media
  cargo test --lib live_probe -- --nocapture` → clean friendly
  access-denied from the credentialed NAS, no hang/crash. For a new-feature
  slice the revert-proof degenerates (reverting removes the module and the
  tests with it); the mutation checks above are the non-vacuous form.

## Coder dispute (if any)
None.

## Known gaps
- `stat`/positioned reads intentionally deferred to slices 3–4 (dead-code
  gate); the wrapper API will grow with its first users.
- After this slice, on Linux `add_smb_folder` still requires a readable
  mount (`smb_mount_root`) until slice 3 — new shares can be added and
  browsed but folders can't serve media yet. Intermediate state on a
  feature branch; merge is gated on all six slices.
- No automated end-to-end SMB session test (no local smbd available);
  credentialed listing is covered by the owner playtest before merge.
- Deviation from plan wording: plan named the `pavao` crate; implementation
  uses `pavao-sys` directly for the reasons above. Same library underneath;
  recorded here rather than re-opening the plan.

## Reviewer comments
- Reviewer: codex (codex-cli 0.142.5), headless one-shot, JSON schema-forced.
- Reviewed SHA `fde07aae0efeef6f1449ee0217a2be0987d072f9`, base
  `21e950cd297a92682563eba94a9e5b4234318c97`. 2026-07-04 (UTC).
- Verdict: **accepted**; guard_confirmed: **true**.
- Comments: "No material defects found in the pinned diff." Guard proof
  held (baseline pass → documented mutation produced the expected 2
  `smb_client` failures → restored pass). Note: the sandbox blocked
  `git worktree add` (read-only `.git/worktrees`), so the reviewer used an
  independent disposable clone at the reviewed SHA instead — the coder's
  working tree was not modified, satisfying the isolation rule's intent.
