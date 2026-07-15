#!/bin/sh
# tauri-driver's --native-driver target. Runs the checksum-pinned Ubuntu
# WebKitWebDriver matching the E2E venue's system WebKitGTK.
dir="$(dirname "$(readlink -f "$0")")/vendor/wkdriver"
[ -x "$dir/WebKitWebDriver" ] || {
  echo "wkdriver-wrapper: vendored driver missing — run tests/e2e/fetch-driver.sh" >&2
  exit 1
}
exec "$dir/WebKitWebDriver" "$@"
