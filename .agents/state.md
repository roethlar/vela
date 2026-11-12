# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- Push policy is `always` since 2026-11-12 (owner; `.agents/push-policy.md`):
  push after each commit without asking. History rewrites still need a go.
- Windows mpv idle-exit fix, 2026-11-12, `bf1b1c3` (1.0.62; plan
  `.agents/plans/windows-mpv-idle-exit.md`). On Windows, mpv started by Vela
  idled at end of file instead of exiting, so Continue Playing, playlist
  advancement, the final watched-state write and the recents stamp never
  ran (owner report on 1.0.59; reproduced with a bare mpv probe on
  netwatch-01). Vela now passes `--idle=no` after every user-controlled
  option. The plan passed an owner-directed openreview (agy,
  gemini-4-argon-high: `best_approach`). Local JS and stable-Rust checks
  and five red-proofs pass; a netwatch-01 probe shows the option wins over
  a user `--idle=yes`. NOT done: MSRV check, `cargo audit`, E2E, any
  Windows build, and the owner's acceptance on the affected install.
  Pushed 2026-11-12; see the plan's log for the CI result. Workaround for
  older releases: add `--idle=no` to the Advanced
  mpv arguments.
- Missing-npm build diagnostic, 2026-11-12 (`.agents/plans/build-missing-npm-diagnostic.md`;
  owner ruled D1 "A" and gave the go). A build host without `npm` used to
  get a Node stack trace. `scripts/check-js-toolchain.mjs` now reports a
  missing or out-of-range npm in one line (`e58a17e`), and `scripts/build.sh`
  names a missing `node` or `npm` before the toolchain check (`b519083`);
  nothing is installed for the builder. Seven red-proofs, `npm run check`
  (78 pass) and the manual repro on the CachyOS host pass. Pushed to both
  remotes at `f2c6449`; GitHub CI run 43265242786 passed every job
  (Frontend, Rust, cargo audit). Windows was not exercised. Awaiting owner
  confirmation.
- npm audit gate moved to `--audit-level=moderate`, 2026-11-02 (decision
  `.agents/decisions.md` 2026-11-02). `main` had failed npm audit since
  2026-09-18 on one low advisory, deepmerge GHSA-gcqx-w48m-w2h4
  (CVE-2026-93753), reported four times up the chain through
  vite-plugin-svelte, kit and adapter-static. No patched deepmerge exists, the
  newest vite-plugin-svelte (7.3.1) still depends on it, and so does the
  `remove-sveltekit` branch, so neither the Kit 3 bump `npm audit fix --force`
  offers nor the SvelteKit removal clears it. The forced bump was tried on
  2026-11-02, broke the build (SvelteKit 3 drops `svelte.config.js`), left all
  four findings, and was reverted. VulnCheck rejected the CVE on 2026-11-02
  after a maintainer-side dispute; the GitHub advisory is not yet withdrawn.
  CI, the release workflow, `docs/development.md` and
  `.agents/repo-guidance.md` now run `npm audit --audit-level=moderate`.
  Verified on netwatch-01 (Node 26.12.1/npm 12.0.2): that command exits 0 on
  the committed lock while plain `npm audit` still prints the 4 low findings.
  GitHub CI run 41564348672 at `f156f14` (pushed 2026-11-02) passed every
  job, Frontend audit step included: the first green `main` run since the
  2026-08-08 one noted below.
- GitHub and Gitea histories reconciled, 2026-10-24. GitHub `main` (the
  2026-10-23 Node, npm and Cargo security work and the SvelteKit plans) and the
  Gitea copy at `http://magneto:3001/michael/vela.git` (owner work to
  2026-10-03, head `d67bc4d`: the toolchain range, AVFoundation-first macOS
  audio for 1.0.61, the signed macOS wrapper, Linux prerequisite checks) had
  diverged from `1bd738e`. One merge commit joins them, with no history
  rewrite. Conflicts were resolved by intent: the toolchain per the amended
  2026-10-03 decision (next bullet); `Cargo.lock` keeps the newer of each
  crate (the 2026-10-23 versions below supersede Gitea's h2 0.4.16, rustls
  0.23.45 and rustls-webpki 0.103.15); `package-lock.json` was regenerated with
  npm 12.0.2 from GitHub's audited lock, so devalue 5.9.4, vite 8.3.5 and
  source-map-js 1.2.2 all hold. The magneto Gitea is the canonical `origin`;
  host `q` (q:3000) is retired (`.agents/repo-guidance.md`, Remotes & Sync).
