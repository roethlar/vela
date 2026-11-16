#!/usr/bin/env bash
# macOS 11+, arm64 + x86_64 on one Apple Silicon host. Bash 3.2 compatible.
# First: source packaging/libmpv/macos/prepare-tools.sh
# Then: bash scripts/build-libmpv-macos.sh [step...]
# Output: $LIBMPV_ROOT/out/libmpv-macos-universal/{*.dylib,licenses/,MANIFEST}
# No Swift or mpv-owned window: Vela supplies the OpenGL context via libmpv.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
recipe="$repo/packaging/libmpv"
LIBMPV_ROOT="${LIBMPV_ROOT:-$repo/.libmpv}"
VERSIONS="$recipe/versions.txt"
die() { printf 'build-libmpv-macos: %s\n' "$*" >&2; exit 1; }
field() { awk -v n="$1" -v i="$2" '$1 == n { print $i }' "$VERSIONS"; }
sources() { awk -v n="$1" '$1 == n { for (i = 5; i <= NF; i++) print $i }' "$VERSIONS"; }

# Validate before removing anything. Named steps run for BOTH architectures;
# collect also joins them. A complete run starts with clean build trees.
steps="${*:-freetype fribidi harfbuzz libass dav1d lua uchardet ffmpeg libplacebo mpv collect}"
for s in $steps; do
  case "$s" in
    freetype|fribidi|harfbuzz|libass|dav1d|lua|uchardet|ffmpeg|libplacebo|mpv|collect) ;;
    *) die "unknown step '$s'" ;;
  esac
done
[ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] || die 'requires an Apple Silicon macOS host'
mkdir -p "$LIBMPV_ROOT"
LIBMPV_ROOT="$(cd "$LIBMPV_ROOT" && pwd)"
# Meson machine files and FFmpeg's flag strings cannot quote arbitrary paths.
case "$LIBMPV_ROOT" in *[!a-zA-Z0-9_./-]*) die 'LIBMPV_ROOT must have no spaces or shell metacharacters' ;; esac
SRC_DIR="$LIBMPV_ROOT/src"
WORK_ROOT="$LIBMPV_ROOT/work-macos"
OUT="$LIBMPV_ROOT/out/libmpv-macos-universal"
VELA_LIBMPV_TOOLS="${VELA_LIBMPV_TOOLS:-$LIBMPV_ROOT/tools-macos}"
export PATH="$VELA_LIBMPV_TOOLS/bin:/usr/bin:/bin:/usr/sbin:/sbin"
for tool in meson ninja cmake nasm pkg-config python3; do
  [ -x "$VELA_LIBMPV_TOOLS/bin/$tool" ] || die 'source packaging/libmpv/macos/prepare-tools.sh first'
done
JOBS="${JOBS:-$(sysctl -n hw.logicalcpu)}"
DOWNLOAD_RETRIES="${LIBMPV_DOWNLOAD_RETRIES:-4}"
export MACOSX_DEPLOYMENT_TARGET=11.0
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
export CC="$(xcrun --sdk macosx -f clang)" CXX="$(xcrun --sdk macosx -f clang++)"
export OBJC="$CC" OBJCXX="$CXX" AR="$(xcrun -f ar)" RANLIB="$(xcrun -f ranlib)"
# Target discovery never sees Homebrew, the caller's flags, or package registries.
unset PKG_CONFIG_PATH PKG_CONFIG_SYSROOT_DIR PKG_CONFIG_DIR PKG_CONFIG_TOP_BUILD_DIR
unset CMAKE_PREFIX_PATH CMAKE_TOOLCHAIN_FILE CMAKE_INCLUDE_PATH CMAKE_LIBRARY_PATH CMAKE_FRAMEWORK_PATH
unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH OBJC_INCLUDE_PATH LIBRARY_PATH CPPFLAGS
unset CFLAGS CXXFLAGS OBJCFLAGS OBJCXXFLAGS LDFLAGS ARCHFLAGS CONFIG_SITE
unset PYTHONPATH PYTHONHOME
for variable in ${!DYLD_@}; do unset "$variable"; done
export CC_FOR_BUILD="$CC" CXX_FOR_BUILD="$CXX" CFLAGS_FOR_BUILD= CXXFLAGS_FOR_BUILD= LDFLAGS_FOR_BUILD=
export LC_ALL=C
[ "$#" -ne 0 ] || rm -rf "$WORK_ROOT" "$OUT"
mkdir -p "$SRC_DIR" "$WORK_ROOT"

