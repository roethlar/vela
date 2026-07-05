# smb-3: Native SMB listing through the Vfs provider (Linux)

**Severity**: — (planned slice 3 of `.agents/plans/smb-native-client.md`, not a defect)
**Status**: In progress (pending review)
**Branch**: `smb-native` (stacked; commit follows smb-2's `a904eb2`)
**Commit**: `bef25e2384637d59a46cb8a183bc9a61e73fe41d` (base `a904eb26442b76bd837f4e9061a9c94924ac3550`, the accepted smb-2 head)

## Evidence
Approved plan slice 3 (design §§2, 4): SMB mounts must be served by the
local-family pipeline over the native client, with no OS mount.

## Predicted observable failure
Before: on Linux, SMB sources only serve if a gvfs/kio mount path resolves;
with none, sections/items/children/search all fail. After: an SMB mount's
folders list natively over libsmbclient (sections, items, children, search,
listing cache, kind detection), with playback explicitly deferred to the
proxy slice.

## What
`source/smb_vfs.rs` (new, Linux-family): `SmbVfs` implements the slice-2
`Vfs` trait over `SmbConnection`. Boot and live-rebuild registration inject
it per SMB mount; folder paths become share-relative (`/movies`).

## Approach
- One provider per mount; lazy connect, dropped on call failure so the next
  call reconnects (server reboot doesn't wedge the source).
- Listings are the only network primitive: each `read_dir_sorted` caches
  children's kind/size, making the walkers' per-entry probes and the
  7-candidate artwork probes memory-hits; a listed directory is
  authoritative for absence.
- Containment: `normalize()` is purely logical (no symlinks client-side),
  refusing `..` and unrooted paths — provider paths can never address
  outside the share. Replaces `safe_user_media_root` for native members
  (that check canonicalizes *local* paths and would reject every
  share-relative path; the plan's root-narrowing intent is preserved by
  share scoping). Native folder paths are also excluded from asset-protocol
  allow-listing, which would otherwise grant same-named *local* dirs.
- `mount_smb`→`establish_smb_share` (slice 1) + `add_smb_folder` now
  validate over the native connection on Linux; `smb_client` regains
  `stat()` and `SmbEntry.size` for that and for `file_len`.
- `resolve_stream` on a native member returns a clear "lands in the next
  update" error instead of handing mpv a provider path.
- SMB `.nfo` sidecar reads deferred (`read_to_string` → `None`) until the
  proxy slice adds positioned reads; SMB items enrich from filenames and
  the online cache meanwhile.

## Files changed
- `src-tauri/src/source/smb_vfs.rs` — new provider (+ normalize tests)
- `src-tauri/src/source/local.rs` — `LocalFamilyMember.vfs`,
  `local_family` provider closure, `native_remote` playback gate
- `src-tauri/src/source/mod.rs` — module + `smb_vfs_path` helper
- `src-tauri/src/lib.rs` — boot registration (native folders + provider,
  allow-list exclusion); non-Linux helpers cfg-gated
- `src-tauri/src/commands.rs` — live rebuild mirror, native
  `add_smb_folder` validation
- `src-tauri/src/smb_client.rs` — `stat()` + `SmbEntry.size` re-added

## Guard proof
- `smb_vfs::tests::normalize_rejects_escapes_and_unrooted`: mutating
  `normalize` to resolve `..` (`out.pop()`) fails the test; restored, all
  66 pass. This is the containment-critical property.
- `smb_vfs::tests::normalize_roots_and_cleans` pins the path namespace.
- Existing local-family tests updated for the new `local_family` arity and
  still green (member ordering, safe-root filtering for non-native).

## Coder dispute (if any)
None.

## Known gaps
- `SmbVfs` cache/absence logic is not unit-tested (would need a fake
  connection behind another seam); it is exercised by the owner playtest
  (browse a real share) before merge.
- Listing-metadata cache never invalidates within a provider's lifetime;
  providers are rebuilt on every source mutation and app start, and the
  background revalidation walk refreshes listings, so staleness is bounded
  by the existing listing-cache semantics.
- Live credentialed listing not run in-session (no credentials in config);
  the env-gated live probe still covers connect + root listing against the
  real NAS anonymously (friendly access-denied).

## Reviewer comments
Round 1 — reopened.
- Reviewer: codex (codex-cli 0.142.5); reviewed `bef25e2…`, base `a904eb2…`.
  2026-07-04 (UTC). Verdict: **reopened**; guard_confirmed: **true**
  (normalize `..` mutation failed the test in its clone, restore passed 66).
- Finding: `src-tauri/src/lib.rs:140` — boot builds `asset_folders` from
  EVERY local_family member and setup allow-lists them (~line 235); on
  Linux, native SMB members carry provider paths (`/movies`, even `/`), so
  startup would allow-list same-named LOCAL directories — the exact hole
  the slice closed in refresh_local_source, missed at boot. Accepted as
  correct; fixing via a shared, unit-tested `asset_folder_paths` helper
  used by both boot and refresh.
