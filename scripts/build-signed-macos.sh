#!/usr/bin/env bash
# Build Vela's universal macOS DMG with the Developer ID identity already in
# this user's login Keychain, notarize it through an App Store Connect API key,
# then independently verify the packaged app and disk image.
#
# One-time Keychain/certificate provisioning deliberately lives outside this
# script. Builds never read a .p12, its password, or the password list.
#
# Usage:
#   scripts/build-signed-macos.sh
#
# Credential discovery:
#   VELA_SIGNING_DIR defaults to the repo's sibling ../Certificates directory.
#   Its AppStoreConnectCertInfo.csv must contain exactly one
#   IssuerID,Name,KeyID,KeyFile row. Alternatively, set all three of
#   APPLE_API_ISSUER, APPLE_API_KEY, and APPLE_API_KEY_PATH.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd -P)"
cd "$repo_root"

die() {
  echo "signed macOS build: $*" >&2
  exit 1
}

case "${1:-}" in
  -h|--help)
    awk 'NR>1 && /^#/ {sub(/^# ?/,""); print; next} NR>1 {exit}' "$0"
    exit 0
    ;;
  "") ;;
  *) die "this wrapper takes no build options; it always produces the universal DMG" ;;
esac
[ "$#" -eq 0 ] || die "this wrapper takes no build options; it always produces the universal DMG"

[ "$(uname -s)" = Darwin ] || die "Developer ID signing is supported only on macOS"

for tool in security xcrun codesign spctl hdiutil lipo plutil shasum; do
  command -v "$tool" >/dev/null 2>&1 || die "required tool is unavailable: $tool"
done

identity="${APPLE_SIGNING_IDENTITY:-Developer ID Application: MICHAEL COELHO (27R2KCAHN7)}"
identities="$(security find-identity -v -p codesigning 2>&1)" || {
  printf '%s\n' "$identities" >&2
  die "could not inspect the login Keychain's code-signing identities"
}
printf '%s\n' "$identities" | grep -F "\"$identity\"" >/dev/null ||
  die "Keychain identity is unavailable: $identity"

api_env_count=0
[ -n "${APPLE_API_ISSUER:-}" ] && api_env_count=$((api_env_count + 1))
[ -n "${APPLE_API_KEY:-}" ] && api_env_count=$((api_env_count + 1))
[ -n "${APPLE_API_KEY_PATH:-}" ] && api_env_count=$((api_env_count + 1))

if [ "$api_env_count" -ne 0 ] && [ "$api_env_count" -ne 3 ]; then
  die "set APPLE_API_ISSUER, APPLE_API_KEY, and APPLE_API_KEY_PATH together"
fi

