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

`npm run check` also runs the marker script mpv loads
(`src-tauri/resources/mpv-scripts/vela-markers.lua`) under LuaJIT and Lua 5.2,
the two Luas mpv runs it with (`luajit` and `lua5.2`); a Lua that is not
installed is skipped, with that reason, except in a CI run (`CI=true`, as
GitHub sets it), where it fails the check.

Linux end-to-end tests drive the real debug app, WebKitGTK, and mpv in a private
headless weston (a Wayland compositor) with a throwaway Vela config:

```bash
npm run e2e                    # build and run every scenario
npm run e2e -- --skip-build    # reuse the current debug binary
npm run e2e -- playback        # run one scenario by name
```

The E2E venue needs `tauri-driver`, weston, ffmpeg, mpv, bsdtar, and curl. The
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
Settings → Playback → Video player offers. The Linux and Windows packages the release
workflow builds carry it; a package you build yourself carries it when you
ask, and a development build can be pointed at it: see "Trying the embedded
player" and "Packaging the set" below. By default playback still runs the
system's `mpv` program as described under Architecture.

The embedded player will not use whichever mpv library a machine happens to
have. Vela builds one pinned set of libraries (mpv, FFmpeg, libplacebo,
libass, FreeType, HarfBuzz, FriBidi, dav1d, Lua and uchardet) and ships it
inside its Linux packages, Windows installers and universal macOS app. The set is always an LGPL
build, with mpv and its media libraries separate from Vela's executable.

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

For Windows x86_64, run `bash scripts/build-libmpv-windows.sh --container`
on Linux (or in WSL with a Linux container engine).
The separate recipe uses Debian Trixie's mingw-w64 cross compiler and writes
`.libmpv/out/libmpv-windows-x86_64/`: `libmpv-2.dll`, its non-system DLL
dependencies, licences and the same file-hash/version manifest format. It
uses the Linux component pins, D3D11 in both mpv and libplacebo (shaderc and
SPIRV-Cross), D3D11VA/DXVA2 decoding, WASAPI, DirectWrite font discovery,
Schannel HTTPS, Lua 5.2 and uchardet. Shader compiler support, zlib, iconv
and MinGW runtimes are static; the original library set remains separate
DLLs. `windows-license-notes.txt` records the licences, pin provenance and
outstanding zlib selection caveat. CI's `libmpv-windows` job builds/caches
and uploads `libmpv-windows-x86_64`; `libmpv-windows-check` downloads it on
Windows and runs `python -B packaging/libmpv/check-libmpv-windows.py DIR`.
That check verifies the manifest before loading, restricts DLL search to
the set and Windows, audits loaded module paths, reports the client API and
plays a two-second lavfi pattern to EOF with null outputs. It does not
exercise D3D11 drawing, HDR, hardware decoding or HTTPS. The Windows host
loads this set from the installed app; the release workflow packages it into
both installers (see "Windows installers with the set" below).

The Windows recipe applies `packaging/libmpv/windows/ffmpeg-schannel-revocation.patch`
after verifying and extracting the pinned FFmpeg archive, and fails if the
patch cannot apply. It adds only `SCH_CRED_IGNORE_NO_REVOCATION_CHECK` and
`SCH_CRED_IGNORE_REVOCATION_OFFLINE` to Schannel's verifying branch: chain,
host-name and revocation checks remain on, but unavailable revocation
information no longer prevents playback. The non-verifying branch is
unchanged. Like the macOS mpv build patch, it ships under `licenses/` (in
`ffmpeg/`), with a required MANIFEST `patch` record and file hash; it cannot
stand in for FFmpeg's licence texts. Both Windows CI/release cache keys hash
the patch. Preserve it with the build recipe and pinned sources, and
re-check its scope and application whenever FFmpeg is bumped.

The same pinned FFmpeg source uses GnuTLS on Linux (`--enable-gnutls`):
`libavformat/tls_gnutls.c` checks the peer chain with
`gnutls_certificate_verify_peers2` and the host with
`gnutls_x509_crt_check_hostname`, without requiring a revocation lookup or
positive revocation response. macOS uses SecureTransport
(`--enable-securetransport`): `libavformat/tls_securetransport.c` sets the
peer domain and uses the normal system trust evaluation (or `SecTrustEvaluate`
with a supplied CA), without adding a mandatory revocation-response policy.
Neither has Schannel's hard failure for missing revocation information, so
neither needs this patch. This is a source/configuration finding, not a
runtime HTTPS test of those platforms; system trust policy still applies.

