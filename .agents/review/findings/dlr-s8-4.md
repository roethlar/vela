# dlr-s8-4: package collection republishes stale artifacts

**Severity**: MEDIUM — a successful release build leaves older installers in
`dist/`, so an operator can distribute the wrong Vela version.
**Status**: Verified
**Branch**: `main` (approved dependency-refresh Slice 8)
**Commit**: `bff2905`

## Evidence

At reviewed Slice 8 head `d7ec74d`, the final Linux command
`scripts/build.sh --bundles deb,rpm` successfully produced 0.1.51 deb and rpm
packages, then copied five files into `dist/`: the new packages plus 0.1.50
deb/rpm and a 0.1.39 AppImage. `scripts/build.sh:18-21` promises that `dist/`
is recreated with only the current build, but the explicit Linux override sets
`subdirs=(appimage deb rpm)` regardless of the requested bundle list and the
collector copies every matching file already present in those directories.

## Predicted observable failure

After building only deb and rpm for version 0.1.51 on a host whose Tauri bundle
directories contain prior outputs, `dist/` also contains unrequested and older
installers. Any automation or human selecting from `dist/` can publish or
install stale code even though the build itself was green.

## What

Make the Linux artifact directories match the explicit bundle request and
clear each generated target directory before the build, so collection can only
see artifacts produced by the current invocation.

## Approach

The explicit Linux override now derives its artifact directories from the
validated requested bundle list. Before invoking Tauri, the script removes
only those known generated directories; it deliberately never removes the
Arch package directory, which also contains source files. A hermetic Node test
runs the real Bash script with fake `node`, `npm`, and `uname` commands, seeds
stale AppImage, deb, and rpm outputs, and requires a deb/rpm build to collect
only the new requested packages. The existing frontend check gate runs it.

## Files changed

- `scripts/build.sh` — select exact Linux bundle directories and clear only
  those generated targets before building.
- `tests/build-script.test.mjs` — hermetic stale-artifact regression.
- `package.json` — activate the guard in the canonical check.

## Guard proof

- In a detached `bff2905` worktree, restoring the old
  `subdirs=(appimage deb rpm)` selection and deleting the pre-build cleanup
  made `node --test tests/build-script.test.mjs` fail on the exact five-file
  stale result: 0.1.39 AppImage, 0.1.50 deb/rpm, and 0.1.51 deb/rpm. Restoring
  the committed script made the test pass with a clean worktree.
- `npm run check` passes the new guard plus Svelte diagnostics, and
  `npm run build` passes.
- The fixed script and guard were checksum-verified on the Linux VM. The real
  `scripts/build.sh --bundles deb,rpm` build produced 0.1.51 packages and
  `dist/` contained exactly `Vela_0.1.51_arm64.deb` and
  `Vela-0.1.51-1.aarch64.rpm`; the deb metadata reports version 0.1.51/arm64
  and `file` recognizes both package formats.

## Coder dispute (if any)

None. The final package command directly reproduced the promised invariant's
failure.

## Known gaps

The guard exercises Linux deb/rpm collection because that is the observed
failure. The same pre-build cleanup is shared by Tauri bundle targets on macOS
and Windows; final macOS packaging remains a required integration check, while
Windows packaging remains GitHub-hosted and owner-gated.

## Reviewer comments

Claude Code 2.1.210 (`claude-fable-5`) reviewed head
`f7bc3446d44994dfadd11ccf71aedb226f5dcaf8` against base
`58279a05328564a6064f7b3747f56ae446fe2017` at
2026-07-15T19:12:17Z. Verdict: **accepted**;
`guard_confirmed:true`; comments: none. In its detached disposable worktree,
Claude independently restored both old collection behaviors, observed the
exact five-file stale-artifact failure, restored the reviewed head, observed
the guard pass, confirmed canonical activation and cleanup safety, and left
the tree clean.
