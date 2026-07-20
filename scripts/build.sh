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
# Usage:
#   scripts/build.sh                 # build the host's default bundle
#   scripts/build.sh --native        # macOS: host arch only (skip universal)
#   scripts/build.sh --bundles deb,rpm,appimage   # override the bundle targets
set -euo pipefail
cd "$(dirname "$0")/.."

universal=1            # macOS: build a universal (arm64 + x86_64) binary by default.
bundles=""             # empty => pick a sensible default for the host below.

while [ $# -gt 0 ]; do
  case "$1" in
    --native)    universal=0 ;;
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

# A mismatched local toolchain can rewrite the lockfile or produce a bundle
# different from CI. The pins live in .node-version and packageManager.
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
echo
echo "==> Artifacts (also in $bundle_dir):"
found=0
for d in "${subdirs[@]}"; do
  if [ "$d" = "." ]; then dir="$bundle_dir"; else dir="$bundle_dir/$d"; fi
  [ -d "$dir" ] || continue
  while IFS= read -r f; do
    cp -R "$f" "$dist/"
    echo "    $dist/$(basename "$f")"
    found=1
  done < <(find "$dir" -mindepth 1 -maxdepth 1 ! -name 'rw.*' \
            \( -name '*.AppImage' -o -name '*.dmg' -o -name '*.app' \
             -o -name '*-setup.exe' -o -name '*.msi' \
             -o -name '*.deb' -o -name '*.rpm' \
             -o -name '*.pkg.tar.zst' -o -name '*.pkg.tar.xz' \) | sort)
done
[ "$found" -eq 1 ] || echo "    (nothing matched — check the build output above)"
