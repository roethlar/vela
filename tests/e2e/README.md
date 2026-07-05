# E2E harness

Drives the real debug Vela binary through `tauri-driver` + a vendored
WebKitWebDriver, on a throwaway config dir (`XDG_CONFIG_HOME` points into a
temp dir per scenario — the owner's `~/.config/vela` is never touched).
Design and deviations: `.agents/plans/e2e-harness.md`.

```sh
npm run e2e                    # build debug app, run all scenarios
npm run e2e -- --skip-build    # reuse the existing debug binary
npm run e2e -- smoke           # run one scenario by name
```

- Requires a graphical session (Linux), `tauri-driver`
  (`cargo install tauri-driver`), `bsdtar`, and `curl`.
- First run downloads the vendored driver via `fetch-driver.sh` into
  `vendor/` (gitignored, sha256-pinned).
- Screenshots and driver logs land in `artifacts/` (gitignored) for owner
  flip-through.
- Scenarios live in `scenarios/*.mjs`: `export default { name, async run(ctx) }`
  with `ctx = { driver, screenshot(name), repoRoot, configRoot }`; assert
  with `node:assert/strict`.
