#!/usr/bin/env bash
# Windows x86_64 counterpart of build-libmpv.sh. Separate because PE imports,
# a cross sysroot and Windows system DLLs have no ELF/glibc equivalents.
# Usage: bash scripts/build-libmpv-windows.sh --container [step...]
# Output: $LIBMPV_ROOT/out/libmpv-windows-x86_64/{*.dll,licenses/,MANIFEST}
# LIBMPV_ROOT defaults to .libmpv. Downloads are shared with the Linux build;
# work-windows-x86_64 and the output are disjoint. A full run starts clean.
set -euo pipefail
shopt -s inherit_errexit

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
recipe="$repo/packaging/libmpv"
LIBMPV_ROOT="${LIBMPV_ROOT:-$repo/.libmpv}"
die() { printf 'build-libmpv-windows: %s\n' "$*" >&2; exit 1; }

if [ "${1:-}" = --container ]; then
  shift
  engine="$(command -v podman || command -v docker || true)"
  [ -n "$engine" ] || die "--container needs podman or docker"
  mkdir -p "$LIBMPV_ROOT"
  LIBMPV_ROOT="$(cd "$LIBMPV_ROOT" && pwd)"
  "$engine" build -t vela-libmpv-windows-build -f "$recipe/Containerfile.windows" "$recipe"
  exec "$engine" run --rm --security-opt label=disable \
    -v "$repo":/repo:ro -v "$LIBMPV_ROOT":/libmpv -e LIBMPV_ROOT=/libmpv \
    vela-libmpv-windows-build bash /repo/scripts/build-libmpv-windows.sh "$@"
fi

VERSIONS="$recipe/versions.txt"
SRC_DIR="$LIBMPV_ROOT/src"
WORK="$LIBMPV_ROOT/work-windows-x86_64"
PREFIX="$WORK/prefix"
OUT="$LIBMPV_ROOT/out/libmpv-windows-x86_64"
JOBS="${JOBS:-$(nproc)}"
DOWNLOAD_RETRIES="${LIBMPV_DOWNLOAD_RETRIES:-4}"
TARGET=x86_64-w64-mingw32
field() { awk -v n="$1" -v i="$2" '$1 == n { print $i }' "$VERSIONS"; }
sources() { awk -v n="$1" '$1 == n { for (i = 5; i <= NF; i++) print $i }' "$VERSIONS"; }

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
         && echo "$sha  $file.part" | sha256sum -c --quiet - >/dev/null 2>&1; then
        mv "$file.part" "$file"; got=1; break
      fi
      rm -f "$file.part"
      echo "build-libmpv-windows: no good download from $url" >&2
    done
    [ -n "$got" ] || die "could not download '$name' from any of its sources"
  fi
  echo "$sha  $file" | sha256sum -c --quiet - >&2 || die "checksum mismatch: $file"
  printf '%s' "$file"
}

unpack() {
  local name="$1" file dir
  file="$(fetch "$name")"
  dir="$WORK/src/$name"
  rm -rf "$dir"; mkdir -p "$dir"
  tar -xf "$file" -C "$dir" --strip-components=1 --no-same-owner
  static_winpthread
  printf '%s' "$dir"
}

# winpthread, static, under both names a link asks for it by (see LDFLAGS).
# Done by the first unpack of a run, after its source passed its checksum, so
# that a bad source is refused before any compiler is asked anything.
static_winpthread() {
  [ -z "${winpthread_ready:-}" ] || return 0
  local winpthread; winpthread="$("$CC" -print-file-name=libwinpthread.a)"
  [ -f "$winpthread" ] || die "the toolchain has no static libwinpthread.a"
  install -m644 "$winpthread" "$PREFIX/lib/libwinpthread.a" >&2
  install -m644 "$winpthread" "$PREFIX/lib/libpthread.a" >&2
  winpthread_ready=1
}