For macOS 11+, run `bash packaging/libmpv/macos/prepare-tools.sh`, then
`bash scripts/build-libmpv-macos.sh` on Apple Silicon. The same Linux pins
produce `.libmpv/out/libmpv-macos-universal/`: separate arm64/x86_64 dylibs
joined with `lipo`, licences and a MANIFEST. FFmpeg uses system
SecureTransport, VideoToolbox, AudioToolbox, zlib and iconv; libass uses
CoreText. mpv uses the OpenGL render API (`plain-gl`) and the CoreAudio
and AVFoundation audio outputs, with Swift and its own macOS windows
disabled. Its VideoToolbox-to-OpenGL interop is built on that path by the
build patch (`packaging/libmpv/macos/mpv-sdk-only.patch`, which this pin
otherwise allows only with Swift), so hardware decoding draws straight from
VideoToolbox's frames, and falls back to copying them when the interop
cannot load; the build fails without it. Every install
name/reference uses `@rpath`, with `@loader_path` on
each dylib; in the app they resolve siblings in `Contents/Frameworks` without
adding a run path to Vela's executable.
CI's `libmpv-macos` job caches the set by all recipe inputs and the recorded
runner/toolchain, checks native and Rosetta playback with
`/usr/bin/python3 -I -B packaging/libmpv/check-libmpv-macos.py DIR`, and
uploads `libmpv-macos-universal`. The check audits MANIFEST integrity,
architectures, deployment versions, dependencies, clean EOF and loaded
library paths: every loaded library must be in the set's MANIFEST,
`/usr/lib` or `/System`, except the running interpreter's own
installation (`sys.base_prefix`; on a hosted runner, Xcode's Python
framework), so a Homebrew library or any other foreign image still
fails. The manual `libmpv-macos` probe retains diagnostic evidence. See
`packaging/libmpv/macos/NOTES.txt` for the build-file patch and limitations.
The packaging overlay declares macOS 11.0, and the build sets
`MACOSX_DEPLOYMENT_TARGET=11.0` for both Rust targets. This raises the app's
`LSMinimumSystemVersion` from Tauri's default 10.13 to the set's 11.0 floor;
it does not prove runtime compatibility on macOS 11. See "macOS app with
the set" below for local builds and the signed-app check. The first cold
set build and its check passed on a hosted Mac on 2026-11-16 (probe run
43969988737), and CI's job runs on every push and pull request;
visual/hardware/HTTPS checks remain separate.

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

Experimental, on Linux (Wayland), Windows and macOS. Settings → Playback →
Video player offers it, and says why when it cannot be used. Every released
package carries its library; a Linux package you build yourself carries it
when built with `--with-libmpv` (see "Packaging the set" below), and
`scripts/build-macos.sh` builds the macOS app with it. The plan and its
state are in `.agents/plans/embedded-player.md`.

1. Build the set (above), or download CI's.
2. Start a development build with the set's directory named:

   ```bash
   VELA_LIBMPV_DIR="$PWD/.libmpv/out/libmpv" npm run tauri dev
   ```

   Only a development build reads that variable. A release build loads the
   library from where its package put it and nowhere else: on Linux its
   resource directory, on Windows beside `Vela.exe`, on macOS the app's
   `Contents/Frameworks`.
3. In Settings → Playback → Video player choose "Vela's window
   (experimental)" (it is saved at once; no restart is needed). The choice is
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
session, a compositor without `wl_subcompositor` or `wp_viewporter`, and a
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
it has always been. The release workflow asks, for every Linux and Windows package it
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

#### macOS app with the set

On an Apple Silicon Mac with Xcode tools, Rust, the repo's Node/npm versions,
Homebrew and Rosetta, build the universal set once from the repo root:

```sh
bash packaging/libmpv/macos/prepare-tools.sh
bash scripts/build-libmpv-macos.sh
bash scripts/build-macos.sh
```

