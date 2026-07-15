# dlr-s8-4: package collection republishes stale artifacts

**Severity**: MEDIUM — a successful release build leaves older installers in
`dist/`, so an operator can distribute the wrong Vela version.
**Status**: Open
**Branch**: `main` (approved dependency-refresh Slice 8)
**Commit**: pending

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

Pending implementation. Add a hermetic Node test that runs the real Bash
script with fake `node`, `npm`, and `uname` commands, seeds stale AppImage,
deb, and rpm outputs, and requires a deb/rpm build to collect only the new
requested packages. Wire that guard into the existing frontend check gate.

## Files changed

- `scripts/build.sh` — pending exact bundle-directory selection and pre-build
  generated-output cleanup.
- `tests/build-script.test.mjs` — pending hermetic stale-artifact regression.
- `package.json` — pending guard activation in the canonical check.

## Guard proof

Pending: after the fix lands, restore the old all-Linux-subdirectories and
no-cleanup behavior in a detached disposable worktree, require the hermetic
test to fail on stale files, restore the commit, and require the test plus
canonical check to pass.

## Coder dispute (if any)

None. The final package command directly reproduced the promised invariant's
failure.

## Known gaps

The guard exercises Linux deb/rpm collection because that is the observed
failure. The same pre-build cleanup is shared by Tauri bundle targets on macOS
and Windows; final macOS packaging remains a required integration check, while
Windows packaging remains GitHub-hosted and owner-gated.

## Reviewer comments

Pending external Claude Fable 5 review.
