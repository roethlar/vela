# Plan: current dependencies and Node 26 immediate-next-LTS baseline

Status: **IMPLEMENTING — all eight slices landed; final canonical verification
and external review of the Slice 8 integration/version range are pending.** The
owner approved the
complete plan on 2026-07-15 and separately approved Node 26, the npm security
posture, and the Linux E2E VM Node/npm alignment during plan drafting.

Implementation log:

- Slice 1 pins Node 26.5.0/npm 12.0.1, aligns `@types/node`, moves the
  JavaScript actions to current majors, and aligns the E2E VM through a
  reversible user-local install. Local clean install/check/build and the full
  locked Rust suite passed; the VM clean install and real-app smoke passed.
  Review r1 admitted `dlr-s1-1`: direct `execFileSync('npm.cmd')` would block
  the Windows release leg. `adc0104` uses the platform shell for the static npm
  query; Grok accepted the full corrected slice at r2. The Codex self-review
  that found the defect is author evidence, not independent review. Durable
  finding: `.agents/review/findings/dlr-s1-1.md`.
- Slice 2 (`986fa2e`) replaces only the obsolete Ubuntu appindicator
  prerequisite spelling with Tauri's current Ayatana GTK3 development package.
  Canonical's jammy/noble indexes and Tauri's prerequisite guide confirm it;
  YAML plus the full local command set passed. Grok 0.2.101 accepted exact base
  `3e7fd4c` and head `986fa2e` with no comments.
- Slice 3 (`28159ea`) moves the compatible frontend graph to Kit 2.69.3,
  plugin-svelte 7.2.0, Svelte 5.56.5, svelte-check 4.7.2, TypeScript 6.0.3,
  Vite 8.1.4, Tauri JS API 2.11.1, and CLI 2.11.4. It scopes `cookie` 0.7.2
  to Kit, explicitly denies the optional `fsevents` native hook, adds
  fail-closed npm audit gates, and moves the CSS-only Geist import out of
  TypeScript. Removing only the cookie override makes npm audit fail on
  GHSA-pxg6-pf52-xh8x; the landed graph audits clean. Local canonical checks,
  the Vite HMR protocol proof, Tauri build, and Linux real-app E2E 18/18 passed;
  Cargo manifests stayed byte-identical. Grok 0.2.101 accepted exact base
  `e26add4` and head `28159ea` with no comments. A concurrent Codex CLI run was
  stopped and discarded after the owner clarified that author-model
  self-review does not count.
- Slice 4 (`770bfba`) refreshes the compatible Cargo lock and adds an explicit
  Rust 1.89 CI check while retaining rolling stable for check, clippy, and
  tests. The graph moves from 504 to 487 packages and 41 to 39 duplicate-name
  families with no new duplicate family; `getrandom`'s removed WASI Preview 3
  branch explains the large WIT/Wasm deletion. Cargo audit remains at zero
  vulnerabilities and drops the fixed `anyhow` warning, leaving 17 visible
  upstream warnings. Rust 1.89 check, stable check/clippy/95 tests, frontend
  and Tauri builds, and Linux real-app E2E 18/18 passed. `Cargo.toml` stayed
  byte-identical. Grok 0.2.101 accepted exact base `20fc059` and head `770bfba`
  with no comments.
- Slice 5 (`8559c59`) raises only `directories` to 6.0.0 and consolidates
  `dirs-sys` on 0.5.0. Executable before/after probes preserved the exact
  macOS, default Linux, and XDG-overridden Linux config directories; upstream
  v5/v6 platform source is byte-identical, and the Windows binding-only null
  handle change preserves current-user semantics. Rust 1.89 and stable gates,
  audit, frontend checks, and Linux real-app E2E 18/18 passed; `sortpersist`
  proved exact-file write and restart readback. Grok 0.2.101 accepted exact
  base `fb16141` and head `8559c59` with no comments.
