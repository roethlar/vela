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
`tests/e2e/artifacts/`. A run that builds starts the executable that
Cargo's own report of that build names for the app. With `--skip-build` it
builds nothing and starts where `cargo metadata` predicts the debug binary
is, which does not follow a Cargo configuration file's `build.target`; such a
run is recorded as not built (see tests/e2e/README.md). Every session starts
that one path, and is refused if the file was written over or put in its
place after it was found.

## The shipped libmpv set

This is the library behind the experimental embedded player, which
Settings → Player → Video player offers. The Linux packages the release
workflow builds carry it; a package you build yourself carries it when you
ask, and a development build can be pointed at it: see "Trying the embedded
player" and "Packaging the set" below. By default playback still runs the
system's `mpv` program as described under Architecture.

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
  checksum is written. A line may name mirror URLs after the first; a source
  that fails or serves other bytes is skipped, and the build stops if none
  has the pinned bytes.
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
way Vela will, compares every file with the manifest, makes one HTTPS
request, and decodes three small clips in software. Each clip must decode
to what `packaging/libmpv/fixtures/expected.json` records for it: the same
video frames exactly, and the same amount of audio at the same level within
a small tolerance. Two deliberately damaged clips (one with dead video, one
with dead audio) must fail that comparison, so a comparison that has
stopped noticing is caught on every run. `packaging/libmpv/make-fixtures.sh`
says how the clips were made and how to record new results:

```bash
packaging/libmpv/check-in-container.sh docker.io/library/ubuntu:22.04
packaging/libmpv/check-in-container.sh docker.io/library/debian:12
packaging/libmpv/check-in-container.sh registry.fedoraproject.org/fedora:latest
```

### Trying the embedded player

Experimental and Linux on Wayland only. Settings → Player → Video player
offers it, and says why when it cannot be used. The released Linux packages
carry its library; a package you build yourself carries it when built with
`--with-libmpv` (see "Packaging the set" below). The plan and its state are
in `.agents/plans/embedded-player.md`.

1. Build the set (above), or download CI's.
2. Start a development build with the set's directory named:

   ```bash
   VELA_LIBMPV_DIR="$PWD/.libmpv/out/libmpv" npm run tauri dev
   ```

   Only a development build reads that variable. A release build loads the
   library from its own resource directory and nowhere else.
3. In Settings → Player → Video player choose "Vela's window
   (experimental)" (it is saved at once) and restart Vela. The choice is
   stored as
   `"player": "embedded"` in `~/.config/vela/config.json`; choosing mpv's own
   window removes the field.

The setting is read when Vela starts, because it decides how the window is
created: with `"player": "embedded"` the window is transparent, so that video
on a surface underneath it can show through, and WebKitGTK's GPU renderer
is left on, which every other start turns off. Change the setting and
restart. Before starting that way Vela leaves a marker file in its config
directory and the page removes it once it has loaded; if a start does not
get that far, the next one starts the ordinary way.

