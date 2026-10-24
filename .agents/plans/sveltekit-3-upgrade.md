# Plan: SvelteKit 3 upgrade to clear the npm audit gate

## Status

**DRAFT, revision 1 (2026-10-23). Awaiting the owner's plan approval. No
implementation is authorized.** The owner asked for this plan on 2026-10-23.
Approving it also approves two things below: removing the Kit-scoped `cookie`
override that the 2026-07-15 dependency decision approved, and the
`version.pollInterval: 0` behavior-preservation setting. Record the owner's
approval wording here and in `.agents/decisions.md` before Slice 1 starts.

Implementation log:

- (none yet)

## Goal

`npm audit` fails closed on `main` in CI (`.github/workflows/ci.yml`) and in
the release workflow's `npm-audit` job (`.github/workflows/release.yml`,
which `bundle` and the Arch job `need`). Releases are therefore blocked as
well. The cause is two low-severity advisories:

- GHSA-pfhg-q3qv-9q6r (CVE-2026-114060): open redirect through `use:enhance`
  enhanced form actions on routes that accept `//`-prefixed paths. CVSS v4 2.1.
- GHSA-27mw-82g8-phjj: XSS through an unescaped `cache-control` value from
  `setHeaders` written into a prerendered page's `<meta http-equiv>`. CVSS 3.1
  0.0.

Both cover every `@sveltejs/kit` release below 3.0.0. npm reports the range as
`<=3.0.0-next.32`. Neither advisory has a 2.x backport: 2.70.3 is the newest
2.x and is still in range. `@sveltejs/adapter-static` 1.0.0-next.0 through
3.0.10 gets flagged only because it depends on Kit. 3.0.0 is the first fixed
release. `npm audit` proposes 3.1.0 because it is `latest`.

The work is done when all of the following hold:

1. `npm audit` exits 0 on a clean Node 26.12.1/npm 12.0.1 install. There must be
   no audit exception, no threshold change, and no `npm audit fix --force`.
2. The frontend still builds as the same static SPA: `build/index.html`
   fallback, `ssr = false`, Tauri `frontendDist: ../build`, and the Vite dev
   server on strict port 1420 with the existing `TAURI_DEV_HOST` HMR wiring.
3. `npm run check` (node tests plus svelte-check) is green, with no new
   svelte-check errors or warnings and no unintended change to TypeScript
   coverage.
4. The Linux real-app E2E suite is no worse than a baseline taken on the
   pre-upgrade commit.

## Advisory applicability (assessment, not a waiver)

Neither vulnerable path is reachable in Vela:

- Vela has no form actions, no `use:enhance`, no `+page.server.*` or
  `+server.*`, and no SvelteKit server at runtime. adapter-static emits files
  that the Tauri webview loads over the app's custom protocol. `grep -rn
  "use:enhance" src` is empty. The only `<form>` is
  `src/lib/PlaylistsView.svelte`'s create form, a plain `onsubmit` with
  `preventDefault()` that calls `invoke()` and has no `action`.
  GHSA-pfhg-q3qv-9q6r needs enhanced form actions.
- Vela has no `load` functions, no `setHeaders`, and prerenders nothing.
  `src/routes/+layout.ts` exports only `ssr = false`. The one generated HTML file
  is the adapter's SPA fallback. GHSA-27mw-82g8-phjj needs a prerendered page
  whose `cache-control` comes from untrusted input at build time.
- The app surface is `src/routes/+layout.ts`, `+layout.svelte`, `+page.svelte`
  and `src/lib/*`. It imports nothing from `$app/*`, `$env/*` or
  `@sveltejs/kit`, and has no `<a href>` navigation.

The exposure is nil. The upgrade is still mandatory because the gate fails
closed under the 2026-07-14 known-vulnerabilities rule, which the 2026-07-15
dependency decision extended to npm. That decision forbids blanket
suppression, and npm has no per-advisory ignore.

## Audited targets (registry, 2026-10-23)

Re-query `npm view <pkg> dist-tags` and the peer ranges right before Slice 1.
Take a newer stable patch or minor that stays inside these constraints.
Never take a prerelease, per the 2026-07-15 decision.

| Package | Locked now | Target | Why |
| --- | --- | --- | --- |
| `@sveltejs/kit` | 2.70.3 | `^3.1.0` (3.1.0) | first fixed major; newest stable |
| `@sveltejs/adapter-static` | 3.0.10 | `^4.0.0` (4.0.0) | 4.0.0 peers `@sveltejs/kit ^3.0.0-next.0`; only breaking change is "require SvelteKit 3" |
| `svelte` | 5.56.5 | `^5.57.2` (5.57.2) | Kit 3 peers `svelte ^5.57.1` |
| `svelte-check` | 4.7.2 | `^4.7.7` (4.7.7) | `sv migrate sveltekit-3` sets a minimum of `^4.7.5`. 4.7.5+ loads Svelte options from `vite.config.*` once `svelte.config.js` is gone |
| `vite` | 8.3.5 | unchanged | Kit 3 peers `^8.0.12` |
| `@sveltejs/vite-plugin-svelte` | 7.2.0 | unchanged | Kit 3 peers `^7.0.0`; 7.3.1 exists but nothing requires it, so leave it out of this slice |
| `typescript` | 6.0.3 | unchanged (`~6.0.3`) | Kit 3 peers `^6.0.0`; TypeScript 7 stays excluded |
| `cookie` (via Kit) | 0.7.2 (override) | 2.0.1, Kit's own `^2.0.1` | see the override row below |

Other facts about the targets:

- Kit 3 `engines.node` is `>=22.17`, so the repo's 26.12.1 pin satisfies it.
- `@opentelemetry/api` is an optional Kit 3 peer. Do not install it.
- Kit's `svelte-kit` bin and its `sync` command still exist in 3.1.0, so
  `npm run check` keeps its shape.

Feasibility probe (2026-10-23, scratch copy of `package.json` and the lock, npm
11.20.0 with `--package-lock-only --ignore-scripts`; not the canonical npm): the
four target bumps plus override removal resolved with no ERESOLVE, gave
`cookie` 2.0.1, removed `@types/cookie`, and `npm audit` reported 0
vulnerabilities. Treat this as evidence only. The committed lock must come
from npm 12.0.1.

The `cookie` override must be removed. Kit 3's server runtime
(`runtime/server/cookie.js`, `fetch.js`) imports `parseCookie`,
`parseSetCookie` and `stringifySetCookie`, which exist only in cookie 2. The
2026-07-15 override pins cookie to 0.7.2, which would force an out-of-range
major onto Kit 3, whose named ESM imports would fail to resolve. Expect that
to break the build-time server render `builder.generateFallback` uses to write
`build/index.html`. This is inferred from the import list and has not been
executed. The override existed only because Kit 2 asked
for vulnerable `cookie ^0.6.0` (GHSA-pxg6-pf52-xh8x), and Kit 3 no longer does.

## Kit 3 breaking changes vs. Vela

Sources: <https://svelte.dev/docs/kit/migrating-to-sveltekit-3>,
<https://svelte.dev/docs/cli/sv-migrate>, the adapter-static CHANGELOG, and
the published 3.1.0 / 4.0.0 / sv 1.1.1 tarballs.

Changes that touch Vela:

1. `svelte.config.js` is unsupported. Kit 3's `vite-plugin-sveltekit-resolve-root`
   throws `config_file_unsupported` if `svelte.config.js` or `svelte.config.ts`
   exists in the root. Config moves into `sveltekit({...})` in `vite.config.js`,
   with former `kit.*` keys at the top level. `preprocess` is a Kit option there,
   and options Kit does not recognize are forwarded to vite-plugin-svelte with
   `configFile: false`.
2. The `tsconfig.json` base moves. It must extend `$app/tsconfig`, which
   `svelte-kit sync` writes to `node_modules/$app/tsconfig.json`, instead of
   `./.svelte-kit/tsconfig.json`. It must also declare its own `include` and
   `exclude`. The old generated config included `src/**/*.{js,ts,svelte}`,
   `vite.config.{js,ts}`, and `test(s)/**/*.{js,ts,svelte}`, which matched none of
   Vela's `tests/*.mjs`. **Coverage hazard:** `sv migrate`'s tsconfig task adds
   `"tests"` to `include` when that directory exists. With `allowJs` and `checkJs`
   that would newly type-check every `tests/**/*.mjs` (unit, E2E and live
   harness). That is a scope expansion, not a migration, so remove it (Slice 1
   step 4).
3. `version.pollInterval` now defaults to one hour (it used to be 0, no
   polling). A Tauri bundle is embedded in the binary and can't be redeployed
   under a running app, so polling `/_app/version.json` does nothing useful.
   Set it to `0` to keep Kit 2 behavior.
4. The peer floors require bumping `svelte` and `svelte-check` (table above).
5. The `cookie` override must go (section above).

Changes reviewed and found not to apply (no usage in `src/`, config, or tests):
`$app/stores` removal, `$app/environment` → `$app/env`, `$env/*`
deprecation, `$app/paths` `base`/`assets`/`resolveRoute` removal,
`$app/navigation` `goto`/`invalidateAll`/`pushState`/`preloadData` changes,
`$app/forms`, hooks/`handleError`/`error()` signature changes, `src/params`
consolidation, remote functions, server-only filename rules, `+server`
responses, `csrf.checkOrigin`, cookie API renames, external `redirect`,
service worker changes, `files.lib`, `preloadStrategy`, `prerender.origin`,
the "link to current page calls `refreshAll()`" rule (no anchors), and
`data-sveltekit-*="off"` → `false`. `src/app.html` uses
`data-sveltekit-preload-data="hover"`, which is still valid, and keeps the
required `%sveltekit.head%`/`%sveltekit.body%` tags. Dev-server CORS for static
assets is no longer set by Kit, but the Tauri dev webview loads
`http://localhost:1420` itself, so asset requests are same-origin.
adapter-static 4.0.0 keeps the same `pages`/`assets`/`fallback`/`precompress`/
`strict` options and still throws "Encountered dynamic routes" when there is no
`fallback`.

`npx sv migrate sveltekit-3` exists (sv 1.1.1). Its tasks are package-json,
tsconfig, svelte-config, environment, paths, external-redirects,
shallow-routing, params, imports, lib-alias, app-state, and
collect-migration-instructions. The last one writes `MIGRATION_TASKS.md` in the
repo root and may add `@migration-task` comments. Its package-json task only
raises versions to minimums (`@sveltejs/kit ^3.0.0`, `svelte ^5.57.1`,
`svelte-check ^4.7.5`), so it does not meet this plan's newest-stable targets
by itself.

## Scope

In scope: `package.json`, `package-lock.json`, `vite.config.js`,
`tsconfig.json`, deleting `svelte.config.js`, one new guard test and its entry
in the `check` script, plan/state/decision records, and the E2E VM Node
re-alignment that `.agents/machines.md` already records as pending.

Out of scope:

- vite-plugin-svelte 7.3, TypeScript 7, and any other freshness bump not
  required by Kit 3.
- Rust or Cargo changes.
- A version bump or release. The fix rides the next owner-authorized release,
  and release, push and CI-triggering actions stay separately gated by
  `.agents/push-policy.md`.
- Enabling `checkJs` over `tests/`.
- Any `src/` code change. If Kit 3 or Svelte 5.57 forces one, stop and amend
  this plan.

## Slice 0: E2E venue on the pinned toolchain (no repo code)

The repo pins Node 26.12.1/npm 12.0.1, so `scripts/check-js-toolchain.mjs`
rejects the VM's 26.5.0. Without this slice there is no E2E baseline and no
post-upgrade E2E.

1. Apply the reversible re-alignment recipe in the `.agents/machines.md`
   "Pending (2026-10-23)" bullet exactly as written: SHASUMS-verified official
   arm64 archive, `~/.local/opt/node-v26.12.1`, repoint only the three
   `~/.local/bin` symlinks, install npm 12.0.1 with that Node, and prove that
   `bash -lc` reports v26.12.1/12.0.1. Authority: the 2026-07-15 decision's
   grant to change Node/npm on the E2E VM (and no other host), together with the
   owner's 2026-10-23 pin go. Keep the 26.5.0 tree as the rollback.
2. In the VM's detached validation worktree (`~/dev/vela-main`, see
   machines.md), check out the pre-upgrade base commit (current `main`). Run
   `node scripts/check-js-toolchain.mjs`, `npm ci`, then the full
   `npm run e2e` without `--skip-build`. Record the pass count and the exact
   failing scenario names. That is the baseline. Recorded known flakes:
   `continueon`, `playverbs`, `refresh`.
3. Update the machines.md baseline and remove the Pending bullet. Commit that
   as docs only (`docs: realign E2E VM to Node 26.12.1`).

Rollback: repoint the three symlinks at `~/.local/opt/node-v26.5.0` and delete
the 26.12.1 directory.

## Slice 1: atomic Kit 3 migration plus config guard

This slice can't be split. Kit 3 rejects `svelte.config.js`, adapter-static 3
does not accept Kit 3, and Kit 3 refuses Svelte below 5.57.1.

Prerequisites: a host where `node scripts/check-js-toolchain.mjs` passes
(Node 26.12.1 and npm 12.0.1 exactly; a different npm is a failed prerequisite,
not permission to regenerate the lock), and an LF checkout. On a
`core.autocrlf=true` Windows checkout, 5 `node --test` cases already fail on
unmodified `main` (`.agents/state.md`), so `npm run check` can't prove anything
there.

Before editing, capture on the base commit: `npm audit` output (the red
evidence, naming both GHSAs), `npx svelte-kit sync && npx svelte-check
--tsconfig ./tsconfig.json --output machine` (summary line),
`npx tsc -p tsconfig.json --listFilesOnly | grep -v node_modules`,
`npx tsc -p tsconfig.json --showConfig`, and `find build -type f | sort` after
`npm run build`.

Steps:

1. Run `npx sv@1.1.1 migrate sveltekit-3 --no-install --tasks all` (or the
   newest stable sv after re-query). Read `MIGRATION_TASKS.md` and grep for
   `@migration-task`. Any item that applies to Vela, beyond what this plan
   lists, stops the slice for a plan amendment. Then delete
   `MIGRATION_TASKS.md` and any `@migration-task` comments. Neither gets
   committed.
2. Fix `package.json` to exactly this: `@sveltejs/kit ^3.1.0`,
   `@sveltejs/adapter-static ^4.0.0`, `svelte ^5.57.2`, `svelte-check ^4.7.7`,
   with every other dependency range unchanged. Delete the whole `overrides`
   block, since its only entry is the Kit cookie pin. Keep `allowScripts`,
   `engines` and `packageManager`.
3. Make `vite.config.js` carry the config and delete `svelte.config.js`. The
   end state of the plugin call:

   ```js
   import adapter from "@sveltejs/adapter-static";
   import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
   // ...
   plugins: [
     sveltekit({
       preprocess: vitePreprocess(),
       // Tauri has no Node server: adapter-static with an index.html fallback is SPA mode.
       // See https://svelte.dev/docs/kit/single-page-apps and https://v2.tauri.app/start/frontend/sveltekit/
       adapter: adapter({ fallback: "index.html" }),
       // The bundle is embedded in the app binary; there is no deployment to poll for.
       version: { pollInterval: 0 },
     }),
   ],
   ```

   Keep the `defineConfig(async () => ...)` wrapper, `clearScreen`, and the
   whole `server` block (port 1420, `strictPort`, `TAURI_DEV_HOST` host/HMR
   1421, `src-tauri` watch ignore) byte-identical.
4. Make `tsconfig.json` read `"extends": "$app/tsconfig"`,
   `"include": ["src", "vite.config.js"]`, and
   `"exclude": ["src/service-worker"]`, with the existing `compilerOptions`
   kept. If sv deletes duplicates of inherited options, keep that only when the
   `--showConfig` check below shows no resolved change. Remove `"tests"` if sv
   added it.
5. Regenerate the lock with npm 12.0.1 using `npm install` (no `--force`, no
   `--legacy-peer-deps`, no `npm audit fix`). Inspect the lock diff: only
   the Kit/adapter/svelte/svelte-check closure and the cookie change should
   move. Any other package moving needs a reason.
6. Add `tests/sveltekit-config.test.mjs` and append it to the explicit
   `node --test` list in the `check` script. If it is not in that list it never
   runs (guard the wiring). It must:
   - call `resolveConfig({ configFile: <repo>/vite.config.js }, "build",
     "production")` from `vite`. That is the same resolution `svelte-kit`'s
     CLI uses (`load_vite_config` plus `extract_svelte_config`);
   - find the plugin named `vite-plugin-sveltekit-setup` and **fail** if it or
     its `api.options` is missing, so a renamed plugin can't pass vacuously;
   - assert `api.options.version.pollInterval === 0`;
   - assert `api.options.adapter.name === "@sveltejs/adapter-static"`.

   If `resolveConfig` turns out to be impractical under `node --test`, await
   the default export's config function, flatten its `plugins`, and look up the
   same plugin. Record which approach was used in the implementation log.
7. Move the explanatory header comment from `svelte.config.js` (the
   SPA-mode note) into `vite.config.js`, as in step 3.
   `src/routes/+layout.ts` doesn't name the config file, so it stays unchanged.
   `docs/development.md` says "SvelteKit/Svelte 5" and names no Kit major, so it
   stays unchanged too.

Verification (all on the pinned toolchain and an LF checkout):

- `node scripts/check-js-toolchain.mjs`, `npm ci`, `npm audit` (exit 0),
  `npm ls --all` (no invalid, missing or peer errors; exactly one `cookie`,
  2.x), and `npm outdated` (inspect only; vite-plugin-svelte 7.3.x and
  TypeScript 7 are expected rows).
- `npm run check`: all node tests plus svelte-check at 0 errors and 0 warnings,
  matching the baseline.
- `npx tsc -p tsconfig.json --listFilesOnly | grep -v node_modules`: same set
  as the baseline, and in particular no `tests/` entry. `--showConfig`: the
  resolved compiler options match the baseline apart from Kit-generated
  `paths`/`rootDirs`/`types`/`lib` spelling. Investigate any other difference.
- `npm run build`: no new warnings. `build/index.html` exists. Compare the
  `find build -type f` shape with the baseline: the same top-level layout,
  `_app/immutable/**`, `_app/version.json`, and static assets; hashed names
  will differ. Note any new top-level file and give the reason for it.
- Dev server: `npm run dev` binds 1420 with `strictPort`. In a disposable
  worktree, edit a visible string in `+page.svelte` and confirm it updates
  through HMR without a restart, as the 2026-07-15 refresh Slice 3 did. Then
  dispose of the worktree.
- `npm run tauri -- build --debug --no-bundle` succeeds, and the debug app
  boots to the Home surface on the authoring desktop. That is the WKWebView or
  WebView2 smoke; state which one was run.
- VM: full `npm run e2e` (no `--skip-build`) at the slice head. A pass needs a
  count of at least the Slice 0 baseline and no failing scenario outside the
  baseline's failure set. Re-run a flaky scenario that fails in both on its own
  (`npm run e2e -- <name>`) once. A failure that is new compared with the
  baseline blocks the slice.
- Cargo gates: not required, since no Rust or Cargo file changes. Say so in
  the log rather than implying they ran.

Guard red-proofs. Use a disposable `git worktree add --detach <tmp> <slice
head>` with its own `npm ci` and prove each behavior separately. After each
injection, restore with `git checkout -- .` from that committed head, never a
file backup, then show green again:

1. Polling off: delete the `version` key, then expect
   `node --test tests/sveltekit-config.test.mjs` to fail on the pollInterval
   assertion (3600000 vs 0) and `npm run check` to fail at the same assertion.
   The second failure proves the wiring.
2. Adapter identity: remove the `adapter` key, then expect the guard test to
   fail on the adapter assertion, not on the plugin lookup.
3. SPA fallback (upstream-owned guard): remove `fallback`, then expect
   `npm run build` to fail with adapter-static "Encountered dynamic routes"
   naming `src/routes`.
4. No half-revert to Kit 2 config (upstream-owned guard): restore
   `svelte.config.js` from the base commit, then expect `npm run build` to fail
   with `` `svelte.config.js` is no longer used ``.
5. Audit: the base commit's captured `npm audit` (non-zero, both GHSAs)
   against the head's exit 0. If the base capture was lost, re-run it in a
   disposable worktree of the base.

Commit: `chore: upgrade SvelteKit to 3 for the npm audit advisories`, one
commit. The decision and plan records go in Slice 2.

Rollback: `git revert <slice-1 sha>` as a new commit, then `npm ci`. That brings
back Kit 2.70.3 and the cookie override, and the audit gate goes red again on
the same two lows. Don't rewrite history. The VM side needs nothing, because
Slice 0 stays valid for Kit 2.

## Slice 2: records

1. Plan status and implementation log (Slice 0 and Slice 1 evidence, review
   verdict).
2. Add a dated `.agents/decisions.md` entry with the owner's approval wording.
   Mark the 2026-07-15 dependency decision's `cookie` override paragraph
   "superseded by Kit 3 (cookie ^2.0.1, unaffected)". Its fail-closed npm
   audit, no-blanket-suppression and no-force-fix rules stay in force.
3. `.agents/state.md`: replace the "remaining red CI gate is npm audit" text
   in the two Now bullets with the landed state. Keep `.agents/repo-guidance.md`
   verification commands unchanged unless they actually changed. They don't:
   `npm run check` keeps its name.
4. Commit as docs only (`docs: record SvelteKit 3 upgrade`).

GitHub CI on the `github` remote is the final proof of the CI and release
audit gates. Pushing needs a separate owner go (`.agents/push-policy.md`).
Until then, state that CI is unverified.

## Review

Review Slice 1 on its pinned `base..head` range, routed per
`.agents/decisions.md` 2026-07-26: `codex`, called plainly, never a Claude
harness. The owner chooses `codereview` or `openreview` for each invocation.
Each admitted finding becomes its own fix commit with its own red-proof. Stop
and surface after three consecutive review cycles that produce no verifiable
delta.

## Residual risks

- Svelte 5.56 to 5.57 is a minor bump under the whole component tree.
  svelte-check, the build, and the full E2E suite are the evidence. Runtime
  regressions in rarely-exercised UI could escape all three.
- Kit 3 changes client-router internals (`updated.current` triggers, `goto`
  semantics). Vela never calls them. The single-route boot is covered by E2E
  on WebKitGTK only. macOS and Windows webviews get one manual smoke at most.
- Windows (WebView2) and macOS release bundles are proven only by an
  owner-authorized release-workflow run.
- `sv migrate` output depends on the sv version. The explicit end states in
  steps 2 to 4 are the contract, not the tool's output.