- Slice 6 migrates Plex XML to `serde-xml-rs` 0.8.2 (`fa3d04f`) and adds the
  required real-server browse/detail/episode/play/watch coverage plus
  signal-safe cleanup (`69a8f83`, selector correction `3a002fa`). The
  dependency-only worktree produced the expected three targeted failures;
  six independent mapping regressions then failed their exact assertions after
  the fix. Rust 1.89/stable checks, clippy, all 97 tests, Cargo audit, frontend
  check/build, Linux real-app E2E 18/18, and live Plex 1/1 passed. The first
  live attempt correctly failed on a harness selector that compared an episode
  title together with its rendered index tag; `3a002fa` isolates the title text
  node, and the rerun passed. Post-run proof found credentials removed, Plex
  plus its watchdog active, and the watch fixture unwatched at zero progress.
  Grok 0.2.101 independently red-proved an XML attribute guard and accepted
  exact base `c8b9159` and head `3a002fa` with `guard_confirmed: true` and no
  comments at 2026-07-15T17:23:11Z. At the owner's request, Claude Code
  2.1.210 using `claude-fable-5` then independently ran its own red/restore/
  green guard and accepted the same exact base/head with
  `guard_confirmed: true` and no comments at 2026-07-15T17:34:39Z.
- Slice 7 (`1d619fd`) raises the direct reqwest dependency to 0.13.4 with
  defaults disabled and explicit `json`, `query`, `charset`, `http2`,
  `system-proxy`, and `native-tls-no-alpn`. The resolved graph has one reqwest
  0.13.4, shared with Tauri, and no reqwest 0.12 or enabled reqwest
  `default-tls`, rustls, or ALPN-bearing `native-tls` feature. Removing only
  `query` failed compilation at Vela's real Plex, Jellyfin, and release query
  call sites; restoring it returned the locked build green. Rust 1.89/stable
  checks, clippy, all 97 tests, Cargo audit (zero vulnerabilities; 17 visible
  upstream warnings), frontend check/build, Linux real-app E2E 18/18, live
  direct-HTTPS Plex, live Jellyfin, Linux ARM64 deb/rpm packaging, and the
  macOS x86_64+arm64 universal app/DMG build passed. The configured live
  sources contain no Emby server, so live Emby remains an explicit coverage
  gap. Grok 0.2.101 independently red-proved the `query` guard, restored its
  disposable worktree green, and accepted exact base `8a563c9` and head
  `1d619fd` with `guard_confirmed: true` and no comments at
  2026-07-15T17:54:14Z.
- Slice 8 re-verified the VM's user-local Node 26.5.0/npm 12.0.1 paths, full
  Linux E2E 18/18, live Plex, and live Jellyfin; no configured Emby venue
  exists. The registry still reports installed `tauri-driver` 2.0.6 as the
  newest stable release. Integration audit finding `dlr-s8-1` found the local
  package scripts bypassed the pinned JavaScript executables; `4cba5db` adds a
  canonical assertion reused by local/CI/release paths, and Grok 0.2.101
  independently red-proved both version legs and accepted exact base `33163c5`
  and head `0934628` with no comments. Audit finding `dlr-s8-2` found Ubuntu
  26.04 now supplies WebKitWebDriver 2.52.3 matching the VM's WebKitGTK, so
  `ec7c43e` replaces the skewed Debian 2.50.6/ICU72 fixture with official
  SHA-pinned ARM64/AMD64 packages and cache identity. The ARM64 session/IPC/UI
  probe and full E2E 18/18 passed; Grok independently proved the package/cache/
  checksum guard and accepted exact base `76c844c` and head `f3e5601` with no
  comments. `dc73627` applies the plan's one final version bump to 0.1.51. The
  post-version canonical suite and pinned integration-range review remain the
  closing gates.

Decision record: `.agents/decisions.md`, 2026-07-15. Audited against clean
`main` at `a0e936b` on 2026-07-15. Re-query every registry and release channel
immediately before its slice: the exact versions below are the verified targets
at the audit point, not permission to ignore a newer stable patch that remains
inside the same compatibility constraints.

Plan audit: three independent read-only domain passes reviewed the complete
draft. Round 1 found and this revision closes four material classes: npm 12 was
declared without activation, VM alignment followed tests that needed it, the
MSRV compiler was not installed, and reqwest `native-tls` would have enabled
ALPN while its proposed TLS-removal proof was vacuous. Round 2 found no
remaining or new material blocker in the JS/Actions, Rust/Tauri, or
runtime/VM scopes. This convergence is technical evidence, not owner approval.

