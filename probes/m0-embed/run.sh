#!/bin/bash
# Run from the repository root. The workflow preserves everything in PROBE_OUT.
set -uo pipefail
out="${PROBE_OUT:?set PROBE_OUT to the artifact directory}"
mkdir -p "$out" || exit 1
out="$(cd "$out" && pwd)" || exit 1
export PROBE_OUT="$out"
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'm0-embed requires macOS' >&2
  exit 1
fi

if ! brew install mpv > "$out/brew-install.log" 2>&1; then
  echo 'brew install mpv failed:' >&2
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
export M0_LIBMPV="$prefix/lib/libmpv.2.dylib"
if [[ ! -f "$M0_LIBMPV" ]]; then
  echo "Missing $M0_LIBMPV" >&2
  exit 1
fi
cat "$out/libmpv.txt"

probe="$(cd probes/m0-embed && pwd)" || exit 1
export CARGO_TARGET_DIR="$probe/target"
if ! (cd "$probe" && cargo build --release) > "$out/build.log" 2>&1; then
  echo 'm0-embed build failed:' >&2
  tail -n 60 "$out/build.log" >&2
  exit 1
fi

: > "$out/result.txt"
pid=''
watchdog=''
cleanup() {
  # Only processes started by this script; also clean up if the job is cancelled.
  if [[ -n "$watchdog" ]]; then kill "$watchdog" 2>/dev/null || true; fi
  if [[ -n "$pid" ]]; then kill -KILL "$pid" 2>/dev/null || true; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
for variant in wid wid-gpu; do
  "$CARGO_TARGET_DIR/release/m0-embed" "$variant" > "$out/console-$variant.log" 2>&1 &
  pid=$!
  # Enforce twenty seconds from launch, including the eight-second capture delay.
  (
    sleep 20
    if kill -0 "$pid" 2>/dev/null; then
      echo 'exceeded 20s; sent SIGKILL' > "$out/timeout-$variant.txt"
      kill -KILL "$pid" 2>/dev/null || true
    fi
  ) &
  watchdog=$!
  # An early picture too: the first run crashed about 3 s in, just after
  # mpv showed its first frame.
  sleep 2.4
  screencapture -x "$out/screen-$variant-early.png" 2>> "$out/screencapture-$variant.err" || true
  sleep 5.6
  if screencapture -x "$out/screen-$variant.png" 2> "$out/screencapture-$variant.err"; then
    capture=ok
  else
    capture=failed
    cat "$out/screencapture-$variant.err" >&2
  fi
  wait "$pid"
  status=$?
  pid=''
  kill "$watchdog" 2>/dev/null || true
  wait "$watchdog" 2>/dev/null || true
  watchdog=''
  echo "$variant: exit=$status screencapture=$capture" >> "$out/result.txt"
  # macOS writes a crash report for a process that died of a signal.
  sleep 3
  for report in "$HOME"/Library/Logs/DiagnosticReports/m0-embed*; do
    [[ -f "$report" ]] && mv "$report" "$out/crash-$variant-$(basename "$report")"
  done
done
cat "$out/result.txt"
ls "$out" | grep -E '^crash-' || echo 'no crash reports'
