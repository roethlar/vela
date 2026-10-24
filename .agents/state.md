# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- Rust lockfile refresh, 2026-10-23. CI on `main` went red after the 2026-08-08
  green run. The causes were new upstream advisories and a new stable lint, not
  repo changes. `cargo update -p` moved async-trait 0.1.89 -> 0.1.92, which
  resolves clippy 1.99's `double_must_use` in its generated code (19 hits at
  `src/source/mod.rs:844`). It also moved h2 0.4.15 -> 0.4.20 (RUSTSEC-2026-0258),
  rustls 0.23.42 -> 0.23.46 (RUSTSEC-2026-0348, -0349 and two more), and
  rustls-webpki 0.103.13 -> 0.103.16, and added syn 3.0.9 for async-trait. Every
  new version's MSRV is <= 1.71, within the 1.89 floor. Only `Cargo.lock`
  changed. The Rust gates can't run on the Windows authoring host (no MSVC build
  tools), so the GitHub CI run on this commit is their verification. The
  remaining red CI gate on `main` is npm audit, on two low @sveltejs/kit
  advisories. The SvelteKit removal clears them; it is on an unmerged branch
  (see the next-but-one bullet).
- The Node pin moved from 26.5.0 to 26.12.1 (2026-10-23, owner go), for the
  October 2026 Node security release. `.node-version` and `engines`
  (`>=26.12.1 <27`) are updated, and npm stays at 12.0.1. Node only builds and
  tests Vela; the shipped app runs no Node, so no end user was exposed. Verified
  on Windows with Node 26.12.1: `npm ci` and `npm run build` pass.
  `node --test tests/` has 5 failures from CRLF checkout line endings
  (`core.autocrlf=true`). They fail the same way on unmodified `main`, and a copy
  of `main` checked out with LF endings doesn't hit them. `npm run check` fails
  on the same test. Not run: the toolchain check (this host has npm 11.20.0) and
  the Cargo commands (no Rust change). The E2E VM still needs re-aligning; see
  the Pending note in `.agents/machines.md`.
- The npm lockfile got a non-breaking `npm audit fix` on 2026-10-23, with no
  package.json change. It fixed the high advisories in devalue (5.9.4), vite
  (8.3.5, with rolldown 1.2.15), and source-map-js (1.2.2).
- SvelteKit removal, 2026-10-23. The owner approved
  `.agents/plans/remove-sveltekit.md` and ruled G1 "rename"; the decision is
  `.agents/decisions.md` 2026-10-23. Slices 1-4 are implemented on branch
  `remove-sveltekit`, which is NOT merged to `main`. The branch is at
  version 1.0.62.
  - The frontend is plain Svelte 5 and Vite. Root `index.html` loads
    `src/main.ts`, which mounts `src/App.svelte`. Static files are in
    `public/`, and `$lib` is defined in tsconfig `paths`.
    `npm run build` runs `scripts/check-frontend-build.mjs`, and
    `tests/frontend-shell.test.mjs` guards the shell.
  - Verified on the Windows host, in an LF worktree with npm 12.0.1: `npm ci`;
    `npm audit` (0 vulnerabilities); `npm run check` (68 pass, 2 skipped;
    svelte-check 295 files, 0 errors, 0 warnings); `npm run build`; and every
    local guard red-proof (see the plan's implementation log).
  - NOT verified, and blocking the merge: Slice 0 (VM Node re-alignment and the
    E2E baseline); Slice 1's E2E run and its F1/F2 red-proofs; the full VM
    `npm run e2e`; the VM webview-mount red-proof; the Mac desktop smoke and
    `tauri dev`/HMR; the Cargo commands; and the codex Review, where the
    owner chooses `codereview` or `openreview`.
  - CI is unverified until the PR's GitHub run completes. `main` keeps
    failing npm audit until the branch merges.
- The 2026-08-05 README refresh split the old monolith: README.md is now
  end-user-facing with new real-library screenshots (`docs/images/vela-*.png`,
  server name scrubbed to "Plex"); deep user reference moved to
  `docs/usage.md`, build/dev/architecture to `docs/development.md`. The
  2026-07-25 transcoding-statement guards in `tests/transcoding-ui.test.mjs`
  were repointed from README.md to docs/usage.md with owner approval.
- Vela 1.0.60 is published as GitHub's Latest release from exact commit
  `10b968b583a44c4738ece324b5fcbb9d58276235` — the FIRST SIGNED release:
  the macOS universal DMG is Developer ID signed and notarized (spctl
  `Notarized Developer ID`), the Windows MSI + NSIS are Authenticode signed
  (Azure Trusted Signing); Linux and Arch artifacts remain unsigned by
  design. Its canonical workflow, checksum, local signature-inspection, and
  publication evidence lives in `.agents/plans/v1-release-readiness.md`.
- Product behavior remains as settled in `.agents/decisions.md`: Vela is a
  multi-server Plex/Jellyfin/experimental-Emby client, delegates HDR playback
  to external mpv, uses title-level watched state across duplicate copies, and
  offers Prefer Best, Prefer Compatible, Prefer Fastest Source, Ask Every Time,
  and per-title Play Version.

- Release code signing merged at `b9c6199` (2026-08-07) shipped in 1.0.60.
  The wiring detail and durable tauri-cli signing facts live in
  `docs/history/state-archive.md` (2026-08-08 rotation). The merged
  `ci/release-code-signing` branch still exists locally and on `github`;
  deleting it is owner-gated.

## Next

- Launch marketing drafts are in-repo: GitHub social preview
  `docs/images/social-preview.png` (1280×640) and Reddit copy
  `docs/marketing/reddit-launch-post.md`. Still awaiting owner go to set the
  GitHub repo social image and to post on Reddit.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.
## Blockers

- No known product blocker.
- The unrelated continuation/mpv and `refresh` E2E flakes remain recorded.
  They reproduced during the 1.0.59 full Linux run in `continueon`,
  `playverbs`, and `refresh` (36/39); the changed `sortpersist` scenario passed.

## Verification

- Canonical commands live in `.agents/repo-guidance.md` (Verification). Do not
  duplicate here.
- Linux live-server venue details live in `.agents/machines.md`.
- Release verification immutable artifact hashes live in
  `.agents/plans/v1-release-readiness.md`.

## Active Sources

- `AGENTS.md` and `.agents/repo-guidance.md`
- `.agents/decisions.md`
- `.agents/machines.md`
- `.agents/push-policy.md`
- `.agents/plans/library-sorting.md` and
  `.agents/plans/show-last-episode-sort.md` (landed sorting baseline)
- `.agents/plans/library-sort-direction.md` (landed at `c0d1412`, 1.0.59)
- `.agents/plans/v1-release-readiness.md` (published-release evidence)
- `.agents/plans/remove-sveltekit.md` (approved; implemented on unmerged
  branch `remove-sveltekit`; merge blockers in its status and log)
- `README.md`, `RELEASE_NOTES.md`, `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