## Objective

Bring every repo-versioned runtime, package, crate, action, lockfile, and
security gate to the newest stable mutually compatible set. Use Node 26 because
the owner explicitly chose the immediate next LTS before its October 2026 LTS
promotion. For ecosystems without an LTS channel, "current" means newest
stable, never a prerelease and never a version outside a direct dependency's
declared peer/MSRV/platform range.

The work is complete only when:

- clean installs resolve without `--force`, `--legacy-peer-deps`, or audit
  autofix downgrades;
- npm and Cargo locks contain the selected current graph;
- npm and Cargo vulnerability audits exit zero;
- the frontend static SPA, Rust backend, Plex XML, direct HTTPS server paths,
  packaging inputs, and Linux real-app suite retain behavior;
- each code slice is committed, externally reviewed by Grok on a pinned diff,
  and any admitted review fix is its own commit;
- Vela is versioned once, at the end, as 0.1.51.

## Audited targets

### Runtime and workflow tooling

| Surface | Baseline | Target |
| --- | --- | --- |
| Node used by CI/release | 20 (EOL) | 26.5.0, pinned in `.node-version` |
| npm package manager | implicit/host-dependent | 12.0.1 |
| Node type declarations | 25.9.1 | 26.1.1 |
| `actions/checkout` | v4 | v7 |
| `actions/setup-node` | v4 | v7, reading the repo version file |
| `actions/upload-artifact` | v4 | v7 |
| `tauri-apps/tauri-action` | v0 | v1 |
| `Swatinem/rust-cache` | v2 | v2 (already current major) |
| Rust CI toolchain | rolling stable | rolling stable (already current 1.97) |
| declared Rust MSRV | 1.89, untested | 1.89, enforced in CI |

`tauri-action` v1 keeps every input Vela currently uses. Its relevant new
failure behavior is intentional: a tagged rerun asking for `releaseDraft: true`
fails if that release has already been published instead of mutating a
non-draft release. Keep the release runner on Ubuntu 22.04; it is an intentional
old-glibc build floor, not an outdated application dependency.

Node 26.5.0 bundles npm 11.17.0; `packageManager` is metadata and does not
install npm 12. Every local/CI/release/VM path must therefore activate npm
12.0.1 explicitly and assert both executable versions before installing the
project. The exact Node engine range is `>=26.5.0 <27`.

### Frontend graph

| Direct package | Locked baseline | Target |
| --- | --- | --- |
| `@fontsource-variable/geist` | 5.2.9 | 5.2.9 (current) |
| `@tauri-apps/api` | 2.11.0 | 2.11.1 |
| `@tauri-apps/plugin-dialog` | 2.7.1 | 2.7.1 (current) |
| `@sveltejs/adapter-static` | 3.0.10 | 3.0.10 (current) |
| `@sveltejs/kit` | 2.60.1 | 2.69.3 |
| `@sveltejs/vite-plugin-svelte` | 5.1.1 | 7.2.0 |
| `@tauri-apps/cli` | 2.11.2 | 2.11.4 |
| `svelte` | 5.55.8 | 5.56.5 |
| `svelte-check` | 4.4.8 | 4.7.2 |
| `typescript` | 5.6.3 | 6.0.3 |
| `vite` | 6.4.2 | 8.1.4 |

TypeScript 7.0.2 is deliberately excluded: current SvelteKit 2.69.3 declares
`^5.3.3 || ^6.0.0`, and even the SvelteKit 3 prerelease does not accept 7.
Vite 8 and plugin-svelte 7 are one atomic compatibility group; Vite 8 replaces
Rollup/esbuild with Rolldown/Oxc. Vela has no Rollup, esbuild, optimizeDeps, or
custom output hooks, but static SPA boot, HMR, and the Tauri bundle still need
behavioral checks.

