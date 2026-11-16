#!/usr/bin/env bash
# Type-check Vela's Rust for Windows and macOS from Linux. `cargo check` does
# not link, so neither platform's SDK is needed, with one exception handled
# below. CI's "Rust compiles" jobs on macOS and Windows remain the real
# compile; this only finds type errors before a push.
#
# Usage: scripts/check-other-platforms.sh [windows|macos|both]   (default both)
#
# Needs the rustup targets x86_64-pc-windows-msvc and aarch64-apple-darwin
# (`rustup target add --toolchain stable ...`), and clang and llvm-ar for the
# macOS check. CHECK_OTHER_TARGET_DIR sets cargo's target directory (default:
# src-tauri/target/other-platforms); CHECK_OTHER_LOG_DIR where the logs go
# (default: that target directory).
#
# macOS: objc2-exception-helper's build script compiles an Objective-C file,
# which needs Apple's SDK. scripts/fake-apple-cc.sh stands in for the
# compiler: it answers cc-rs's compiler-family probe with the real clang and
# writes an empty object for the compile. Nothing built that way runs or links.
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
which="${1:-both}"
case "$which" in windows|macos|both) ;; *) echo "usage: $0 [windows|macos|both]" >&2; exit 2 ;; esac
export CARGO_TARGET_DIR="${CHECK_OTHER_TARGET_DIR:-$repo/src-tauri/target/other-platforms}"
logs="${CHECK_OTHER_LOG_DIR:-$CARGO_TARGET_DIR}"
mkdir -p "$logs"
cd "$repo/src-tauri" || exit 2
rc=0
check() {
  local name="$1" target="$2"; shift 2
  local log="$logs/check-$name.log"
  env "$@" cargo +stable check --locked --all-targets --target "$target" > "$log" 2>&1
  local r=$?
  echo "$name ($target): exit=$r, warnings: $(grep -cE '^warning' "$log"), log: $log"
  if [ "$r" -ne 0 ]; then grep -E '^error' -A8 "$log" | head -n 80; rc=1; fi
}
if [ "$which" = windows ] || [ "$which" = both ]; then
  check windows x86_64-pc-windows-msvc
fi
if [ "$which" = macos ] || [ "$which" = both ]; then
  check macos aarch64-apple-darwin \
    CC_aarch64_apple_darwin="$here/fake-apple-cc.sh" AR_aarch64_apple_darwin=llvm-ar
fi
exit "$rc"
