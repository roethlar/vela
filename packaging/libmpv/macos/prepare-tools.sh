#!/usr/bin/env bash
# Source this before the build, in CI or the probe. Host tools may use
# Homebrew; target headers/libraries may not. Never brew-install mpv/FFmpeg.
set -euo pipefail
macos_recipe="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
macos_repo="$(cd "$macos_recipe/../../.." && pwd)"
export LIBMPV_ROOT="${LIBMPV_ROOT:-$macos_repo/.libmpv}"
mkdir -p "$LIBMPV_ROOT"
LIBMPV_ROOT="$(cd "$LIBMPV_ROOT" && pwd)"
export VELA_LIBMPV_TOOLS="$LIBMPV_ROOT/tools-macos"
brew install cmake ninja nasm pkgconf
python3 -m venv "$VELA_LIBMPV_TOOLS/venv"
"$VELA_LIBMPV_TOOLS/venv/bin/python3" -m pip install --disable-pip-version-check -r "$macos_recipe/tools.txt"
mkdir -p "$VELA_LIBMPV_TOOLS/bin"
for macos_tool in cmake ninja nasm pkg-config; do
  ln -sf "$(command -v "$macos_tool")" "$VELA_LIBMPV_TOOLS/bin/$macos_tool"
done
for macos_tool in meson python3; do
  # Launch at the venv path itself. A further symlink to Python can make its
  # pyvenv.cfg lookup miss the venv (and libplacebo then cannot import Jinja).
  rm -f "$VELA_LIBMPV_TOOLS/bin/$macos_tool"
  cat > "$VELA_LIBMPV_TOOLS/bin/$macos_tool" <<EOF
#!/bin/sh
exec '$VELA_LIBMPV_TOOLS/venv/bin/$macos_tool' "\$@"
EOF
  chmod +x "$VELA_LIBMPV_TOOLS/bin/$macos_tool"
done
(
  export PATH="$VELA_LIBMPV_TOOLS/bin:/usr/bin:/bin:/usr/sbin:/sbin"
  printf 'runner %s %s\n' "${ImageOS:-local}" "${ImageVersion:-local}"
  sw_vers
  xcodebuild -version
  xcrun clang --version
  xcrun --sdk macosx --show-sdk-version
  xcrun --sdk macosx --show-sdk-path
  python3 --version
  python3 -m pip freeze
  meson --version
  cmake --version
  ninja --version
  nasm -v
  pkg-config --version
) > "$LIBMPV_ROOT/toolchain-macos.txt"
cat "$LIBMPV_ROOT/toolchain-macos.txt"
