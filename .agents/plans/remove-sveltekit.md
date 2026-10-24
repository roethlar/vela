# Plan: Remove SvelteKit; keep Svelte 5 and Vite as a plain single-page app

## Status

**APPROVED, revision 1 (owner, 2026-10-23). G1 ruled "rename". Slices 1-4 are
implemented on branch `remove-sveltekit`, which is not merged. Before merge:
Slice 0, the VM E2E proofs, the Mac desktop smoke, and the Review step (see the
implementation log).**

- Approval: the owner answered "go with your rec" when the recommendation was
  to remove SvelteKit and keep Svelte 5 and Vite. The owner then set the
  standing direction "do everything in the most correct manner possible,
  commit and push as needed."
- G1: ruled "rename" (`src/routes/+page.svelte` becomes `src/App.svelte`), the
  plan's recommendation. Slice 3 runs.

Owner direction (2026-10-23): remove SvelteKit from Vela instead of upgrading
it to Kit 3. Keep Svelte 5 and Vite. Node stays as a build and test tool; the
owner chose this over a Node-free rewrite. This plan supersedes
`.agents/plans/sveltekit-3-upgrade.md`, which was never approved or
implemented. Its research is reused here: the advisory assessment, the
tsconfig/tests coverage hazard, the guard red-proof method, and Slice 0.

Approving this plan also approves:

- removing the Kit-scoped `cookie: 0.7.2` override that the 2026-07-15
  dependency decision approved (it has no target once Kit is gone);
- replacing the `.agents/repo-guidance.md` Mission Detail statement "SvelteKit
  is configured as a static SPA for Tauri";
- the patch version bump in Slice 2, which the 2026-06-20 "bump version on code
  change" decision requires.

Record the owner's approval wording, and the G1 ruling when it lands, on this
status line before Slice 0 starts. The `.agents/decisions.md` entry that
carries the same wording lands in Slice 4.

Owner gate (ruled "rename", 2026-10-23):

- **G1, rename the root component.** Slice 3 runs only if the owner rules
  "rename". Slices 0, 1, 2 and 4 do not depend on G1. If the owner rules
  "keep", delete Slice 3 from this plan and drop the rename paragraph from the
  Slice 4 decision text.

Implementation log (2026-10-23, branch `remove-sveltekit` off `main`
`b365873`):

- Venue: the Windows authoring host, per Host constraints. All work ran in an
  LF worktree (`git -c core.autocrlf=false worktree add`) with Node 26.12.1 and
  `npx -y npm@12.0.1` for every npm command. Not run there:
  `node scripts/check-js-toolchain.mjs` (the PATH npm is 11.20.0) and every
  Cargo command (no MSVC build tools). `scripts/bump.sh` can't run (no
  `python3`), so both bumps used a line-for-line Node port of its embedded
  Python, kept outside the repo. Each bump diff touched only the seven version
  fields (`package.json`, the two `package-lock.json` root copies,
  `tauri.conf.json`, `Cargo.toml`, the `vela` entry in `Cargo.lock`, the
  `PKGBUILD` `pkgver`) and `BUILD_DATE`, which became `2026-10-24` because the
  script uses the UTC date.
- Slice 0: NOT RUN. It needs the owner's Mac and the E2E VM. No E2E baseline
  exists yet.
- Slice 1 (`149f2c1`): the font assertions are in `smoke`. Only a syntax and
  module-load check ran. The on-Kit green run and red-proofs F1 and F2 are NOT
  RUN (VM only).
