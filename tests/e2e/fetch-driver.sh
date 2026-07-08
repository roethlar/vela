#!/usr/bin/env bash
# Fetch the vendored WebKitWebDriver (Debian 2.50.6) plus its ICU 72 libs
# into tests/e2e/vendor/wkdriver/.
#
# Why a Debian binary on Arch: no distro ships a WebKitWebDriver for
# webkit2gtk 2.52 — see .agents/plans/e2e-harness.md (deviation 2026-07-05).
# URLs are pinned to a Debian point release; when Debian rolls it (the
# download 404s), bump the *_DEB names and sha256s below, or fetch the same
# filenames from snapshot.debian.org.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
vendor="$here/vendor/wkdriver"

DRIVER_DEB="webkit2gtk-driver_2.50.6-1~deb12u2_amd64.deb"
DRIVER_URL="http://ftp.debian.org/debian/pool/main/w/webkit2gtk/$DRIVER_DEB"
DRIVER_SHA=c58ac09a893b8b2766ef1c9cb91f24ad26cd0d0b47c10af610db68b412d71756
ICU_DEB="libicu72_72.1-3+deb12u1_amd64.deb"
ICU_URL="http://ftp.debian.org/debian/pool/main/i/icu/$ICU_DEB"
ICU_SHA=f7f6f99c6d7b025914df2447fc93e11d22c44c0c8bdd8b6f36691c9e7ddcef88

if [[ -x "$vendor/WebKitWebDriver" && "${1:-}" != "--force" ]]; then
  exit 0
fi

command -v bsdtar >/dev/null || {
  echo "fetch-driver: bsdtar (libarchive) is required" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

curl -fsSLO "$DRIVER_URL"
curl -fsSLO "$ICU_URL"
sha256sum -c --quiet <<EOF
$DRIVER_SHA  $DRIVER_DEB
$ICU_SHA  $ICU_DEB
EOF

bsdtar -xOf "$DRIVER_DEB" data.tar.xz | bsdtar -x
bsdtar -xOf "$ICU_DEB" data.tar.xz | bsdtar -x

rm -rf "$vendor"
mkdir -p "$vendor/lib"
cp usr/bin/WebKitWebDriver "$vendor/"
cp usr/lib/x86_64-linux-gnu/libicudata.so.72* \
   usr/lib/x86_64-linux-gnu/libicuuc.so.72* \
   usr/lib/x86_64-linux-gnu/libicui18n.so.72* \
   "$vendor/lib/"

echo "fetch-driver: vendored WebKitWebDriver 2.50.6 into tests/e2e/vendor/wkdriver" >&2
