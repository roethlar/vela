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

On Linux, `scripts/build.sh` checks those system packages itself on pacman,
apt, and dnf hosts. When some are missing it lists them with the exact install
command and offers to run it; `--install-deps` installs without asking, and a
run with no terminal attached prints the command and stops. Rust and Node are
not installed for you; a missing `cargo`, `node`, or `npm` is named before
anything builds.

Install a Node and npm inside the `engines` ranges in `package.json` (Node 26
bundles npm 11, so npm 12 or newer must be installed separately), then run:

```bash
git clone https://github.com/roethlar/vela.git
cd vela
node scripts/check-js-toolchain.mjs
npm ci
./scripts/build.sh
```

On a Mac whose login Keychain already contains the Developer ID identity, use
the explicit signed-build wrapper to build the universal DMG, submit it for
notarization, staple it, and independently verify both the app and disk image:

```bash
./scripts/build-signed-macos.sh
```

The wrapper reads App Store Connect API metadata from the sibling
`../Certificates` directory by default. Set `VELA_SIGNING_DIR` to use another
private directory. It never imports a `.p12` or reads its password; one-time
Keychain provisioning must already be complete, and any missing identity,
credential, signature, staple, Gatekeeper acceptance, or architecture fails
the build.

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
npm audit --audit-level=moderate
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

## The shipped libmpv set

This is build infrastructure for an embedded player that is still in
development. No released Vela loads it yet; playback today runs the system's
`mpv` program as described under Architecture.

The embedded player will not use whichever mpv library a machine happens to
have. Vela builds one pinned set of libraries (mpv, FFmpeg, libplacebo,
libass, FreeType, HarfBuzz, FriBidi, dav1d, Lua and uchardet) and ships it
inside its Linux packages. The set is always an LGPL build, and every library
in it is a separate shared object.

```bash
scripts/build-libmpv.sh --container     # needs podman or docker; several minutes
```

The result lands in `.libmpv/out/libmpv/` (ignored by git): the libraries,
`licenses/` with each component's licence text, and a `MANIFEST` listing the
component versions, every file with its checksum, the glibc version the set
needs, and the system libraries it loads from the machine. `.libmpv/` also
keeps the downloaded sources and the build tree, so a second run only redoes
the steps you name (`scripts/build-libmpv.sh --container mpv collect`).
CI builds the same set in its `libmpv` job and uploads it as the
`libmpv-linux-x86_64` artifact; `gh run download --name libmpv-linux-x86_64`
fetches it without building.

How the set is put together:

- **Pins.** `packaging/libmpv/versions.txt` is the only place a version or
  checksum is written. A download that does not match its checksum stops the
  build.
- **Baseline.** The build runs in `packaging/libmpv/Containerfile`, Ubuntu
  22.04, the distribution Vela's own Linux bundles are built on. The set then
  runs on that release and anything newer; the build fails if a library needs
  a newer glibc.
- **Private names.** Vela's process already has the system's FreeType,
  HarfBuzz, FriBidi and dav1d loaded, because WebKitGTK uses them. So every
  library in the set except `libmpv.so.2` is renamed (`libvela-avcodec.so.63`
  and so on) and finds its neighbours through an `$ORIGIN` run path. Load
  `libmpv.so.2` by absolute path with `RTLD_NOW | RTLD_DEEPBIND`; anything
  else lets the system's copies take over.
- **System libraries.** The set still loads the C and C++ runtimes, `libEGL`,
  `libva`, `libva-drm`, `libdrm`, `libpulse`, `libasound`, `libfontconfig`,
  `libgnutls` and `libz` from the machine: they have to match its GPU driver,
  sound server, fonts and certificate store. The build fails if the set needs
  any other system library.
- **One patch.** `packaging/libmpv/patches/` holds a change to mpv's
  `meson.build` (no mpv source file) that keeps VA-API hardware decoding
  without mpv's DRM/KMS video output. Its header explains why.

Check a built set with the script the CI job uses. It loads the library the
way Vela will, compares every file with the manifest, decodes three small
clips in software and makes one HTTPS request:

```bash
packaging/libmpv/check-in-container.sh docker.io/library/ubuntu:22.04
packaging/libmpv/check-in-container.sh docker.io/library/debian:12
packaging/libmpv/check-in-container.sh registry.fedoraproject.org/fedora:latest
```

### Bumping a component

1. Edit the component's line in `packaging/libmpv/versions.txt`: version, URL,
   and the SHA-256 of the new tarball (download it and run `sha256sum`).
2. For an mpv bump, check that both hunks of the patch still apply and still
   mean what the patch header says. Read mpv's `meson.build` changes for new
   dependencies; the build names every feature it wants and turns the rest
   off.
3. `scripts/build-libmpv.sh --container`, then the three container checks
   above.
4. `npm run check`. `tests/dependency-policy.test.mjs` compares the pins with
   the build script and guards the LGPL configuration, the list of system
   libraries and the CI job.

Shipping these libraries means Vela owns their security updates. Before a
release, read the security notices of FFmpeg, mpv, dav1d, FreeType and
HarfBuzz since the pinned versions. A known vulnerability in a pinned
component that Vela's use can reach blocks the release until the pin is
bumped, exactly as an `npm audit` or `cargo audit` failure does. This check
is manual: nothing automated compares the pins with an advisory database.

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