Or download a checked `libmpv-macos-universal` CI/release artifact into
`.libmpv/out/libmpv-macos-universal/` and run only the last command. The
wrapper installs npm dependencies and both Rust targets, validates the set,
builds `src-tauri/target/universal-apple-darwin/release/bundle/macos/Vela.app`
and its DMG, mounts the finished DMG read-only, and checks that app's actual
set natively and under Rosetta.
It uses an unsigned local app by default. For a signed/notarized build,
provide `APPLE_SIGNING_IDENTITY` for an installed Developer ID certificate
and `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH` for notarization,
then run `bash scripts/build-macos.sh --signed`. That mode requires the
signatures, stapled ticket and Gatekeeper assessment to pass. The older
`build-signed-macos.sh` uses the general build without this set; use this
wrapper for an embedded-player package.

`scripts/prepare-macos-package.sh` runs
`packaging/libmpv/write-macos-overlay.py`, which validates the set's original
MANIFEST against its files and the pins, and writes
`.libmpv/tauri.libmpv-macos.conf.json`. Every versioned dylib name comes from
the MANIFEST, as an absolute path in `bundle.macOS.frameworks`. Only
`licenses/` and `MANIFEST` are resource mappings to
`Contents/Resources/libmpv/`; the map merges with the base `mpv-scripts/`.
The overlay sets `minimumSystemVersion` to `11.0` and `hardenedRuntime` to
true. It is passed explicitly to Tauri; the ordinary base config is unchanged.

Tauri CLI 2.11.4 copies dylib framework entries to `Contents/Frameworks`,
then signs them before the app with its identity and temporary CI keychain.
Its `sign.rs` requests runtime flags only for executable sign targets, so
preparation also writes `.libmpv/sign-tools/codesign`. The build puts this
launcher on PATH; `packaging/libmpv/macos-codesign.py` forwards Tauri's exact
identity/keychain arguments to `/usr/bin/codesign`, adding
`--options runtime --timestamp` only for signing dylibs inside an app's
`Contents/Frameworks`. Signing completes before Tauri notarizes/staples the
app and builds the DMG. Verification uses Apple's tool directly. There is
no signing after notarization and no disabled library validation.

The player selects `<executable directory>/../Frameworks/libmpv.2.dylib`.
Development builds may use an absolute `VELA_LIBMPV_DIR`; release builds
ignore it. Exact name, absolute path and loaded-image checks remain in force.
The dylibs' `@rpath` dependencies and `@loader_path` run paths resolve within
Frameworks; Vela's executable needs no install-name or run-path edits.

After notarization the release leg runs:

```sh
bash scripts/check-macos-package.sh \
  src-tauri/target/universal-apple-darwin/release/bundle true
```

`check-macos-package.sh` mounts the final DMG read-only and calls
`check-libmpv-macos.py SET --app MOUNT/Vela.app`; the mount is detached even
on a failed check. `check-package-macos.py`, called by that checker, requires
exactly the set's
dylibs in Frameworks and its original notices and MANIFEST in Resources.
It verifies both architecture signatures of the app and every dylib, requiring
the same Developer ID Team ID, hardened runtime and secure timestamps. It
extracts temporary thin copies of both the input and shipped dylibs, removes
both signatures with `codesign --remove-signature`, and compares every
remaining byte for each architecture. This proves the shipped code, data and
load commands are the manifest-checked set's while allowing the signatures
and universal-file padding to change. Neither original is modified, and the
shipped MANIFEST is not rewritten to bless new code.

Only then do fresh native arm64 and Rosetta x86_64 workers repeat the checks
and load the **untouched app's Frameworks/libmpv.2.dylib**. Playback must reach
clean EOF; dyld/vmmap audits permit only that set or system libraries. The
existing strict Python-image rule also remains. Gatekeeper must still accept
the app after the check. `--unsigned-app` is only for local/secretless builds:
it omits Developer ID requirements but keeps code/notices/architecture checks
and both playback workers. A signed release never passes that flag.

These gates are implemented and exercised with portable test doubles on
Linux; real Tauri signing, signature removal, notarization and native/Rosetta
playback of this bundle still require the reviewer's manual release run.
Null-output playback does not test Vela's GL layer, visible controls, hardware
decoding, HDR/EDR, audio output, network playback or an actual macOS 11 host.

#### Windows installers with the set

On x64 Windows, install the usual Tauri prerequisites (MSVC Build Tools,
WebView2, the `x86_64-pc-windows-msvc` Rust toolchain and the Node/npm versions
accepted by `package.json`), plus PowerShell 7, x64 Python 3.13 (`python` on
PATH), and 7-Zip (`7z` on PATH or the normal `Program Files/7-Zip` location).
Windows supplies `msiexec`; lessmsi is not needed.

