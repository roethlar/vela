#!/usr/bin/env bash
# Check the actual app shipped in the universal DMG, mounted read-only.
# Usage: bash scripts/check-macos-package.sh BUNDLE_DIR true|false
# true requires Developer ID, notarization and Gatekeeper; false is explicitly
# for secretless/local builds. Both modes prove code/notices and play both CPUs.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"
[ "$#" -eq 2 ] || { echo 'usage: check-macos-package.sh BUNDLE_DIR true|false' >&2; exit 2; }
bundle="$1"
signed="$2"
case "$signed" in true|false) ;; *) echo 'macOS signing state is missing' >&2; exit 2 ;; esac
set -- "$bundle"/dmg/*.dmg
[ "$#" -eq 1 ] && [ -f "$1" ] || { echo 'expected exactly one universal DMG' >&2; exit 1; }
mount_dir="$(mktemp -d "${TMPDIR:-/tmp}/vela-package.XXXXXX")"
mounted=0
cleanup() {
  if [ "$mounted" -eq 1 ]; then /usr/bin/hdiutil detach "$mount_dir" >&2; fi
  if [ -d "$mount_dir" ]; then rmdir "$mount_dir"; fi
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
/usr/bin/hdiutil attach "$1" -readonly -nobrowse -mountpoint "$mount_dir"
mounted=1
app="$mount_dir/Vela.app"
set -- .libmpv/out/libmpv-macos-universal --app "$app" --log-dir .libmpv/logs-macos-app
if [ "$signed" = true ]; then
  /usr/bin/xcrun stapler validate "$app"
else
  set -- "$@" --unsigned-app
fi
# The checker removes signatures only from temporary thin copies. The
# workers load the original signed files inside the read-only mounted app.
/usr/bin/python3 -I -B packaging/libmpv/check-libmpv-macos.py "$@"
if [ "$signed" = true ]; then
  /usr/sbin/spctl --assess --type execute --verbose=4 "$app"
fi
/usr/bin/hdiutil detach "$mount_dir"
mounted=0
