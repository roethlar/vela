#!/usr/bin/env bash
# A stand-in C/Objective-C compiler for type-checking Vela for macOS on Linux
# (scripts/check-other-platforms.sh). `cargo check` never links, but a build
# script (objc2-exception-helper) compiles a .m file, which needs Apple's SDK.
# cc-rs first asks the compiler its family (-E on a small file): the real clang
# answers that. A compile writes an empty arm64 Mach-O object wherever -o (or
# -Fo) says. Nothing built this way ever runs.
for arg in "$@"; do
  [ "$arg" = "-E" ] && exec clang --target=arm64-apple-macos11 "$@"
done
out=''
prev=''
for arg in "$@"; do
  [ "$prev" = "-o" ] && out="$arg"
  case "$arg" in
    -o) ;;
    -o*) out="${arg#-o}" ;;
    -Fo*) out="${arg#-Fo}" ;;
  esac
  prev="$arg"
done
[ -n "$out" ] || exit 0
exec clang --target=arm64-apple-macos11 -c -x c /dev/null -o "$out"