# No host pkg-config/CMake paths, user flags, or compiler header overrides.
unset PKG_CONFIG_PATH PKG_CONFIG_SYSROOT_DIR CMAKE_PREFIX_PATH CMAKE_TOOLCHAIN_FILE
unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH LIBRARY_PATH CPPFLAGS
export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"
export CC="$TARGET-gcc-posix" CXX="$TARGET-g++-posix"
export AR="$TARGET-ar" RANLIB="$TARGET-ranlib" STRIP="$TARGET-strip"
export CC_FOR_BUILD=gcc CXX_FOR_BUILD=g++ CFLAGS_FOR_BUILD= CXXFLAGS_FOR_BUILD= LDFLAGS_FOR_BUILD=
export CFLAGS="-O2 -D_WIN32_WINNT=0x0A00 -DWINVER=0x0A00 -I$PREFIX/include"
export CXXFLAGS="$CFLAGS -Wa,-mbig-obj"
# libgcc and libstdc++ static. winpthread is static through the copies placed
# first on the library path (see where $PREFIX is made), which every link, the
# compiler's own late -lpthread included, finds before the toolchain's import
# library; only the objects a link uses come in. (Whole-archive, tried first,
# put winpthread's VERSION resource into every DLL, which collides with
# FFmpeg's own.) The DLL import closure in collect rejects any runtime leak.
export LDFLAGS="-L$PREFIX/lib -static-libgcc -static-libstdc++ -Wl,--nxcompat,--dynamicbase"

meson_build() {
  local src="$1"; shift
  meson setup "$src/_build" "$src" --cross-file "$WORK/windows.ini" \
    --prefix="$PREFIX" --libdir=lib --buildtype=release --wrap-mode=nodownload \
    -Ddefault_library=shared -Dauto_features=disabled "$@"
  ninja -C "$src/_build" -j "$JOBS"
  ninja -C "$src/_build" install
}

cmake_setup() {
  local src="$1"; shift
  cmake -S "$src" -B "$src/_build" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$WORK/windows.cmake" -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" -DCMAKE_INSTALL_LIBDIR=lib \
    -DCMAKE_FIND_USE_PACKAGE_REGISTRY=OFF -DCMAKE_FIND_USE_SYSTEM_PACKAGE_REGISTRY=OFF \
    -DFETCHCONTENT_FULLY_DISCONNECTED=ON "$@"
}

cmake_build() {
  local src="$1"; shift
  cmake_setup "$src" "$@"
  cmake --build "$src/_build" --parallel "$JOBS"
  cmake --install "$src/_build"
}

build_zlib() {
  local src; src="$(unpack zlib)"
  cmake_build "$src" -DZLIB_BUILD_SHARED=OFF -DZLIB_BUILD_STATIC=ON -DZLIB_BUILD_TESTING=OFF
  # zlib names the Windows static archive libzs.a; FFmpeg probes -lz.
  install -m644 "$PREFIX/lib/libzs.a" "$PREFIX/lib/libz.a"
}

build_libiconv() {
  local src; src="$(unpack libiconv)"
  (
    cd "$src"
    ./configure --host="$TARGET" --build="$(gcc -dumpmachine)" --prefix="$PREFIX" \
      --enable-static --disable-shared --disable-nls --disable-rpath
    # The LGPL libraries and headers only; not the GPL iconv program or docs.
    # What the top Makefile's lib/localcharset.h rule does: libcharset, with
    # its header installed into lib/, where lib/ includes it from.
    make -C libcharset -j"$JOBS"
    make -C libcharset install-lib libdir="$PWD/lib" includedir="$PWD/lib"
    make -C lib -j"$JOBS"
    make -C libcharset install
    make -C lib install
    # What the top Makefile's install-lib rule does for the header (include/
    # has no Makefile of its own).
    install -Dm644 include/iconv.h.inst "$PREFIX/include/iconv.h"
  )
}

build_freetype() {
  local src; src="$(unpack freetype)"
  meson_build "$src" -Dzlib=system -Dharfbuzz=disabled -Dmmap=disabled
}

build_fribidi() {
  local src; src="$(unpack fribidi)"
  meson_build "$src" -Ddocs=false -Dbin=false -Dtests=false
}

build_harfbuzz() {
  local src; src="$(unpack harfbuzz)"
  meson_build "$src" -Dfreetype=enabled -Dtests=disabled -Dutilities=disabled \
    -Dsubset=disabled -Draster=disabled -Dvector=disabled -Dgpu=disabled
}

build_libass() {
  local src; src="$(unpack libass)"
  meson_build "$src" -Ddirectwrite=enabled -Dfontconfig=disabled -Dasm=enabled
}

build_dav1d() {
  local src; src="$(unpack dav1d)"
  meson_build "$src" -Denable_tools=false -Denable_tests=false -Denable_examples=false
}