From the repo root, obtain the `libmpv-windows-x86_64` artifact from a
successful CI run matching this checkout. GitHub CLI prompts for a run:

```powershell
gh run download --repo roethlar/vela --name libmpv-windows-x86_64 --dir .libmpv/out/libmpv-windows-x86_64
pwsh -NoProfile -File scripts/build-windows.ps1
```

Alternatively, cross-build the set with
`bash scripts/build-libmpv-windows.sh --container` on Linux/WSL and copy the
whole output directory to that same Windows path. The wrapper verifies the
input MANIFEST and prerequisites before building, runs the toolchain check
and `npm ci`, clears old NSIS/MSI outputs, then runs:

```powershell
npm run tauri -- build --bundles nsis,msi --config src-tauri/tauri.libmpv-windows.conf.json
python -B packaging/libmpv/check-package-windows.py --set .libmpv/out/libmpv-windows-x86_64 --bundle-dir src-tauri/target/release/bundle
```

The wrapper stops on any failed command. It leaves the unsigned installers
under `src-tauri/target/release/bundle/nsis/` and `msi/`, with no copy to
`dist/`. It expects the standard Cargo host output directory (unset
`CARGO_TARGET_DIR` and `CARGO_BUILD_TARGET`). The older `scripts/build.ps1`
continues to build without the set.

The overlay maps the **whole directory**, including `licenses/` and
`MANIFEST`, from `../.libmpv/out/libmpv-windows-x86_64/` to `libmpv/`.
Tauri's Windows resource directory is the executable's directory (confirmed
in the locked Tauri 2.11.5 / tauri-utils 2.9.3 source), so the installed
library is `<install directory>/libmpv/libmpv-2.dll`, with `libmpv/` beside
Vela.exe. The base resource map is merged, retaining the mpv scripts.
The overlay's licence metadata follows `windows-license-notes.txt`, including
the Windows combined library's LGPL-3.0-or-later and its static support
notices; Vela's own licence stays MIT.

`check-package-windows.py` requires both formats and checks **every** matching
installer. NSIS is unpacked by `7z x`; MSI is expanded with
`msiexec /a ... /qn /norestart TARGETDIR=... /L*V ...` into a fresh temporary
administrative image. It does not run the NSIS installer or install Vela as a
product. An extraction error fails the check; an MSI failure prints its log.
The check locates exactly one Vela.exe, refuses links/reparse points, and
requires `libmpv/` in that executable's directory. Its MANIFEST must be
byte-identical to the input set's, and the existing Windows manifest checker
must accept every file's SHA-256 and size, the licences and component pins,
with no extra files. Only then does a fresh Python process run
`check-libmpv-windows.py` on that **unpacked** set to load and play to EOF.
Each installer gets its own process and temporary directory; both are
finished before the next is checked. This proves headless loading/playback,
not D3D11 drawing, HDR, hardware decoding, HTTPS or interactive installation.

#### In the release workflow

