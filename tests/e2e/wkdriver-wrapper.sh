#!/bin/sh
# tauri-driver's --native-driver target. Runs the vendored Debian
# WebKitWebDriver with its own ICU 72; the sonames are versioned
# (libicu*.so.72 vs the system's), so the injected LD_LIBRARY_PATH never
# shadows the system ICU inside the app the driver launches.
dir="$(dirname "$(readlink -f "$0")")/vendor/wkdriver"
[ -x "$dir/WebKitWebDriver" ] || {
  echo "wkdriver-wrapper: vendored driver missing — run tests/e2e/fetch-driver.sh" >&2
  exit 1
}
LD_LIBRARY_PATH="$dir/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  exec "$dir/WebKitWebDriver" "$@"
