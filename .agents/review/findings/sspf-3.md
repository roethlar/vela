# sspf-3: live probe panics on a credentialed share after connect went lazy

**Severity**: LOW — the opt-in env-gated live SMB probe panics instead of
reporting the documented friendly access-denied outcome; diagnostic tool, not
shipped behavior
**Status**: Verified
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: `401fd1b`

## Evidence
`src-tauri/src/smb_client.rs` `live_probe_env_gated` did
`SmbConnection::connect(...)` then, in the `Ok` arm,
`conn.list_dir("").expect("connected but listing failed")`. Its doc comment
promises that on a credentialed share the probe reports a friendly
access-denied error. Before this slice, `connect()` performed `list_dir("")`
internally, so a credentialed share failed inside `connect` → the probe's `Err`
arm printed gracefully. This slice made `connect` lazy. Filed by codex on
review round 3 (base `adbeb867`, head `79f3979`).

## Predicted observable failure
`VELA_SMB_LIVE=server/share cargo test live_probe` against a credentialed share
(the documented expected case, anonymous creds): `connect` now returns `Ok`
(no network yet); `list_dir("")` triggers auth → access-denied `Err` →
`.expect(...)` PANICS, failing the probe, instead of printing the friendly
error.

## What
The probe encoded the old assumption that `connect` verifies reachability. The
lazy-connect change moved the first network op to `list_dir`, and the probe
asserted on it with `expect`.

## Approach
Chain `connect().and_then(|conn| conn.list_dir("")...)` and report BOTH errors
via a single `Err` arm — no `expect`/panic — so a credentialed share prints
access-denied as the doc comment promises. `list_dir("")` is now explicitly the
reachability/auth op. Test-only change; `connect`'s lazy semantics are
unchanged. Doc comment updated to state connect is lazy.

## Files changed
- `src-tauri/src/smb_client.rs` — `live_probe_env_gated` chains
  connect→list_dir and reports both errors; doc comment updated.

## Guard proof
Not hermetically guard-provable: the probe is env-gated on a live SMB server
(`VELA_SMB_LIVE`) and returns early without one, so no CI test can drive the
credentialed-share path. Verified by inspection (the `expect` panic path is
removed; both error legs now report) and `cargo check`/`clippy`/`test` clean
(89 tests; the probe skips). Manual check: an owner `VELA_SMB_LIVE` run against
the real NAS is the end-to-end confirmation.

## Coder dispute (if any)
None — admitted; a real consequence of the connect() semantics change in this
slice.

## Known gaps
End-to-end confirmation of the credentialed-share message depends on an owner
live NAS run (owner-gated). The logic is straightforward and inspected.

## Reviewer comments
codex (codex-cli 0.142.5), `-s read-only`, JSON mode. Round 3 (2026-07-05):
**reopened** on this finding, reviewed head `79f39798`, base `adbeb86765`,
`guard_confirmed: true`. Final: **accepted** at round 4, reviewed head
`401fd1bc`, base `adbeb86765`, `guard_confirmed: true`, no comments — the
untestable-here nature accepted as noted (env-gated probe).
