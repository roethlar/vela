# dlr-s8-1: local package builds bypass the pinned JavaScript toolchain

**Severity**: MEDIUM — local release scripts can install dependencies and
produce bundles with a Node/npm pair different from CI and the committed lock.
**Status**: In progress
**Branch**: `main` (approved dependency-refresh Slice 8)
**Commit**: pending

## Evidence

Before this fix, `scripts/build.sh` and `scripts/build.ps1` called plain
`npm install` when `node_modules` was absent or stale, then invoked the Tauri
build without checking either executable. The approved dependency plan requires
Node 26.5.0/npm 12.0.1 on every local install path; `.node-version` and
`packageManager` are metadata and do not activate npm 12 themselves.

## Predicted observable failure

A developer with Node 26's bundled npm 11, or another Node 26 patch, can run a
packaging script successfully. That path may rewrite `package-lock.json` with
the wrong package manager or produce an installer that was never exercised by
the pinned CI/runtime baseline.

## What

Add one executable check that derives the expected versions from the canonical
repo pins, then require it in both local packaging scripts and all CI/release
install paths. Document the same prerequisite for a clean development install.

## Approach

`scripts/check-js-toolchain.mjs` reads `.node-version` and
`package.json.packageManager`, resolves `npm --version` through the platform
shell (including Windows `npm.cmd`), and fails unless both executables match.
The Bash and PowerShell packaging scripts call it before inspecting or
installing dependencies. Workflow assertions reuse the same checker after
activating npm 12, removing three hand-copied version comparisons.

## Files changed

- `scripts/check-js-toolchain.mjs` — canonical executable assertion.
- `scripts/build.sh` — assert before any local install/build.
- `scripts/build.ps1` — assert before any local install/build.
- `.github/workflows/ci.yml` — reuse the canonical assertion.
- `.github/workflows/release.yml` — reuse it in audit and bundle jobs.
- `README.md` — require the pinned toolchain and clean `npm ci` setup.

## Guard proof

Pending: inject a fake npm 11 executable and prove the checker plus both local
packaging entry points fail before building; separately change only
`.node-version` in a disposable worktree and prove the Node mismatch fails.
Restore the committed state and require the checker and normal build gates to
pass.

## Coder dispute (if any)

None. The audit finding is admitted under the approved dependency plan.

## Known gaps

The PowerShell path can be executed on macOS, but a GitHub-hosted Windows bundle
remains the final Windows runtime proof after a separately approved push/run.

## Reviewer comments

Pending external Grok review.
