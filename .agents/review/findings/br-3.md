# br-3: bump.sh leaves package-lock.json stale

**Severity**: LOW — every version bump re-dirties the lockfile: the next
`npm install` rewrites the tracked file on a fresh checkout, and lockfile
consumers see the wrong root version.
**Status**: Verified
**Branch**: n/a (no-branches adaptation)
**Commit**: (filled at commit)

## Evidence
`scripts/bump.sh` updates Cargo.toml, Cargo.lock, package.json,
tauri.conf.json, PKGBUILD — not package-lock.json (root `version` at :3 and
`packages[""].version` at :9). Recurred at 0.1.40 and 0.1.41 immediately
after the same drift was hand-fixed for 0.1.39 (`96c5836`).

## Predicted observable failure
`npm install` on a clean 0.1.41 checkout modifies `package-lock.json`
(version fields), dirtying the tree; automation comparing lockfile versions
reports 0.1.39.

## What
The bump tool's update set is incomplete for npm's second version copy.

## Approach
bump.sh's python block also rewrites both lockfile version fields (plain
JSON edit, key order preserved by targeted regex or json load/dump matching
npm's 2-space format); plus a one-time sync of the current lockfile to
0.1.41 in the same commit.

## Files changed
- `scripts/bump.sh` — lockfile version fields join the update set
- `package-lock.json` — synced to the current version

## Guard proof
In a scratch worktree: run `scripts/bump.sh 9.9.9` → package-lock.json root
version fields read 9.9.9 and `npm install --package-lock-only` produces no
diff. Reverting the bump.sh change and re-running reproduces the stale
lockfile (red).

## Reviewer comments
(appended after the per-finding verdict)