build_lua() {
  local src ver; src="$(unpack lua)"; ver="$(field lua 2)"
  (
    cd "$src/src"
    objs=()
    for c in *.c; do
      case "$c" in lua.c|luac.c) continue ;; esac
      "$CC" $CFLAGS -DLUA_COMPAT_ALL -DLUA_BUILD_AS_DLL -c "$c" -o "${c%.c}.o"
      objs+=("${c%.c}.o")
    done
    "$CC" -shared -o lua52.dll "${objs[@]}" $LDFLAGS -Wl,--out-implib,liblua5.2.dll.a
    install -m644 lua52.dll "$PREFIX/bin/"
    install -m644 liblua5.2.dll.a "$PREFIX/lib/"
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
Cflags: -I\${includedir} -DLUA_BUILD_AS_DLL
EOF
}

build_uchardet() {
  local src; src="$(unpack uchardet)"
  cmake_build "$src" -DBUILD_BINARY=OFF -DBUILD_STATIC=OFF -DBUILD_SHARED_LIBS=ON \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5
}

build_shaderc() {
  local src glslang tools headers
  src="$(unpack shaderc)"; glslang="$(unpack glslang)"
  tools="$(unpack spirv-tools)"; headers="$(unpack spirv-headers)"
  # No git-sync-deps, ExternalProject or downloads: all four archives are pinned.
  cmake_setup "$src" -DBUILD_SHARED_LIBS=OFF \
    -DSHADERC_GLSLANG_DIR="$glslang" -DSHADERC_SPIRV_TOOLS_DIR="$tools" \
    -DSHADERC_SPIRV_HEADERS_DIR="$headers" -DSPIRV-Headers_SOURCE_DIR="$headers" \
    -DSHADERC_SKIP_INSTALL=OFF -DSHADERC_SKIP_TESTS=ON -DSHADERC_SKIP_EXAMPLES=ON \
    -DSHADERC_SKIP_EXECUTABLES=ON -DSHADERC_SKIP_COPYRIGHT_CHECK=ON \
    -DSHADERC_ENABLE_WERROR_COMPILE=OFF -DSHADERC_ENABLE_WGSL_OUTPUT=OFF \
    -DSHADERC_ENABLE_HLSL=OFF -DENABLE_HLSL=OFF -DENABLE_GLSLANG_BINARIES=OFF \
    -DGLSLANG_TESTS=OFF -DENABLE_CTEST=OFF -DBUILD_EXTERNAL=OFF -DENABLE_OPT=ON \
    -DSPIRV_SKIP_TESTS=ON -DSPIRV_SKIP_EXECUTABLES=ON -DSPIRV_WERROR=OFF \
    -DSPIRV_TOOLS_BUILD_STATIC=ON -DSPIRV_BUILD_FUZZER=OFF \
    -DSPIRV_BUILD_LIBFUZZER_TARGETS=OFF
  # Generate all upstream install/export rules together (glslang references
  # SPIRV-Tools exports), but build/stage only the combined static archive.
  cmake --build "$src/_build" --target shaderc_combined --parallel "$JOBS"
  install -m644 "$src/_build/libshaderc/libshaderc_combined.a" "$PREFIX/lib/"
  install -d "$PREFIX/include/shaderc"
  install -m644 "$src"/libshaderc/include/shaderc/*.h "$PREFIX/include/shaderc/"
  cat > "$PREFIX/lib/pkgconfig/shaderc.pc" <<EOF
prefix=$PREFIX
libdir=\${prefix}/lib
includedir=\${prefix}/include
Name: shaderc
Description: Pinned static shaderc and its glslang/SPIRV-Tools dependencies
Version: $(field shaderc 2)
Libs: -L\${libdir} -lshaderc_combined $(static_cxx_runtime)
Cflags: -I\${includedir}
EOF
}

# The C++ runtime the static shader libraries need, as absolute paths to the
# static archives: Meson turns a -lstdc++ into the path of the toolchain's
# import library (libstdc++.dll.a) whatever -Wl,-Bstatic surrounds it, which
# put both the DLL's and the archive's definitions into libplacebo's link.
static_cxx_runtime() {
  local stdcxx; stdcxx="$("$CXX" -print-file-name=libstdc++.a)"
  [ -f "$stdcxx" ] || die "the toolchain has no static libstdc++.a"
  printf '%s %s' "$stdcxx" "$PREFIX/lib/libwinpthread.a"
}

build_spirv_cross() {
  local src; src="$(unpack spirv-cross)"
  cmake_build "$src" -DSPIRV_CROSS_STATIC=ON -DSPIRV_CROSS_SHARED=OFF \
    -DSPIRV_CROSS_CLI=OFF -DSPIRV_CROSS_ENABLE_TESTS=OFF \
    -DSPIRV_CROSS_ENABLE_GLSL=ON -DSPIRV_CROSS_ENABLE_HLSL=ON \
    -DSPIRV_CROSS_ENABLE_MSL=OFF -DSPIRV_CROSS_ENABLE_CPP=OFF \
    -DSPIRV_CROSS_ENABLE_REFLECT=OFF -DSPIRV_CROSS_ENABLE_UTIL=OFF \
    -DSPIRV_CROSS_ENABLE_C_API=ON
  # Both Meson projects request this module name. The C ABI is also available
  # statically; without SPIRV_CROSS_SHARED the header declares no dllimport.
  cat > "$PREFIX/lib/pkgconfig/spirv-cross-c-shared.pc" <<EOF
prefix=$PREFIX
libdir=\${prefix}/lib
includedir=\${prefix}/include/spirv_cross
Name: spirv-cross-c-shared
Description: Pinned SPIRV-Cross C API, linked statically (HLSL backend)
Version: $(pkg-config --modversion spirv-cross-c)
Libs: -L\${libdir} -lspirv-cross-c -lspirv-cross-hlsl -lspirv-cross-glsl -lspirv-cross-core $(static_cxx_runtime)
Cflags: -I\${includedir}
EOF
}

build_ffmpeg() {
  local src; src="$(unpack ffmpeg)"
  (
    cd "$src"
    # FFmpeg's configure starts from CFLAGS/LDFLAGS in the environment and
    # then appends --extra-*: with both, the whole-archive winpthread went into
    # every link twice (duplicate symbols and VERSION resource). Flags go in
    # once, through --extra-*.
    env -u CFLAGS -u CXXFLAGS -u LDFLAGS \
    ./configure --prefix="$PREFIX" --libdir="$PREFIX/lib" --shlibdir="$PREFIX/bin" \
      --target-os=mingw32 --arch=x86_64 --enable-cross-compile --cross-prefix="$TARGET-" \
      --cc="$CC" --cxx="$CXX" --pkg-config=pkg-config \
      --enable-shared --disable-static --disable-programs --disable-doc --disable-debug \
      --disable-autodetect --disable-gpl --disable-nonfree --disable-version3 \
      --enable-zlib --enable-iconv --enable-schannel --enable-libdav1d \
      --enable-d3d11va --enable-dxva2 --enable-w32threads --disable-pthreads \
      --disable-indevs --disable-outdevs --enable-indev=lavfi \
      --extra-cflags="$CFLAGS" --extra-cxxflags="$CXXFLAGS" --extra-ldflags="$LDFLAGS" \
      --extra-libs=-liconv
    # --extra-libs: with --disable-autodetect, configure probes only the C
    # library's iconv (Windows' has none) and keeps --enable-iconv without
    # ever adding -liconv; the set's static libiconv is linked explicitly.
    grep -qx '#define FFMPEG_LICENSE "LGPL version 2.1 or later"' config.h \
      || die "FFmpeg did not configure as LGPL 2.1 or later"
    # Explicit requests can be silently disabled by some FFmpeg probes.
    for feature in ZLIB ICONV SCHANNEL LIBDAV1D D3D11VA DXVA2; do
      grep -qx "#define CONFIG_$feature 1" config.h || die "FFmpeg lacks $feature"
    done
    grep -qx '#define CONFIG_LAVFI_INDEV 1' config_components.h || die "FFmpeg lacks lavfi input"
    make -j"$JOBS"
    make install
  )
}

build_libplacebo() {
  local src headers; src="$(unpack libplacebo)"; headers="$(unpack vulkan-headers)"
  mkdir -p "$src/3rdparty/Vulkan-Headers"
  cp -a "$headers/." "$src/3rdparty/Vulkan-Headers/"
  meson_build "$src" -Dd3d11=enabled -Dshaderc=enabled -Dglslang=disabled \
    -Dvulkan=disabled -Dopengl=disabled -Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false
}

build_mpv() {
  local src; src="$(unpack mpv)"
  meson_build "$src" \
    -Dgpl=false -Dlibmpv=true -Dcplayer=false -Dbuild-date=false \
    -Dlua=lua5.2 -Diconv=enabled -Dzlib=enabled -Duchardet=enabled -Dvector=enabled \
    -Dd3d11=enabled -Dshaderc=enabled -Dspirv-cross=enabled \
    -Dd3d-hwaccel=enabled -Dd3d9-hwaccel=enabled -Dwasapi=enabled \
    -Dwin32-threads=enabled -Dlibavdevice=enabled -Dgl-win32=enabled
  # gl-win32: mpv's gl option defaults to enabled (auto_features leaves it so)
  # and needs a backend; WGL, on the system's opengl32.dll, is the OpenGL
  # fallback for vo=gpu where Direct3D 11 cannot start.
}

# Only Windows inbox DLLs are allowed outside the set. In particular, no
# libgcc_s_seh-1.dll, libstdc++-6.dll, libwinpthread-1.dll or libssp-0.dll.
ALLOWED_SYSTEM="
  advapi32.dll avrt.dll bcrypt.dll crypt32.dll d3d11.dll d3d9.dll
  dcomp.dll dinput8.dll dwmapi.dll dwrite.dll dxgi.dll dxva2.dll gdi32.dll
  imm32.dll kernel32.dll ksuser.dll msvcrt.dll ncrypt.dll ntdll.dll ole32.dll
  oleaut32.dll opengl32.dll pathcch.dll powrprof.dll propsys.dll psapi.dll rpcrt4.dll
  secur32.dll setupapi.dll shcore.dll shell32.dll shlwapi.dll user32.dll
  userenv.dll usp10.dll uxtheme.dll version.dll winmm.dll ws2_32.dll wtsapi32.dll
"
# Also Windows API sets (api-ms-win-*, ext-ms-win-*): names the system loader
# resolves to its own DLLs, never files anyone ships (mpv imports
# api-ms-win-core-path-l1-1-0.dll, present since Windows 8).
allowed_system() {
  case "${1,,}" in api-ms-win-*.dll|ext-ms-win-*.dll) return 0 ;; esac
  case " $(echo $ALLOWED_SYSTEM) " in *" ${1,,} "*) return 0 ;; esac
  return 1
}
needed() { "$TARGET-objdump" -p "$1" | awk '/DLL Name:/ { print tolower($3) }'; }

collect() {
  local lib dep found name f roles="" system="" queue="libmpv-2.dll" seen=" "
  rm -rf "$OUT"; mkdir -p "$OUT/licenses"
  while [ -n "$queue" ]; do
    lib="${queue%% *}"; queue="${queue#"$lib"}"; queue="${queue# }"
    case "$seen" in *" $lib "*) continue ;; esac
    seen="$seen$lib "
    found="$(find "$PREFIX/bin" "$PREFIX/lib" -maxdepth 1 -type f -iname "$lib" -print)"
    [ -n "$found" ] && [[ "$found" != *$'\n'* ]] || die "missing or ambiguous DLL: $lib"
    "$TARGET-objdump" -f "$found" | grep -q 'file format pei-x86-64' || die "not x86_64 PE: $lib"
    install -m644 "$found" "$OUT/$lib"
    "$STRIP" --strip-unneeded "$OUT/$lib"
    for dep in $(needed "$OUT/$lib"); do
      if allowed_system "$dep"; then
        system="$system$dep"$'\n'
      else
        queue="${queue:+$queue }$dep"
      fi
    done
  done
  # Verify the complete import closure after copying and stripping.
  for lib in "$OUT"/*.dll; do
    for dep in $(needed "$lib"); do
      [ -f "$OUT/$dep" ] || allowed_system "$dep" || die "unshipped dependency: $dep"
    done
  done
  lic() { local name="$1"; shift; mkdir -p "$OUT/licenses/$name"
    local f; for f in "$@"; do
      install -Dm644 "$WORK/src/$name/$f" "$OUT/licenses/$name/$f"
      roles="$roles"$'\n'"licence $name licenses/$name/$f"
    done
  }
  lic mpv LICENSE.LGPL Copyright
  lic ffmpeg COPYING.LGPLv2.1 COPYING.LGPLv3 COPYING.GPLv3 LICENSE.md
  lic libplacebo LICENSE
  lic libass COPYING
  lic freetype LICENSE.TXT docs/FTL.TXT
  lic harfbuzz COPYING
  lic fribidi COPYING
  lic dav1d COPYING
  lic uchardet COPYING
  lic zlib LICENSE
  lic libiconv COPYING.LIB libcharset/COPYING.LIB
  lic shaderc LICENSE
  lic glslang LICENSE.txt
  lic spirv-tools LICENSE
  lic spirv-headers LICENSE
  # These compiled Khronos headers carry additional MIT-style notices.
  lic spirv-cross LICENSE spirv.h spirv.hpp GLSL.std.450.h
  lic vulkan-headers LICENSE.md LICENSES/Apache-2.0.txt LICENSES/MIT.txt
  mkdir -p "$OUT/licenses/lua"
  sed -n '/^\* Copyright (C)/,/^\*\*\*\*\*/p' "$WORK/src/lua/src/lua.h" > "$OUT/licenses/lua/LICENSE"
  [ -s "$OUT/licenses/lua/LICENSE" ] || die "could not extract Lua's licence text"
  roles="$roles"$'\n'"licence lua licenses/lua/LICENSE"
  install -m644 "$recipe/windows-license-notes.txt" "$OUT/licenses/NOTES.txt"
  # Runtime objects are linked statically. Carry the distribution's full
  # compiler/runtime notices (including GCC's Runtime Library Exception).
  mkdir -p "$OUT/licenses/toolchain"
  cp /usr/share/doc/gcc-mingw-w64-x86-64-posix/copyright "$OUT/licenses/toolchain/gcc-copyright"
  cp /usr/share/doc/mingw-w64-x86-64-dev/copyright "$OUT/licenses/toolchain/mingw-copyright"
  for name in $(awk '$1 !~ /^#/ && NF { print $1 }' "$VERSIONS"); do
    printf '%s\n' "$roles" | grep -q "^licence $name " || die "no licence text collected for component '$name'"
  done
  {
    echo '# libmpv set for Vela. Generated by scripts/build-libmpv-windows.sh; do not edit.'
    echo 'arch x86_64'
    echo 'platform windows'
    awk '$1 !~ /^#/ && NF { print "component", $1, $2 }' "$VERSIONS"
    printf '%s\n' "$roles" | sed '/^$/d' | LC_ALL=C sort -u
    printf '%s' "$system" | sed '/^$/d' | LC_ALL=C sort -u | sed 's/^/system /'
    ( cd "$OUT" && find . -type f ! -name MANIFEST -printf '%P\n' | LC_ALL=C sort \
      | while IFS= read -r f; do
          printf 'file %s %s %s\n' "$(sha256sum "$f" | cut -d' ' -f1)" "$(stat -c %s "$f")" "$f"
        done )
  } > "$OUT/MANIFEST"
  cat "$OUT/MANIFEST"
}

