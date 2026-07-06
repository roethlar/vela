# sspf-13: Bug 2 max_conns added unconditionally breaks macOS SSH mounts

**Severity**: HIGH — a normal macOS SSH mount fails outright (unsupported option).
**Status**: Verified
**Branch**: (no-branches adaptation — landed on `main`)
**Commit**: `314d76c` (fix); found reviewing slice `0bbff29`

## Evidence
`src-tauri/src/sshfs.rs` — `mount()` is `#[cfg(unix)]` (Linux AND macOS). The
`0bbff29` slice added `max_conns=4` to the options for all Unix. macOS SSH uses
sshfs-mac (the sshfs 2.10 line via macFUSE; see `not_found_message`), which does
not implement `max_conns`.

## Predicted observable failure
On macOS, `sshfs … -o max_conns=4` errors on the unsupported option and the mount
fails immediately — every macOS SSH folder add/mount breaks. (macOS SSH is
"parked" for live testing but the code path is shipped.)

## What
The seek fix was applied on a platform whose sshfs can't accept it, converting a
Linux-only optimization into a macOS regression.

## Approach
Gate `max_conns` to Linux. Split the option list into
`sshfs_options_for(target_os)` (appends `max_conns=4` only when
`target_os == "linux"`); `sshfs_options()` calls it with `std::env::consts::OS`.
Splitting on an explicit OS string rather than a `#[cfg]` makes both platform
branches unit-testable from any host.

## Files changed
- `src-tauri/src/sshfs.rs` — `sshfs_options_for`/`sshfs_options`; both-branch unit
  tests.

## Guard proof
`linux_sshfs_options_request_parallel_sftp_channels` (max_conns present for
"linux") and `macos_sshfs_options_omit_max_conns` (absent for "macos"). Reverting
the gate (unconditional `max_conns`) fails the macOS test; restoring passes. Full
suite 105 passed, clippy -D warnings clean. The hermetic loopback test now sources
`sshfs_options()` and still mounts+reads on Linux.

## Reviewer comments
- **r1** 2026-07-06 `codex` (codex-cli 0.142.5), reviewed `0bbff29` base `61efc4e`,
  `guard_confirmed: false`, verdict **reopened**. Comment (HIGH): "`max_conns=4` is
  added unconditionally for all Unix mounts, but Vela's macOS path recommends
  `sshfs-mac`/sshfs 2.10, whose supported option list does not include `max_conns`
  → normal macOS SSH mounts can fail immediately with an unsupported/bad option
  instead of mounting." Admitted; gated to Linux in `314d76c`.
