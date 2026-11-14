#!/usr/bin/env bash
# Build Vela's installable bundle for whatever this host can actually produce.
#
# Tauri does not cross-compile in practice, so each machine builds for itself:
#   Windows -> NSIS installer     (src-tauri/target/release/bundle/nsis, *-setup.exe)
#   macOS   -> .app inside a .dmg (universal by default; dmg/ + macos/)
#   Linux, Debian/Ubuntu family -> AppImage (.../bundle/appimage)
#   Linux, Arch family          -> native pacman package via `npm run build:arch`
#                                  (packaging/arch/*.pkg.tar.zst)
#
# Why Linux splits by distro: linuxdeploy's AppImage tooling assumes a Debian-
# family layout (and an older glibc). On Arch it breaks — modern gdk-pixbuf has
# no external loader dir for the GTK plugin to copy, and RELR relocations defeat
# its bundled `strip`. The repo's Arch package (packaging/arch/PKGBUILD) is the
# supported path there; portable AppImages are built on ubuntu in CI. Pass
# --bundles to override the host default (e.g. force appimage, or build deb,rpm).
#
# Final artifacts are also copied to a top-level dist/ (recreated on every
# successful build, gitignored), so the installer is always at dist/<name>
# regardless of the deep target-triple/profile/bundle path Tauri writes to.
# dist/ lives outside src-tauri/target, so it survives a `cargo clean`.
#
# This script does NOT change the version. A build is only meaningfully unique
# when the source is, so the version is bumped when the code changes (run
# scripts/bump.sh as part of a code change), not here at build time.
#
# Run from anywhere; it cd's to the repo root.
#
# mpv is intentionally NOT bundled — Vela detects it at runtime and offers to
# install it, keeping these packages small and the player user-updatable.
#
# The one exception is opt-in and Linux only. Vela's embedded player
# (experimental, and not offered anywhere in the app) does not run the mpv
# program: it loads Vela's own build of mpv's library, the "libmpv set", from
# the package's resources and from nowhere else. A default build does not
# package that set and is the package it has always been. --with-libmpv
# packages it, from .libmpv/out/libmpv (where scripts/build-libmpv.sh
# --container leaves it) or from the directory named as --with-libmpv=DIR.
# Every package such a build produces is then unpacked and checked by
# packaging/libmpv/check-package.sh: the whole set must be where Vela will
# look for it, unchanged, and must load from there. A package that fails stops
# the build, and nothing is copied to dist/. What the check itself needs
# (bsdtar, Python 3, and unsquashfs for an AppImage) is asked for before
# anything is built. See
# docs/development.md, "Packaging the set".
#
# Usage:
#   scripts/build.sh                 # build the host's default bundle
#   scripts/build.sh --native        # macOS: host arch only (skip universal)
#   scripts/build.sh --bundles deb,rpm,appimage   # override the bundle targets
#   scripts/build.sh --install-deps  # Linux: install missing system packages
#                                    # without asking first
#   scripts/build.sh --with-libmpv   # Linux: also package the libmpv set
#   scripts/build.sh --with-libmpv=DIR   # ... taken from DIR
#
# On Linux the script checks the system build prerequisites up front (pacman,
# apt, or dnf hosts). When packages are missing it lists them and offers to
# install them; without a terminal to ask on, it prints the install command
# and stops instead.
set -euo pipefail
invoked_from="$PWD"    # a directory named on the command line is relative to here
cd "$(dirname "$0")/.."

universal=1            # macOS: build a universal (arm64 + x86_64) binary by default.
bundles=""             # empty => pick a sensible default for the host below.
install_deps=0         # Linux: 1 => install missing system packages unprompted.
with_libmpv=0          # Linux: 1 => package the embedded player's libmpv set.
libmpv_set=""          # where that set is; empty => .libmpv/out/libmpv here.

