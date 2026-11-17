#!/usr/bin/env bash
# Make Tauri's AppImage start Vela on Wayland, with the host's own Wayland
# client libraries (.agents/plans/appimage-wayland.md).
#
# Tauri builds the AppImage with its fork of linuxdeploy's GTK plugin, whose
# AppRun hook exports GDK_BACKEND=x11 unconditionally (tauri-apps/tauri#8541):
# every start went to X11, which Vela does not support in any way. And
# linuxdeploy bundles the build machine's libwayland-*: WebKit's web process,
# given those instead of the host's, cannot open the host's EGL display
# ("Could not create default EGL display: EGL_BAD_PARAMETER") and aborts, on
# Mesa and on NVIDIA alike. Wayland's client libraries must match the host's
# graphics stack, as libGL and libEGL must, so they come from the host.
#
# For each AppImage named: unpack it, delete the hook's GDK_BACKEND line,
# remove usr/lib/libwayland-*, and pack it again in place with a pinned
# appimagetool and AppImage runtime (both checked by SHA-256 before use).
# packaging/libmpv/check-package.sh then holds the result.
#
# Usage: packaging/appimage/fix-appimage.sh APPIMAGE...
set -euo pipefail

APPIMAGETOOL_URL='https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage'
APPIMAGETOOL_SHA256='ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0'
RUNTIME_URL='https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64'
RUNTIME_SHA256='2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d'

die() { echo "fix-appimage: $*" >&2; exit 1; }
[ "$#" -gt 0 ] || die "name the AppImage(s) to fix"

tools="${XDG_CACHE_HOME:-$HOME/.cache}/vela-appimage-tools"
mkdir -p "$tools"

# fetch URL SHA256 FILE: FILE holds exactly what was pinned, downloaded once.
fetch() {
  local url="$1" sum="$2" file="$3"
  if [ ! -f "$file" ] || ! echo "$sum  $file" | sha256sum --check --status; then
    rm -f "$file"
    curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
      --retry 5 --output "$file.part" "$url"
    echo "$sum  $file.part" | sha256sum --check --status \
      || { rm -f "$file.part"; die "$url does not have the pinned SHA-256"; }
    mv "$file.part" "$file"
  fi
  chmod +x "$file"
}
fetch "$APPIMAGETOOL_URL" "$APPIMAGETOOL_SHA256" "$tools/appimagetool-1.9.1-x86_64.AppImage"
fetch "$RUNTIME_URL" "$RUNTIME_SHA256" "$tools/runtime-20251108-x86_64"

# The tools are AppImages too; no FUSE is needed to run them this way.
export APPIMAGE_EXTRACT_AND_RUN=1

for appimage in "$@"; do
  [ -f "$appimage" ] || die "no such AppImage: $appimage"
  appimage="$(cd -- "$(dirname -- "$appimage")" && pwd -P)/$(basename -- "$appimage")"
  work="$(mktemp -d)"
  ( cd "$work" && "$appimage" --appimage-extract >/dev/null )
  root="$work/squashfs-root"
  [ -x "$root/AppRun" ] || die "$appimage did not unpack to an AppDir"

  edited=0
  for hook in "$root"/apprun-hooks/*; do
    [ -f "$hook" ] || continue
    if grep -qE '^[[:space:]]*(export[[:space:]]+)?GDK_BACKEND=' "$hook"; then
      sed -i -E '/^[[:space:]]*(export[[:space:]]+)?GDK_BACKEND=/d' "$hook"
      edited=$((edited + 1))
    fi
  done
  removed="$(find "$root/usr/lib" -maxdepth 1 -name 'libwayland-*' -print -delete | wc -l)"

  ARCH=x86_64 "$tools/appimagetool-1.9.1-x86_64.AppImage" --no-appstream \
    --runtime-file "$tools/runtime-20251108-x86_64" "$root" "$appimage.fixed" >/dev/null
  chmod --reference="$appimage" "$appimage.fixed"
  mv "$appimage.fixed" "$appimage"
  chmod -R u+rwX "$work" && rm -rf "$work"
  echo "fix-appimage: $(basename -- "$appimage"): $edited hook(s) no longer set GDK_BACKEND, $removed bundled libwayland-* removed"
done
