#!/usr/bin/env bash
# Build the libmpv set that Vela ships for its embedded player (Linux).
#
# Pinned, checksummed sources: packaging/libmpv/versions.txt.
# LGPL configuration only: mpv with -Dgpl=false, FFmpeg without --enable-gpl,
# --enable-nonfree or --enable-version3. Every shipped library is a separate
# shared object with a Vela-private soname and an $ORIGIN run path, so the set
# finds itself and can never be replaced by a library of the same name that
# Vela's own process already has loaded (WebKitGTK loads the system's FreeType,
# HarfBuzz, FriBidi and dav1d).
#
# Usage:
#   scripts/build-libmpv.sh --container [step...]   build inside the baseline image
#   scripts/build-libmpv.sh [step...]               build with this machine's tools
#
# The supported way is --container (podman or docker): it builds
# packaging/libmpv/Containerfile, the oldest distribution Vela's Linux bundles
# target, and runs this script inside it. Without --container the result depends
# on this machine's glibc and is only good for local experiments.
#
# Output: $LIBMPV_ROOT/out/libmpv/ (libraries, licenses/, MANIFEST). LIBMPV_ROOT
# defaults to .libmpv in the repo root and also holds the download cache (src/)
# and the build tree (work/). Steps default to everything; naming steps re-runs
# just those, in the order given (see `steps=` at the end).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
recipe="$repo/packaging/libmpv"
LIBMPV_ROOT="${LIBMPV_ROOT:-$repo/.libmpv}"

log() { printf '\n=== %s ===\n' "$*"; }
die() { printf 'build-libmpv: %s\n' "$*" >&2; exit 1; }

if [ "${1:-}" = "--container" ]; then
  shift
  engine="$(command -v podman || command -v docker || true)"
  [ -n "$engine" ] || die "--container needs podman or docker"
  mkdir -p "$LIBMPV_ROOT"
  "$engine" build -t vela-libmpv-build -f "$recipe/Containerfile" "$recipe"
  # label=disable: SELinux hosts would otherwise deny the bind mounts.
  exec "$engine" run --rm --security-opt label=disable \
    -v "$repo":/repo:ro -v "$LIBMPV_ROOT":/libmpv -e LIBMPV_ROOT=/libmpv \
    vela-libmpv-build /repo/scripts/build-libmpv.sh "$@"
fi

VERSIONS="$recipe/versions.txt"
PATCHES="$recipe/patches"
SRC_DIR="$LIBMPV_ROOT/src"
WORK="$LIBMPV_ROOT/work"
OUT="$LIBMPV_ROOT/out"
JOBS="${JOBS:-$(nproc)}"
# Highest glibc symbol version the set may need: the build baseline's.
GLIBC_MAX="${GLIBC_MAX:-2.35}"

PREFIX="$WORK/prefix"   # shipped libraries are installed here first
mkdir -p "$SRC_DIR" "$WORK/src" "$PREFIX/lib/pkgconfig" "$OUT"

# field NAME N: the Nth whitespace-separated field of NAME's line.
field() { awk -v n="$1" -v i="$2" '$1 == n { print $i }' "$VERSIONS"; }

# fetch NAME: make sure the pinned tarball is present and matches; print its path.
fetch() {
  local name="$1" ver sha url file
  ver="$(field "$name" 2)"; sha="$(field "$name" 4)"; url="$(field "$name" 5)"
  [ -n "$url" ] || die "no entry for '$name' in $VERSIONS"
  file="$SRC_DIR/$name-$ver.tar.${url##*.tar.}"
  if [ ! -f "$file" ]; then
    curl -fsSL --retry 3 -o "$file.part" "$url" || die "download failed: $url"
    mv "$file.part" "$file"
  fi
  echo "$sha  $file" | sha256sum -c --quiet - >&2 || die "checksum mismatch: $file"
  printf '%s' "$file"
}

# unpack NAME: fresh source tree; print its path.
unpack() {
  local name="$1" file dir
  file="$(fetch "$name")"
  dir="$WORK/src/$name"
  rm -rf "$dir"; mkdir -p "$dir"
  tar -xf "$file" -C "$dir" --strip-components=1 --no-same-owner
  printf '%s' "$dir"
}