- JS toolchain is range-checked with a security floor (decision
  `.agents/decisions.md` 2026-10-03, amended 2026-10-24):
  `scripts/check-js-toolchain.mjs` accepts any Node in `engines.node`
  (`>=26.12.1 <27`; the lower bound excludes Node 26 releases before the
  October 2026 security release) and any npm in `engines.npm` (`>=12`);
  `packageManager` is gone. CI/release build on a fixed baseline, Node 26.12.1
  (`.node-version`) / npm 12.0.2 (four `npm install --global` sites). Node
  only builds and tests Vela; the shipped app runs no Node, so no end user was
  exposed. The Linux E2E VM's user-local Node 26.5.0 is now below the floor;
  see the Pending note in `.agents/machines.md`.
- Node tests are line-ending-agnostic, 2026-10-24. Five tests failed on a
  Windows `core.autocrlf=true` checkout because their patterns spell line
  breaks as `\n`. `tests/dependency-policy.test.mjs`,
  `tests/live-release-safety.test.mjs` and `tests/release-workflow.test.mjs`
  now normalize CRLF to LF when they read source. No `.gitattributes` rule was
  added: pinning LF would change the bytes every Windows checkout builds from,
  CI's included (`scripts/build.ps1`, `.github/scripts/azure-sign.ps1`,
  `packaging/arch/PKGBUILD` among them), which is code-like and would need a
  plan. Each of the 14 newline-dependent assertions in those files was
  red-proved separately in both a CRLF and an LF checkout and restored green.
  Test-only; nothing shipped changes.
- Rust lockfile refresh, 2026-10-23. CI on `main` went red after the 2026-08-08
  green run. The causes were new upstream advisories and a new stable lint, not
  repo changes. `cargo update -p` moved async-trait 0.1.89 -> 0.1.92, which
  resolves clippy 1.99's `double_must_use` in its generated code (19 hits at
  `src/source/mod.rs:844`). It also moved h2 0.4.15 -> 0.4.20 (RUSTSEC-2026-0258),
  rustls 0.23.42 -> 0.23.46 (RUSTSEC-2026-0348, -0349 and two more), and
  rustls-webpki 0.103.13 -> 0.103.16, and added syn 3.0.9 for async-trait. Every
  new version's MSRV is <= 1.71, within the 1.89 floor. Only `Cargo.lock`
  changed. The Rust gates could not run on the Windows authoring host that day
  (no MSVC build tools then; see `.agents/machines.md`), so the GitHub CI run
  on this commit is their verification. The npm audit gate that stayed red
  afterwards is covered by the 2026-11-02 audit-threshold bullet above.
- The npm lockfile got a non-breaking `npm audit fix` on 2026-10-23, with no
  package.json change. It fixed the high advisories in devalue (5.9.4), vite
  (8.3.5, with rolldown 1.2.15), and source-map-js (1.2.2).
