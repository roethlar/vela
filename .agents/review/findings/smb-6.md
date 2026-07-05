# smb-6: Packaging, docs, plan status, and handoff

**Severity**: — (planned slice 6 of `.agents/plans/smb-native-client.md`, not a defect)
**Status**: In progress (pending review)
**Branch**: `smb-native` (stacked; final slice)
**Commit**: see index — head of `smb-native` at dispatch (base `a213cb21e3822c396c9ec44091f03e5645119286`, the accepted smb-5 head)

## Evidence
Approved plan slice 6 (design §5). PKGBUILD's dependency correction was
pulled forward into smb-5 by review; this slice carries the rest.

## Predicted observable failure
Before: deb/rpm bundles would install without libsmbclient; README
described the mount-era Linux flow and claimed SMB credentials ride mount
process arguments unconditionally; the plan read as merely approved; the
state handoff pointed at in-progress work.

## What / Approach
- `tauri.conf.json`: deb + rpm bundles depend on `libsmbclient`.
- `README.md`: SMB feature bullet rewritten (native Linux: no mounts, no
  root, loopback range proxy, `velasmb:` posters, `smbclient` package
  required; macOS/Windows unchanged). Credential paragraph: on Linux,
  credentials never leave the process (auth callback); mount-argument
  exposure now scoped to macOS/Windows; proxy posture documented.
- `.agents/plans/smb-native-client.md`: Status → IMPLEMENTED with the two
  reviewed deviations named (pavao-sys; velasmb artwork scheme).
- `.agents/state.md`: handoff block (what landed, owner playtest
  checklist, owner-gated merge, version bump left for merge); token
  stance and test counts aligned; branch commit count corrected.
- `.agents/review/index.md`: review phase marked complete pending owner
  gate.

## Files changed
`src-tauri/tauri.conf.json`, `README.md`,
`.agents/plans/smb-native-client.md`, `.agents/state.md`,
`.agents/review/index.md` (+ this finding doc).

## Guard proof
Docs/packaging slice. Verification run: `npm run build:arch` completed —
`vela-0.1.9-1-x86_64.pkg.tar.zst` produced, `pacman -Qip` shows
`Depends On: … smbclient`; release profile compiles pavao-sys + the full
app. tauri.conf.json parsed as JSON after edit. Full CI set green at the
prior head (this slice touches no Rust/TS source).

## Coder dispute (if any)
None.

## Known gaps
- deb/rpm dependency names reasoned from Debian/Fedora package indexes
  ("libsmbclient" in both), not installed-tested (no deb/rpm host here).
- Version stays 0.1.9 on the branch; bump is the owner's release call at
  merge.

## Reviewer comments
(pending)