export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
HARDEN_C="-fstack-protector-strong -D_FORTIFY_SOURCE=2"
HARDEN_LD="-Wl,-z,relro -Wl,-z,now"
export CFLAGS="$HARDEN_C" CXXFLAGS="$HARDEN_C" LDFLAGS="$HARDEN_LD"

# meson_build SRC [options...]. Every optional feature is off unless named, so
# the result does not depend on what the build machine has installed.
meson_build() {
  local src="$1"; shift
  meson setup "$src/_build" "$src" --prefix="$PREFIX" --libdir=lib \
    --buildtype=release --wrap-mode=nodownload \
    -Ddefault_library=shared -Dauto_features=disabled "$@"
  ninja -C "$src/_build" -j "$JOBS"
  ninja -C "$src/_build" install
}

build_freetype() {
  log "freetype $(field freetype 2)"
  local src; src="$(unpack freetype)"
  meson_build "$src" -Dzlib=system -Dmmap=enabled
}

build_fribidi() {
  log "fribidi $(field fribidi 2)"
  local src; src="$(unpack fribidi)"
  meson_build "$src" -Ddocs=false -Dbin=false -Dtests=false
}

build_harfbuzz() {
  log "harfbuzz $(field harfbuzz 2)"
  local src; src="$(unpack harfbuzz)"
  meson_build "$src" -Dfreetype=enabled
}

build_libass() {
  log "libass $(field libass 2)"
  local src; src="$(unpack libass)"
  meson_build "$src" -Dfontconfig=enabled -Dasm=enabled
}

build_dav1d() {
  log "dav1d $(field dav1d 2)"
  local src; src="$(unpack dav1d)"
  meson_build "$src" -Denable_tools=false -Denable_tests=false -Denable_examples=false
}

build_lua() {
  log "lua $(field lua 2)"
  local src ver; src="$(unpack lua)"; ver="$(field lua 2)"
  (
    cd "$src/src"
    # The library only: everything except the two command-line programs.
    objs=()
    for c in *.c; do
      case "$c" in lua.c|luac.c) continue ;; esac
      gcc -O2 -fPIC $HARDEN_C -DLUA_COMPAT_ALL -DLUA_USE_POSIX -DLUA_USE_DLOPEN \
        -c "$c" -o "${c%.c}.o"
      objs+=("${c%.c}.o")
    done
    gcc -shared $HARDEN_LD -Wl,-soname,liblua5.2.so.0 -o liblua5.2.so.0 "${objs[@]}" -lm -ldl
    install -Dm755 liblua5.2.so.0 "$PREFIX/lib/liblua5.2.so.0"
    ln -sf liblua5.2.so.0 "$PREFIX/lib/liblua5.2.so"
    install -d "$PREFIX/include/lua5.2"
    install -m644 lua.h luaconf.h lualib.h lauxlib.h lua.hpp "$PREFIX/include/lua5.2/"
  )
  cat > "$PREFIX/lib/pkgconfig/lua5.2.pc" <<EOF
prefix=$PREFIX
libdir=\${prefix}/lib
includedir=\${prefix}/include/lua5.2
Name: Lua
Description: Lua language engine
Version: $ver
Libs: -L\${libdir} -llua5.2
Libs.private: -lm -ldl
Cflags: -I\${includedir}
EOF
}

build_uchardet() {
  log "uchardet $(field uchardet 2)"
  local src; src="$(unpack uchardet)"
  cmake -S "$src" -B "$src/_build" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$PREFIX" -DCMAKE_INSTALL_LIBDIR=lib \
    -DBUILD_BINARY=OFF -DBUILD_STATIC=OFF -DBUILD_SHARED_LIBS=ON \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5
  ninja -C "$src/_build" -j "$JOBS"
  ninja -C "$src/_build" install
}

