# smb-2: Vfs provider-trait refactor of the local-family source

**Severity**: — (planned slice 2 of `.agents/plans/smb-native-client.md`, not a defect)
**Status**: In progress (pending review)
**Branch**: `smb-native` (stacked; commit 2)
**Commit**: `fc4203a55b7db4c3aedba39a1e18b3e166740355` (base `fde07aae0efeef6f1449ee0217a2be0987d072f9`, the reviewed smb-1 head)

## Evidence
Approved plan slice 2 (design §2): the local-family pipeline (sections,
listing cache, metadata, watch state, per-mount naming) must be reusable by
a native SMB provider instead of being duplicated.

## Predicted observable failure
None — this is the plan's explicitly no-behavior-change refactor slice. The
claim under review is the *absence* of change: every fs touchpoint in
`source/local.rs` and metadata sidecar reading now routes through
`source/vfs.rs::Vfs`, with `StdFs` making exactly the std::fs calls made
before, and the full test suite stays green.

## What
New `source/vfs.rs`: `Vfs` trait (read_dir_sorted / is_dir / is_file /
canonicalize / file_len / read_to_string) + `StdFs` impl. `LocalSource`
gains a `vfs: Arc<dyn Vfs>` field, `new()` defaults to `StdFs`, and
`with_vfs()` is the slice-3 injection point.

## Approach
Mechanical threading: `within_roots`/`within_root` canonicalize via the
provider (Option instead of io::Result); the three walkers, search,
`detect_kind`/`looks_like_show`, `largest_video_in`, and the
items/children/resolve_stream entry checks call provider methods;
`metadata::enrich` takes `&dyn Vfs` and sidecar `.nfo`/artwork probing goes
through it. `read_dir_sorted` moved from a free fn onto the trait with the
identical body. No logic, ordering, error-message, or caching change.

## Files changed
- `src-tauri/src/source/vfs.rs` — new (trait, StdFs, parity unit test)
- `src-tauri/src/source/local.rs` — threading only
- `src-tauri/src/source/metadata.rs` — enrich/read_sidecar/read_nfo/
  local_artwork/meta_base take the provider
- `src-tauri/src/source/mod.rs` — module registration

## Guard proof
Refactor slice: the guard is the existing suite (64 tests, includes
local-source walking/metadata tests) green before and after, plus the new
`vfs::tests::std_fs_reads_sorted_and_tolerates_missing_dirs` pinning StdFs
semantics (sorted listing, missing-dir → empty, canonicalize-miss → None).
No new behavior exists to revert-prove; reviewer should check instead that
the diff introduces no semantic drift (e.g. is_dir/is_file swaps,
canonicalize Ok→Some fidelity, filter order).

## Coder dispute (if any)
None.

## Known gaps
- `local_artwork` over a future SMB provider would emit share paths the
  webview can't load; slice 3 decides whether SMB skips artwork sidecars.
  StdFs behavior is unchanged.
- `metadata.rs` cache-file I/O (config dir) deliberately stays on std::fs —
  it is app state, not media filesystem.

## Reviewer comments
(pending)
