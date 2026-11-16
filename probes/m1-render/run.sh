#!/bin/bash
# Run from the repository root. The workflow preserves everything in PROBE_OUT.
set -uo pipefail
out="${PROBE_OUT:?set PROBE_OUT to the artifact directory}"
mkdir -p "$out" || exit 1
out="$(cd "$out" && pwd)" || exit 1
export PROBE_OUT="$out"
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'm1-render requires macOS' >&2
  exit 1
fi

echo 'preparing m1-render' > "$out/result.txt"
if ! brew install mpv > "$out/brew-install.log" 2>&1; then
  echo 'brew install mpv failed' > "$out/result.txt"
  tail -n 60 "$out/brew-install.log" >&2
  exit 1
fi
if ! brew info mpv > "$out/brew-info.txt" 2>&1; then
  cat "$out/brew-info.txt" >&2
  exit 1
fi
prefix="$(brew --prefix)" || { echo 'brew --prefix failed' >&2; exit 1; }
head -n 3 "$out/brew-info.txt" > "$out/libmpv.txt"
if ! ls -l "$prefix"/lib/libmpv*.dylib >> "$out/libmpv.txt" 2>&1; then
  echo "No Homebrew libmpv dylib in $prefix/lib" >&2
  cat "$out/libmpv.txt" >&2
  exit 1
fi
export M1_LIBMPV="$prefix/lib/libmpv.2.dylib"
if [[ ! -f "$M1_LIBMPV" ]]; then
  echo "Missing $M1_LIBMPV" >&2
  exit 1
fi
cat "$out/libmpv.txt"

probe="$(cd probes/m1-render && pwd)" || exit 1
export CARGO_TARGET_DIR="$probe/target"
if ! (cd "$probe" && cargo build --release) > "$out/build.log" 2>&1; then
  echo 'm1-render build failed' > "$out/result.txt"
  tail -n 60 "$out/build.log" >&2
  exit 1
fi

: > "$out/result.txt"
: > "$out/started"
pid=''
watchdog=''
cleanup() {
  # Only processes started by this script; also clean up on cancellation.
  if [[ -n "$watchdog" ]]; then kill "$watchdog" 2>/dev/null || true; fi
  if [[ -n "$pid" ]]; then kill -KILL "$pid" 2>/dev/null || true; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
export RUST_BACKTRACE=1
for variant in gl gl-sw; do
  "$CARGO_TARGET_DIR/release/m1-render" "$variant" > "$out/console-$variant.log" 2>&1 &
  pid=$!
  (
    sleep 25
    if kill -0 "$pid" 2>/dev/null; then
      echo 'exceeded 25s; sent SIGKILL' > "$out/timeout-$variant.txt"
      kill -KILL "$pid" 2>/dev/null || true
    fi
  ) &
  watchdog=$!
  sleep 3
  early=ok
  screencapture -x "$out/screen-$variant-3s.png" 2>> "$out/screencapture-$variant.err" || early=failed
  sleep 5
  late=ok
  screencapture -x "$out/screen-$variant-8s.png" 2>> "$out/screencapture-$variant.err" || late=failed
  wait "$pid"
  status=$?
  pid=''
  kill "$watchdog" 2>/dev/null || true
  wait "$watchdog" 2>/dev/null || true
  watchdog=''
  timed_out=no
  [[ -f "$out/timeout-$variant.txt" ]] && timed_out=yes
  echo "$variant: exit=$status timeout=$timed_out screenshot-3s=$early screenshot-8s=$late" >> "$out/result.txt"
  grep '^summary ' "$out/console-$variant.log" >> "$out/result.txt" || true
done
cat "$out/result.txt"
# Any crash reports macOS wrote meanwhile, wherever it put them (as in M0).
sleep 5
find "$HOME/Library/Logs/DiagnosticReports" /Library/Logs/DiagnosticReports \
  -newer "$out/started" -type f -print 2>/dev/null | while read -r report; do
  cp "$report" "$out/crash-$(basename "$report")"
done
ls "$out" | grep -E '^crash-' || echo 'no crash reports'
# The outcomes are evidence, including exit 3, crashes, and watchdog timeouts.
exit 0
