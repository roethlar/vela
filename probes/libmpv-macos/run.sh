#!/usr/bin/env bash
# Manual platform-probe.yml: probe=libmpv-macos, os=macos-latest.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"
: "${PROBE_OUT:?platform-probe.yml supplies PROBE_OUT}"
mkdir -p "$PROBE_OUT"
PROBE_OUT="$(cd "$PROBE_OUT" && pwd)"
export LIBMPV_ROOT="${LIBMPV_ROOT:-$repo/.libmpv}"
set_dir="$LIBMPV_ROOT/out/libmpv-macos-universal"

keep_evidence() {
  status=$?
  trap - EXIT
  set +e
  [ ! -f "$set_dir/MANIFEST" ] || cp "$set_dir/MANIFEST" "$PROBE_OUT/MANIFEST"
  [ ! -f "$LIBMPV_ROOT/toolchain-macos.txt" ] || cp "$LIBMPV_ROOT/toolchain-macos.txt" "$PROBE_OUT/"
  for dylib in "$set_dir"/*.dylib; do
    [ -f "$dylib" ] || continue
    /usr/bin/otool -arch all -L "$dylib"
    /usr/bin/lipo -archs "$dylib"
  done > "$PROBE_OUT/otool-L.txt" 2>&1
  /usr/bin/python3 -I -B - "$LIBMPV_ROOT/work-macos" "$PROBE_OUT/configure-logs" <<'PY'
from pathlib import Path
import shutil, sys
root, out = map(Path, sys.argv[1:])
for source in root.rglob('*'):
    if source.is_file() and source.name in {
        'config.log', 'config.h', 'config_components.h', 'meson-log.txt',
        'CMakeCache.txt', 'CMakeConfigureLog.yaml', 'macos.ini', 'macos.cmake',
    }:
        target = out / source.relative_to(root)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
PY
  printf 'probe exit: %s\n' "$status" > "$PROBE_OUT/result.txt"
  exit "$status"
}
trap keep_evidence EXIT

bash packaging/libmpv/macos/prepare-tools.sh 2>&1 | tee "$PROBE_OUT/tools.log"
bash scripts/build-libmpv-macos.sh 2>&1 | tee "$PROBE_OUT/build.log"
if ! /usr/bin/arch -x86_64 /usr/bin/python3 -I -c 'import platform; assert platform.machine() == "x86_64"'; then
  /usr/sbin/softwareupdate --install-rosetta --agree-to-license 2>&1 | tee "$PROBE_OUT/rosetta.log"
fi
/usr/bin/python3 -I -B packaging/libmpv/check-libmpv-macos.py "$set_dir" \
  --log-dir "$PROBE_OUT" 2>&1 | tee "$PROBE_OUT/check.log"