Current SvelteKit still requests `cookie ^0.6.0`, affected by
GHSA-pxg6-pf52-xh8x. Add only a Kit-scoped npm override to `cookie 0.7.2`, the
closest patched line. The stricter validation is irrelevant to Vela's static
SPA, which defines no SvelteKit server cookie code, but the build and E2E suite
must still prove compatibility. Do not use npm's proposed force-fix: it
downgrades Kit/adapter packages to obsolete releases.

### Rust graph

| Direct crate | Locked baseline | Target/work |
| --- | --- | --- |
| `tauri` | 2.11.2 | 2.11.5 |
| `tauri-build` | 2.6.2 | 2.6.3 |
| `serde_json` | 1.0.149 | 1.0.150 |
| `uuid` | 1.23.1 | 1.23.5 |
| `directories` | 5.0.1 | 6.0.0 |
| `reqwest` | 0.12.28 | 0.13.4 |
| `serde-xml-rs` | 0.6.0 | 0.8.2 |
| other direct crates | current | retain current; refresh transitives |

The compatible lock refresh updates the Tauri runtime family and fixes the
`anyhow 1.0.102` unsoundness warning at 1.0.103. Latest Tauri still owns the
unmaintained GTK3/`unic-*` and GLib warnings; they are upstream-bound warnings,
not known vulnerabilities, and must be reported rather than hidden or claimed
fixed.

Rust has no LTS channel. Keep edition 2021 and `rust-version = "1.89"`; an
edition or MSRV increase is a language/platform decision, not dependency
freshness. The new CI MSRV check is the guard against a transitive crate
silently violating the declared floor.

## Scope boundaries

In scope:

- `package.json`, `package-lock.json`, `Cargo.toml`, and `Cargo.lock`;
- repo-visible Node/npm selection and GitHub Actions dependencies;
- CI vulnerability enforcement and MSRV enforcement;
- Tauri's Linux prerequisite package name where the current supported package
  is available on both CI images;
- machine documentation and the owner-authorized Node/npm alignment on the
  existing Linux E2E VM;
- source/test migrations strictly required by the selected dependency APIs.

Out of scope:

- raising the Ubuntu 22.04 release image and therefore Vela's glibc floor;
- changing Rust edition or the MSRV, or pinning rolling stable to one compiler;
- changing reqwest from native platform TLS/trust/proxy behavior to the new
  rustls/AWS-LC default;
- OS-managed mpv, FFmpeg, WebKitGTK, GTK, Xcode, MSVC, WebView2, WKWebView,
  Xvfb, curl, bsdtar, or makepkg versions;
- replacing the SHA-pinned Debian WebKitWebDriver 2.50.6/ICU 72 fixture unless
  the implementation audit finds a newer packaged driver that passes the
  existing WebKitGTK 2.52 automation handshake. It is a compatibility fixture,
  not a floating application package;
- feature work, formatting sweeps, edition migration, or unrelated stale
  metadata/comments.

## Slice 1 — Node 26/npm 12 and JavaScript action runtimes

Commit one coherent runtime/tooling slice:

1. Add `.node-version` pinned to Node 26.5.0. Add
   `engines.node: ">=26.5.0 <27"` and `packageManager: npm@12.0.1`. Align
   `@types/node` to 26.1.1 and regenerate the matching lock subset in this
   slice, before requiring `npm ci`.
2. Make both CI and release `setup-node` steps read `.node-version`. Update all
   checkout/setup-node/upload-artifact uses to v7 and tauri-action to v1. Set
   `package-manager-cache: false` explicitly so adding `packageManager` does not
   silently enable setup-node's automatic npm cache.
3. After every CI/release setup-node invocation, install npm 12.0.1 from the
   npm registry and assert exact `node --version` and `npm --version` values
   before `npm ci`. The registry integrity check is part of npm installation;
   do not fetch or execute an unverified bootstrap script.
4. Keep `Swatinem/rust-cache@v2` and `dtolnay/rust-toolchain@stable`; their
   selectors are current by design.
5. Set the same exact Node/npm pair locally; a different local executable is a
   failed prerequisite, not permission to rewrite the lock with another npm.

Verify with a clean npm 12 install, frontend check/build, the full locked Rust
command set, YAML parse/actionlint if available, and inspection that no workflow
still selects Node 20 or a superseded action major. Confirm every v7 action's
published runtime no longer uses Node 20. The action-major behavior remains
pending GitHub-hosted execution until an owner-approved push.