`.github/workflows/release.yml` puts the set into every Linux, Windows and macOS package it
builds (the owner's ruling of 2026-11-16, `.agents/decisions.md`):

- Its `libmpv` job builds the set from the release commit's pins, or restores
  it from the cache that CI's job of the same name fills (the same key), proves
  it on three clean distributions, and hands it to the two package jobs as a
  workflow artifact. The set is never a release asset of its own: the
  `inventory` job takes `vela-*` only.
- The Linux leg of `bundle` gives `tauri-action` `--config
  src-tauri/tauri.libmpv.conf.json` and then runs `check-package.sh --set
  .libmpv/out/libmpv` on the AppImage, the deb and the rpm. Only `inventory`
  attaches the checked bundles, as on every platform. `NO_STRIP=1` keeps the
  AppImage tooling's own `strip` away from the set's libraries, as
  `scripts/build.sh` does.
- The `arch` job sets `VELA_PACKAGE_LIBMPV` to the set's absolute path for
  `npm run build:arch` (that job does not go through `scripts/build.sh`) and
  runs the check on the package.
- Both jobs install what the check needs: `bsdtar`, Python 3, `unsquashfs`
  for the AppImage, and the system libraries the set loads.
- The `libmpv-windows` job runs after the audits, on `ubuntu-22.04`. It
  restores CI's Windows cache (the identical key), builds the pinned set on
  a miss, and always runs `check-libmpv-windows.py --manifest-only` before
  uploading the `libmpv-windows-x86_64` artifact. `bundle` waits for it.
- The Windows leg downloads that artifact into
  `.libmpv/out/libmpv-windows-x86_64`, uses x64 Python 3.13, and passes
  `--bundles nsis,msi --config src-tauri/tauri.libmpv-windows.conf.json` to
  Tauri. It then runs `check-package-windows.py` against every installer,
  before uploading `vela-windows`. As for Linux, only `inventory` attaches
  these checked installers to a tagged draft. Manual runs publish nothing.
  The Windows installers remain unsigned.
- The `libmpv-macos` job runs on `macos-latest` after the audits. It records
  the Apple toolchain before restoring the exact CI cache key, builds on a
  miss, checks native/Rosetta playback and uploads the universal set. CI's
  manual-only condition is not copied to releases: a failed set stops them.
- The macOS bundle leg downloads the set, generates the overlay and scoped
  signing launcher, prepares Rosetta, and builds the universal app/DMG with
  Tauri. After signing/notarization it checks the app's signatures, staple,
  local-network declaration and DMG signature, then proves and plays the set
  inside the app mounted from the final DMG as above and reassesses
  Gatekeeper before uploading the `vela-macos` artifact.
- No `bundle` leg gives `tauri-action` a tag or release name, so it only
  builds. After `bundle` and `arch` pass all checks, `inventory` alone
  attaches assets on a tag push: it requires exactly one DMG from
  `vela-macos`, the three Linux bundles and both Windows installers, then
  creates a missing draft with `--verify-tag` and the title `Vela <tag>`, or
  reuses an existing draft. It refuses an already published release before
  uploading anything. One `gh release upload --clobber` attaches the DMG,
  Linux and Windows bundles, Arch package and `SHA256SUMS`. Manual runs,
  including those targeting a tag, only produce workflow artifacts.
  The former macOS `.app.tar.gz` release asset is no longer uploaded: it
  duplicated the app in the DMG, was never in `SHA256SUMS`, and the DMG is
  what the package checks prove.

`tests/release-workflow.test.mjs` guards the attachment boundary and runs the
inventory's upload script against a local `gh` stub for missing, draft and
published releases, and missing or duplicate DMGs.
`tests/libmpv-packaging.test.mjs` holds the packaging lines of the workflow as far as
words can: a step that brought the set in, or left it out, without the names
the test knows would get past it, so a change to either workflow still has to
be read.

CI (`ci.yml`) builds the set and proves it in its own `libmpv` job, and builds
no package. Its `frontend` job installs `bsdtar` and squashfs-tools before
`npm run check`, so the tests of the check (most of
`tests/libmpv-packaging.test.mjs`, which pack and unpack packages they make
themselves) run there and are not skipped; it installs LuaJIT and Lua 5.2 for
the same reason, for `tests/markers-script.test.mjs`. In a CI run (`CI=true`)
a missing tool fails the check rather than skipping the tests that need it.

What has been seen, and what follows from it:

- **A real AppImage keeps the set as it is.** The release workflow's first
  run with the set (by hand on `main`, 2026-11-16) built the AppImage, the deb,
  the rpm and the Arch package on GitHub's runners, and the check passed all
  four: the set whole and unmodified at `usr/lib/Vela/libmpv` (under the
  AppImage's own root for the AppImage), loading from there. Should a later
  version of the AppImage tooling rewrite one of the set's libraries, the check
  fails the release and says which.
- **The packages' licence field** names the set's licences when the set is
  packaged: `bundle.license` in `src-tauri/tauri.libmpv.conf.json` for
  the bundles built with that overlay, and `license` in
  `packaging/arch/PKGBUILD` when `VELA_PACKAGE_LIBMPV` names the set.
  A default build's licence field is unchanged. Of the bundles only the rpm
  writes the field anywhere (its `License` tag); Tauri's deb bundler emits
  no licence field at all and an AppImage has none. The licence texts
  themselves are in every package that carries the set.

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

- **Desktop shell:** Tauri 2 with a single-page Svelte 5 frontend (built by
  Vite) in `src/` and a Rust backend in `src-tauri/`.
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
