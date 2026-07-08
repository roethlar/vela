# smb-6: Packaging, docs, plan status, and handoff

**Severity**: — (planned slice 6 of `.agents/plans/smb-native-client.md`, not a defect)
**Status**: Verified (accepted round 3; awaiting owner-gated merge)
**Branch**: `smb-native` (stacked; final slice)
**Commit**: `e71eb4ee3a279889f5d7157458c9ed0f9efe4b2e` (slice + fix-ups; base `a213cb21e3822c396c9ec44091f03e5645119286`)

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
Round 1 — reopened. Reviewer: codex (codex-cli 0.142.5); reviewed
`be31756…`, base `a213cb2…`. 2026-07-04 (UTC). guard_confirmed: **true**.
Six findings; five accepted and fixed: deb depends now
"libsmbclient0 | libsmbclient" (covers both Debian generations); README
artifact name de-versioned; README status line no longer says "SMB
mounting"; state.md merge gate corrected (NOT fast-forward — main gained
2 ISSUES commits after branching; brittle commit count replaced with the
rev-list command); index header no longer runs ahead of the table.

## Coder dispute (recorded, per the silent-veto rule)
Finding 6 (state.md:16 / repo-guidance.md:70 still grep-hit "gvfs|kio")
is DISPUTED, not fixed: both hits are accurate NEGATIVE statements
("gvfs/kio machinery deleted", "no OS mounts, no root, no gvfs/kio") in
live docs whose job is exactly to record that the dependency is gone.
The grep criterion in the dispatch prompt was the coder's own
over-strict phrasing, not a repo rule; removing truthful negative
mentions would make the change harder to discover, not safer. Round 2
is asked to judge the rationale; if the reviewer still holds the
finding, it routes to the owner as contested.

Round 2 — reopened (one finding) + dispute resolved in coder's favor.
- Reviewed `15b5097…`, base `a213cb2…`. 2026-07-04 (UTC).
  guard_confirmed: **true** (clippy, 71 tests, JSON parse green).
- Finding (accepted): the round-1 fix-up had put the Debian alternation
  into the RPM depends (tauri passes rpm entries literally — invalid
  requirement) and left deb plain. Fixed: deb =
  "libsmbclient0 | libsmbclient", rpm = "libsmbclient".
- Dispute outcome: reviewer ACCEPTED the gvfs/kio rationale — the
  remaining state.md/repo-guidance hits are accurate negative statements
  documenting the deleted machinery. Finding withdrawn, no owner
  adjudication needed.

Round 3 — accepted. Reviewed `e71eb4e…`, same base. 2026-07-04 (UTC).
guard_confirmed: **true** (clippy, 71 tests, JSON dependency-shape check
green in the reviewer's isolated checkout). deb/rpm depends confirmed.
