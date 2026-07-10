#!/usr/bin/env bash
# Fetch the vendored WebKitWebDriver (Debian 2.50.6) plus its ICU 72 libs
# into tests/e2e/vendor/wkdriver/.
#
# Why a Debian binary on Arch: no distro ships a WebKitWebDriver for
# webkit2gtk 2.52 — see .agents/plans/e2e-harness.md (deviation 2026-07-05).
# URLs are pinned to a Debian point release; when Debian rolls it (the
# download 404s), bump the *_DEB names and sha256s below, or fetch the same
# filenames from snapshot.debian.org. Arch-aware (amd64/arm64) since the
# 2026-07-09 E2E re-home: the suite also runs on aarch64 Linux hosts.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
vendor="$here/vendor/wkdriver"

case "$(uname -m)" in
  x86_64)
    DEB_ARCH=amd64
    DEB_LIBDIR=x86_64-linux-gnu
    DRIVER_SHA=c58ac09a893b8b2766ef1c9cb91f24ad26cd0d0b47c10af610db68b412d71756
    ICU_SHA=f7f6f99c6d7b025914df2447fc93e11d22c44c0c8bdd8b6f36691c9e7ddcef88
    ;;
  aarch64)
    DEB_ARCH=arm64
    DEB_LIBDIR=aarch64-linux-gnu
    DRIVER_SHA=5856d9b6ed06f9083ce7b3490c6becb1de1339ca17bdc300eda7c52f46f276ca
    ICU_SHA=4f5d892fd81110435e45ed0a5f1b12899d7ff989d51db283cbc043f5631646d8
    ;;
  *)
    echo "fetch-driver: unsupported architecture $(uname -m) (amd64/arm64 only)" >&2
    exit 1
    ;;
esac

DRIVER_DEB="webkit2gtk-driver_2.50.6-1~deb12u2_${DEB_ARCH}.deb"
DRIVER_URL="http://ftp.debian.org/debian/pool/main/w/webkit2gtk/$DRIVER_DEB"
ICU_DEB="libicu72_72.1-3+deb12u1_${DEB_ARCH}.deb"
ICU_URL="http://ftp.debian.org/debian/pool/main/i/icu/$ICU_DEB"

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
cp "usr/lib/$DEB_LIBDIR/libicudata.so.72"* \
   "usr/lib/$DEB_LIBDIR/libicuuc.so.72"* \
   "usr/lib/$DEB_LIBDIR/libicui18n.so.72"* \
   "$vendor/lib/"

echo "fetch-driver: vendored WebKitWebDriver 2.50.6 into tests/e2e/vendor/wkdriver" >&2
