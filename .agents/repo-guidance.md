# Repo-Specific Guidance
<!-- Extends AGENTS.md; never overrides it. Rules and pointers only — state
     lives in .agents/state.md. -->

## Mission Detail

- Vela is a Tauri 2 desktop app with a SvelteKit/TypeScript frontend in `src/`
  and a Rust backend in `src-tauri/`.
- The app browses Plex, Jellyfin, and Emby media servers through a common
  media-source abstraction. Local folder, SMB, and SSH/SFTP sources were
  REMOVED (decision `.agents/decisions.md` 2026-07-08 — Vela is a
  multi-server client; do not resurrect them).
- Playback is intentionally delegated to the system `mpv` binary in its own
  window for HDR passthrough. Do not embed video in the webview unless the
  owner explicitly changes that product decision.
- SvelteKit is configured as a static SPA for Tauri. Vite dev uses port `1420`
  with strict port behavior.
- Linux release packaging lives in `src-tauri/bundle/linux/` for Tauri bundles
  and `packaging/arch/` for the Arch package.

## Reading Order

1. `AGENTS.md` and this file.
2. `.agents/state.md` for current active work and blockers.
3. `.agents/decisions.md` for durable decisions and supersessions.
4. `README.md` and `ISSUES.md`.
5. `.review/deduped_action_list.md` and `.review/gpt_review.md` as historical
   evidence only — both carry a 2026-06-10 supersession banner; their current
   facts live in `.agents/state.md` and `.agents/decisions.md`, not in the
   `.review/` files themselves.

## Verification

This section is the canonical home for the verification commands (carved in
from the retired `.agents/repo-map.json`, 2026-07-08; verified against
`package.json` scripts and `.github/workflows/ci.yml` on 2026-07-15):

- `node scripts/check-js-toolchain.mjs` (repo root) — require the exact Node
  and npm executables pinned by `.node-version` and `packageManager`.
- `npm ci` (repo root) — clean, lockfile-exact JavaScript install.
- `npm audit` (repo root) — fail closed on known npm vulnerabilities.
- `npm run check` (repo root) — frontend type and Svelte validation.
- `npm run build` (repo root) — production frontend build, or any change
  that can affect the Tauri frontend bundle.
- `cargo +1.89.0 check --locked` (from `src-tauri/`) — declared MSRV compile
  floor.
- `cargo +stable check --locked` (from `src-tauri/`) — rolling-stable compile
  validation.
- `cargo +stable clippy --all-targets --locked -- -D warnings` (from
  `src-tauri/`).
- `cargo +stable test --locked` (from `src-tauri/`) — Rust unit tests.
- `cargo audit --file Cargo.lock` (from `src-tauri/`) — fail closed on known
  Rust vulnerabilities; unsoundness/unmaintained notices remain visible.
- `npm run e2e` (repo root) — end-to-end UI/playback validation on Linux
  (drives the real debug app; needs tauri-driver, Xvfb, ffmpeg, mpv, bsdtar,
  curl — see `tests/e2e/README.md`). `-- --skip-build` reuses the existing
  debug binary; `-- <name>` runs one scenario.

CI note: `.github/workflows/ci.yml` runs on the `github` remote
(`https://github.com/roethlar/vela.git` — confirmed active 2026-07-04 via
`gh api .../actions/runs`), NOT on the gitea `origin` (no
`.gitea/workflows/`). Run the commands locally before claiming completion;
CI only covers pushes that reach the github remote.

Rules the command list doesn't carry on its own:

- Run every Cargo command above from `src-tauri/`, not the repo root.
- For changes that can affect both sides of the Tauri app, run the full CI
  command set: toolchain assertion, `npm ci`, npm audit/check/build, Rust 1.89
  check, stable check/clippy/test, and Cargo audit.
- Packaging changes should also run the affected packaging command when
  practical: `npm run build:linux` for Tauri Linux bundles or
  `npm run build:arch` for the Arch package.

## Remotes & Sync

- `origin`: `http://q.internal:3000/michael/vela.git` (fetch and push).
- Push policy lives in `.agents/push-policy.md`.

## Earned Practices

- Keep token and credential handling conservative. Plex/Jellyfin/Emby poster
  and stream URLs may carry tokens as an accepted local-only exposure. Do not
  add logs, errors, analytics, or copied UI text that expose token-bearing
  URLs, auth tokens, SMB passwords, or config contents. (See
  `.agents/decisions.md`, 2026-05-23.)
- Keep config persistence defensive. The config may contain Plex/Jellyfin/Emby
  tokens and legacy SMB credentials (inert, next bullet); preserve owner-only
  Unix permissions, atomic saves, fail-closed parsing and validation, and
  cross-process locking. Never replace an unreadable or invalid config with
  guessed/default runtime settings. Recovery is an explicit user action that
  first preserves the invalid file as a private byte-for-byte backup, then
  creates a fresh config (decision `.agents/decisions.md` 2026-07-22).
- Old configs must keep loading after the local-source removal. The legacy
  `local_folders`/`smb_mounts`/`ssh_mounts` fields are tolerated serde
  fields — parsed, ignored, and preserved on save (never stripped,
  credentials included) so a rollback build still sees them. Guarded by the
  slice-1 round-trip tests; do not strip or migrate these fields. (See
  `.agents/plans/drop-local-sources.md`, compatibility rails.)
- Do not hold async runtime workers or shared locks across blocking OS,
  filesystem, process, or network work. Use the existing lock boundaries and
  `spawn_blocking` patterns.
- Generated outputs and dependency/build directories are not source of truth.
  Do not edit `build/`, `.svelte-kit/`, `node_modules/`, `src-tauri/target/`,
  `src-tauri/gen/`, or packaged Arch output under `packaging/arch/pkg/`.

### Guard discipline (earned the hard way, r17-r24 of library-refresh-scan)

Carved out of `.agents/state.md` 2026-07-14 so it survives that entry's
rotation. The evidence is the `## Code review log` in
`.agents/plans/library-refresh-scan.md`; the summary is in
`docs/history/state-archive.md`.

- **Red-proof every guard, always.** Land the fix, THEN inject the regression,
  THEN demand the test fail for the RIGHT reason. Restore from a committed
  state, never a stale file backup (that silently reverted work once). A long
  and still-growing list of guards in that loop turned out VACUOUS — disarmed by
  the author's own later fixes, written vacuous while actively trying not to, or
  left guarding a behavior that could be deleted outright with the suite green.
  **Not one ever failed or warned.** Every one was caught only by injecting the
  regression. Re-prove a guard whenever behavior around it changes.
- **Prove each behavior a fix claims SEPARATELY.** One fix claiming three
  behaviors needs three injections (r19 needed three; r20 needed four).
- **The newest fix is the most dangerous code in the repo — not the original.**
  Across eight consecutive rounds, the author's fixes carried defects at the same
  rate as the code they fixed, each one opening the next door into the same
  failure class. Review the newest fix hardest; never treat "this one is simple"
  as a reason to skip it.
- **A self-audit is not a check.** Three unverified author assumptions were
  overturned by reviewers (r8-4, r12-1, r20-2). r20-2 was a defect the author
  looked straight at during his own audit and waved through on an assumption one
  grep would have falsified. When the author's reasoning says "this one is fine",
  that is the moment to go and look.
- **Before recording anything as unguardable, go and read the failure path.**
  Two surfaces were recorded as unguardable and both were wrong, both times
  because the author reasoned about the code instead of reading it. Building the
  guard then found a real bug.
