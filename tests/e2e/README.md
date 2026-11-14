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

A full `--embedded` run (no scenario named) first runs
`npm run test:libmpv`: Rust tests, ignored in an ordinary `cargo test`,
that load that set without a window and check what the player relies on
mpv for when it changes HDR targets (each ask answered with its own
request; a refused value answered as refused; the shutdown event; a
change of a target reported, whoever made it, and never with an older
value once Vela's latest ask for it is answered; a target set back by
another client in the same pass as Vela's ask reported and put back). No
scenario can: weston never makes a play change mpv's target. If they fail,
or cannot load the set, the run fails before any scenario. A run of named
scenarios skips them and says so; `npm run test:libmpv` runs them alone.

A scenario can belong to one player with `onlyPlayer: 'embedded'` or
`onlyPlayer: 'external'`. In a run of the other player it is reported as
`SKIP` and not counted; named explicitly, it fails instead.
`embeddedplay` is the scenario that proves a play really is embedded;
`playerfallback` is about what happens where it cannot be;
`embeddedownconfig` plays with a user's mpv.conf that tries to override
Vela's own options;
`embeddedlatestart` stages an embedded play that does not start in time
and checks that the same play carries on in the external player while the
late one stays out of it.

The player view's controls have four scenarios of their own, all
embedded-only. Each drives the page for real (WebDriver clicks, and keys and
pointer moves through WebDriver's Actions, which arrive as trusted events)
and reads the outcome from mpv over its IPC socket:

- `embeddedcontrols`: the `player-state` events arrive; pause, the seek bar,
  the seek keys in both directions, mute, volume, fullscreen on and off
  (checked by the page's real height and mpv's `osd-dimensions`, not by
  Tauri's answer) and stop, through the controls and through the keys.
- `embeddedskip`: a button policy and an intro that ends past the end of
  the clip. The play is paused through the page, the page's Skip Intro
  button is pressed, and the marker must clear through the script while the
  play stays up. The script draws no button of its own there.
- `embeddedcrop`: Vela's own `vela-crop.lua` crops a letterboxed clip by
  itself in automatic mode, on a fresh and on a resumed play, and only on
  the Crop control in manual mode; `C` takes the crop away. It is the
  embedded counterpart of `autocrop`, which covers the stock script of the
  external player and is external-only.
- `embeddedfullscreenend`: fullscreen asked for through the view's control,
  then each way a play can end: a playlist's clean end with the next play
  started by the backend keeps it across the gap; an end in an error (the
  mock server refuses the stream), mpv told to quit over its socket, a clean
  end with no successor, Close, and a playlist's clean end whose next play
  the backend cannot start (the mock resolves no stream) each leave it
  within 2.5 s of the view
  going, far below the page's 50 s backstop. It prints the lifecycle events
  the page received for each, in order.

The controls hide after three seconds of playing untouched, and a hidden
control takes no click: `showPlayerBar` in `helpers.mjs` moves the pointer
over the view first. `cropscript` (ordinary venue, system mpv, no app) holds
the crop script's own contract, as `markerpayload` does for the marker
script. The debug app runs the scripts copied into
`src-tauri/target/debug/mpv-scripts/` at build time, so after a change to a
script the scenarios that drive the app need a run without `--skip-build`.
