#!/bin/bash
# Probe M2: does Vela's own embedded player work on a Mac? A debug Vela,
# built from this checkout with a probe-only hook (hook.py), plays a mock
# Jellyfin library's clip in its own window on a hosted macOS runner,
# driven by a script in its page (drive.js), with screenshots every two
# seconds. The libmpv set comes from CI's newest successful run on main.
# Hosted Macs draw OpenGL with Apple's software renderer and have no EDR
# screen, so this sees standard range on the software surface only.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
out="${PROBE_OUT:?}"
mkdir -p "$out"
cd "$repo" || exit 2
fail() { echo "M2: $*" | tee -a "$out/result.txt" >&2; exit 1; }

{ sw_vers; uname -m; system_profiler SPDisplaysDataType 2>/dev/null; } > "$out/system.txt"

# The set.
set_dir="$repo/.libmpv/out/libmpv-macos-universal"
run_id="$(gh run list --repo "$GITHUB_REPOSITORY" --workflow ci.yml --branch main --status success \
  --limit 1 --json databaseId --jq '.[0].databaseId')" || fail "no successful CI run on main"
mkdir -p "$set_dir"
gh run download "$run_id" --repo "$GITHUB_REPOSITORY" -n libmpv-macos-universal -D "$set_dir" \
  || fail "could not download the set from CI run $run_id"
echo "the set from CI run $run_id" >> "$out/system.txt"
ls -l "$set_dir" >> "$out/system.txt"

# Tools: the repository's npm, ffmpeg for the clip.
npm install --global npm@12.0.2 > "$out/tools.log" 2>&1 || fail "npm 12"
node scripts/check-js-toolchain.mjs >> "$out/tools.log" 2>&1 || fail "the JS toolchain check"
brew install ffmpeg >> "$out/tools.log" 2>&1 || fail "ffmpeg"

# A debug Vela with the hook, its page embedded (as the end-to-end suite builds it).
python3 "$here/hook.py" "$repo" >> "$out/tools.log" 2>&1 || fail "the hook"
npm ci --no-audit --no-fund > "$out/npm-ci.log" 2>&1 || fail "npm ci"
npx tauri build --debug --no-bundle > "$out/build.log" 2>&1 || { tail -60 "$out/build.log"; fail "the debug build"; }
bin="$repo/src-tauri/target/debug/vela"
test -x "$bin" || fail "no debug binary at $bin"

# The mock server and a throwaway home.
root="${RUNNER_TEMP:-/tmp}/m2-root"
home="${RUNNER_TEMP:-/tmp}/m2-home"
node "$here/serve.mjs" "$root" "$home/Library/Application Support/com.vela.vela" > "$out/mock.log" 2>&1 &
mock=$!
config="$home/Library/Application Support/com.vela.vela/config.json"
for _ in $(seq 1 120); do [ -f "$config" ] && break; sleep 1; done
[ -f "$config" ] || { cat "$out/mock.log"; fail "the mock server did not write the configuration"; }

# Vela, with screenshots every two seconds.
( for i in $(seq -w 1 45); do screencapture -x "$out/shot-$i.png" 2>/dev/null; sleep 2; done ) &
shots=$!
HOME="$home" VELA_LIBMPV_DIR="$set_dir" VELA_PROBE_SCRIPT="$here/drive.js" RUST_BACKTRACE=1 \
  "$bin" > "$out/vela.log" 2>&1 &
vela=$!
for _ in $(seq 1 120); do
  grep -q 'probe: "\(DONE\|ERROR\)' "$out/vela.log" && break
  kill -0 "$vela" 2>/dev/null || break
  sleep 1
done
sleep 2
screencapture -x "$out/shot-final.png" 2>/dev/null
alive=no; kill -0 "$vela" 2>/dev/null && alive=yes
kill "$vela" 2>/dev/null; wait "$vela" 2>/dev/null
kill "$shots" "$mock" 2>/dev/null
cp "$home/Library/Application Support/com.vela.vela/"*.json "$out/" 2>/dev/null
rm -f "$out/connections.json"   # the mock's token only, but it is not evidence
cp "$root/mpv.log" "$out/mpv.log" 2>/dev/null
find "$HOME/Library/Logs/DiagnosticReports" -name 'vela*' -newer "$out/system.txt" -exec cp {} "$out/" \; 2>/dev/null

grep 'probe:' "$out/vela.log" > "$out/notes.txt"
echo "Vela still running at the end: $alive" >> "$out/result.txt"
grep -q 'probe: "DONE"' "$out/vela.log" || fail "the drive script did not finish; see notes.txt and vela.log"
grep -q 'second play: PLAYER CLOSED' "$out/vela.log" || fail "the second play did not close"
echo "M2: two plays opened and closed in Vela's window; judge the screenshots" | tee -a "$out/result.txt"
