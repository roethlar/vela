#!/usr/bin/env bash
# Build a universal app/DMG with the pinned set already downloaded or built
# at .libmpv/out/libmpv-macos-universal. See docs/development.md.
# Optional --signed requires signing/notarization to pass; otherwise build
# without Apple credentials. Uses standard Tauri output, no copy to dist/.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"
die() { printf 'build-macos: %s\n' "$*" >&2; exit 1; }
signed=false
case "${1:-}" in
  --signed) signed=true; shift ;;
  '') ;;
  *) die 'usage: bash scripts/build-macos.sh [--signed]' ;;
esac
[ "$#" -eq 0 ] || die 'usage: bash scripts/build-macos.sh [--signed]'
[ "$(uname -s)" = Darwin ] || die 'requires macOS'
[ -z "${CARGO_TARGET_DIR:-}${CARGO_BUILD_TARGET:-}" ] || die 'unset CARGO_TARGET_DIR and CARGO_BUILD_TARGET'
if [ "$signed" = false ]; then
  unset APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY
  unset APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
fi
if ! /usr/bin/arch -x86_64 /usr/bin/python3 -I -c 'import platform; assert platform.machine() == "x86_64"'; then
  /usr/sbin/softwareupdate --install-rosetta --agree-to-license
fi
node scripts/check-js-toolchain.mjs
npm ci
rustup target add aarch64-apple-darwin x86_64-apple-darwin
bash scripts/prepare-macos-package.sh
/usr/bin/python3 -I -B packaging/libmpv/check-libmpv-macos.py .libmpv/out/libmpv-macos-universal
bundle=src-tauri/target/universal-apple-darwin/release/bundle
# Remove only this wrapper's generated bundles, so an old DMG cannot pass.
rm -rf "$bundle/macos" "$bundle/dmg"
PATH="$repo/.libmpv/sign-tools:$PATH" MACOSX_DEPLOYMENT_TARGET=11.0 \
  npm run tauri -- build --target universal-apple-darwin --bundles app,dmg \
  --config .libmpv/tauri.libmpv-macos.conf.json
set -- "$bundle"/dmg/*.dmg
[ "$#" -eq 1 ] && [ -f "$1" ] || die 'expected exactly one universal DMG'
app="$bundle/macos/Vela.app"
if [ "$signed" = true ]; then
  xcrun stapler validate "$app"
  codesign --verify --strict --verbose=2 "$1"
  codesign -dv --verbose=4 "$1" 2>&1 | grep 'Authority=Developer ID Application'
fi
bash scripts/check-macos-package.sh "$bundle" "$signed"
printf 'Checked universal app and DMG in %s\n' "$bundle"