After local verification, align the E2E VM before any later Linux test. This is
an authorized, non-root, reversible toolchain install:

1. Snapshot `command -v`/versions, the relevant `~/.local/bin` entries, and the
   system package state. Do not remove or replace Ubuntu's `/usr/bin/node` or
   `/usr/bin/npm`, and do not add an apt repository.
2. Download the official arm64 Node 26.5.0 tarball and its SHASUMS file from
   `nodejs.org`, verify the archive SHA-256, and extract it into the versioned
   user directory `~/.local/opt/node-v26.5.0`.
3. Point only `~/.local/bin/node`, `npm`, and `npx` at that versioned tree. Use
   the new Node executable to install npm 12.0.1 into the same tree; never pipe
   a remote installer into a shell. Preserve the VM's existing unrelated
   `~/.local/bin/claude` entry.
4. The existing `~/.profile` already prepends `~/.local/bin` when present.
   Prove `bash -lc` — the E2E launch mode — resolves the user-local paths and
   exact versions; also prove the Ubuntu packages remain installed and direct
   `/usr/bin/node` is unchanged.
5. Rollback is removal of only those three symlinks and the one versioned
   directory, restoring the snapshotted system executables. Perform it if any
   version/PATH/install check fails.
6. Update `.agents/machines.md` with the observed successful result and run a
   clean install plus one E2E smoke before proceeding. All later E2E/live checks
   therefore run on the target runtime, not the old Node 22/npm 9 pair.

## Slice 2 — current Linux build prerequisite name

On both Ubuntu 22.04 and the current `ubuntu-latest`, prove
`libayatana-appindicator3-dev` exists, then replace the obsolete
`libappindicator3-dev` prerequisite spelling in CI and release. A missing
package is a real roadblock; do not add an `||` fallback that conceals it.
Keep the Ubuntu 22.04 runner and every other system package family unchanged.

Validate YAML/actionlint locally. Prove the package on both runner images using
official package indexes or matching clean containers; GitHub-hosted execution
remains the final proof after an approved push.

## Slice 3 — frontend compatibility set and npm security gate

Commit the frontend graph atomically:

1. Re-query direct/latest and peer metadata, then update all direct npm lower
   bounds to the current compatible targets above. Do not install TypeScript 7.
2. Add the Kit-scoped `cookie: 0.7.2` override and add npm audit to both the CI
   frontend gate and the release workflow before packaging. Preserve the
   owner's fail-closed policy at every severity; do not lower the audit
   threshold or add a blanket exception.
3. Run npm 12's install-script inventory. Remove or update the exact
   `allowScripts.esbuild` entry after inspecting the new graph. Vite 8 no
   longer uses esbuild; do not retain an inert stale allowlist entry, silently
   skip a required native hook, or authorize unrelated install scripts.
4. Regenerate `package-lock.json` from a clean npm 12 resolution. Never use
   `--force` or `--legacy-peer-deps`; an ERESOLVE is evidence to investigate.
5. Keep the existing SPA fallback and Tauri dev-server contract. Make only
   migration changes required by official Kit/Svelte/Vite/TypeScript APIs.

Verification and guard proof:

- `npm ci`, `npm run check`, `npm run build`, and `npm audit` must all pass;
- inspect `npm ls --all` for invalid/peer errors and adjudicate `npm outdated`:
  TypeScript 7 is an expected incompatible-latest row, while an unexamined
  compatible row fails the slice. `npm outdated` is inspection, not a required
  zero exit;
- require the install-script inventory to contain no unreviewed required hook;
- in a disposable worktree, run the dev server and browser client, add a
  temporary visible marker, and witness the marker update through HMR without
  restarting the server; dispose the worktree afterward;
- temporarily remove the cookie override, regenerate only the lock in a
  disposable worktree, and prove `npm audit` fails on the named advisory;
- restore the committed slice and prove a clean install plus audit is green;
- run the full Linux real-app E2E suite on the Slice-1-aligned VM.

## Slice 4 — compatible Cargo lock refresh and MSRV enforcement