build_ffmpeg() {
  log "ffmpeg $(field ffmpeg 2)"
  local src; src="$(unpack ffmpeg)"
  (
    cd "$src"
    # LGPL only. Do not add a licence-widening flag here; the policy test
    # (tests/dependency-policy.test.mjs) fails if one appears.
    ./configure --prefix="$PREFIX" --libdir="$PREFIX/lib" --shlibdir="$PREFIX/lib" \
      --enable-shared --disable-static --enable-pic \
      --disable-programs --disable-doc --disable-avdevice \
      --disable-autodetect \
      --enable-zlib --enable-iconv --enable-gnutls \
      --enable-libdav1d --enable-vaapi --enable-libdrm \
      --extra-cflags="$HARDEN_C" --extra-ldflags="$HARDEN_LD"
    make -j"$JOBS"
    make install
    # Fail the build if FFmpeg itself says the result is not LGPL.
    grep -qx '#define FFMPEG_LICENSE "LGPL version 2.1 or later"' config.h \
      || die "FFmpeg did not configure as LGPL 2.1 or later: $(grep FFMPEG_LICENSE config.h)"
  )
}

build_libplacebo() {
  log "libplacebo $(field libplacebo 2)"
  local src file; src="$(unpack libplacebo)"
  # libplacebo compiles Vulkan stubs even with Vulkan off and wants the
  # headers where its git submodule would be. Headers only; nothing links.
  file="$(fetch vulkan-headers)"
  mkdir -p "$src/3rdparty/Vulkan-Headers"
  tar -xf "$file" -C "$src/3rdparty/Vulkan-Headers" --strip-components=1 --no-same-owner
  meson_build "$src" -Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false
}

build_mpv() {
  log "mpv $(field mpv 2)"
  local src; src="$(unpack mpv)"
  # Build-file change only (see the patch header): VA-API without mpv's
  # DRM/KMS video output, so the baseline's own libdrm is enough.
  patch -d "$src" -p1 < "$PATCHES/mpv-vaapi-without-drm-output.patch"
  meson_build "$src" \
    -Dgpl=false -Dlibmpv=true -Dcplayer=false -Dbuild-date=false \
    -Dlua=lua5.2 -Diconv=enabled -Dzlib=enabled -Duchardet=enabled -Dvector=enabled \
    -Dalsa=enabled -Dpulse=enabled \
    -Ddrm=enabled -Degl=enabled -Dgl=enabled -Dplain-gl=enabled \
    -Dvaapi=enabled -Dvaapi-drm=enabled
}

# System libraries the set is allowed to need. Anything else is a defect:
# either ship it or stop depending on it.
ALLOWED_SYSTEM="
  libc.so.6 libm.so.6 libdl.so.2 libpthread.so.0 librt.so.1
  ld-linux-x86-64.so.2 ld-linux-aarch64.so.1
  libgcc_s.so.1 libstdc++.so.6
  libz.so.1 libdrm.so.2 libva.so.2 libva-drm.so.2 libEGL.so.1
  libpulse.so.0 libasound.so.2 libfontconfig.so.1 libgnutls.so.30
"
allowed_system() { case " $(echo $ALLOWED_SYSTEM) " in *" $1 "*) return 0 ;; esac; return 1; }

needed() { patchelf --print-needed "$1"; }

