#!/usr/bin/env bash
# Fetch Ubuntu's WebKitWebDriver 2.52.3 into tests/e2e/vendor/wkdriver/.
#
# Ubuntu 26.04 now packages a driver matching the E2E venue's WebKitGTK; see
# .agents/plans/e2e-harness.md (2026-07-15 amendment). URLs and sha256s are
# pinned to that exact update for both supported architectures.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
vendor="$here/vendor/wkdriver"

case "$(uname -m)" in
  x86_64)
    DEB_ARCH=amd64
    DRIVER_BASE=https://archive.ubuntu.com/ubuntu
    DRIVER_SHA=b9ee5970c048e0e685fc35567d1c8b16be8a433c1d72cc74428a58cd65c8355a
    ;;
  aarch64)
    DEB_ARCH=arm64
    DRIVER_BASE=https://ports.ubuntu.com/ubuntu-ports
    DRIVER_SHA=15db04ce64da81ef59397006ba8e0fcef60ad4a5fd436b8fe90d668d58fada2a
    ;;
  *)
    echo "fetch-driver: unsupported architecture $(uname -m) (amd64/arm64 only)" >&2
    exit 1
    ;;
esac

DRIVER_DEB="webkitgtk-webdriver_2.52.3-0ubuntu0.26.04.2_${DEB_ARCH}.deb"
DRIVER_URL="$DRIVER_BASE/pool/universe/w/webkit2gtk/$DRIVER_DEB"
STAMP="$vendor/.package"

if [[ -x "$vendor/WebKitWebDriver" && -f "$STAMP" && "${1:-}" != "--force" ]] &&
   grep -Fxq "$DRIVER_DEB" "$STAMP"; then
  exit 0
fi

command -v bsdtar >/dev/null || {
  echo "fetch-driver: bsdtar (libarchive) is required" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

curl -fsSLo "$DRIVER_DEB" "$DRIVER_URL"
sha256sum -c --quiet <<EOF
$DRIVER_SHA  $DRIVER_DEB
EOF

bsdtar -xOf "$DRIVER_DEB" data.tar.zst | bsdtar -x

rm -rf "$vendor"
mkdir -p "$vendor"
cp usr/bin/WebKitWebDriver "$vendor/"
printf '%s\n' "$DRIVER_DEB" > "$STAMP"

echo "fetch-driver: vendored WebKitWebDriver 2.52.3 into tests/e2e/vendor/wkdriver" >&2