- Slice 2 (`ecf0d9c`), base captures taken at `149f2c1`:
  - A1: base `npm audit` exit 1, naming GHSA-pfhg-q3qv-9q6r and
    GHSA-27mw-82g8-phjj. Head: exit 0 on a clean `npm ci`, "found 0
    vulnerabilities". `npm ls @sveltejs/kit @sveltejs/adapter-static cookie`
    is empty. `npm ls --all` shows no invalid, missing or peer errors; its
    only change from the base is one fewer UNMET OPTIONAL entry, Kit's
    `@opentelemetry/api`.
  - Lock: exactly the 11 probe packages were removed. Nothing was added. The
    only changed entry is the root `packages[""]`, which lost the two Kit
    devDependencies.
  - `npm run check`: node tests 68 pass, 2 skipped, 0 fail (base: 64 pass,
    2 skipped). svelte-check went from `COMPLETED 303 FILES` to `295 FILES`,
    0 errors, 0 warnings. The non-`node_modules` `tsc --listFilesOnly` set is
    exactly `src/main.ts`, `src/lib/{errors,imageReveal,types}.ts` and
    `vite.config.js`. `--showConfig` compiler options differ from the base
    only in `paths` (re-based, `$app/types` dropped), the dropped `rootDirs`,
    and the added `types`. `include`/`exclude` differ only in spelling; Kit's
    service-worker excludes matched no file.
  - `npm run build`: no warnings; the post-build check passes. The output is
    `build/index.html`, `build/favicon.png`, and `build/assets/` with one `.js`,
    one `.css` and five `.woff2`. There is no `build/_app`. The five woff2
    sha256 values equal the base's.
  - Dev server: `vite dev` bound 1420 and served `index.html` with the
    `/src/main.ts` entry. A second `vite dev` exited 1 with "Port 1420 is
    already in use". NOT RUN: HMR in a disposable worktree, `npm run tauri dev`,
    the Mac desktop smoke (WKWebView), the VM `npm run e2e`, and
    `cargo +stable check --locked`.
  - `resolveConfig({ root, configFile }, "build", "production")` works under
    `node --test`; `tests/frontend-shell.test.mjs` uses it, not the fallback.
  - Red-proofs, in a disposable LF worktree at `ecf0d9c` with its own `npm ci`,
    restored each time with `git checkout HEAD -- .` and shown green again:
    - T1-T5: svelte-check reported `Type 'string' is not assignable to type
      'number'` at `src/lib/Icon.svelte` 2:9, `src/routes/+page.svelte` 2:9,
      `src/main.ts` 10:14, `src/lib/types.ts` 205:14, and `vite.config.js`
      47:36.
    - T6: `src/main.ts` 3:8, "Cannot find module or type declarations for
      side-effect import of './app.css'".
    - T7: `npm run check` stops first at S3 (`tsconfig paths["$lib"] must be
      ["./src/lib"]`). svelte-check run directly reports `Cannot find module
      '$lib/...'` for each `$lib` import. `npm run build` fails because
      Rolldown can't resolve `$lib/EmptyState.svelte`.
    - S1a, S1b and S1c each fail S1 on their own message (outDir/frontendDist,
      emptyOutDir, post-build wiring). S2a and S2b each fail S2 (port,
      strictPort). S3a fails S3, and `npm run build` then fails with the
      misleading "failed to load virtual css module" message from the probe.
      S4a fails S4 (`id="app"` count) and S4b fails S4 (import order).
      `npm run check` exits 1 for every one of them.
    - B1 fails B1 ("0 module scripts"). B2 also fails, because without an
      entry Vite emits no stylesheet, and S4 fails. B2 fails B2 (no Geist
      `@font-face`, no woff2 url). B3 fails B3 ("5 CSS url() values inline a
      font as data:font"); B2's woff2-file assertion also fails, as it must
      when every font is inlined. B4 and B5 each fail only their own
      assertion, and B5 also fails `ui-foundation` ("first paint must accept
      the complete theme catalog"). B6: `149f2c1` (Kit) was built in that
      worktree, then the worktree was checked out at `ecf0d9c` with
      `emptyOutDir: false`. The build check failed only at B6. After the
      restore, the rebuild cleared `_app` and passed.
    - Webview mount: the local half ran. `getElementById("missing")` fails S4.
      The VM `smoke` half (`app render`) is NOT RUN.
- Slice 3 (`7b01fae`): `git grep -n "+page.svelte" -- tests src scripts
  ISSUES.md` returns nothing. `npm run check` is green, with svelte-check still
  at 295 files. `npm run build` is green, including the post-build check.
  Version 1.0.61 to 1.0.62. Red-proof at `7b01fae`: after
  `git mv src/App.svelte src/Gone.svelte`, all six node test files fail.
  `config-integrity`, `transcoding-ui` and `watch-edit-position` fail with
  ENOENT on `src/App.svelte`. `image-reveal` (2 tests), `ui-foundation`
  (5 tests) and `ui-motion` (4 tests) fail their assertions. Restored and
  green. S4b was re-proven at `7b01fae`, because its import line changed. The
  VM `smoke`/`uifoundation` runs are NOT RUN.
- Review: NOT RUN. The Review section needs the owner to choose `codereview` or
  `openreview` for the codex dispatch.
- Push: the owner's standing direction ("commit and push as needed") covers
  pushing this branch and opening a PR against `main`. Merging is not part of
  that direction. GitHub CI on the PR is the first CI run of `npm run check` and
  `npm run build` since the audit gate went red.
- CI: PR #2 (https://github.com/roethlar/vela/pull/2), `ci.yml` run
  40059537619 at `367bdf6`, passed every job:
  - Frontend: `npm ci`, `npm audit` ("found 0 vulnerabilities"),
    `npm run check` (70 pass, 0 fail; svelte-check 0 errors, 0 warnings), and
    `npm run build` ("check-frontend-build: B1-B6 passed");
  - Rust: the 1.89 MSRV check, and stable check, clippy `-D warnings` and
    test;
  - cargo audit.

  The release workflow's `npm-audit` job wasn't run, because no release was
  dispatched.

## Goal

`npm audit` fails closed on `main` in CI (`.github/workflows/ci.yml` Frontend
job) and in the release workflow's `npm-audit` job
(`.github/workflows/release.yml`; `bundle` and the Arch job `need` it). Because
the audit step fails first, CI never reaches `npm run check` or
`npm run build`, and releases are blocked. The Rust jobs are green. The cause
is two low-severity `@sveltejs/kit` advisories, GHSA-pfhg-q3qv-9q6r and
GHSA-27mw-82g8-phjj, which also flag `@sveltejs/adapter-static` because it
depends on Kit. Neither has a 2.x fix.

Vela uses no SvelteKit feature. `src/` imports nothing from `$app/*`,
`$env/*` or `@sveltejs/kit`. There are no load functions, form actions,
`+server`/`+page.server` files, hooks, or `<a>` elements; no `history.*`,
`location.*` or `sessionStorage` use. There is one route, and
`src/routes/+layout.ts` exports only `ssr = false`. Kit contributes only a
static SPA shell, generated TypeScript config, and the advisories. Removing it
takes both advisories out of the dependency graph.

Done means all of the following hold:

1. `npm audit` exits 0 on a clean Node 26.12.1/npm 12.0.1 install, with no
   audit exception, no threshold change, and no `npm audit fix --force`.
   `npm ls @sveltejs/kit @sveltejs/adapter-static cookie` lists nothing.
2. The frontend is a Svelte 5 single-page app built by Vite with
   `@sveltejs/vite-plugin-svelte`. A root `index.html` loads `src/main.ts`,
   which mounts the root component with Svelte 5's `mount()`. Vite writes the
   build to `build/`, which Tauri embeds (`frontendDist: ../build`, unchanged).
   The dev server stays on strict port 1420 with the existing `TAURI_DEV_HOST`
   HMR wiring.
3. `npm run check` (node tests plus svelte-check) is green with 0 errors and 0
   warnings, and svelte-check provably still checks `src/` (red-proofs T1-T7).
4. The new guards hold, each red-proofed separately (Slices 1 and 2).
5. The Linux real-app E2E suite is no worse than a baseline taken on the
   pre-migration commit, and the Geist font plus app CSS are applied in the
   webview.

## Advisory applicability (assessment, not a waiver)

Carried over from the superseded plan and still true at `c112253`:

- GHSA-pfhg-q3qv-9q6r (open redirect through `use:enhance` enhanced form
  actions) needs enhanced form actions. Vela has no `use:enhance` and no form
  actions. Its only `<form>` (`src/lib/PlaylistsView.svelte`) is a plain
  `onsubmit` with `preventDefault()` that calls `invoke()`.
- GHSA-27mw-82g8-phjj (XSS through an unescaped `setHeaders` `cache-control`
  value in a prerendered page) needs `setHeaders` and prerendering. Vela has
  neither. The only generated HTML file is adapter-static's SPA fallback.

The exposure is nil. The work is still required because the gate fails closed
under the 2026-07-14 known-vulnerabilities rule, which the 2026-07-15 dependency
decision extended to npm. That decision forbids blanket suppression, and npm has
no per-advisory ignore. After this plan, neither advisory's package is
installed, so no applicability argument is needed going forward.

## What SvelteKit provides today, and its replacement

| Kit piece today | Replacement (end state in Slice 2) |
| --- | --- |
| `src/app.html` with `%sveltekit.assets%`, `%sveltekit.head%`, `%sveltekit.body%`, `data-sveltekit-preload-data` | Root `index.html` (moved with `git mv`). It keeps the pre-paint theme script, the `<div style="display: contents">` wrapper (now `id="app"`), and a `<script type="module" src="/src/main.ts">` entry. The Kit placeholders and preload attribute are removed. |
| Kit's client bootstrap: an inline script that imports `start.js`/`app.js` and calls `kit.start(app, element)`, which mounts a generated Root around layout and page | `src/main.ts`, which imports `./app.css` and then the root component, and calls `mount(App, { target: document.getElementById("app") })`. |
| `src/routes/+layout.svelte` (`import "../app.css"` plus `{@render children()}`) | Deleted. The global CSS import moves to `src/main.ts`, ahead of the component import so `app.css` still comes first in the cascade. `app.css`'s own `@import "@fontsource-variable/geist"` carries the fonts unchanged. |
| `src/routes/+layout.ts` (`export const ssr = false`) | Deleted. With no SSR, there is nothing to turn off. |
| `src/routes/+page.svelte` (the whole app) | Unchanged in Slice 2 and mounted directly. Renamed to `src/App.svelte` in Slice 3 only if G1 is ruled "rename". |
| `static/` (only `favicon.png`), served at `/` | `public/favicon.png` (moved with `git mv`); Vite's default `publicDir` copies it to `build/favicon.png`. `index.html` references `/favicon.png`. |
| `$lib` alias, generated into `.svelte-kit/tsconfig.json` `paths` | `tsconfig.json` `paths` (`$lib`, `$lib/*` to `./src/lib`) is the single definition. Vite 8 reads it through `resolve.tsconfigPaths: true`, and svelte-check reads it natively. No `src/` import changes. |
| `.svelte-kit/tsconfig.json` as the `extends` base | A standalone `tsconfig.json` that carries the generated base's compiler options itself, plus `types: ["svelte", "vite/client"]` (TS 6 no longer auto-includes types, and `vite/client` declares the `./app.css` side-effect import). `include` matches the generated base's effective coverage (see the hazard note below). |
| `svelte-kit sync` in `check` | Removed. `check` ends `&& svelte-check --tsconfig ./tsconfig.json`. No other svelte-check invocation change is needed (probe-verified). |
| `svelte.config.js` (`preprocess` plus `kit.adapter`) | `svelte.config.js` keeps only `preprocess: vitePreprocess()`. vite-plugin-svelte and svelte-check both read it. |
| `sveltekit()` Vite plugin plus adapter-static writing `build/` (`index.html`, `_app/immutable/**`, `_app/version.json`, `favicon.png`) | `svelte()` plugin; `build.outDir: "build"` and `build.emptyOutDir: true`. The output is `build/index.html`, `build/favicon.png`, and `build/assets/*` (one JS, one CSS, five Geist woff2). `_app/` and `version.json` disappear; nothing reads them. |
| Dev server via `vite dev` with Kit middleware | Plain `vite dev`. `server` block (port 1420, `strictPort`, `TAURI_DEV_HOST` host and HMR 1421, `src-tauri` watch ignore) stays byte-identical. `tauri.conf.json` `beforeDevCommand`/`devUrl` are unchanged. |
| CSP and asset paths | `src-tauri/tauri.conf.json` is unchanged. Tauri hashes the inline pre-paint script into `script-src` at build time, as it does today for that script and Kit's bootstrap script. The new entry and stylesheet are external same-origin `/assets/*` files covered by `'self'`. Fonts are `/assets/*.woff2` under `default-src 'self'`. Vite `base` stays the default `/`, which matches Kit's absolute `/_app/` paths. Do not set `build.target` (Kit set none, so the Vite default is unchanged). |

Kit runtime behaviors that disappear, reviewed as inert for Vela: link-click
router interception (Vela has no `<a>`), scroll restoration and
`history.scrollRestoration = "manual"` (no navigation), the `svelte-announcer`
aria-live element (it announces only on navigation), version polling (off in
Kit 2), the `__sveltekit_*` global, and `data-sveltekit-preload-data` (no
links). E2E is the evidence that nothing relied on them.

**Coverage hazard (carried over).** Kit's generated config included
`src/**/*.{js,ts,svelte}`, `vite.config.{js,ts}`, and
`test(s)/**/*.{js,ts,svelte}`. None of that matches Vela's `tests/**/*.mjs`, so
the tests have never been type-checked. With `allowJs` and `checkJs`, adding
`tests` to `include` would newly type-check every unit, E2E and live harness
file. That is a scope expansion, so it is excluded.

## Feasibility probe (2026-10-23, evidence only)

Run on the Windows authoring host against an LF `git archive` export of
`c112253` in a scratch directory, with Node 26.12.1, npm 11.20.0 and the existing
`node_modules`. This was not the canonical npm, and nothing in the repo was
changed.

- Baseline (Kit): `node --test tests/*.test.mjs` gave 64 pass, 2 skipped, 0
  fail on LF. `svelte-kit sync && svelte-check --output machine` gave
  `COMPLETED 303 FILES 0 ERRORS 0 WARNINGS`. Non-`node_modules` files from
  `tsc --listFilesOnly` were the four `.svelte-kit/*.d.ts` files,
  `vite.config.js`, `src/lib/{errors,imageReveal,types}.ts`, and
  `src/routes/+layout.ts`.
- Candidate end state (the Slice 2 file contents below): `vite build` passed.
  It emitted `build/index.html`, `build/favicon.png`,
  `build/assets/index-*.js` (170 kB), `build/assets/index-*.css` (56.4 kB, the
  same total as Kit's two CSS files), and the same five Geist woff2 files with
  the same content hashes as Kit's build (`DjL33-gN`, `BEAKL7Jp`, `DC-KSUi6`,
  `BgDaEnEv`, `6IgcOCM7`). The built `index.html` kept the pre-paint script and
  gained `<script type="module" crossorigin src="/assets/index-*.js">` and a
  stylesheet link. The CSS contained no `data:font` URLs.
- svelte-check gave `COMPLETED 295 FILES 0 ERRORS 0 WARNINGS`. The
  non-`node_modules` tsc set was exactly `src/main.ts`,
  `src/lib/{errors,imageReveal,types}.ts`, and `vite.config.js`.
- Red: a type error injected separately into `src/main.ts`,
  `src/routes/+page.svelte` and `src/lib/errors.ts` was reported each time.
  Deleting `types` gave `Cannot find module or type declarations for
  side-effect import of './app.css'`. Deleting `paths` gave `Cannot find module
  '$lib/...'`.
- Deleting `svelte.config.js` changed nothing: svelte-check stayed at 0 and
  every built asset hash was identical. `vitePreprocess` is currently inert
  because Svelte 5 strips TypeScript natively. It is kept for parity with
  today's config and for future `<style lang>` or TS-only syntax. It has no
  guard, and none is claimed.
- With neither a Vite alias nor `tsconfigPaths`, the build failed, but with a
  misleading message: `failed to load virtual css module
  .../src/lib/*.svelte?svelte&type=style&lang.css` followed by `[lightningcss
  minify] Invalid empty selector`. That message means "`$lib` is not resolved".
  `resolve.tsconfigPaths: true` produced asset hashes identical to an explicit
  `resolve.alias`.
- `vite dev` bound 1420. It served `index.html` with `/src/main.ts` and
  rewrote `$lib/...` imports to `/src/lib/...`.
- Lock (npm 11 `install --package-lock-only --ignore-scripts`, after removing
  the two Kit packages and `overrides`): exactly 11 packages were removed
  (`@polka/url`, `@standard-schema/spec`, `@sveltejs/adapter-static`,
  `@sveltejs/kit`, `@types/cookie`, `cookie`, `kleur`, `mrmime`,
  `set-cookie-parser`, `sirv`, `totalist`), none were added or changed, and
  `npm audit` reported 0 vulnerabilities.
- Vite's `resolveConfig({ configFile }, "build", "production")` worked under
  `node --test`. It returned `build.outDir: "build"`, `server.port: 1420`,
  `strictPort: true`, and `resolve.tsconfigPaths: true`.

## Scope

In scope: `package.json`, `package-lock.json`, `svelte.config.js`,
`vite.config.js`, `tsconfig.json`, `index.html` (from `src/app.html`),
`src/main.ts` (new), deleting `src/routes/+layout.ts` and
`src/routes/+layout.svelte`, `public/favicon.png` (from `static/`),
`.gitignore`, one comment in `src/lib/Settings.svelte`, the version bump files
written by `scripts/bump.sh`, `scripts/check-frontend-build.mjs` (new),
`tests/frontend-shell.test.mjs` (new), `tests/ui-foundation.test.mjs` (one path),
`tests/e2e/scenarios/smoke.mjs` (font assertion),
`tests/e2e/scenarios/uifoundation.mjs` (one comment), the G1-gated rename, the
`docs/development.md` architecture line, the plan, state, decision and
guidance records, and the E2E VM Node re-alignment that `.agents/machines.md`
already records as pending.

Out of scope:

- Any `src/` behavior change. If the migration forces one, stop and amend this
  plan.
- `src-tauri/tauri.conf.json` (CSP, `frontendDist`, `devUrl`,
  `beforeDevCommand`, `beforeBuildCommand` all stay), and any Rust change
  beyond the version fields `scripts/bump.sh` writes.
- Freshness bumps: vite-plugin-svelte 7.3, svelte, svelte-check, vite,
  TypeScript 7. The versions of the kept packages do not move. The `typescript`
  range stays `~6.0.3`.
- Type-checking `tests/` (see the coverage hazard).
- `.github/workflows/*`. They call `npm audit`, `npm run check` and
  `npm run build` by name, and those names stay.
- A release, push, or CI-triggering action. These stay separately gated by
  `.agents/push-policy.md`; the fix rides the next owner-authorized release.
- Historical records that cite `src/routes/+page.svelte` or `src/app.html`
  (`.agents/review/**`, `.agents/plans/**`, `docs/history/**`, `.review/**`,
  `RELEASE_NOTES.md`). They describe the past and are not edited.
- Later candidates, not part of this plan: replacing `@tauri-apps/cli` with the
  cargo `tauri-cli`, and dropping `@tauri-apps/api` in favour of
  `app.withGlobalTauri`. Either needs its own plan.

## Host constraints

- **E2E venue.** The Linux VM (`.agents/machines.md`, Linux VM section) is
  reachable only from the owner's Mac over the host-only network. Slice 0,
  Slice 1's E2E proofs, and Slice 2's E2E run and VM red-proof all run from the
  Mac. Commits that are not pushed reach the VM by the scp route machines.md
  sanctions. Preferred form: `git bundle create <f> <base>..<head>` on the Mac,
  `scp` it, then on the VM inside `~/dev/vela-main` run `git fetch <f>` and
  `git checkout --detach <sha>`, and confirm `git rev-parse HEAD` equals the Mac
  SHA. `git push vm` needs the owner's explicit go (`.agents/push-policy.md`).
  Never run `git checkout -- .` on the VM's original `~/dev/vela` tree.
- **Windows authoring clone.** It has `core.autocrlf=true`, which makes 5
  `node --test` cases fail on unmodified `main`, and npm 11.20.0 instead of the
  pinned 12.0.1. It has no `python3`, so `scripts/bump.sh` cannot run, and no
  MSVC build tools, so there is no Tauri or Rust build. If Slice 2 is done
  there:
  - run everything in an LF worktree:
    `git -c core.autocrlf=false worktree add --detach <tmp> <sha>`, and prefix
    every later git command in it with `-c core.autocrlf=false`, including
    red-proof restores, or the checkout turns back to CRLF;
  - use `npx -y npm@12.0.1` for every npm command (`--version` must print
    12.0.1, then `ci`, `install`, `audit`, `run check`, `run build`);
  - record `node scripts/check-js-toolchain.mjs` as not run. It shells out to
    the `npm` on PATH, so it can't pass there. Never treat that as permission
    to regenerate the lock with npm 11.
  - Prefer the Mac for Slice 2 when its toolchain assertion passes there. It
    also has `python3` for the bump and WKWebView for the desktop smoke.

## Slice 0: E2E venue on the pinned toolchain (no repo code)

The repo pins Node 26.12.1/npm 12.0.1, so `scripts/check-js-toolchain.mjs`
rejects the VM's 26.5.0. Without this slice there is no E2E baseline. Run it
from the Mac.

1. Apply the reversible re-alignment recipe in the `.agents/machines.md`
   "Pending (2026-10-23)" bullet exactly as written: SHASUMS-verified official
   arm64 archive, `~/.local/opt/node-v26.12.1`, repoint only the three
   `~/.local/bin` symlinks, install npm 12.0.1 with that Node, and prove that
   `bash -lc` reports v26.12.1/12.0.1. Authority: the 2026-07-15 decision's
   grant to change Node/npm on the E2E VM (no other host), plus the owner's
   2026-10-23 pin go. Keep the 26.5.0 tree as the rollback.
2. In `~/dev/vela-main`, check out the pre-migration base (current `main`)
   detached. Run `bash -lc "node scripts/check-js-toolchain.mjs && npm ci &&
   npm run e2e"` (no `--skip-build`). Record the pass count and the exact
   failing scenario names; that is the baseline. Recorded known flakes:
   `continueon`, `playverbs`, `refresh`.
3. Update the machines.md Linux VM baseline and remove the Pending bullet.
   Commit as docs only: `docs: realign E2E VM to Node 26.12.1`.

Rollback: repoint the three symlinks at `~/.local/opt/node-v26.5.0` and delete
the 26.12.1 directory.

## Slice 1: runtime font guard on the Kit baseline (test only)

Nothing today asserts that the Geist font loads. `uifoundation` already asserts
computed theme tokens (`--surface`, `--text`, `--accent`), so app CSS
application is covered. This slice adds the font guard before the migration so
the same assertion is green on Kit and must stay green after it.

1. In `tests/e2e/scenarios/smoke.mjs`, after the existing `app render` wait
   and before the screenshot, add:
   - `await driver.waitFor(...)` on
     `return [...document.fonts].some((f) => f.family.replace(/["']/g, "") === "Geist Variable" && f.status === "loaded")`
     with the label `'Geist Variable font loaded'`;
   - an `assert.match` that `getComputedStyle(document.body).fontFamily`
     (read through `driver.exec`) matches `/^["']?Geist Variable["']?\s*,/`,
     with the message `'body must use the Geist UI font'`.

   Do not use `document.fonts.check()`. It returns true when no face matches,
   so it would pass vacuously.
2. Run `npm run e2e -- smoke` (no `--skip-build`) on the VM at the slice head.
   It must pass on unmodified Kit. If the new wait fails on unmodified Kit,
   stop: the assertion is wrong for WebKitGTK. Amend this plan; do not weaken
   it to pass.

Red-proofs on the VM, in a disposable `git worktree add --detach <tmp> <slice-1
sha>` with its own `npm ci`. Each injection needs a rebuild (no
`--skip-build`). Restore with `git checkout -- .` inside that disposable
worktree only, then show green again.

- F1, the face loads: delete `@import "@fontsource-variable/geist";` from
  `src/app.css`. Expect `smoke` to fail at `Geist Variable font loaded`.
- F2, the body uses it: change `font-family: var(--font-ui);` in `src/app.css`'s
  `html, body` rule to `font-family: system-ui;`. Expect `smoke` to fail at
  `body must use the Geist UI font`. The `h1, h2` rule keeps
  `--font-display`, so the face still loads and F1's wait passes, which proves
  the two assertions are independent.

Commit: `test: assert the Geist font loads in the E2E smoke`. No version
bump, because nothing that ships changes.

Rollback: `git revert <slice-1 sha>`.

## Slice 2: atomic SvelteKit removal plus shell guards

The migration can't be split. The `sveltekit()` plugin requires Kit, and Kit's
removal makes `src/app.html`, `src/routes/+layout.*`, `.svelte-kit/tsconfig.json`
and `svelte-kit sync` dead in the same step.

Prerequisites: a host where `node scripts/check-js-toolchain.mjs` passes, or
the Windows recipe in Host constraints; an LF checkout; and `python3` for
`scripts/bump.sh`.

Before editing, capture these on the base (Slice 1 head) into a scratch
directory outside the repo:

- `npm audit` output (the red evidence, naming both GHSAs);
- `npx svelte-kit sync && npx svelte-check --tsconfig ./tsconfig.json --output machine`
  (the `COMPLETED` line);
- `npx tsc -p tsconfig.json --listFilesOnly | grep -v node_modules`;
- `npx tsc -p tsconfig.json --showConfig`;
- after `npm run build`, `find build -type f | sort` and `sha256sum build/_app/immutable/assets/*.woff2`.

Leave that Kit `build/` in place; red-proof B6 needs it.

Steps:

1. Dependencies. In `package.json`, delete `@sveltejs/kit` and
   `@sveltejs/adapter-static` from `devDependencies`, and delete the whole
   `overrides` block, whose only entry is the Kit cookie pin. Every other range,
   `allowScripts`, `engines` and `packageManager` stay unchanged. Regenerate the
   lock with npm 12.0.1 `npm install` (no `--force`, no `--legacy-peer-deps`,
   no `npm audit fix`). The lock diff must remove exactly the 11 packages listed
   in the probe and change nothing else. Any added or changed package stops the
   slice until it is explained. Step 14's bump later changes only the lock's
   two root `version` fields.
2. Scripts. Set `"build": "vite build && node scripts/check-frontend-build.mjs"`.
   In `"check"`, append `tests/frontend-shell.test.mjs` to the explicit
   `node --test` list, and replace `&& svelte-kit sync && svelte-check --tsconfig
   ./tsconfig.json` with `&& svelte-check --tsconfig ./tsconfig.json`. `dev`
   (`vite dev`), `preview`, `tauri`, `e2e`, `e2e:live`, `build:arch` and
   `build:linux` stay unchanged.
3. `svelte.config.js` becomes exactly:

   ```js
   // Read by @sveltejs/vite-plugin-svelte and svelte-check. Vela has no SvelteKit;
   // see vite.config.js for the app shell.
   import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

   /** @type {import("@sveltejs/vite-plugin-svelte").SvelteConfig} */
   export default {
     preprocess: vitePreprocess(),
   };
   ```

4. `vite.config.js` becomes the following. The `clearScreen` comment block and
   the whole `server` block are byte-identical to today's:

   ```js
   // Vela's frontend is a single-page Svelte 5 app. Tauri has no Node server: it
   // embeds the static files Vite writes to build/ (src-tauri/tauri.conf.json
   // "frontendDist": "../build"). tests/frontend-shell.test.mjs keeps this file,
   // tsconfig.json, index.html, src/main.ts and tauri.conf.json in agreement.
   import { defineConfig } from "vite";
   import { svelte } from "@sveltejs/vite-plugin-svelte";

   const host = process.env.TAURI_DEV_HOST;

   // https://vite.dev/config/
   export default defineConfig(async () => ({
     plugins: [svelte()],

     resolve: {
       // `$lib` is defined once, in tsconfig.json "paths"; svelte-check reads the same entry.
       tsconfigPaths: true,
     },

     build: {
       outDir: "build",
       // Clear stale output (including a pre-migration build/_app/) before every build.
       emptyOutDir: true,
     },

     // Vite options tailored for Tauri development ... (existing comments and
     // `clearScreen` / `server` block unchanged)
   }));
   ```

5. `tsconfig.json` becomes exactly the following. Keep it comment-free JSON,
   because `tests/frontend-shell.test.mjs` parses it with `JSON.parse`:

   ```json
   {
     "compilerOptions": {
       "allowJs": true,
       "checkJs": true,
       "esModuleInterop": true,
       "forceConsistentCasingInFileNames": true,
       "resolveJsonModule": true,
       "skipLibCheck": true,
       "sourceMap": true,
       "strict": true,
       "moduleResolution": "bundler",
       "module": "esnext",
       "target": "esnext",
       "lib": ["esnext", "DOM", "DOM.Iterable"],
       "verbatimModuleSyntax": true,
       "isolatedModules": true,
       "noEmit": true,
       "types": ["svelte", "vite/client"],
       "paths": {
         "$lib": ["./src/lib"],
         "$lib/*": ["./src/lib/*"]
       }
     },
     "include": ["src/**/*.js", "src/**/*.ts", "src/**/*.svelte", "vite.config.js"]
   }
   ```

   The first nine options are today's. `module`, `target`, `lib`,
   `verbatimModuleSyntax`, `isolatedModules` and `noEmit` are copied from the
   generated `.svelte-kit/tsconfig.json`. Kit's `rootDirs` and `$app/types`
   path served only generated `$types` and are dropped.
6. Entry HTML. Run `git mv src/app.html index.html`, then edit it:
   - `href="%sveltekit.assets%/favicon.png"` becomes `href="/favicon.png"`;
   - delete the `%sveltekit.head%` line;
   - `<body data-sveltekit-preload-data="hover">` becomes `<body>`;
   - `<div style="display: contents">%sveltekit.body%</div>` becomes
     `<div id="app" style="display: contents"></div>`, followed on the next line
     by `<script type="module" src="/src/main.ts"></script>`.

   The pre-paint theme `<script>` stays byte-identical and stays in `<head>`.
7. `src/main.ts` (new) is exactly:

   ```ts
   import { mount } from "svelte";
   // Global tokens and fonts first, so app.css precedes component CSS in the cascade.
   import "./app.css";
   import App from "./routes/+page.svelte";

   const target = document.getElementById("app");
   if (!target) throw new Error("index.html is missing the #app mount target");

   export default mount(App, { target });
   ```

8. Run `git rm src/routes/+layout.ts src/routes/+layout.svelte` and
   `git mv static/favicon.png public/favicon.png`. `static/` is then empty and
   disappears.
9. In `.gitignore`, delete the `/.svelte-kit` line. Then delete the generated
   `.svelte-kit/` directory in every clone you use (this one, the Mac, and the
   VM worktree), because it is no longer ignored.
10. Comments only: in `src/lib/Settings.svelte`, change "app.html applies the
    saved value before paint" to "index.html applies the saved value before
    paint". In `tests/e2e/scenarios/uifoundation.mjs`, change "exercises
    app.html's pre-paint localStorage path" to "exercises index.html's
    pre-paint localStorage path".
11. In `tests/ui-foundation.test.mjs`, change the `appHtml` read from
    `path.join(srcRoot, "app.html")` to `path.join(repoRoot, "index.html")`.
    The test name and the assertion stay. This is the existing pre-paint
    theme-catalog guard. It now reads the moved file and fails with ENOENT if
    pointed at a missing one.
12. Add `scripts/check-frontend-build.mjs`, a Node ESM script with no
    dependencies beyond `node:*`. It runs after `vite build` from the repo root,
    reads `build/`, and exits non-zero with a message naming the failed
    assertion. Its assertions are:
    - B1: `build/index.html` has exactly one `<script type="module" ... src="...">`,
      its `src` matches `^/assets/[^/]+\.js$`, and that file exists.
    - B2: `build/index.html` links at least one stylesheet, every linked
      stylesheet exists, and together they contain an `@font-face` whose
      `font-family` is `Geist Variable`. Match `/@font-face\s*\{[^}]*font-family:\s*["']?Geist Variable/`
      so a minifier change in quoting doesn't break it. At least one
      `url(/assets/*.woff2)` is present, and every `url(/assets/...)` in the CSS
      exists under `build/`.
    - B3: no CSS `url(` holds a `data:font` URI. The CSP has no `font-src`, so
      `default-src 'self'` would block an inlined font.
    - B4: `build/favicon.png` exists.
    - B5: `build/index.html` still contains the pre-paint script's
      `var ids = [` array.
    - B6: `build/_app` does not exist (no stale SvelteKit output embedded).
13. Add `tests/frontend-shell.test.mjs` (`node:test`, `node:assert/strict`).
    Load Vite config with `resolveConfig({ configFile: <repo>/vite.config.js },
    "build", "production")` from `vite`, plus `src-tauri/tauri.conf.json`,
    `package.json`, `tsconfig.json`, `index.html` and `src/main.ts`. Write four
    separate tests, each failing with its own message:
    - S1 (build output is what Tauri embeds):
      `path.resolve(repo, config.build.outDir)` equals
      `path.resolve(repo, "src-tauri", tauri.build.frontendDist)`;
      `config.build.emptyOutDir === true`; `tauri.build.beforeBuildCommand ===
      "npm run build"`; and `pkg.scripts.build === "vite build && node
      scripts/check-frontend-build.mjs"`. That last check guards the
      wiring: an unwired post-build check never runs.
    - S2 (dev server): `config.server.port === 1420`,
      `config.server.strictPort === true`, and `new URL(tauri.build.devUrl).port
      === "1420"`.
    - S3 (`$lib` single definition): `config.resolve.tsconfigPaths === true`,
      tsconfig `paths["$lib"]` deep-equals `["./src/lib"]`, and
      `paths["$lib/*"]` deep-equals `["./src/lib/*"]`.
    - S4 (entry wiring): `index.html` contains exactly one `id="app"` element
      and exactly one `<script type="module" src="/src/main.ts"></script>`,
      and the pre-paint `var ids = [` script comes before the module script.
      `src/main.ts` contains `getElementById("app")` and `mount(`, and its
      `import "./app.css"` comes before its root-component import.

    If `resolveConfig` turns out to be impractical under `node --test` on the
    implementing host (it worked in the probe), import the default export,
    `await` its config function, and read the same fields. Record which
    approach was used in the implementation log.
14. Run `scripts/bump.sh` (patch, 1.0.60 to 1.0.61) per the 2026-06-20
    decision. It writes `package.json`, `package-lock.json`, `tauri.conf.json`,
    `Cargo.toml`, `Cargo.lock`, the `BUILD_DATE` in `commands.rs`, and
    `PKGBUILD`. The bump is also what lets `smoke`'s footer assertion prove the
    VM is running the new embedded frontend rather than a stale one.

Verification (pinned toolchain, LF checkout):

- `node scripts/check-js-toolchain.mjs` (or recorded as not run, per Host
  constraints), `npm ci`, `npm audit` (exit 0), and
  `npm ls @sveltejs/kit @sveltejs/adapter-static cookie` (empty). `npm ls --all`
  must show no invalid, missing or peer errors.
- `npm run check`: every node test passes, including the new
  `frontend-shell` and the repointed `ui-foundation`, and svelte-check reports
  0 errors and 0 warnings. Expect about 295 files at the current versions. The
  non-`node_modules` `tsc --listFilesOnly` set must be exactly `src/main.ts`,
  `src/lib/errors.ts`, `src/lib/imageReveal.ts`, `src/lib/types.ts` and
  `vite.config.js`, which is the base set minus `.svelte-kit/*` and
  `src/routes/+layout.ts`, plus `src/main.ts`. No `tests/` entry is allowed.
  `--showConfig` must match the base apart from the dropped `rootDirs` and
  `$app/types` path, the added `types`, and `include`/`exclude` spelling.
  Investigate any other difference.
- `npm run build`: no new warnings. The post-build check passes. `find build
  -type f | sort` shows `build/index.html`, `build/favicon.png`, and
  `build/assets/*` (one `.js`, one `.css`, five `.woff2`), and no `build/_app`.
  The five woff2 sha256 values equal the base's.
- Dev server: `npm run dev` binds 1420 and a second `npm run dev` fails on the
  port (strict). On the Mac, `npm run tauri dev` opens the app on the dev
  server. In a disposable worktree, edit a visible string in
  `src/routes/+page.svelte` and confirm it updates through HMR without a
  restart. Then dispose of the worktree.
- Mac desktop smoke: `npm run tauri -- build --debug --no-bundle`, then launch
  the debug app. It must boot to the Welcome or Home surface, render text in
  Geist, and apply the saved theme before paint (no flash). The Web Inspector
  console must show no CSP violation. This is the WKWebView check; say so in
  the log. WebView2 is not checked here (see Residual risks).
- VM: full `npm run e2e` (no `--skip-build`) at the slice head. A pass needs a
  count of at least the Slice 0 baseline and no failing scenario outside the
  baseline's failure set. `smoke` (footer `Vela v1.0.61`, Geist loaded) and
  `uifoundation` (tokens, pre-paint after restart) must both pass. Re-run a
  flaky scenario that fails in both once on its own (`npm run e2e -- <name>`).
  A failure that is new compared with the baseline blocks the slice.
- Cargo: the bump touches `Cargo.toml`, `Cargo.lock` and `commands.rs`. On the
  Mac, run `cargo +stable check --locked` from `src-tauri/` if Rust is
  installed. The VM's `tauri build --debug` also compiles the bumped crate.
  Clippy, tests, MSRV and cargo audit are left to CI on the next authorized
  push; say so in the log rather than implying they ran.

Guard red-proofs. Use a disposable `git worktree add --detach <tmp> <slice-2
sha>` with its own `npm ci` (Windows: the LF recipe). Prove each behavior
separately. After each injection, restore with `git checkout -- .` from that
committed head, never a file backup, then show green again. "Check fails"
means `npm run check` exits non-zero at the named assertion; "build fails"
means `npm run build` exits non-zero at the named assertion.

Audit:

- A1: the base capture's `npm audit` (non-zero, naming both GHSAs) against the
  head's exit 0. If the base capture was lost, re-run it in a disposable
  worktree of the base.

Type check really checks (svelte-check through `npm run check`; each must
report the injected error at the injected file):

- T1: add `const bad: number = "x";` inside `src/lib/Icon.svelte`'s
  `<script lang="ts">`.
- T2: the same, inside `src/routes/+page.svelte`'s script.
- T3: append `export const bad: number = "x";` to `src/main.ts`. This proves
  the new entry is covered.
- T4: append the same to `src/lib/types.ts`.
- T5: append `/** @type {number} */ export const bad = "x";` to
  `vite.config.js`. This proves `checkJs` still covers the config, as Kit's
  include did.
- T6: delete `"types"` from `tsconfig.json`. Expect the `./app.css`
  side-effect-import error in `src/main.ts`.
- T7: delete `"paths"` from `tsconfig.json`. Expect `Cannot find module
  '$lib/...'` from svelte-check; `npm run build` also fails, as described in
  the probe. S3 fails too, but T7 is about svelte-check.

Shell config (`tests/frontend-shell.test.mjs`; run `node --test
tests/frontend-shell.test.mjs`, then `npm run check` for wiring):

- S1a: `outDir: "dist"`. S1 must fail on the outDir/frontendDist assertion.
- S1b: `emptyOutDir: false`. S1 must fail on the emptyOutDir assertion.
- S1c: set `build` back to plain `"vite build"`. S1 must fail on the
  post-build-check wiring assertion.
- S2a: `port: 1421`. S2 must fail. S2b: `strictPort: false`. S2 must fail.
- S3a: delete `tsconfigPaths: true`. S3 must fail. Also expect `npm run build`
  to fail with the misleading "failed to load virtual css module" message
  from the probe.
- S4a: change `id="app"` in `index.html` to `id="root"`. S4 must fail.
- S4b: move `import "./app.css";` below the root-component import in
  `src/main.ts`. S4 must fail on the ordering assertion.

Post-build check (`npm run build`):

- B1: delete the `<script type="module" src="/src/main.ts">` line from
  `index.html`. The build check must fail at B1, and S4 fails too.
- B2: delete `@import "@fontsource-variable/geist";` from `src/app.css`. The
  build check must fail at B2.
- B3: add `assetsInlineLimit: 1_000_000` to `build`. The fonts become `data:`
  URIs, and the build check must fail at B3.
- B4: `git rm public/favicon.png`. The build check must fail at B4.
- B5: delete the pre-paint `<script>` block from `index.html`. The build check
  must fail at B5, and `ui-foundation` must fail its first-paint catalog
  assertion.
- B6: build the base commit (Kit) in the disposable worktree so `build/_app`
  exists, check out the slice head, set `emptyOutDir: false`, and run
  `npm run build`. The build check must fail at B6. Restore and rebuild, and
  B6 passes because `emptyOutDir: true` cleared `_app`.

Webview mount (VM, one rebuild): in a disposable VM worktree at the slice
head, change `getElementById("app")` in `src/main.ts` to
`getElementById("missing")`. `npm run e2e -- smoke` must fail at `app render`.
That proves `smoke` detects a bundle that loads but doesn't mount. S4 fails
locally for the same injection.

Commit: `refactor: remove SvelteKit; mount Svelte 5 directly with Vite`, one
commit including the bump. The decision and records go in Slice 4.

Rollback: `git revert <slice-2 sha>` as a new commit, then `npm ci`, and
`npx svelte-kit sync` to regenerate `.svelte-kit/`. That brings back Kit 2.70.3,
adapter-static and the cookie override, and the audit gate goes red again on
the same two lows. Don't rewrite history. The VM side needs nothing.

## Slice 3: rename the root component (only if G1 is ruled "rename")

Without Kit, `src/routes/+page.svelte` carries routing-convention naming with
no routing meaning, inside a `routes/` directory with no router.

1. Run `git mv src/routes/+page.svelte src/App.svelte`; `src/routes/` is then
   empty. In `src/main.ts`, change the import to `import App from "./App.svelte";`.
2. Repoint every live reference. These counts were recorded at `c112253`; use
   `git grep -n "+page.svelte" -- tests src scripts ISSUES.md` to find them.
   - `tests/config-integrity.test.mjs` (1), `tests/image-reveal.test.mjs` (2),
     `tests/transcoding-ui.test.mjs` (1), `tests/ui-foundation.test.mjs` (5),
     `tests/ui-motion.test.mjs` (11): change `src/routes/+page.svelte` to
     `src/App.svelte`.
   - `tests/watch-edit-position.test.mjs` (4): change the read path to
     `path.join(repoRoot, "src", "App.svelte")`, the messages to `App.svelte
     must ...`, and the virtual filename `"+page.svelte.ts"` to `"App.svelte.ts"`.
   - Comments in `tests/e2e/scenarios/curation.mjs` (1) and
     `tests/e2e/scenarios/refresh.mjs` (2), and the two path citations in
     `ISSUES.md`: change the path and keep the line numbers, since the content
     is unchanged.
   - After the edits, that grep must return nothing.
3. Run `scripts/bump.sh` (patch), per the 2026-06-20 decision. Compiled
   component names change, so the bundle changes.

Verification: `npm run check` green; `npm run build` green (post-build check
included); svelte-check file count unchanged from Slice 2; VM `npm run e2e --
smoke` and `npm run e2e -- uifoundation` (no `--skip-build`) pass.

Red-proof (no vacuous path): in a disposable worktree at the slice head, run
`git mv src/App.svelte src/Gone.svelte` without touching anything else.
`node --test` must report failures in each of the six node test files listed
above. A file that still passes is reading a stale path or passing vacuously;
fix it before committing. Restore with `git checkout -- .` and show green.

Commit: `refactor: rename the root component to src/App.svelte`.

Rollback: `git revert <slice-3 sha>`.

## Slice 4: records

1. In this plan, set the status line and the implementation log: Slices 0-3
   evidence, red-proof results, which `resolveConfig` approach was used, and
   the review verdict.
2. `.agents/decisions.md`: add a dated entry after the newest one,
   `## <date> - The frontend is plain Svelte 5 and Vite; SvelteKit is removed`.
   Its body:
   - Status: APPROVED, with the owner's approval wording and date, and a
     pointer to this plan.
   - Decision: Vela's frontend no longer uses SvelteKit or adapter-static. It
     is a Svelte 5 single-page app built by Vite with
     `@sveltejs/vite-plugin-svelte`: a root `index.html`, `src/main.ts`
     mounting the root component with `mount()`, Vite `public/` for static
     files, `$lib` defined once in tsconfig `paths` and consumed through Vite's
     `resolve.tsconfigPaths`, and output in `build/` for Tauri. Node remains a
     build and test tool; the owner chose that over a Node-free rewrite, and
     chose removal over the Kit 3 upgrade. If G1 was ruled "rename", add that
     historical records citing `src/routes/+page.svelte` refer to what is now
     `src/App.svelte`.
   - Reason: Vela used no Kit feature, and Kit's only contributions were
     generated config, a breaking major, and the two advisories that fail the
     closed npm audit gate.
   - Supersedes: the 2026-07-15 dependency decision's Kit-scoped `cookie`
     override (no target remains, and `cookie` left the graph); that decision's
     example "TypeScript remains on current compatible 6 while SvelteKit
     excludes 7"; and the Mission Detail line "SvelteKit is configured as a
     static SPA for Tauri." Say explicitly that the TypeScript 7 exclusion no
     longer comes from any installed peer range (svelte-check peers
     `typescript >=5.0.0`). The `~6.0.3` range is now the only fence, kept until
     a separate refresh proves TypeScript 7 with svelte-check. Say also that
     the 2026-07-15 fail-closed npm audit, no-blanket-suppression and
     no-force-fix rules stay in force.

   In the 2026-07-15 entry itself, append to the cookie-override paragraph:
   "Superseded <date>: SvelteKit was removed (see the <date> entry); the
   override and `cookie` are gone." Append the same kind of pointer to the
   "SvelteKit excludes 7" sentence. Don't rewrite the original text.
3. `.agents/repo-guidance.md`:
   - Mission Detail, first bullet: "a Svelte 5/TypeScript frontend in `src/`".
   - Mission Detail, the SvelteKit bullet becomes: "The frontend is a
     single-page Svelte 5 app built by Vite (no SvelteKit; decision
     `.agents/decisions.md` <date>). Root `index.html` loads `src/main.ts`,
     which mounts the root component. `$lib` is defined only in `tsconfig.json`
     `paths`. Vite dev uses port `1420` with strict port behavior. The build
     goes to `build/`, which Tauri embeds (`frontendDist: ../build`);
     `tests/frontend-shell.test.mjs` keeps these in agreement."
   - Earned Practices, generated outputs: drop `.svelte-kit/` from the
     do-not-edit list.
   - Verification: the `npm run build` line becomes "production frontend build
     plus its post-build layout check (`scripts/check-frontend-build.mjs`)". The
     `npm run check` line keeps its wording.
4. `docs/development.md` Architecture: "Tauri 2 with a single-page Svelte 5
   frontend (built by Vite) in `src/` and a Rust backend in `src-tauri/`."
5. `.agents/state.md`: replace the npm-audit text in the Now bullets with the
   landed state, and say CI is unverified until an authorized push. Under
   Active Sources, set this plan's status.
6. Commit as docs only: `docs: record SvelteKit removal`.

GitHub CI on the `github` remote is the final proof of the CI and release audit
gates, and the first run of `npm run check`/`npm run build` in CI since the gate
went red. Pushing needs a separate owner go (`.agents/push-policy.md`). Until
then, state that CI is unverified.

## Review

Review Slice 2 (and Slice 3, if it runs) on the pinned `base..head` range,
routed per `.agents/decisions.md` 2026-07-26: `codex`, called plainly, never a
Claude harness. The owner chooses `codereview` or `openreview` for each
invocation. Each admitted finding becomes its own fix commit with its own
red-proof. Stop and surface after three consecutive review cycles that produce
no verifiable delta.

## Residual risks

- Kit's dropped runtime behaviors (listed above) are reviewed as inert. The E2E
  suite on WebKitGTK is the evidence. A behavior that only rarely-exercised UI
  depends on could escape it.
- E2E covers WebKitGTK only. WKWebView gets the manual Mac smoke. WebView2 is
  proven only by an owner-authorized release-workflow run, or a manual check on
  the Windows validation host in `.agents/machines.md` if the owner asks.
- CSS order: Kit loaded `app.css` as the layout node's stylesheet before the
  page's. Vite emits one stylesheet in import order, and S4 pins `app.css`
  first. Rules of equal specificity across the two used to come from separate
  files and now share one. `uifoundation`'s token and focus assertions and its
  screenshots are the evidence.
- `vitePreprocess` is kept but inert today, and no guard covers it (probe).
- The TypeScript 7 fence is now only the `~6.0.3` range (decision text in
  Slice 4).
- A future svelte-check or vite-plugin-svelte major may change how
  `svelte.config.js` or tsconfig `paths` are read. The T- and S-guards fail
  loudly if so.