while [ $# -gt 0 ]; do
  case "$1" in
    --native)    universal=0 ;;
    --install-deps) install_deps=1 ;;
    --with-libmpv)   with_libmpv=1 ;;
    --with-libmpv=*) with_libmpv=1; libmpv_set="${1#*=}"
                     [ -n "$libmpv_set" ] || { echo "--with-libmpv= needs a directory" >&2; exit 2; } ;;
    --bundles)   shift; bundles="${1:?--bundles needs a value}" ;;
    --bundles=*) bundles="${1#*=}" ;;
    -h|--help)
      # Print the header comment block (everything between the shebang and the
      # first non-comment line), stripping the leading "# ".
      awk 'NR>1 && /^#/ {sub(/^# ?/,""); print; next} NR>1 {exit}' "$0"
      exit 0 ;;
    *) echo "Unknown option: $1 (try --help)" >&2; exit 2 ;;
  esac
  shift
done

# linuxdeploy + appimagetool are old-runtime AppImages that dlopen libfuse.so.2
# (absent on FUSE3-only hosts) and strip bundled libs with an ancient `strip`
# that chokes on modern .relr.dyn sections. These two env vars sidestep both;
# they have no effect on deb/rpm and produce identical output.
appimage_workarounds() { export APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1; }

# Arch family? (ID=arch, or ID_LIKE lists arch — Manjaro, EndeavourOS, ...)
is_arch() { [ -r /etc/os-release ] && grep -qiE '^(ID|ID_LIKE)=.*\barch\b' /etc/os-release; }

# --- Detect the host OS ------------------------------------------------------
case "$(uname -s)" in
  Linux*)                 os=linux ;;
  Darwin*)                os=macos ;;
  MINGW*|MSYS*|CYGWIN*)   os=windows ;;
  *) echo "Unsupported host OS: $(uname -s)" >&2; exit 1 ;;
esac

# --- Decide what to build ----------------------------------------------------
mode=tauri                              # tauri => `tauri build`; arch => `build:arch`
extra_args=()
bundle_dir="src-tauri/target/release/bundle"
subdirs=()
case "$os" in
  linux)
    if [ -n "$bundles" ]; then         # explicit override wins
      requested=()
      IFS=',' read -r -a requested <<< "$bundles"
      for bundle in "${requested[@]}"; do
        case "$bundle" in
          appimage|deb|rpm) subdirs+=("$bundle") ;;
          *) echo "Unsupported Linux bundle: $bundle (use appimage, deb, or rpm)" >&2; exit 2 ;;
        esac
      done
      appimage_workarounds
    elif is_arch; then                 # native AppImage can't be built here
      mode=arch
      bundle_dir="packaging/arch"
      subdirs=(.)
    else
      bundles=appimage; subdirs=(appimage)
      appimage_workarounds
    fi
    ;;
  windows) bundles="${bundles:-nsis}"; subdirs=(nsis) ;;
  macos)
    : "${bundles:=dmg}"                # dmg also emits the .app under macos/
    subdirs=(dmg macos)
    if [ "$universal" -eq 1 ]; then
      extra_args+=(--target universal-apple-darwin)
      bundle_dir="src-tauri/target/universal-apple-darwin/release/bundle"
      # The universal build links both arches; make sure rustup has them.
      if command -v rustup >/dev/null 2>&1; then
        rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
      fi
    fi
    ;;
esac