Without changing direct major constraints, refresh the Cargo lock to the newest
compatible graph. Confirm the expected Tauri/build/runtime, serde_json, uuid,
anyhow, and transitive updates; inspect surprising removals or duplicate major
families before committing.

Add an explicit `dtolnay/rust-toolchain@1.89.0` install and
`cargo +1.89.0 check --locked` CI path without replacing rolling stable for
clippy/tests. Run all locked Rust checks, Cargo audit, the declared-MSRV check,
`cargo tree -d`, and the frontend build. Record the still-upstream-bound audit
warnings; zero vulnerabilities, not zero warnings, is the acceptance
condition. `cargo install cargo-audit --locked` intentionally follows the
latest scanner on rolling stable; it is a security tool, not a linked build
dependency, and its advisory database must stay current.

## Slice 5 — `directories` 6 path preservation

Raise only `directories` to 6.0.0. `ProjectDirs::from("com", "vela", "vela")`
is the sole call. Compare its before/after config paths on macOS and Linux,
including the E2E suite's isolated `XDG_CONFIG_HOME`; do not accept a path move
because the config contains credentials. Windows must compile in the release
workflow and the official v6 changelog/API must show no Windows path-policy
change.

Run the full locked Rust checks, Cargo audit, MSRV check, frontend build, and
the config-persistence E2E coverage. If a new path guard is needed, red-prove it
by substituting a wrong application/organization component and restoring from
the committed state.

## Slice 6 — `serde-xml-rs` 0.8 Plex XML migration

This is the highest-risk source slice and must remain isolated.

1. Before source changes, raise only the crate in a disposable worktree and run
   the targeted Plex XML tests. Record the dependency-only red result.
2. Migrate every Plex XML attribute mapping in `plex_library.rs` to the 0.8
   `@attribute` syntax. Keep `Video`, `Directory`, `Metadata`, `Guid`, `Media`,
   `Part`, `Genre`, `Director`, `Writer`, `Country`, `Role`, and `Stream` as
   child-element mappings. Vela uses neither old `$value` content mapping nor
   complex tuples; do not manufacture a content migration.
3. Update the obsolete comment that names serde_xml_rs 0.6 while preserving the
   manual hub parser unless tests prove 0.8 can replace it without losing order
   or repeated children. A dependency refresh is not permission to refactor a
   proven parser.
4. Strengthen fixtures so they independently assert:
   - library section scalar attributes;
   - listing Video and Directory attributes, timestamps, parent keys, Guid,
     Media, and Part children;
   - detail attributes plus Genre/Director/Writer/Country/Role/Media/Part/Stream
     descendants.

Red-prove each category separately after the fix by removing a representative
optional `@` mapping and demanding its exact assertion fail. A required-field
injection may instead prove the exact expected deserialization error. Restore
from the committed source after every injection. Then run all locked Rust checks, Cargo
audit, MSRV, frontend build, hermetic E2E, and live Plex browse/detail/episode/
scan/watch/play paths. The Jellyfin-only hermetic mock is not proof of this
slice.

## Slice 7 — reqwest 0.13 with preserved native TLS

Raise reqwest to 0.13.4 with no silent network-stack change:

```toml
reqwest = {
  version = "0.13",
  default-features = false,
  features = ["json", "query", "charset", "http2", "system-proxy", "native-tls-no-alpn"]
}
```

Re-confirm the exact 0.13.4 feature names before editing. Vela uses `.query()`
in Plex, Jellyfin, and release/download paths; `query` must be explicit. It does
not use reqwest `.form()`, so do not enable `form` without evidence. Native TLS
without ALPN preserves reqwest 0.12's prior platform roots, private trust,
proxy behavior, and TLS negotiation instead of adopting either reqwest 0.13's
rustls/AWS-LC default or 0.13 `native-tls`'s newly enabled ALPN. This is also the
lower-risk macOS universal build.

Guard and verification:

- remove `query` temporarily and prove compilation fails at a real Vela query
  call, then restore;
- structurally inspect `cargo tree -e features -i reqwest@0.13.4` and the
  manifest: require `native-tls-no-alpn`, and reject reqwest `default-tls`,
  `rustls`, `native-tls`, or any direct 0.12 feature. Do not use a remove-all-TLS
  injection; it proves only that HTTPS needs some backend and is vacuous for
  backend identity;
