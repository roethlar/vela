# E2E harness

Drives the real debug Vela binary through `tauri-driver` + a vendored
WebKitWebDriver, on a throwaway config dir (`XDG_CONFIG_HOME` points into a
temp dir per scenario — the owner's `~/.config/vela` is never touched).
Design and deviations: `.agents/plans/e2e-harness.md`.

```sh
npm run e2e                    # build debug app, run all scenarios
npm run e2e -- --skip-build    # reuse the existing debug binary
npm run e2e -- smoke           # run one scenario by name
npm run e2e -- --embedded      # the same suite, played in the embedded player
```

- Requires Linux, `tauri-driver` (`cargo install tauri-driver`), `Xvfb`
  (`xorg-server-xvfb`), `bsdtar`, `curl`, `openssl`, and — for the playback-driving
  scenarios — `ffmpeg` and `mpv`. Server flows run against in-process mock
  Jellyfin servers (`mockjf.mjs`) whose Range-capable streams serve
  ffmpeg-generated clips; no real server or network is touched.
- Runs headless on a private Xvfb display by default — screenshots on the
  live Wayland desktop hang whenever the test window is occluded/unfocused
  (no frame callbacks). `VELA_E2E_HEADED=1` opts into the real desktop to
  watch a run; `VELA_E2E_DISPLAY=:N` picks the Xvfb display;
  `VELA_E2E_DEBUG=1` logs each WebDriver call with timing.
- First run downloads the vendored driver via `fetch-driver.sh` into
  `vendor/` (gitignored, sha256-pinned).
- Screenshots and driver logs land in `artifacts/` (gitignored) for owner
  flip-through.
- Scenarios live in `scenarios/*.mjs`: `export default { name, async run(ctx) }`
  with `ctx = { driver, screenshot(name), repoRoot, configRoot }`; assert
  with `node:assert/strict`.

## The embedded player

`--embedded` runs the suite with the embedded player instead of the external
one (`.agents/plans/embedded-player.md`). Two things change:

- Every seeded config gets `"player": "embedded"`. A scenario plays the way
  it always did, through mpv's IPC socket; the mpv on the other end is the
  library inside Vela's own process.
- The venue is a private headless Wayland compositor (`weston
  --backend=headless --renderer=gl`) instead of Xvfb, because that player
  needs Wayland. Weston has no colour management and no fractional scaling,
  so nothing about HDR or scaled displays is exercised here.

It additionally needs `weston` and a built libmpv set: by default the one
`scripts/build-libmpv.sh --container` leaves in `.libmpv/out/libmpv`, or
wherever `VELA_LIBMPV_DIR` points.

A scenario can belong to one player with `onlyPlayer: 'embedded'` or
`onlyPlayer: 'external'`. In a run of the other player it is reported as
`SKIP` and not counted; named explicitly, it fails instead.
`embeddedplay` is the scenario that proves a play really is embedded;
`playerfallback` is about what happens where it cannot be;
`embeddedlatestart` stages an embedded play that does not start in time
and checks that the same play carries on in the external player while the
late one stays out of it.
