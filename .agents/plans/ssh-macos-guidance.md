# Plan: SSH source setup guidance in the UI (macOS-aware)

Status: DRAFT — not approved for implementation. Implements the 2026-07-04
decision (`.agents/decisions.md`): keep the sshfs dependency; handle macOS
with in-UI setup help/hint text. Covers the remaining work in the `ISSUES.md`
SSH entry. macOS live mount testing stays parked; this plan is UI/message
work only.

## Facts

- The requirement surfaces only at add-failure time today: `mount()` errors
  with "sshfs was not found. Install sshfs, then try again."
  (`src-tauri/src/sshfs.rs:31-39`); detection is `find_program` (PATH via
  `which`, plus `/usr/bin`, `/usr/local/bin`, `/opt/homebrew/bin`,
  `sshfs.rs:248-254`).
- The add-SSH form lives in `src/lib/Settings.svelte` (form state
  `:157-163`, `mountSsh()` `:365`, invokes `mount_ssh`).
- On macOS, "install sshfs" via brew core is a dead end (Linux-only libfuse
  dependency); the working route is the `macfuse` cask + a macFUSE-compatible
  sshfs build (e.g. `gromgit/fuse/sshfs-mac`) + system-extension approval
  (Apple Silicon: Recovery reduced-security step), and those builds proved
  unstable on the owner's machine (decision, 2026-07-04).
- Mounts run ssh with `BatchMode=yes` and Vela stores no SSH passwords
  (`sshfs.rs:40-55`), so key auth and a pre-trusted host key are hard
  requirements — the mount error already hints at this (`sshfs.rs:80-82`).

## Design

1. New small Tauri command `sshfs_status` returning `{ found: bool, path:
   Option<String> }` using the existing `find_program` (no subprocess run;
   existence check only — the path shown is not sensitive). Register in
   `lib.rs`'s handler list.
2. `Settings.svelte`, SSH section: query `sshfs_status` when the panel
   opens (alongside the existing `Promise.all` loads at `:228-240`) and
   render a static hint block above the form:
   - Always: "Requires sshfs. Vela connects with your SSH keys, agent, and
     config — no passwords. Connect to new hosts once with plain `ssh`
     first so the host key is trusted."
   - Status line: "sshfs detected at <path>" or "sshfs not found".
   - When not found, platform-specific install guidance (platform via
     Tauri's OS plugin or a field on `sshfs_status`):
     - macOS: macFUSE cask + `gromgit/fuse/sshfs-mac`, extension approval
       (Apple Silicon: Recovery → reduced security), restart; caution that
       these builds can be unstable on recent macOS.
     - Linux: install `sshfs` from the distro package manager.
3. Make the mount-time error platform-aware to match (macOS appends the
   route summary instead of the bare "Install sshfs, then try again",
   `sshfs.rs:39`). Keep messages free of user paths/credentials — static
   text plus the detected binary path only.

Non-goals: no dependency bundling, no in-app SFTP client, no install
automation, no change to mount mechanics or the BatchMode/no-password
stance, no macOS live-mount validation (parked).

## Verification

- Full CI set (both sides change): `npm run check`, `npm run build`; from
  `src-tauri/`: `cargo check --locked`, `cargo clippy --all-targets --locked
  -- -D warnings`, `cargo test --locked`.
- Rust unit test for the platform-aware error/guidance string selection.
- Manual: on this macOS machine with sshfs present the panel shows
  "detected"; temporarily `brew unlink sshfs-mac` → panel shows the macOS
  guidance block; relink. Linux visual check when next on the Linux box.

## Open points to settle at approval

1. Exact hint copy (owner may want it shorter; current draft is ~5 lines).
2. Whether the hint block also appears on the SMB panel's SSH sibling or
   only the SSH form (proposed: SSH form only).