fetch() {
  local name="$1" ver sha urls url file got=""
  ver="$(field "$name" 2)"; sha="$(field "$name" 4)"; urls="$(sources "$name")"
  [ -n "$urls" ] || die "no entry for '$name' in $VERSIONS"
  [[ "$sha" =~ ^[0-9a-f]{64}$ ]] || die "invalid checksum for '$name'"
  url="${urls%%$'\n'*}"
  file="$SRC_DIR/$name-$ver.tar.${url##*.tar.}"
  if [ ! -f "$file" ]; then
    for url in $urls; do
      if curl -fsSL --retry "$DOWNLOAD_RETRIES" --retry-all-errors --retry-delay 2 --connect-timeout 20 \
           -o "$file.part" "$url" \
         && echo "$sha  $file.part" | shasum -a 256 -c - >/dev/null 2>&1; then
        mv "$file.part" "$file" || die "cannot keep $file"
        got=1; break
      fi
      rm -f "$file.part"
      echo "build-libmpv-macos: no good download from $url" >&2
    done
    [ -n "$got" ] || die "could not download '$name' from any of its sources"
  fi
  echo "$sha  $file" | shasum -a 256 -c - >&2 || die "checksum mismatch: $file"
  printf '%s' "$file"
}

unpack() {
  local name="$1" file dir
  # Bash 3.2 clears errexit inside command substitutions. Explicitly carry
  # failure through BOTH levels; a bad fetch must never reach extraction.
  file="$(fetch "$name")" || exit 1
  dir="$WORK/src/$name"
  rm -rf "$dir" || exit 1
  mkdir -p "$dir" || exit 1
  tar -xf "$file" -C "$dir" --strip-components=1 || die "cannot unpack $name"
  printf '%s' "$dir"
}

meson_build() {
  local src="$1"; shift
  # Flags live in the machine file only, including Objective-C and assembly.
  env -u CFLAGS -u CXXFLAGS -u OBJCFLAGS -u OBJCXXFLAGS -u LDFLAGS \
    meson setup "$src/_build" "$src" --cross-file "$WORK/macos.ini" \
    --prefix="$PREFIX" --libdir=lib --buildtype=release --wrap-mode=nodownload \
    -Ddefault_library=shared -Dauto_features=disabled "$@"
  ninja -C "$src/_build" -j "$JOBS"
  ninja -C "$src/_build" install
}

build_freetype() {
  local src; src="$(unpack freetype)" || exit 1
  meson_build "$src" -Dzlib=system -Dharfbuzz=disabled -Dmmap=enabled
}

build_fribidi() {
  local src; src="$(unpack fribidi)" || exit 1
  meson_build "$src" -Ddocs=false -Dbin=false -Dtests=false
}

build_harfbuzz() {
  local src; src="$(unpack harfbuzz)" || exit 1
  meson_build "$src" -Dfreetype=enabled -Dtests=disabled -Dutilities=disabled \
    -Dsubset=disabled -Draster=disabled -Dvector=disabled -Dgpu=disabled
}

build_libass() {
  local src; src="$(unpack libass)" || exit 1
  meson_build "$src" -Dcoretext=enabled -Dfontconfig=disabled -Ddirectwrite=disabled -Dasm=enabled
}

build_dav1d() {
  local src; src="$(unpack dav1d)" || exit 1
  meson_build "$src" -Denable_asm=true -Denable_tools=false -Denable_tests=false -Denable_examples=false
}

