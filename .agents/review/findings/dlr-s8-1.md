# dlr-s8-1: local package builds bypass the pinned JavaScript toolchain

**Severity**: MEDIUM — local release scripts can install dependencies and
produce bundles with a Node/npm pair different from CI and the committed lock.
**Status**: Verified — external reviewer Grok accepted r1
**Branch**: `main` (approved dependency-refresh Slice 8)
**Commit**: `4cba5db`

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

- A PATH-prepended fake npm reporting 11.17.0 made the checker,
  `scripts/build.sh --native`, and `pwsh scripts/build.ps1 --native` each exit
  1 with the exact expected-12/got-11 mismatch before any build command ran.
- In a disposable worktree at `4cba5db`, changing only `.node-version` from
  26.5.0 to 26.5.1 made the checker exit 1 with the exact expected-26.5.1/
  got-26.5.0 mismatch. Restoring the committed pin returned it green and left
  the worktree byte-clean before removal.
- At the restored head, the checker, Bash syntax, PowerShell parser, frontend
  check/build, and the default macOS universal packaging script passed.

## Coder dispute (if any)

None. The audit finding is admitted under the approved dependency plan.

## Known gaps

The PowerShell path can be executed on macOS, but a GitHub-hosted Windows bundle
remains the final Windows runtime proof after a separately approved push/run.
The documented Bash-only `scripts/build.sh --native` path passes the new
toolchain check and then hits a pre-existing macOS Bash 3 empty-array failure at
the later Tauri invocation. The required default universal path is green; the
unrelated native-path defect is outside this finding and must not be silently
folded into it.

## Reviewer comments

**r1 — 2026-07-15T18:13:15Z — accepted.** Grok 0.2.101 independently
reviewed exact base `33163c5` and head `0934628`, injected fake npm 11 across
the checker and both available packaging entry points, injected a wrong Node
pin, observed the exact failures, restored its disposable worktree green, and
returned `guard_confirmed: true` with no comments.