if [ "$api_env_count" -eq 0 ]; then
  signing_dir="${VELA_SIGNING_DIR:-$repo_root/../Certificates}"
  metadata="$signing_dir/AppStoreConnectCertInfo.csv"
  [ -f "$metadata" ] || die "App Store Connect metadata is missing: $metadata"

  header="$(LC_ALL=C sed -n '1p' "$metadata" | tr -d '\r')"
  bom="$(printf '\357\273\277')"
  header="${header#"$bom"}"
  [ "$header" = "IssuerID,Name,KeyID,KeyFile" ] ||
    die "unexpected metadata header in $metadata"

  data_rows="$(awk 'NR > 1 { sub(/\r$/, ""); if (length($0)) count++ } END { print count + 0 }' "$metadata")"
  [ "$data_rows" -eq 1 ] || die "metadata must contain exactly one credential row: $metadata"
  data_row="$(awk 'NR > 1 { sub(/\r$/, ""); if (length($0)) { print; exit } }' "$metadata")"
  field_count="$(printf '%s\n' "$data_row" | awk -F, '{ print NF }')"
  [ "$field_count" -eq 4 ] || die "metadata credential row must contain exactly four fields"

  IFS=',' read -r APPLE_API_ISSUER _ APPLE_API_KEY key_file <<< "$data_row"
  [ -n "$APPLE_API_ISSUER" ] || die "metadata IssuerID is empty"
  [ -n "$APPLE_API_KEY" ] || die "metadata KeyID is empty"
  case "$key_file" in
    ""|*/*|*\\*) die "metadata KeyFile must be a filename inside the signing directory" ;;
  esac
  APPLE_API_KEY_PATH="$signing_dir/$key_file"
fi

case "$APPLE_API_KEY_PATH" in
  /*) ;;
  *) APPLE_API_KEY_PATH="$repo_root/$APPLE_API_KEY_PATH" ;;
esac
[ -f "$APPLE_API_KEY_PATH" ] || die "App Store Connect private key is missing: $APPLE_API_KEY_PATH"

# Separate substitutions: GNU `stat -f` prints filesystem status before failing,
# which must not be concatenated with the fallback's mode.
key_mode="$(stat -f '%Lp' "$APPLE_API_KEY_PATH" 2>/dev/null)" ||
  key_mode="$(stat -c '%a' "$APPLE_API_KEY_PATH" 2>/dev/null)" ||
  die "could not inspect private-key permissions: $APPLE_API_KEY_PATH"
case "$key_mode" in
  400|600) ;;
  *) die "private key must be owner-only (0400 or 0600); found mode $key_mode" ;;
esac

export APPLE_SIGNING_IDENTITY="$identity"
export APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH
# This path signs with the installed Keychain identity and notarizes via the
# .p8 file. Refuse to inherit Tauri's alternate CI certificate-import mode.
unset APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_API_KEY_P8 KEYCHAIN_PASSWORD

echo "==> Validating Developer ID and App Store Connect credentials"
xcrun notarytool history \
  --key "$APPLE_API_KEY_PATH" \
  --key-id "$APPLE_API_KEY" \
  --issuer "$APPLE_API_ISSUER" \
  --output-format json >/dev/null || die "App Store Connect authentication failed"

"$repo_root/scripts/build.sh"

set -- "$repo_root"/dist/*.dmg
[ "$#" -eq 1 ] && [ -f "$1" ] || die "expected exactly one DMG in $repo_root/dist"
dmg="$1"

echo "==> Verifying signed DMG"
codesign --verify --strict --verbose=2 "$dmg"
codesign -dv --verbose=4 "$dmg" 2>&1 | grep -F "Authority=$identity" >/dev/null ||
  die "DMG does not carry the expected Developer ID authority"

mount_dir="$(mktemp -d "${TMPDIR:-/tmp}/vela-signed-build.XXXXXX")"
mounted=0
cleanup() {
  if [ "$mounted" -eq 1 ]; then
    hdiutil detach "$mount_dir" >/dev/null 2>&1 || true
  fi
  rm -rf "$mount_dir"
}
trap cleanup EXIT HUP INT TERM

hdiutil attach "$dmg" -readonly -nobrowse -mountpoint "$mount_dir" >/dev/null ||
  die "could not mount the built DMG for verification"
mounted=1

set -- "$mount_dir"/*.app
[ "$#" -eq 1 ] && [ -d "$1" ] || die "expected exactly one app inside the DMG"
app="$1"

echo "==> Verifying signed, stapled app"
codesign --verify --deep --strict --verbose=2 "$app"
codesign -dv --verbose=4 "$app" 2>&1 | grep -F "Authority=$identity" >/dev/null ||
  die "app does not carry the expected Developer ID authority"
xcrun stapler validate "$app"
spctl --assess --type execute --verbose=4 "$app"

executable_name="$(plutil -extract CFBundleExecutable raw "$app/Contents/Info.plist")"
[ -n "$executable_name" ] || die "packaged app has no CFBundleExecutable"
binary="$app/Contents/MacOS/$executable_name"
[ -f "$binary" ] || die "packaged executable is missing: $binary"
architectures="$(lipo -archs "$binary")"
case " $architectures " in *" x86_64 "*) ;; *) die "packaged executable lacks x86_64: $architectures" ;; esac
case " $architectures " in *" arm64 "*) ;; *) die "packaged executable lacks arm64: $architectures" ;; esac

hdiutil detach "$mount_dir" >/dev/null
mounted=0
rm -rf "$mount_dir"
trap - EXIT HUP INT TERM

echo "==> Signed, notarized universal DMG verified"
shasum -a 256 "$dmg"