build_lua() {
  local src ver; src="$(unpack lua)" || exit 1; ver="$(field lua 2)"
  (
    cd "$src/src"
    objs=()
    for c in *.c; do
      case "$c" in lua.c|luac.c) continue ;; esac
      "$CC" $CFLAGS -DLUA_COMPAT_ALL -DLUA_USE_MACOSX -c "$c" -o "${c%.c}.o"
      objs+=("${c%.c}.o")
    done
    "$CC" -dynamiclib $LDFLAGS -Wl,-install_name,@rpath/liblua5.2.0.dylib \
      -Wl,-compatibility_version,5.2 -Wl,-current_version,"$ver" \
      -o liblua5.2.0.dylib "${objs[@]}"
    install -m644 liblua5.2.0.dylib "$PREFIX/lib/"
    ln -sf liblua5.2.0.dylib "$PREFIX/lib/liblua5.2.dylib"
    mkdir -p "$PREFIX/include/lua5.2"
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
Cflags: -I\${includedir}
EOF
}

build_uchardet() {
  local src; src="$(unpack uchardet)" || exit 1
  env -u CFLAGS -u CXXFLAGS -u LDFLAGS \
    cmake -S "$src" -B "$src/_build" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$WORK/macos.cmake" -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" -DCMAKE_INSTALL_LIBDIR=lib \
    -DCMAKE_FIND_USE_PACKAGE_REGISTRY=OFF -DCMAKE_FIND_USE_SYSTEM_PACKAGE_REGISTRY=OFF \
    -DFETCHCONTENT_FULLY_DISCONNECTED=ON -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
    -DBUILD_BINARY=OFF -DBUILD_STATIC=OFF -DBUILD_SHARED_LIBS=ON
  cmake --build "$src/_build" --parallel "$JOBS"
  cmake --install "$src/_build"
}

build_ffmpeg() {
  local src feature; src="$(unpack ffmpeg)" || exit 1
  (
    cd "$src"
    # Configure appends --extra-* to environment flags: pass them ONCE.
    # With autodetect off, iconv is probed in the C library only. Link the
    # SDK's system iconv explicitly too; never discover Homebrew libiconv.
    env -u CFLAGS -u CXXFLAGS -u OBJCFLAGS -u LDFLAGS \
    ./configure --prefix="$PREFIX" --libdir="$PREFIX/lib" --shlibdir="$PREFIX/lib" \
      --target-os=darwin --arch="$CPU_FAMILY" --cpu=generic --enable-cross-compile \
      --cc="$CC" --cxx="$CXX" --objcc="$OBJC" --ar="$AR" --ranlib="$RANLIB" \
      --pkg-config="$WORK/pkg-config" --install-name-dir=@rpath \
      --enable-shared --disable-static --enable-pic \
      --disable-programs --disable-doc --disable-debug --disable-autodetect \
      --disable-gpl --disable-nonfree --disable-version3 \
      --enable-zlib --enable-iconv --enable-securetransport --enable-libdav1d \
      --enable-videotoolbox --enable-audiotoolbox --enable-pthreads \
      --disable-indevs --disable-outdevs --enable-indev=lavfi \
      --extra-cflags="$CFLAGS" --extra-cxxflags="$CXXFLAGS" --extra-objcflags="$OBJCFLAGS" \
      --extra-ldflags="$LDFLAGS" --extra-libs=-liconv
    grep -qx '#define FFMPEG_LICENSE "LGPL version 2.1 or later"' config.h \
      || die "FFmpeg did not configure as LGPL 2.1 or later"
    for feature in ZLIB ICONV SECURETRANSPORT LIBDAV1D VIDEOTOOLBOX AUDIOTOOLBOX; do
      grep -qx "#define CONFIG_$feature 1" config.h || die "FFmpeg lacks $feature"
    done
    for feature in LAVFI_INDEV TESTSRC2_FILTER HTTPS_PROTOCOL TLS_PROTOCOL; do
      grep -qx "#define CONFIG_$feature 1" config_components.h || die "FFmpeg lacks $feature"
    done
    make -j"$JOBS"
    make install
  )
}