- use live direct-HTTPS Plex as the positive platform-TLS behavior proof, with
  every Plex service restoration rail intact;
- confirm the prior direct 0.12/indirect 0.13 duplicate family is gone;
- run all locked Rust checks, audit, MSRV, frontend build, full hermetic E2E,
  live Plex, and live Jellyfin. Exercise Emby live only if an owner-configured
  server exists; absence is a stated coverage gap, not a reason to invent
  credentials;
- run practical native package builds. macOS universal and Linux bundles are
  local/VM checks; Windows packaging remains GitHub-hosted unless a current
  Windows checkout is available.

## Slice 8 — integration, version, and durable state

After Slices 1–7 are independently green and review-accepted:

1. Re-verify the Slice 1 user-local VM Node/npm paths and run the full Linux E2E
   suite plus the live Plex and Jellyfin scenarios required above.
2. Audit and record that VM tauri-driver 2.0.6 remains current. Audit the
   vendored WebKitWebDriver URL/checksums and current 2.50.6-to-2.52.x
   handshake. Retain the fixture if no newer packaged compatible driver exists;
   do not turn "all dependencies" into an unguarded binary replacement.
3. Run `scripts/bump.sh 0.1.51` only after integration is green. Update the plan
   status, `.agents/state.md`, `.agents/repo-guidance.md` verification list
   (including npm audit/MSRV where canonical), and any changed setup docs.
4. Re-run the full canonical suite after the version/docs commit and require a
   clean worktree.

Do not push or trigger release/CI workflows without a separate owner go. When a
push is authorized, GitHub CI must prove the action v7/Node 26/MSRV/npm-audit
jobs. A manually triggered cross-platform release build is recommended because
the release workflow and Tauri action major changed; triggering it is a separate
outward-facing action and needs explicit approval.

## Review protocol

Every code slice and every review-fix slice goes through Grok reviewloop on the
same pinned base/head diff, with no round cap. The Codex author never counts a
Codex CLI run as review; Claude is the eligible external fallback or
adjudicator. The author runs the guards and red proofs; reviewers do not mutate
the main worktree. Apply the standing 2026-07-14 decision, as amended
2026-07-15, for admission, declined-finding adjudication, and owner escalation.
A round with no material finding is a valid acceptance; a round that produces
no verifiable delta after a reopen is a stall. Stop and surface after three
consecutive stalled cycles.

## Final verification matrix

- clean Node 26.5.0/npm 12.0.1: `npm ci`, `npm ls --all`, `npm outdated`,
  `npm audit`, `npm run check`, `npm run build`;
- from `src-tauri`: `cargo check --locked`, `cargo clippy --all-targets
  --locked -- -D warnings`, `cargo test --locked`, Cargo audit, Rust 1.89
  locked check, duplicate/feature-tree inspection;
- macOS: static/Tauri smoke and universal bundle where practical;
- Linux VM: full hermetic E2E, live Plex, live Jellyfin, and Linux bundles;
- Windows: compile/package through the updated GitHub release workflow when
  outward execution is approved;
- actionlint/YAML validation locally and GitHub-hosted CI after an approved
  push;
- clean git status, all slices committed, plan/state/decision evidence current.

## Known residual risks

- Node 26 is pre-LTS until October 2026 by owner choice. Package/runtime bugs
  discovered before promotion may require patch updates.
- Vite 8 changes the bundler implementation even though Vela's config uses no
  removed hooks; UI and bundle behavior remain the proof.
- serde-xml-rs 0.8 changes the mapping model across every Plex DTO; live Plex
  coverage is mandatory and a second Plex rebind remains unavailable as already
  recorded in state.
- reqwest native TLS preserves the prior architecture, but private-root and
  system-proxy combinations cannot all be synthesized hermetically.
- Latest Tauri still carries upstream unmaintained GTK3/GLib/Unicode warnings.
  They remain visible; no blanket audit suppression is permitted.
- Windows packaging cannot be claimed locally from the macOS/Linux venues.
