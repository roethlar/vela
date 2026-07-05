# SMB: auto-add the share root as a library folder

Status: APPROVED 2026-07-04 via the owner-delegation decision in
`.agents/decisions.md` (direction chosen 2026-07-04: "auto-add share root"
over guided-folder-step and warning-only).

## Problem

Adding an SMB share and selecting a library folder inside it are two separate
steps in Settings → Folders. A share with zero selected folders intentionally
produces no source at all (`local_family` omits empty members,
`src-tauri/src/source/local.rs`), so nothing appears anywhere in the UI — no
sidebar entry, no sections, no media — and nothing tells the user why. The
owner hit this exact trap on 2026-07-04: share `zoey/media/new` added with
working credentials, `folders: []`, media invisible.

## Decision being implemented

Adding a share automatically adds its root as a library folder. The user can
remove it or add narrower subfolders afterwards; the two-step flow remains
available but is no longer required for media to appear.

## Change

- `mount_smb` (`src-tauri/src/commands.rs`): after the share is verified and
  persisted, append a default `SmbFolder` to the new mount:
  - `path: ""` (share root — already valid: `normalize_smb_relative_path`
    accepts empty, `smb_vfs_path("")` yields `/`),
  - `kind: ""` (auto-detect; `detect_kind` already runs over the native Vfs),
  - `name`: the mount's display name (matches `smb_folder_display_name`
    behavior for the root).
  Done inside the same `rebuild_local_locked` mutation that persists the
  mount, so one config write and one registry swap cover both.
- Applies on all platforms (Linux native and macOS/Windows OS-mount paths add
  folders through the same config shape). On the OS-mount path the root
  folder is subject to the existing mounted-path validation at rebuild time,
  unchanged.
- No migration for existing zero-folder shares: a boot-time auto-add would
  resurrect roots the user deliberately removed. Existing trapped shares are
  fixed by re-adding the share or one "Add this folder" click. (The owner's
  current `zoey` share is in this state.)

## Non-goals

- No change to the share browser or the per-folder add/remove flow.
- No warning UI for zero-folder shares (owner chose auto-add instead; a share
  can only reach zero folders again by deliberate removal).

## Verification

- Unit test: `mount_smb`'s mutation closure (or an extracted helper) yields a
  mount whose `folders` contains exactly one root entry with empty path and
  auto kind; guard-prove by reverting the auto-add and confirming the test
  fails.
- Full repo verification per `.agents/repo-map.json` (npm check/build, cargo
  check/clippy/test from `src-tauri/`).
- Live check on the owner's NAS: add the zoey share fresh → the source,
  its section, and the show inside appear with no further steps.