build_libplacebo() {
  local src headers; src="$(unpack libplacebo)" || exit 1
  headers="$(unpack vulkan-headers)" || exit 1
  mkdir -p "$src/3rdparty/Vulkan-Headers"
  cp -R "$headers/." "$src/3rdparty/Vulkan-Headers/"
  # libplacebo's convert.cc parses floats with std::from_chars where the C++
  # library has it and with fast_float otherwise. Apple's libc++ offers no
  # floating-point from_chars for a macOS 11 deployment target, and the
  # release tarball leaves out the git submodule that carries fast_float:
  # it is pinned (role build-macos) and put where libplacebo's meson.build
  # looks for it (3rdparty/fast_float/include).
  fast_float="$(unpack fast-float)" || exit 1
  mkdir -p "$src/3rdparty/fast_float"
  cp -R "$fast_float/include" "$src/3rdparty/fast_float/"
  # Same as Linux: mpv's render API uses mpv's GL renderer. libplacebo
  # supplies utilities; it needs no OpenGL backend, shader compiler or GPU.
  meson_build "$src" -Dopengl=disabled -Dvulkan=disabled -Dd3d11=disabled \
    -Dshaderc=disabled -Dglslang=disabled -Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false
}

build_mpv() {
  local src feature; src="$(unpack mpv)" || exit 1
  patch -d "$src" -p1 < "$recipe/macos/mpv-sdk-only.patch"
  # plain-gl is the backend for an application-supplied GL context.
  # gl-cocoa needs cocoa, which unconditionally compiles the Swift clipboard
  # and calls Swift bridge functions in player/main.c in this pin. Therefore
  # its videotoolbox-gl path cannot be selected without Swift/source changes.
  # FFmpeg VideoToolbox still provides hwdec=videotoolbox-copy.
  meson_build "$src" \
    -Dgpl=false -Dlibmpv=true -Dcplayer=false -Dbuild-date=false \
    -Dlua=lua5.2 -Diconv=enabled -Dzlib=enabled -Duchardet=enabled -Dvector=enabled \
    -Dlibavdevice=enabled -Dcoreaudio=enabled -Dgl=enabled -Dplain-gl=enabled \
    -Dcocoa=disabled -Dgl-cocoa=disabled -Dvideotoolbox-gl=disabled -Dvideotoolbox-pl=disabled \
    -Dswift-build=disabled -Dmacos-cocoa-cb=disabled -Dmacos-media-player=disabled \
    -Dmacos-touchbar=disabled -Davfoundation=disabled -Dvulkan=disabled
  for feature in GL COREAUDIO LUA LIBAVDEVICE; do
    grep -qx "#define HAVE_$feature 1" "$src/_build/config.h" || die "mpv lacks $feature"
  done
  grep -qx '#define HAVE_SWIFT 0' "$src/_build/config.h" || die 'mpv unexpectedly uses Swift'
}

collect() {
  python3 -B "$recipe/macos/macho.py" collect "$PREFIX" "$WORK/stage" "$ARCH"
}

for ARCH in arm64 x86_64; do
  WORK="$WORK_ROOT/$ARCH"
  PREFIX="$WORK/prefix"
  CPU_FAMILY="$ARCH"; [ "$ARCH" != arm64 ] || CPU_FAMILY=aarch64
  mkdir -p "$WORK/src" "$PREFIX/lib/pkgconfig" "$PREFIX/include"
  export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"
  export CFLAGS="-O2 -fPIC -fstack-protector-strong -arch $ARCH -isysroot $SDKROOT -mmacosx-version-min=11.0 -I$PREFIX/include"
  export CXXFLAGS="$CFLAGS" OBJCFLAGS="$CFLAGS" OBJCXXFLAGS="$CFLAGS"
  export LDFLAGS="-arch $ARCH -isysroot $SDKROOT -mmacosx-version-min=11.0 -L$PREFIX/lib -Wl,-headerpad_max_install_names"
  # Enforce the path at the executable boundary too (Meson clears/sets some
  # pkg-config variables itself). The SDK's zlib has no .pc file.
  cat > "$WORK/pkg-config" <<EOF
