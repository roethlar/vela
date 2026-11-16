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
: > "$out/started"
pid=''
watchdog=''
cleanup() {
  # Only processes started by this script; also clean up if the job is cancelled.
  if [[ -n "$watchdog" ]]; then kill "$watchdog" 2>/dev/null || true; fi
  if [[ -n "$pid" ]]; then kill -KILL "$pid" 2>/dev/null || true; fi
  pkill -KILL -f "$CARGO_TARGET_DIR/release/m0-embed" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
# The program runs under lldb, which prints every thread's backtrace if it
# crashes (the runs so far died of SIGSEGV just after mpv's first frame, and
# macOS kept no crash report). A runner may need debugging enabled first.
sudo -n DevToolsSecurity -enable > "$out/devtools.log" 2>&1 || true
# variant  extra mpv options (name=value;...)
variants=(
  'wid|'
  'wid-nodr|vd-lavc-dr=no'
  'wid-gpu|'
)
for entry in "${variants[@]}"; do
  variant="${entry%%|*}"
  export M0_EXTRA="${entry#*|}"
  lldb --batch -o run -k 'thread backtrace all' -k 'quit 1' \
    -- "$CARGO_TARGET_DIR/release/m0-embed" "$variant" > "$out/console-$variant.log" 2>&1 &
  pid=$!
  # Enforce twenty-five seconds from launch, including the captures.
  (
    sleep 25
    if kill -0 "$pid" 2>/dev/null; then
      echo 'exceeded 25s; sent SIGKILL' > "$out/timeout-$variant.txt"
      kill -KILL "$pid" 2>/dev/null || true
      pkill -KILL -f "$CARGO_TARGET_DIR/release/m0-embed" 2>/dev/null || true
    fi
  ) &
  watchdog=$!
  # Pictures early as well: without lldb the program crashed about 2-3 s in.
  sleep 2.5
  screencapture -x "$out/screen-$variant-a.png" 2>> "$out/screencapture-$variant.err" || true
  sleep 1.5
  screencapture -x "$out/screen-$variant-b.png" 2>> "$out/screencapture-$variant.err" || true
  sleep 4
  if screencapture -x "$out/screen-$variant.png" 2>> "$out/screencapture-$variant.err"; then
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
  crashed=no
  grep -q 'stop reason = ' "$out/console-$variant.log" && crashed=yes
  echo "$variant: lldb-exit=$status crashed=$crashed screencapture=$capture extra='$M0_EXTRA'" >> "$out/result.txt"
done
cat "$out/result.txt"
# Any crash reports macOS wrote meanwhile, wherever it put them.
sleep 5
find "$HOME/Library/Logs/DiagnosticReports" /Library/Logs/DiagnosticReports \
  -newer "$out/started" -type f -print 2>/dev/null | while read -r report; do
  cp "$report" "$out/crash-$(basename "$report")"
done
ls "$out" | grep -E '^crash-' || echo 'no crash reports'
grep -h -A12 'stop reason = ' "$out"/console-*.log | head -n 60 || true
