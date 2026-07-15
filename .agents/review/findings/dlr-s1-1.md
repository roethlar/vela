# dlr-s1-1: Windows cannot execute npm.cmd through execFileSync

**Severity**: MEDIUM — the Windows release matrix exits before dependency
installation, so Vela cannot produce or validate Windows bundles.
**Status**: Verified — external reviewer Grok accepted r2
**Branch**: `main` (approved dependency-refresh Slice 1)
**Commit**: `adc0104`

## Evidence

At reviewed head `7fef89a`, `.github/workflows/release.yml` invokes
`execFileSync('npm.cmd', ['--version'])` on Windows. Node's child-process
contract does not execute `.cmd` files directly through `execFile`; they must
run through a shell. The identical CI inline is Linux-only and succeeds there,
but the release job is a macOS/Linux/Windows matrix.

## Predicted observable failure

The `windows-latest` release leg reaches `Activate pinned npm`, installs npm,
then the version assertion throws while launching `npm.cmd`. `npm ci`, the
Tauri build, and Windows artifact upload never run.

## What

Make the exact npm-version assertion use a shell-backed child process on every
platform so Windows resolves `npm.cmd` through `ComSpec`, while retaining the
same pinned Node/npm comparison.

## Approach

Replace `execFileSync` and the platform-specific executable selection with
`execSync('npm --version')` in both workflow copies. The command is a static
literal, so using the platform shell introduces no user-controlled input. Keep
the exact `v26.5.0` / `12.0.1` assertions unchanged.

## Files changed

- `.github/workflows/ci.yml` — shell-backed npm version query.
- `.github/workflows/release.yml` — shell-backed npm version query, including
  the affected Windows matrix leg.

## Guard proof

The original command passed locally on macOS but cannot reproduce Windows
`.cmd` launch semantics. Proof is therefore the Node child-process contract,
YAML/cross-shell inspection, local execution on macOS, and the independent
reviewers' re-review. The Windows-hosted release leg remains the final runtime
proof after an owner-approved push/dispatch; do not claim it locally.

## Coder dispute (if any)

None. The finding is admitted.

## Known gaps

No local Windows execution venue is available in this session.

## Reviewer comments

**r1 author self-review — 2026-07-15T15:32:40Z — reopened.** Codex CLI 0.144.4
reviewed exact base `c02767e` and head `7fef89a` read-only. It reported the
direct `npm.cmd` launch failure at `.github/workflows/release.yml:65`. Per the
owner's 2026-07-15 correction, this is author evidence and does not count as
independent review.

**r1 independent — 2026-07-15T15:32:40Z — accepted.** Grok 0.2.101 reviewed
the same exact base/head read-only and returned no comments without seeing the
Codex result. The author admitted the concrete Codex finding; both reviewer
positions can be satisfied by the narrow fix, so no owner adjudication is
needed under the standing review decision.

**r2 — 2026-07-15T15:41:55Z — accepted by external reviewer Grok.** Grok
0.2.101 reviewed exact base `c02767e` and corrected head `adc0104`, returned
`verdict: accepted`, the exact SHAs, and `comments: []`, and explicitly read
this finding and the full corrected slice. Codex CLI also returned clean, but
that author-model self-review does not count. The exact assertion runs locally
under both Bash and PowerShell syntax; the GitHub-hosted Windows release leg
remains the final platform execution proof.
