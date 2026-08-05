# Development

This document covers building Vela from source, the verification set, and the
architecture. For using Vela, see the [README](../README.md) and
[usage details](usage.md).

## Build an installer

Building needs Rust, Node.js, and
[Tauri's platform prerequisites](https://tauri.app/start/prerequisites/).
Debian-family Linux needs WebKitGTK 4.1, libsoup 3, GTK 3, librsvg,
Ayatana AppIndicator, and `patchelf`; macOS needs Xcode Command Line Tools;
Windows needs WebView2, MSVC Build Tools, and PowerShell 7 for the wrapper
below. The current Arch PKGBUILD targets x86_64.

Install the Node and npm versions pinned by `.node-version` and `package.json`,
then run:

```bash
git clone https://github.com/roethlar/vela.git
cd vela
node scripts/check-js-toolchain.mjs
npm ci
./scripts/build.sh
```

On Windows, use the equivalent PowerShell wrapper:

```powershell
pwsh scripts/build.ps1
```

The wrapper builds the native package for its host and prints the artifact
location. The macOS default is a universal Apple Silicon + Intel DMG;
Debian-family Linux defaults to an AppImage; Arch builds a pacman package; and
Windows builds an NSIS installer. Linux `.deb` and `.rpm` bundles can be forced
with:

```bash
./scripts/build.sh --bundles deb,rpm
```

## Development mode

Run the app in development mode after installing the pinned toolchain and
dependencies:

```bash
npm run tauri dev
```

The core verification set is:

```bash
node scripts/check-js-toolchain.mjs
npm ci
npm audit
npm run check
npm run build

cd src-tauri
cargo +1.89.0 check --locked
cargo +stable check --locked
cargo +stable clippy --all-targets --locked -- -D warnings
cargo +stable test --locked
cargo audit --file Cargo.lock
cd ..
```

Linux end-to-end tests drive the real debug app, WebKitGTK, and mpv on a private
Xvfb display with a throwaway Vela config:

```bash
npm run e2e                    # build and run every scenario
npm run e2e -- --skip-build    # reuse the current debug binary
npm run e2e -- playback        # run one scenario by name
```

The E2E venue needs `tauri-driver`, Xvfb, ffmpeg, mpv, bsdtar, and curl. The
first run downloads the pinned WebKitWebDriver described in
[tests/e2e/README.md](../tests/e2e/README.md); artifacts land under
`tests/e2e/artifacts/`.

## Architecture

- **Desktop shell:** Tauri 2 with a static SvelteKit/Svelte 5 frontend in
  `src/` and a Rust backend in `src-tauri/`.
- **Sources:** Plex and the shared Jellyfin/Emby client implement a common
  `MediaSource` interface behind a source registry. Item keys are namespaced by
  source, allowing unified browsing and cross-source playlists.
- **Playback:** mpv runs out of process. Vela owns launch configuration,
  stream authentication (including credential-safe Plex headers), JSON IPC,
  progress tracking, and sequence handoff; mpv owns decoding and video output.
- **Persistence:** defensive, atomic JSON stores keep application config and
  playlists separate, with cross-process locking and fail-closed parsing.