# --- The libmpv set (opt-in) ---------------------------------------------------
# Settled before anything compiles: a build that was asked to package the set
# and cannot find one says so now, not after the release build.
libmpv_overlay="src-tauri/tauri.libmpv.conf.json"   # adds the set to the bundle's resources
libmpv_check="packaging/libmpv/check-package.sh"
libmpv_tmp=""
cleanup() { [ -z "$libmpv_tmp" ] || rm -rf "$libmpv_tmp"; }
trap cleanup EXIT
if [ "$with_libmpv" -eq 1 ]; then
  [ "$os" = linux ] || {
    echo "--with-libmpv: the libmpv set exists for Linux packages only" >&2
    exit 2
  }
  if [ -z "$libmpv_set" ]; then
    libmpv_set=".libmpv/out/libmpv"
  else
    case "$libmpv_set" in /*) ;; *) libmpv_set="$invoked_from/$libmpv_set" ;; esac
  fi
  if [ ! -f "$libmpv_set/MANIFEST" ] || [ ! -f "$libmpv_set/libmpv.so.2" ]; then
    echo "--with-libmpv: there is no libmpv set in $libmpv_set" >&2
    echo "    Build one with scripts/build-libmpv.sh --container, or name the directory of one: --with-libmpv=DIR" >&2
    exit 1
  fi
  libmpv_set="$(cd "$libmpv_set" && pwd -P)"
  # The packages are checked when they have been built, and that check has
  # needs of its own: bsdtar, Python 3, a client API number that belongs to
  # the pinned mpv, and unsquashfs when an AppImage is built. Whether they are
  # met is known now.
  libmpv_preflight=(--preflight --set "$libmpv_set")
  for d in "${subdirs[@]}"; do
    [ "$d" != appimage ] || libmpv_preflight+=(--appimage)
  done
  "$libmpv_check" "${libmpv_preflight[@]}" || {
    echo "--with-libmpv: the packages could not be checked once built (see above), so nothing is built" >&2
    exit 1
  }
fi

# --- Linux build prerequisites -------------------------------------------------
# Discover missing system packages before anything compiles, so the failure is
# one actionable line instead of a pkg-config error minutes into the build (or
# a binary that cannot load libwebkit2gtk at launch). The Debian list mirrors
# the CI workflows; the Arch list adds the PKGBUILD's runtime depends, which
# makepkg refuses to build without.
pkg_installed() {
  case "$1" in
    pacman) pacman -Q "$2" >/dev/null 2>&1 ;;
    apt)    [ "$(dpkg-query -W -f='${db:Status-Status}' "$2" 2>/dev/null)" = installed ] ;;
    dnf)    rpm -q "$2" >/dev/null 2>&1 ;;
  esac
}

ensure_linux_prereqs() {
  local pm pkgs=() install=() missing=() p answer
  if is_arch && command -v pacman >/dev/null 2>&1; then
    pm=pacman; install=(pacman -S --needed)
    pkgs=(base-devel webkit2gtk-4.1 libsoup3 gtk3 librsvg)
    if [ "$mode" = arch ]; then pkgs+=(mpv hicolor-icon-theme); else pkgs+=(patchelf); fi
  elif command -v apt-get >/dev/null 2>&1 && command -v dpkg-query >/dev/null 2>&1; then
    pm=apt; install=(apt-get install -y)
    pkgs=(build-essential pkg-config libwebkit2gtk-4.1-dev libsoup-3.0-dev
          libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev patchelf)
  elif command -v dnf >/dev/null 2>&1 && command -v rpm >/dev/null 2>&1; then
    pm=dnf; install=(dnf install -y)
    pkgs=(gcc pkgconf-pkg-config webkit2gtk4.1-devel libsoup3-devel gtk3-devel
          librsvg2-devel libappindicator-gtk3-devel patchelf)
  else
    echo "==> Unrecognized package manager; skipping the system prerequisite check" >&2
    return 0
  fi

  for p in "${pkgs[@]}"; do
    pkg_installed "$pm" "$p" || missing+=("$p")
  done
  [ "${#missing[@]}" -gt 0 ] || return 0

  [ "$(id -u)" -eq 0 ] || install=(sudo "${install[@]}")
  echo "==> Missing build prerequisites: ${missing[*]}" >&2
  echo "    Install with: ${install[*]} ${missing[*]}" >&2
  if [ "$install_deps" -ne 1 ]; then
    if [ -t 0 ] && [ -t 1 ]; then
      read -r -p "    Install them now? [Y/n] " answer
      case "$answer" in
        ""|[Yy]*) ;;
        *) echo "Cannot build without them." >&2; exit 1 ;;
      esac
    else
      echo "    Run that command, or re-run with --install-deps." >&2
      exit 1
    fi
  fi
  "${install[@]}" "${missing[@]}"
}

if [ "$os" = linux ]; then
  ensure_linux_prereqs
  command -v cargo >/dev/null 2>&1 || {
    echo "Rust is not installed (no cargo on PATH); install it from https://rustup.rs" >&2
    exit 1
  }
fi

# Node and npm are not installed for you on any host. Name a missing one here:
# without `node` the toolchain check below cannot even start, and bash's bare
# "command not found" says nothing about which versions are wanted.
for tool in node npm; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "$tool is not installed (not on PATH); install a Node and npm inside the package.json engines ranges — see docs/development.md" >&2
    exit 1
  }
done

# An incompatible local toolchain (e.g. Node 26's bundled npm 11) can rewrite
# the lockfile. The accepted ranges live in package.json engines.
node scripts/check-js-toolchain.mjs

# --- Ensure JS deps exist and are current (tauri CLI lives in node_modules) ---
# npm writes node_modules/.package-lock.json on every install, so it's a faithful
# marker of what's actually installed. Reinstall when node_modules is absent OR
# when package-lock.json is newer than that marker — otherwise a dependency added
# after the last install (e.g. a new @fontsource font) is silently missing and
# the frontend build fails to resolve its import.
if [ ! -d node_modules ] || [ package-lock.json -nt node_modules/.package-lock.json ]; then
  echo "==> JS deps missing or stale; running npm install"
  npm install
fi

# Tauri keeps older versioned installers in its generated bundle directories.
# Clear only the selected, known generated targets so dist/ cannot republish a
# stale artifact after an otherwise successful build. The Arch output directory
# also contains package sources, so its `.` entry is deliberately never removed.
if [ "$mode" = tauri ]; then
  for d in "${subdirs[@]}"; do
    [ "$d" = "." ] || rm -rf "$bundle_dir/$d"
  done
fi

# --- Hand the libmpv set to the build, or keep it out -------------------------
# The Tauri bundles take it as one more resource, through a config laid over
# tauri.conf.json for this build only. The Arch package is not bundled by Tauri:
# its PKGBUILD installs the set, and since makepkg cannot pass a PKGBUILD an
# argument, it reads the set's directory from VELA_PACKAGE_LIBMPV. That variable
# is cleared on a default build, so one left in the builder's shell cannot put
# the set into a package that was not asked to carry it.
unset VELA_PACKAGE_LIBMPV
if [ "$with_libmpv" -eq 1 ]; then
  if [ "$mode" = arch ]; then
    export VELA_PACKAGE_LIBMPV="$libmpv_set"
  else
    # The overlay names the set where build-libmpv.sh leaves it. A set kept
    # anywhere else needs the same overlay with that directory in its place;
    # it is written to a temporary file, and the committed one is not edited.
    if [ ! "$libmpv_set" -ef .libmpv/out/libmpv ]; then
      libmpv_tmp="$(mktemp -d)"
      node -e '
        const fs = require("node:fs");
        const [overlayFile, setDirectory] = process.argv.slice(1);
        const overlay = JSON.parse(fs.readFileSync(overlayFile, "utf8"));
        const targets = Object.values(overlay.bundle.resources);
        if (targets.length !== 1) throw new Error(`${overlayFile} must map exactly one directory`);
        overlay.bundle.resources = { [`${setDirectory}/`]: targets[0] };
        process.stdout.write(`${JSON.stringify(overlay, null, 2)}\n`);
      ' "$libmpv_overlay" "$libmpv_set" > "$libmpv_tmp/tauri.libmpv.conf.json"
      libmpv_overlay="$libmpv_tmp/tauri.libmpv.conf.json"
    fi
    extra_args+=(--config "$libmpv_overlay")
  fi
  echo "==> Packaging the libmpv set from $libmpv_set"
fi

# --- Build -------------------------------------------------------------------
# `npm run` uses the repo-local @tauri-apps/cli; beforeBuildCommand in
# tauri.conf.json builds the Svelte frontend first.
if [ "$mode" = arch ]; then
  echo "==> Arch host: building native pacman package (npm run build:arch)"
  npm run build:arch
else
  echo "==> Host: $os   bundles: $bundles${extra_args[*]:+   ${extra_args[*]}}"
  # Bash 3.2 treats an empty array expansion as an unset variable under `set
  # -u`. macOS ships that Bash, and --native deliberately leaves extra_args
  # empty, so omit the expansion entirely in that case.
  if [ "${#extra_args[@]}" -gt 0 ]; then
    npm run tauri -- build --bundles "$bundles" "${extra_args[@]}"
  else
    npm run tauri -- build --bundles "$bundles"
  fi
fi

# --- Collect artifacts into dist/ ---------------------------------------------
# One stable, shallow location per build. Recreated wholesale so it only ever
# holds the CURRENT build's output (stale versions accumulating would recreate
# the very mess this exists to avoid). `rw.*` are bundle_dmg.sh temp images
# that interrupted macOS builds leave behind — never artifacts.
dist="dist"
rm -rf "$dist"
mkdir -p "$dist"
artifacts=()
if [ "$mode" = arch ] && [ "$with_libmpv" -eq 1 ]; then
  # packaging/arch is never cleared, so the packages of earlier builds lie
  # there, and makepkg may have been told to put this build's package
  # somewhere else altogether (PKGDEST in makepkg.conf). Collected as below,
  # an older package that carries the set would be checked, pass, and be
  # handed out as this build's, while the package this build made went
  # unchecked. So makepkg is asked which package this build made, and that
  # one is checked and copied: it has to be there.
  while IFS= read -r f; do
    [ -f "$f" ] || {
      echo "==> --with-libmpv: makepkg names $f as the package of this build, and it is not there; see the build's output above" >&2
      exit 1
    }
    artifacts+=("$f")
  done < <(cd "$bundle_dir" && makepkg --packagelist)
  [ "${#artifacts[@]}" -eq 0 ] || bundle_dir="$(dirname "${artifacts[0]}")"
else
  for d in "${subdirs[@]}"; do
    if [ "$d" = "." ]; then dir="$bundle_dir"; else dir="$bundle_dir/$d"; fi
    [ -d "$dir" ] || continue
    while IFS= read -r f; do
      artifacts+=("$f")
    done < <(find "$dir" -mindepth 1 -maxdepth 1 ! -name 'rw.*' \
              \( -name '*.AppImage' -o -name '*.dmg' -o -name '*.app' \
               -o -name '*-setup.exe' -o -name '*.msi' \
               -o -name '*.deb' -o -name '*.rpm' \
               -o -name '*.pkg.tar.zst' -o -name '*.pkg.tar.xz' \) | sort)
  done
fi

# A build that was asked to package the libmpv set has not succeeded until
# every package it produced is known to carry it: the embedded player has no
# other library to fall back on, so from a package without the whole set that
# player can never start. The packages are checked where the build left them,
# before any is copied; a build that produced nothing has nothing that passed.
if [ "$with_libmpv" -eq 1 ]; then
  echo
  echo "==> Checking the libmpv set in every package ($libmpv_check)"
  if [ "${#artifacts[@]}" -eq 0 ]; then
    echo "==> --with-libmpv: the build left no package to check; see its output above" >&2
    exit 1
  fi
  # The check ends with 1 when a package failed and with anything else that
  # is not 0 when it could not judge the packages at all. The two are not the
  # same news and are not told in the same words.
  libmpv_checked=0
  "$libmpv_check" --set "$libmpv_set" "${artifacts[@]}" || libmpv_checked=$?
  if [ "$libmpv_checked" -eq 1 ]; then
    echo "==> A package does not carry the libmpv set as it must (see above). Nothing was copied to $dist/." >&2
    exit 1
  elif [ "$libmpv_checked" -ne 0 ]; then
    echo "==> The packages could not be checked for the libmpv set (see above; the check ended with status $libmpv_checked). Nothing was copied to $dist/." >&2
    exit 1
  fi
fi

echo
echo "==> Artifacts (also in $bundle_dir):"
if [ "${#artifacts[@]}" -gt 0 ]; then
  for f in "${artifacts[@]}"; do
    cp -R "$f" "$dist/"
    echo "    $dist/$(basename "$f")"
  done
else
  echo "    (nothing matched — check the build output above)"
fi