# Reject unknown steps before touching any output or looking for a compiler.
steps="${*:-zlib libiconv freetype fribidi harfbuzz libass dav1d lua uchardet shaderc spirv_cross ffmpeg libplacebo mpv collect}"
for s in $steps; do
  case "$s" in
    zlib|libiconv|freetype|fribidi|harfbuzz|libass|dav1d|lua|uchardet|shaderc|spirv_cross|ffmpeg|libplacebo|mpv|collect) ;;
    *) die "unknown step '$s'" ;;
  esac
done
[ "$#" -ne 0 ] || { rm -rf "$WORK" "$OUT"; }
mkdir -p "$SRC_DIR" "$WORK/src" "$PREFIX/bin" "$PREFIX/lib/pkgconfig" "$PREFIX/include"
cat > "$WORK/windows.ini" <<EOF
[binaries]
c = '$CC'
cpp = '$CXX'
ar = '$AR'
strip = '$STRIP'
windres = '$TARGET-windres'
dlltool = '$TARGET-dlltool'
pkg-config = 'pkg-config'
[host_machine]
system = 'windows'
cpu_family = 'x86_64'
cpu = 'x86_64'
endian = 'little'
[properties]
needs_exe_wrapper = true
pkg_config_libdir = ['$PREFIX/lib/pkgconfig']
EOF
cat > "$WORK/windows.cmake" <<EOF
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR AMD64)
set(CMAKE_C_COMPILER $CC)
set(CMAKE_CXX_COMPILER $CXX)
set(CMAKE_RC_COMPILER $TARGET-windres)
set(CMAKE_AR $AR)
set(CMAKE_RANLIB $RANLIB)
set(CMAKE_FIND_ROOT_PATH "$PREFIX" "/usr/$TARGET")
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
EOF
for s in $steps; do
  printf '\n=== Windows: %s ===\n' "$s"
  case "$s" in collect) collect ;; *) "build_$s" ;; esac
done