Whenever the embedded player cannot be used, the play goes to the external
player and says why in one line on its display. That includes a play the
embedded player was given and then could not start: the same play carries
on in the external player, and the embedded one is not tried again until
Vela is restarted. Vela's standard error has
the detail (`vela: playing in the external player: ...`, and for a library
that did not load, the loader's own message). Reasons include an X11
session, the NVIDIA proprietary driver (untested, so excluded), a
compositor without `wl_subcompositor` or `wp_viewporter`, and a
`WEBKIT_DISABLE_DMABUF_RENDERER` set in your own environment.

What exists so far: the picture, inside the window, in HDR on a display and
compositor that offer it (KDE's does), a control bar and keys over it
(pause, seeking, volume, tracks, fullscreen, the skip button, Close or `q`
or Escape), and black-bar cropping by Vela's own `vela-crop.lua`. With "use
my own mpv configuration"
on, the embedded player applies your `mpv.conf` but not your scripts folder,
per-script option files or key bindings.

`npm run e2e -- --embedded` runs the E2E suite in this player; see
[tests/e2e/README.md](../tests/e2e/README.md). Every playback behaviour the
plan lists must be shown in both players: each scenario declares which of
those rows it stands for, and `npm run check` fails when a row is shown in
one player and neither shown nor exempted in the other (README, "Parity").
That check reads declarations only. After a full run of each player on the
same commit, each without `--skip-build`, `npm run e2e:verdict`, run with
that commit checked out and clean (it refuses runs of any other, and runs
that did not build the app), checks what the runs did: every scenario
passed (or failed with its recorded product defect), and every row was
reached, in each player, by a scenario that passed there.

### Packaging the set

No package contains the set unless its builder asks for it. A default build
of the deb, rpm, AppImage or Arch package by `scripts/build.sh` is the package
it has always been. The release workflow asks, for every Linux package it
builds ("In the release workflow", below); CI builds no package. On your own
machine the way to ask is a switch on
the build script, for trying the embedded player in an installed Vela. (It
is not the only way in: see the Arch package below.)

```bash
scripts/build-libmpv.sh --container               # once: fills .libmpv/out/libmpv
scripts/build.sh --with-libmpv                    # this host's default package, with the set
scripts/build.sh --bundles deb,rpm --with-libmpv  # or the bundles you name
scripts/build.sh --with-libmpv=/path/to/a/set     # a set kept somewhere else
```

What the switch does depends on who makes the package:

- **deb, rpm and AppImage** are made by Tauri's bundler. The script passes it
  `--config src-tauri/tauri.libmpv.conf.json`, a configuration laid over
  `tauri.conf.json` for that one build, which adds `.libmpv/out/libmpv/` to
  the bundle's resources as `libmpv/`. `tauri.conf.json` itself does not
  mention the set. For a set in another directory the script writes the same
  overlay with that directory in it to a temporary file.
- **The Arch package** is made by `packaging/arch/PKGBUILD`, which installs
  the set when `VELA_PACKAGE_LIBMPV` names its directory, and puts each
  licence text under `/usr/share/licenses/vela/libmpv/` as well. makepkg
  cannot pass a PKGBUILD an argument, so the build script sets that variable
  for it, and clears it on a default build. makepkg does not strip a package
  that carries the set: stripping rewrites the set's libraries.

  That variable is all the PKGBUILD goes by, so it is a second way to ask,
  and one without a check. `npm run build:arch`, or makepkg run by hand, in a
  shell where `VELA_PACKAGE_LIBMPV` is set packages the set, and neither runs
  `check-package.sh`; only `scripts/build.sh --with-libmpv` does. makepkg
  then prints a line that begins `==> VELA_PACKAGE_LIBMPV is set`, with the
  command to check the package yourself. Do not keep the variable exported.

Either way the set ends up in `/usr/lib/Vela/libmpv/`, the one directory an
installed Vela loads it from (in an AppImage, the same path under the
AppImage's own root).

The set's directory is named on the command line and not taken from
`VELA_LIBMPV_DIR`. That variable belongs to a running development build
(above), and a developer may keep it exported, pointing at a set built
outside the container. A package build that obeyed it would ship that set
without having been asked to ship anything.

**The check.** The embedded player has no other library to fall back on, so
a package that was built to carry the set and lacks part of it installs
without complaint and then never plays embedded. A `--with-libmpv` build
therefore is not finished until `packaging/libmpv/check-package.sh` has
passed every package it produced; if one fails, the build fails and nothing
is copied to `dist/`. The script unpacks the package into a temporary
directory (nothing is installed) and requires that:

- the set is in `usr/lib/Vela/libmpv`, as files of the package and not
  through a symbolic link;
- every user may read every file of it, and may enter every directory
  above it (`usr`, `usr/lib`). An installed package belongs to
  root and Vela runs as whoever starts it. Tauri's rpm bundler copies a
  resource's permissions from the build tree as they are (its deb bundler and
  the PKGBUILD set them), so a set copied under a strict umask makes an rpm
  whose libraries no user can load. If the check says so, `chmod -R a+rX` the
  set and build again;
- its `MANIFEST` is the one of the set the build was given. A set whose
  library was changed and whose `MANIFEST` was written to match is whole by
  its own account; only the built set tells it apart, and the library is
  loaded only from that set. So this check needs `--set`, also run by hand,
  and stops without it, as it does for a `--set` with nothing after it;
- its components are the versions `packaging/libmpv/versions.txt` pins, so a
  set left over from before a bump is not shipped;
- every file the `MANIFEST` lists is present and unmodified, and nothing
  else is there;
- `libmpv.so.2` loads from the unpacked package, says it is the pinned mpv
  release with that release's client API version, and every library of the
  set in the process came from the unpacked package. Loading a library runs
  its start-up code, so a package that has failed any of the checks above is
  not loaded at all; a line that begins `skip load` says so;
- an Arch package also has every licence text under
  `usr/share/licenses/vela/libmpv`, each a file of the package itself (not a
  symbolic link, nor reached through one), readable by every user, below
  directories every user may enter.

It needs `bsdtar` and Python 3, and, because it really loads the library, the
system libraries the set leans on (the `system` lines of the `MANIFEST`). An
AppImage needs `unsquashfs` too (squashfs-tools, 4.4 or later): the check
never runs the AppImage, not even to unpack itself, but reads the squashfs
that follows its ELF program; a file named `.AppImage` that is not such a
program is refused before anything else is done with it. Without `unsquashfs` an AppImage is not judged:
it fails, and the check ends with status 2, as for a check that could not be
made. The build script asks it before building anything (`check-package.sh
--preflight`, with `--appimage` when an AppImage is to be built) whether it
can run here: those tools, and its own record of
the pinned mpv's client API version. A machine that cannot check then stops
at once and not after the release build. If the check cannot judge the
packages for another reason, the build fails too and says that the packages
could not be checked, which is not the same as a package that failed. It
can be run by hand, and it has a second mode for the opposite question, that
a package built without the switch holds no part of the set, by name and,
given a set to compare with, by contents:

```bash
packaging/libmpv/check-package.sh --set .libmpv/out/libmpv dist/*.deb
packaging/libmpv/check-package.sh --without-libmpv --set .libmpv/out/libmpv dist/*.deb
```

In either mode a file fails if it cannot be unpacked, or if what comes out of
it is not Vela (it has no `usr/bin/vela` of its own, not one reached through
a symbolic link). An empty file, for one, unpacks
without complaint and certainly holds no part of the set.

On an Arch host `packaging/arch/` keeps the packages of earlier builds. A
build, with or without `--with-libmpv`, asks makepkg which package this build
made (`makepkg --packagelist`) and copies that one alone to `dist/`, from
wherever makepkg was told to put it (`PKGDEST`); a `--with-libmpv` build
checks it first. An older package that carries the set can therefore not
pass in place of the new one, nor be handed out by a build that was not
asked for the set. If makepkg cannot say which package it made, the build
fails and copies nothing. If makepkg
writes its packages under an ending the check does not know (it knows
`.pkg.tar.zst` and `.pkg.tar.xz`), the check fails the package and says so.

#### In the release workflow

`.github/workflows/release.yml` puts the set into every Linux package it
builds (the owner's ruling of 2026-11-16, `.agents/decisions.md`):

- Its `libmpv` job builds the set from the release commit's pins, or restores
  it from the cache that CI's job of the same name fills (the same key), proves
  it on three clean distributions, and hands it to the two package jobs as a
  workflow artifact. The set is never a release asset of its own: the
  `inventory` job takes `vela-*` only.
- The Linux leg of `bundle` gives `tauri-action` `--config
  src-tauri/tauri.libmpv.conf.json` and then runs `check-package.sh --set
  .libmpv/out/libmpv` on the AppImage, the deb and the rpm. `tauri-action`
  uploads in the step that builds, which is before the check, so that leg
  does not attach to the draft release at all; the `inventory` job attaches
  the three bundles after both package jobs have passed. No Linux bundle
  reaches the draft unchecked. `NO_STRIP=1` keeps the AppImage tooling's own
  `strip` away from the set's libraries, as `scripts/build.sh` does.
- The `arch` job sets `VELA_PACKAGE_LIBMPV` to the set's absolute path for
  `npm run build:arch` (that job does not go through `scripts/build.sh`) and
  runs the check on the package.
- Both jobs install what the check needs: `bsdtar`, Python 3, `unsquashfs`
  for the AppImage, and the system libraries the set loads.
- macOS and Windows packages carry none of it.

`tests/libmpv-packaging.test.mjs` holds those lines of the workflow as far as
words can: a step that brought the set in, or left it out, without the names
the test knows would get past it, so a change to either workflow still has to
be read.

CI (`ci.yml`) builds the set and proves it in its own `libmpv` job, and builds
no package. Its `frontend` job installs `bsdtar` and squashfs-tools before
`npm run check`, so the tests of the check (most of
`tests/libmpv-packaging.test.mjs`, which pack and unpack packages they make
themselves) run there and are not skipped.

Not known yet:

- **Whether a real AppImage keeps the set as it is.** The check handles the
  format, but until the release workflow's first run with the set that path
  has not met a real AppImage. It expects the program at `usr/bin/vela` and
  the set at `usr/lib/Vela/libmpv` under the AppImage's own root, as Tauri
  lays an AppImage out. If the AppImage tooling rewrites one of the set's
  libraries, the check fails the release and says which.
- **The packages' licence field** (`bundle.license` in `tauri.conf.json`,
  `license=` in the PKGBUILD) still names only Vela's licence and the
  bundled script's. A package with the set carries every licence text; the
  field has to name those licences once the set ships.

### Bumping a component

1. Edit the component's line in `packaging/libmpv/versions.txt`: version, URL,
   and the SHA-256 of the new tarball (download it and run `sha256sum`).
2. For an mpv bump, check that both hunks of the patch still apply and still
   mean what the patch header says. Read mpv's `meson.build` changes for new
   dependencies; the build names every feature it wants and turns the rest
   off. Then read `MPV_CLIENT_API_VERSION` in the new release's
   `include/mpv/client.h` and bring `CLIENT_API_OF_MPV` and `CLIENT_API` in
   `packaging/libmpv/check-package.sh` up to date; the package check refuses
   to run, and `npm run check` fails, until they name the new release.
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
- **Playback:** by default mpv runs out of process. Vela owns launch
  configuration, stream authentication (including credential-safe Plex
  headers), JSON IPC, progress tracking, and sequence handoff; mpv owns
  decoding and video output. The experimental embedded player (Linux on
  Wayland) loads Vela's shipped libmpv in process instead and draws on a
  Wayland surface under the webview; everything else is shared, through one
  launch description and the same JSON IPC.
- **Persistence:** defensive, atomic JSON stores keep application config and
  playlists separate, with cross-process locking and fail-closed parsing.