- SvelteKit removal, 2026-10-23. The owner approved
  `.agents/plans/remove-sveltekit.md` and ruled G1 "rename"; the decision is
  `.agents/decisions.md` 2026-10-23. Slices 1-4 are implemented on branch
  `remove-sveltekit`, which is NOT merged to `main`. The branch is at
  version 1.0.62, and on 2026-10-24 it merged the reconciled `main`
  (`bc12bc5`, 1.0.61) without a rebase; the plan's 2026-10-24 amendment
  carries the consequences.
  - The frontend is plain Svelte 5 and Vite. Root `index.html` loads
    `src/main.ts`, which mounts `src/App.svelte`. Static files are in
    `public/`, and `$lib` is defined in tsconfig `paths`.
    `npm run build` runs `scripts/check-frontend-build.mjs`, and
    `tests/frontend-shell.test.mjs` guards the shell.
  - Verified on the Windows host, in an LF worktree with npm 12.0.1: `npm ci`;
    `npm audit` (0 vulnerabilities); `npm run check` (68 pass, 2 skipped;
    svelte-check 295 files, 0 errors, 0 warnings); `npm run build`; and every
    local guard red-proof (see the plan's implementation log).
  - Re-verified after the 2026-10-24 merge of `main`, in an LF worktree with
    Node 26.12.1/npm 12.0.2: `node scripts/check-js-toolchain.mjs`; `npm ci`;
    `npm audit` (0 vulnerabilities); `npm run check` (68 pass, 0 fail, 6
    skipped, all POSIX-bash build-script cases; svelte-check 295 files, 0
    errors, 0 warnings); `npm run build` (B1-B6 passed); `node --test
    tests/`. The lock differs from `main`'s by exactly the 11 removed Kit
    packages.
  - NOT verified, and blocking the merge: Slice 0 (VM Node re-alignment and the
    E2E baseline); Slice 1's E2E run and its F1/F2 red-proofs; the full VM
    `npm run e2e`; the VM webview-mount red-proof; the Mac desktop smoke and
    `tauri dev`/HMR; the Cargo commands; and the codex Review, where the
    owner chooses `codereview` or `openreview`.
  - GitHub CI passed on PR #2 (https://github.com/roethlar/vela/pull/2), run
    40059537619 at `367bdf6`. Frontend: `npm ci`, `npm audit` (0
    vulnerabilities), `npm run check` (70 pass; svelte-check 0 errors, 0
    warnings), and `npm run build` (B1-B6 passed). Rust: the 1.89 MSRV check,
    and stable check, clippy `-D warnings` and test. cargo audit also passed.
    After the merge of `main`, run 40061360150 at `af96dc5` passed every job
    on Node 26.12.1/npm 12.0.2: `npm audit` (0 vulnerabilities), `npm run
    check` (77 pass, 0 skipped; svelte-check 0 errors, 0 warnings), `npm run
    build` (B1-B6 passed), the 1.89 MSRV check, stable check, clippy, 364 Rust
    tests, and cargo audit. (`main`'s npm audit gate is covered by the
    2026-11-02 audit-threshold bullet above; this branch does not clear the
    deepmerge advisory either, since it keeps vite-plugin-svelte.)
- 2026-08-20 macOS AirPlay diagnostics selected an AVFoundation-first production
  path, now drafted at `.agents/plans/macos-airplay-sync.md`; implementation is
  not approved. The affected server-backed MKV/E-AC-3 play was physically about
  two seconds audio-late under CoreAudio despite near-zero mpv `avsync`; changing
  that live process to AVFoundation restored physical sync, as it did again after
  a fresh CoreAudio launch. The same AVFoundation process followed AirPlay ->
  built-in speakers -> AirPlay and stayed synchronized, so Vela needs no route
  detector, listener, polling, guessed delay, or new setting. The production
  design adds macOS `--ao=avfoundation,coreaudio` before Advanced mpv arguments,
  with no non-macOS behavior change. Two manual volume-timing attempts were
  inconclusive (the second IPC OSD cue was not visible); upstream mpv 0.41 source
  directly applies AVFoundation mute/volume and pause controls, while packaged
  pause/resume and seek remain explicit acceptance checks. The owner settled
  own-config precedence on 2026-08-20: when `mpv_use_own_config=true`, Vela
  omits its AO preference and the expert's `mpv.conf` owns audio-output
  selection. The implementation is complete for Vela 1.0.61: the shipped play
  path uses a testable base-command helper, macOS appends
  `--ao=avfoundation,coreaudio` before Advanced arguments only when Vela owns
  defaults, and docs/release notes describe the contract. All four focused
  mutation proofs failed for the intended reason and restored green. Pinned
  Node/npm, npm audit/check/build, Rust MSRV/stable check, clippy, all 365 Rust
  tests, RustSec audit, `git diff --check`, and the native 1.0.61 DMG build pass.
  The freshly built DMG is running; packaged physical AirPlay/control
  acceptance remains pending and must not be inferred from automated evidence.
- 2026-08-20 owner review-process constraint: never start more than one Claude
  session without explicit owner approval; never impose a low timeout, end a
  Claude process, or discard any verdict for any reason without explicit owner
  approval. A failed capability/schema check is reported with its verdict
  retained and does not authorize a retry.
- 2026-08-20 explicit local signed-build path is implemented at
  `scripts/build-signed-macos.sh`: macOS-only, Keychain-identity based, loads
  sibling `Certificates` API metadata without reading `.p12` passwords, fails
  closed on partial/unsafe credentials, delegates compilation to `build.sh`,
  then independently checks DMG/app Developer ID signatures, staple,
  Gatekeeper, and both universal architectures. Hermetic guards cover the
  success and pre-build refusal paths; removing only `spctl` failed its guard
  for the exact missing assessment, restored green. Real Apple submission and
  artifact evidence are machine-specific in `.agents/machines.md`.
- 2026-08-20 macOS 27 local-network failure under investigation: Jellyfin at
  `altiera:8096` is reachable from the host and answers its public endpoint,
  but the installed `/Applications/Vela.app` is unsigned/ad-hoc and fails its
  authentication transport request. A fresh locally signed, notarized,
  Gatekeeper-accepted universal build independently confirmed the packaged
  `Info.plist` still lacks Apple's `NSLocalNetworkUsageDescription`; Apple says
  apps that connect directly to local hosts should declare it. `altiera` also
  advertises four IPv6 addresses that refuse 8096 before its working IPv4
  addresses; direct-IP Vela isolation test is still pending. Local signing
  setup details live in `.agents/machines.md`; no product fix is approved yet.

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

- 2026-10-03 build-health pass on an Arch-family x86_64 host:
  `scripts/build.sh` now checks Linux system packages (pacman/apt/dnf) and
  offers to install them (`--install-deps`; no terminal => prints the command
  and stops); the dnf package list is untested (assumption). The E2E driver
  fetch uses Launchpad's permanent URL after the archive pool 404'd the pinned
  2.52.3 package. `devalue` 5.9.4 and `rustls` 0.23.45 clear the then-open
  advisories. On that host the full verification set, E2E 39/39 (driver 2.52.3
  against system WebKitGTK 2.52.6), and the Arch package build passed. Nothing
  here was exercised on macOS or Windows; `scripts/build.ps1` has no
  prerequisite check.
- Release code signing merged at `b9c6199` (2026-08-07) shipped in 1.0.60.
  The wiring detail and durable tauri-cli signing facts live in
  `docs/history/state-archive.md` (2026-08-08 rotation). The merged
  `ci/release-code-signing` branch still exists locally and on `github`;
  deleting it is owner-gated.

## Next

- Complete physical AirPlay sync, route-transition, pause/seek/mute/volume, and
  startup-preroll acceptance on the running Vela 1.0.61 DMG, then close the
  implementation record.
- Owner decision after direct-IP isolation: plan the macOS local-network fix
  (package the usage description, guard the built app metadata, and surface
  actionable transport causes instead of reqwest's shallow request error).

- `.agents/plans/server-identity-sidebar.md` (drafted 2026-11-12, NOT
  approved): per-kind server marks in the sidebar Sources list and Settings,
  and a flat Server Playlists list that collapses when empty. The owner's
  D1-D4 rulings are recorded there verbatim (D4: marks everywhere a bare
  server name is shown); only the go is open.
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
- `.agents/plans/macos-airplay-sync.md` (implemented for 1.0.61; packaged
  physical acceptance pending)
- `README.md`, `RELEASE_NOTES.md`, `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