#!/bin/sh
unset PKG_CONFIG_PATH PKG_CONFIG_SYSROOT_DIR PKG_CONFIG_DIR
export PKG_CONFIG_LIBDIR='$PREFIX/lib/pkgconfig'
exec '$VELA_LIBMPV_TOOLS/bin/pkg-config' "\$@"
EOF
  chmod +x "$WORK/pkg-config"
  zlib_version="$(sed -n 's/^#define ZLIB_VERSION "\([^"]*\)"/\1/p' "$SDKROOT/usr/include/zlib.h")"
  [ -n "$zlib_version" ] || die 'cannot read SDK zlib version'
  cat > "$PREFIX/lib/pkgconfig/zlib.pc" <<EOF
Name: zlib
Description: Apple SDK system zlib
Version: $zlib_version
Libs: -lz
EOF
  cat > "$WORK/macos.ini" <<EOF
[binaries]
c = '$CC'
cpp = '$CXX'
objc = '$OBJC'
objcpp = '$OBJCXX'
ar = '$AR'
strip = 'strip'
pkg-config = '$WORK/pkg-config'
cmake = 'cmake'
[host_machine]
system = 'darwin'
cpu_family = '$CPU_FAMILY'
cpu = '$ARCH'
endian = 'little'
[properties]
needs_exe_wrapper = true
pkg_config_libdir = ['$PREFIX/lib/pkgconfig']
cmake_toolchain_file = '$WORK/macos.cmake'
[built-in options]
c_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0', '-fstack-protector-strong', '-I$PREFIX/include']
cpp_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0', '-fstack-protector-strong', '-I$PREFIX/include']
objc_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0', '-I$PREFIX/include']
objcpp_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0', '-I$PREFIX/include']
c_link_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0', '-L$PREFIX/lib', '-Wl,-headerpad_max_install_names']
cpp_link_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0', '-L$PREFIX/lib', '-Wl,-headerpad_max_install_names']
objc_link_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0', '-L$PREFIX/lib', '-Wl,-headerpad_max_install_names']
objcpp_link_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0', '-L$PREFIX/lib', '-Wl,-headerpad_max_install_names']
EOF
  cat > "$WORK/macos.cmake" <<EOF
set(CMAKE_SYSTEM_NAME Darwin)
set(CMAKE_SYSTEM_PROCESSOR $ARCH)
set(CMAKE_OSX_ARCHITECTURES $ARCH CACHE STRING "" FORCE)
set(CMAKE_OSX_DEPLOYMENT_TARGET 11.0 CACHE STRING "" FORCE)
set(CMAKE_OSX_SYSROOT "$SDKROOT" CACHE PATH "" FORCE)
set(CMAKE_C_COMPILER "$CC")
set(CMAKE_CXX_COMPILER "$CXX")
set(CMAKE_C_FLAGS_INIT "-fstack-protector-strong -I$PREFIX/include")
set(CMAKE_CXX_FLAGS_INIT "-fstack-protector-strong -I$PREFIX/include")
set(CMAKE_SHARED_LINKER_FLAGS_INIT "-L$PREFIX/lib -Wl,-headerpad_max_install_names")
set(CMAKE_FIND_ROOT_PATH "$PREFIX" "$SDKROOT")
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
set(CMAKE_IGNORE_PREFIX_PATH /opt/homebrew /usr/local)
set(CMAKE_INSTALL_NAME_DIR "@rpath")
set(CMAKE_BUILD_WITH_INSTALL_NAME_DIR ON)
EOF
  for s in $steps; do
    printf '\n=== macOS %s: %s ===\n' "$ARCH" "$s"
    case "$s" in collect) collect ;; *) "build_$s" ;; esac
  done
done
case " $steps " in
  *' collect '*)
    python3 -B "$recipe/macos/macho.py" merge "$WORK_ROOT" "$OUT" "$recipe"
    python3 -B "$recipe/check-libmpv-macos.py" "$OUT" --manifest-only
    ;;
esac
