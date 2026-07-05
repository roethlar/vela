# smb-5: Remove the Linux mount machinery; update the add-SMB copy

**Severity**: — (planned slice 5 of `.agents/plans/smb-native-client.md`, not a defect)
**Status**: Verified (accepted round 3; awaiting owner-gated merge)
**Branch**: `smb-native` (stacked; commit follows smb-4's `f2a4640`)
**Commit**: `a213cb21e3822c396c9ec44091f03e5645119286` (deletion `514b093` + drift fix-ups `c541903`/`a213cb2`; base `f2a4640c32790aec7bb3e988d4a95cfb51de613c`)

## Evidence
Approved plan slice 5 (design §4): with browsing (smb-3) and playback
(smb-4) native, the Linux gvfs/kio path is dead code and the owner-facing
error copy that drove this plan ("install/enable kio-fuse or gvfs-fuse")
must go.

## Predicted observable failure
Before: 306 lines of unreachable-on-the-happy-path Linux mount resolution
remained, and the add-SMB UI still instructed users to set up KIO-FUSE/
GVfs. After: Linux has no mount code to reach (compile-time absent, not
runtime-false) and the UI describes the native flow.

## What / Approach
- `smb.rs` 532 → 245 lines: deleted Linux `mount()`, `remount_on_startup`
  (both variants), `resolved_mountpoint` + the whole candidate stack
  (gvfs/kio-fuse discovery, case variants, `try_gio_mount`,
  `run_with_timeout`, `smb_uri`, `linux_mount_error`, `find_program`,
  `command_stdout`, `is_readable_dir`, `current_uid`). Kept: macOS
  (`mount_smbfs`) and Windows (`net use`) flows unchanged, Linux no-op
  `unmount`/`unmount_for_removal` for the cross-platform command paths,
  `pct` re-gated to macOS-only. Module header rewritten.
- `lib.rs`: the boot remount pass is now `#[cfg]`-gated to macOS/Windows
  (previously gated at runtime by `remount_on_startup() == false`).
- `commands.rs`: Linux `smb_mount_root` returns `None` (native records
  have no OS path); `smb_folders_for_ui` already falls back to the
  share-relative path.
- `Settings.svelte`: "Mount share"→"Add share", "Mounting…"→"Connecting…",
  KIO-FUSE/GVfs paragraph replaced with the native description (and an
  honest note that macOS/Windows still attach via the OS), "Mounted
  share" label → "Share", per-mount button "Unmount" → "Remove".

## Files changed
- `src-tauri/src/smb.rs` — deletions + header (−287 net)
- `src-tauri/src/lib.rs` — cfg-gate boot remount
- `src-tauri/src/commands.rs` — Linux smb_mount_root → None
- `src/lib/Settings.svelte` — copy

## Guard proof
Deletion slice: the guard is compilation + the full suite on the platform
that lost the code (`cargo clippy --all-targets -D warnings`, 71 tests
green, `npm run check`/`build` clean). There is no new behavior to
mutation-prove; the reviewer should instead verify (a) no remaining
references to deleted symbols anywhere (including cfg-gated non-Linux
code), and (b) macOS/Windows code paths still reference only symbols that
exist under their cfgs (reason from attributes; cross-compilation isn't
available here).

## Coder dispute (if any)
None.

## Known gaps
- macOS/Windows builds are reasoned about via cfg attributes, not
  compiled (no cross toolchain on this machine) — same standing gap as
  every slice, resolved by the owner's next mac build.
- `unmountSmb` command name and UI function names still say "mount";
  renaming the command surface would churn the frontend API for cosmetic
  gain — left as-is, noted for a possible later cleanup.

## Reviewer comments
Round 1 — reopened. Reviewer: codex (codex-cli 0.142.5); reviewed
`514b093…`, base `f2a4640…`. 2026-07-04 (UTC). guard_confirmed: **true**
(clippy -D warnings + 71 tests green in its isolated checkout). Six
findings, all documentation/dependency drift, all accepted:
smb.rs:35 prepare_mount doc; smb.rs:208 unmount_for_removal comment;
commands.rs:408 mount_smb doc; commands.rs:551 list_smb_directories doc;
.agents/repo-guidance.md:68 stale GVfs/KIO-FUSE earned practice;
packaging/arch/PKGBUILD:13 stale gvfs-smb/kio-fuse optdepends. Fix-up
round 2 rewrites each and moves the PKGBUILD dependency correction
(smbclient into depends) forward from slice 6.

Round 2 — reopened again (nine further drift spots, exhaustive sweep):
Settings.svelte:322 error copy; config.rs:44/63/77/88 (smb_mounts field,
SmbMount, mountpoint, SmbFolder.path docs); commands.rs:386/626
(SmbDirectoryDto, add_smb_folder); local.rs:1 module header;
decisions.md:62 (2026-05-23 entry still "Active" — now marked partially
superseded, preserving the no-root constraint and SSH stance). All
accepted and fixed in the round-3 head.

Round 3 — accepted. Reviewed `a213cb2…`, same base. 2026-07-04 (UTC).
guard_confirmed: **true** (clippy -D warnings + 71 tests green in the
reviewer's isolated checkout). All 15 prior findings verified resolved;
no factually-wrong platform wording found.