collect() {
  log "collect"
  local dest="$OUT/libmpv" stage="$WORK/stage"
  rm -rf "$dest" "$stage"; mkdir -p "$dest/licenses" "$stage"

  # Walk libmpv's dependency closure inside $PREFIX/lib.
  local queue=(libmpv.so.2) seen=" " system=" " lib dep
  while [ "${#queue[@]}" -gt 0 ]; do
    lib="${queue[0]}"; queue=("${queue[@]:1}")
    case "$seen" in *" $lib "*) continue ;; esac
    seen="$seen$lib "
    cp -L "$PREFIX/lib/$lib" "$stage/$lib"
    for dep in $(needed "$stage/$lib"); do
      if [ -e "$PREFIX/lib/$dep" ]; then
        queue+=("$dep")
      else
        case "$system" in *" $dep "*) ;; *) system="$system$dep " ;; esac
      fi
    done
  done

  for dep in $system; do
    allowed_system "$dep" \
      || die "the set needs system library '$dep', which is not on the allowed list"
  done

  # Strip first, then rename: every library but libmpv.so.2 gets a private
  # soname, and every reference to it is rewritten.
  strip --strip-unneeded "$stage"/*
  local old new
  for lib in $seen; do
    [ "$lib" = libmpv.so.2 ] && continue
    old="$lib"; new="libvela-${lib#lib}"
    patchelf --set-soname "$new" "$stage/$old"
    mv "$stage/$old" "$stage/$new"
    for dep in "$stage"/*; do
      patchelf --replace-needed "$old" "$new" "$dep"
    done
  done
  for lib in "$stage"/*; do
    patchelf --set-rpath '$ORIGIN' "$lib"
    install -m644 "$lib" "$dest/$(basename "$lib")"
  done

  # Nothing may still point at an unrenamed name: every dependency must be
  # in the set or on the allowed list.
  for lib in "$dest"/*.so*; do
    for dep in $(needed "$lib"); do
      [ -e "$dest/$dep" ] && continue
      allowed_system "$dep" || die "$(basename "$lib") still needs '$dep' after renaming"
    done
  done

  # glibc baseline.
  local max
  max="$(for lib in "$dest"/*.so*; do objdump -T "$lib"; done \
    | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -1)"
  if [ "$(printf '%s\n%s\n' "$max" "$GLIBC_MAX" | sort -V | tail -1)" != "$GLIBC_MAX" ]; then
    die "the set needs glibc $max, above the baseline $GLIBC_MAX"
  fi

  # Licence texts: one directory per shipped component.
  lic() { local name="$1"; shift; mkdir -p "$dest/licenses/$name"
          local f; for f in "$@"; do install -m644 "$WORK/src/$name/$f" "$dest/licenses/$name/"; done; }
  lic mpv LICENSE.LGPL Copyright
  lic ffmpeg COPYING.LGPLv2.1 LICENSE.md
  lic libplacebo LICENSE
  lic libass COPYING
  lic freetype LICENSE.TXT docs/FTL.TXT
  lic harfbuzz COPYING
  lic fribidi COPYING
  lic dav1d COPYING
  lic uchardet COPYING
  mkdir -p "$dest/licenses/lua"
  # Lua carries its licence at the end of lua.h.
  sed -n '/^\* Copyright (C)/,/^\*\*\*\*\*/p' "$WORK/src/lua/src/lua.h" > "$dest/licenses/lua/LICENSE"
  [ -s "$dest/licenses/lua/LICENSE" ] || die "could not extract Lua's licence text"
  # The one change Vela makes to a shipped component's build travels with it.
  install -m644 "$PATCHES"/*.patch "$dest/licenses/mpv/"
  local name
  for name in $(awk '$1 !~ /^#/ && $3 == "ship" { print $1 }' "$VERSIONS"); do
    [ -n "$(ls -A "$dest/licenses/$name" 2>/dev/null)" ] \
      || die "no licence text collected for shipped component '$name'"
  done

  # Manifest: what was built, from what, and what the machine must provide.
  {
    echo "# libmpv set for Vela. Generated by scripts/build-libmpv.sh; do not edit."
    echo "arch $(uname -m)"
    awk '$1 !~ /^#/ && $3 == "ship" { print "component", $1, $2 }' "$VERSIONS"
    echo "glibc $max"
    for dep in $system; do echo "system $dep"; done | sort
    ( cd "$dest" && for lib in *.so*; do
        printf 'file %s %s %s\n' "$(sha256sum "$lib" | cut -d' ' -f1)" "$(stat -c %s "$lib")" "$lib"
      done )
  } > "$dest/MANIFEST"

  log "result"
  cat "$dest/MANIFEST"
  du -sh "$dest"
}

steps="${*:-freetype fribidi harfbuzz libass dav1d lua uchardet ffmpeg libplacebo mpv collect}"
for s in $steps; do
  case "$s" in
    collect) collect ;;
    freetype|fribidi|harfbuzz|libass|dav1d|lua|uchardet|ffmpeg|libplacebo|mpv) "build_$s" ;;
    *) die "unknown step '$s'" ;;
  esac
done
