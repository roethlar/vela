#!/usr/bin/env bash
# Check that a built Linux package carries the libmpv set of the embedded
# player, or that it carries none of it.
#
# The embedded player loads Vela's own build of mpv's library and nothing else
# (.agents/plans/embedded-player.md, "libmpv is shipped with Vela"): there is
# no fallback to a library the machine has. A package that was built to carry
# the set and lacks one file of it would install cleanly and then never play
# embedded. This script is where such a package is stopped: at build time.
#
# usage: check-package.sh [--without-libmpv] [--set DIR] PACKAGE...
#        check-package.sh --preflight [--without-libmpv] [--set DIR] [--appimage]
#
#   PACKAGE  a .deb, .rpm, .AppImage or pacman package (.pkg.tar.zst, .xz)
#   --set DIR  the set the package was built from, or was built without
#   --preflight  check no package: only say, by status 0 or 2, whether this
#              check could be made here. scripts/build.sh asks before it
#              builds, so that a missing tool stops a --with-libmpv build
#              at its start and not after the whole of it.
#   --appimage  (with --preflight) an AppImage is among the packages to be
#              checked, which needs unsquashfs as well
#
# Each package is unpacked into a temporary directory; nothing is installed,
# and nothing that comes out of a package, or the package itself, is run.
# (With the set, its libmpv.so.2 is loaded, which runs the library's own
# start-up code; that is the one exception.)
# A file that cannot be unpacked, or that unpacks to something other than
# Vela (no usr/bin/vela), fails whichever of the two questions is asked.
#
# Without --without-libmpv the package must CONTAIN the set (it was built by
# scripts/build.sh --with-libmpv):
#   * the set is in the one directory the loader reads, and is really inside
#     the package (no symbolic link leads there);
#   * every user of the machine may read every file of it, not only the
#     package's owner;
#   * with --set, its MANIFEST is that set's MANIFEST, not another set's
#     (without --set this is not compared, and a "skip" line says so);
#   * its components are the versions pinned in versions.txt;
#   * every file the MANIFEST lists is there and unmodified, and nothing else
#     is (check-libmpv.py);
#   * libmpv.so.2 LOADS from the unpacked package, by absolute path, says it is
#     the pinned mpv release with that release's client API version, and every
#     library of the set in the process came from the unpacked package
#     (check-libmpv.py);
#   * a pacman package also has every licence text where Arch keeps licences.
#
# With --without-libmpv the package must contain NO part of the set (a default
# build): nothing named like the set anywhere in it and, with --set, no file
# with the contents of a file of that set under any name.
#
# Exit status: 0 if every package passed, 1 if one did not, 2 if the check
# could not be made (no package was judged then, and the reason is on standard
# error) or an AppImage could not be read for want of unsquashfs (that package
# has a FAIL line that says so). Needs bsdtar (libarchive) to unpack,
# unsquashfs (squashfs-tools, 4.4 or later) for an AppImage and, to load the library,
# Python 3 and the system libraries the set's MANIFEST lists. The temporary
# directory (TMPDIR) must be one a library can be loaded from. What is
# unpacked there is removed when the check ends, also when it is cancelled (it
# then ends with status 129, 130 or 143, as the signal has it).
set -euo pipefail
shopt -s inherit_errexit
# Unpacking applies the umask. Under a stricter one than this, every file
# would come out closed to other users, and the check of who may read the set
# would blame the package for the builder's shell.
umask 022

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Where the loader looks, relative to the root a package installs into. Tauri's
# resource directory for an installed Linux package is /usr/lib/<productName>
# (inside an AppImage, the same path under the AppImage's own root), and the
# loader joins "libmpv" to it: set_directory in
# src-tauri/src/player/embedded/libmpv.rs. tests/libmpv-packaging.test.mjs
# holds this line to both.
SET_PATH="usr/lib/Vela/libmpv"
# The program itself, which every one of the four kinds installs here. It is
# how a package of Vela is told from any other file that unpacks: an empty
# file unpacks without complaint, to nothing, and so does any archive that
# was given a package's name. Neither question can be put to such a tree, and
# "no part of the set" would be true of nothing at all.
APP_PATH="usr/bin/vela"
# Where the pacman package also puts the licence texts (packaging/arch/PKGBUILD).
ARCH_LICENCES="usr/share/licenses/vela/libmpv"
# The client API version of the pinned mpv release: MPV_CLIENT_API_VERSION in
# that release's include/mpv/client.h. versions.txt pins the release and not
# this number, so the number is written here beside the release it was looked
# up for. When versions.txt moves to another release this script stops until
# both lines have been brought up to date.
CLIENT_API_OF_MPV="0.42.0"
CLIENT_API="2.6"

die() { printf 'check-package: %s\n' "$*" >&2; exit 2; }

without=0
preflight=0
appimage=0
reference=""
packages=()
while [ $# -gt 0 ]; do
  case "$1" in
    --without-libmpv) without=1 ;;
    --preflight) preflight=1 ;;
    --appimage) appimage=1 ;;
    # A --set that names nothing is a mistake, not "no set": it is what
    # --set="$DIR" turns into when DIR is empty, and taking it for "no set"
    # would drop the comparison with the built set without a word.
    --set)   shift; [ -n "${1:-}" ] || die "--set needs a directory"; reference="$1" ;;
    --set=*) reference="${1#*=}"; [ -n "$reference" ] || die "--set= needs a directory" ;;
    -h|--help)
      # The header comment, without the leading "# ".
      awk 'NR>1 && /^#/ {sub(/^# ?/,""); print; next} NR>1 {exit}' "${BASH_SOURCE[0]}"
      exit 0 ;;
    --) shift; packages+=("$@"); break ;;
    -*) die "unknown option: $1 (try --help)" ;;
    *)  packages+=("$1") ;;
  esac
  shift
done
if [ "$preflight" -eq 1 ]; then
  # Named packages would not be looked at, and must not seem to have been.
  [ "${#packages[@]}" -eq 0 ] || die "--preflight checks no package; name none"
else
  [ "${#packages[@]}" -gt 0 ] || die "no package named (try --help)"
  # Named packages say for themselves whether one is an AppImage.
  [ "$appimage" -eq 0 ] || die "--appimage goes with --preflight; name the packages instead"
fi

# What the check cannot be made without. --preflight stops after these.
command -v bsdtar >/dev/null 2>&1 || die "bsdtar is not installed (libarchive; on Debian and Ubuntu: libarchive-tools)"
if [ "$appimage" -eq 1 ]; then
  command -v unsquashfs >/dev/null 2>&1 || die "unsquashfs is not installed (squashfs-tools); an AppImage is read with nothing else"
fi
if [ -n "$reference" ]; then
  [ -f "$reference/MANIFEST" ] || die "no libmpv set in $reference (it has no MANIFEST)"
  reference="$(cd "$reference" && pwd -P)"
fi
if [ "$without" -eq 0 ]; then
  command -v python3 >/dev/null 2>&1 || die "python3 is not installed; loading the library needs it"
  mpv_pin="$(awk '$1 == "mpv" { print $2 }' "$here/versions.txt")"
  [ "$mpv_pin" = "$CLIENT_API_OF_MPV" ] || die "versions.txt pins mpv $mpv_pin, but the client API version written in this script ($CLIENT_API) was looked up for mpv $CLIENT_API_OF_MPV. Read MPV_CLIENT_API_VERSION in include/mpv/client.h of mpv $mpv_pin and bring CLIENT_API_OF_MPV and CLIENT_API up to date."
fi
[ "$preflight" -eq 0 ] || exit 0

# The temporary directory holds unpacked packages, tens of megabytes each, and
# is removed however the check ends, a cancelled check included. An EXIT trap
# alone does not manage that. Given a signal it has no trap for, bash runs the
# EXIT trap at once, while the program it started may still be unpacking into
# the directory being removed. And when the whole process group is hung up (a
# closed terminal) it mostly does not get to run it at all: a subshell that is
# hung up hangs up its group again, and at a second signal bash ends on the
# spot. So the three signals a cancelled check gets have a trap of their own.
# bash then lets the program it is waiting for finish, takes the same signal
# twice as once, and leaves by way of the EXIT trap.
tmp=""
cleanup() {
  # From here on those signals are not listened to: one that arrived now
  # would end the shell between the two steps below.
  trap '' HUP INT TERM
  [ -n "$tmp" ] || return 0
  # A package may hold directories its own files cannot be removed from.
  chmod -R u+rwX "$tmp" 2>/dev/null || true
  rm -rf "$tmp" || printf 'check-package: could not remove %s\n' "$tmp" >&2
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
tmp="$(mktemp -d)"

failed=0   # this package
pass() { printf 'ok   %s\n' "$*"; }
fail() { printf 'FAIL %s\n' "$*"; failed=1; }
skipped() { printf 'skip %s\n' "$*"; }   # a comparison that was not made

# squashfs_offset FILE: where the squashfs of an AppImage begins, printed, or
# status 1 when FILE is not an ELF program followed by a squashfs. The AppImage
# runtime finds its image the same way: right after the ELF section header
# table, which is the last part of the program. Nothing of FILE is run; od
# reads its bytes.
squashfs_offset() {
  local file="$1" ident endian shoff entsize count offset size
  ident="$(od -An -v -tx1 -N6 -- "$file" | tr -d ' \n')" || return 1
  case "${ident:10:2}" in
    01) endian=little ;;
    02) endian=big ;;
    *) return 1 ;;
  esac
  case "${ident:0:10}" in
    7f454c4602) # 64-bit
      shoff="$(od -An -tu8 -j40 -N8 --endian="$endian" -- "$file")" || return 1
      entsize="$(od -An -tu2 -j58 -N2 --endian="$endian" -- "$file")" || return 1
      count="$(od -An -tu2 -j60 -N2 --endian="$endian" -- "$file")" || return 1 ;;
    7f454c4601) # 32-bit
      shoff="$(od -An -tu4 -j32 -N4 --endian="$endian" -- "$file")" || return 1
      entsize="$(od -An -tu2 -j46 -N2 --endian="$endian" -- "$file")" || return 1
      count="$(od -An -tu2 -j48 -N2 --endian="$endian" -- "$file")" || return 1 ;;
    *) return 1 ;;
  esac
  shoff="${shoff//[[:space:]]/}" entsize="${entsize//[[:space:]]/}" count="${count//[[:space:]]/}"
  size="$(stat -c %s -- "$file")" || return 1
  # Numbers that do not fit what a file of this size can hold are not taken
  # into the shell's arithmetic, where they would wrap around.
  [[ "$shoff" =~ ^[0-9]{1,15}$ && "$entsize" =~ ^[0-9]+$ && "$count" =~ ^[0-9]+$ ]] || return 1
  [ "$count" -gt 0 ] || return 1
  offset=$((shoff + entsize * count))
  [ "$offset" -lt "$size" ] || return 1
  [ "$(od -An -v -tx1 -j"$offset" -N4 -- "$file" | tr -d ' \n')" = 68737173 ] || return 1   # "hsqs"
  printf '%s\n' "$offset"
}

# unpack PACKAGE INTO: the file tree the package installs, in INTO/root.
# Returns 3 for a file that is none of the four kinds, 4 for an AppImage when
# there is no unsquashfs to read it with, 1 when unpacking failed.
# Every step says so itself: the caller tests this function's result, and bash
# does not stop at a failed command inside a function called that way.
unpack() {
  local package="$1" into="$2" data offset
  mkdir -p "$into/root" || return 1
  case "$package" in
    *.deb)
      # An ar archive; the files it installs are its data.tar.* member.
      mkdir "$into/deb" || return 1
      bsdtar -xf "$package" -C "$into/deb" || return 1
      data="$(find "$into/deb" -maxdepth 1 -type f -name 'data.tar*' -print -quit)" || return 1
      [ -n "$data" ] || { echo "no data.tar member"; return 1; }
      bsdtar -xf "$data" -C "$into/root" || return 1
      ;;
    *.rpm|*.pkg.tar.zst|*.pkg.tar.xz)
      bsdtar -xf "$package" -C "$into/root" || return 1
      ;;
    *.AppImage)
      # An AppImage is a program with a squashfs file system appended to it.
      # It could unpack itself, but that would run it, and what runs is
      # whatever the file holds, with the rights of whoever checks it. So it
      # is never run: the squashfs is read by unsquashfs, where the ELF
      # header says the program ends. A file that is not such an AppImage is
      # refused before anything else is done with it.
      offset="$(squashfs_offset "$package")" || {
        echo "this is not an AppImage this check can read: no squashfs follows an ELF program in it"
        return 1
      }
      command -v unsquashfs >/dev/null 2>&1 || return 4
      rmdir "$into/root" || return 1
      unsquashfs -no-progress -no-xattrs -o "$offset" -d "$into/root" "$package" >/dev/null || return 1
      ;;
    *) return 3 ;;
  esac
  # A package may hold a directory or a file that the account unpacking it is
  # given no right to read. It is part of the package all the same, and
  # neither mode may pass a package for not having looked into a corner of it.
  chmod -R u+rwX "$into/root" || return 1
}

# The package must contain the set, whole, where Vela will look for it.
check_with() {
  local package="$1" root="$2" set="$2/$SET_PATH"
  local stray real_root real_set closed pins have line out status kind component path texts

  if [ ! -d "$set" ]; then
    fail "place: the package has no $SET_PATH, and Vela looks for the set nowhere else"
    while IFS= read -r -d '' stray; do
      fail "place: it does have ${stray#"$root"/}"
    done < <(find "$root" \( -name libmpv -o -name 'libmpv.so*' -o -name 'libvela-*' \) -print0)
    return 0
  fi
  # A link in the package would be followed on the user's machine too, to
  # wherever it points there. The set has to be files of this package.
  real_root="$(cd "$root" && pwd -P)"
  real_set="$(cd "$set" && pwd -P)"
  if [ "$real_set" != "$real_root/$SET_PATH" ]; then
    fail "place: $SET_PATH is reached through a symbolic link (it is really $real_set); the set must be in the package itself"
    return 0
  fi
  pass "place: the set is at $SET_PATH"

  # An installed package's files belong to root, and Vela runs as whoever
  # starts it: a file of the set that only its owner may read is, to every
  # user, a file that is not there. (It happens: Tauri's rpm bundler carries a
  # resource's permissions over from the build tree as they are, so a set
  # copied under a strict umask arrives closed.) Unpacked here the file belongs
  # to the builder, who can read it, so the load below would not notice.
  # What is reported is what the package gives to users other than the owner.
  # The owner's own permissions are no longer the package's: unpacking opened
  # them (see unpack).
  closed=0
  while IFS= read -r -d '' path; do
    fail "access: ${path#"$root"/} cannot be read by every user (other users have '$(stat -c %A "$path" | cut -c8-10)')"
    closed=1
  done < <(find "$root/${SET_PATH%/*}" -maxdepth 0 ! -perm -005 -print0
           find "$set" \( -type d ! -perm -005 -o ! -type d ! -perm -004 \) -print0)
  [ "$closed" -eq 1 ] || pass "access: every user may read every file of the set"

  if [ -n "$reference" ]; then
    if cmp -s "$reference/MANIFEST" "$set/MANIFEST"; then
      pass "source: its MANIFEST is the one in $reference"
    else
      fail "source: its MANIFEST is not the one in $reference; the package holds another set"
    fi
  else
    # Everything below can pass for a set that is whole and loads and is still
    # not the set that was built: one with a library changed and its MANIFEST
    # written to match. Only the built set can tell, so say it was not asked.
    skipped "source: no set was given (--set DIR), so its MANIFEST was not compared with the built set's"
  fi

  pins="$(awk '$1 !~ /^#/ && $3 == "ship" { print $1, $2 }' "$here/versions.txt" | LC_ALL=C sort)"
  have=""
  [ ! -f "$set/MANIFEST" ] || have="$(awk '$1 == "component" { print $2, $3 }' "$set/MANIFEST" | LC_ALL=C sort)"
  if [ "$pins" = "$have" ]; then
    pass "pins: its components are the versions versions.txt pins"
  else
    while IFS= read -r line; do
      fail "pins: versions.txt pins $line, and the set in the package is not built from it"
    done < <(LC_ALL=C comm -23 <(printf '%s\n' "$pins") <(printf '%s\n' "$have") | sed '/^$/d')
    while IFS= read -r line; do
      fail "pins: the set in the package is built from $line, which versions.txt does not pin"
    done < <(LC_ALL=C comm -13 <(printf '%s\n' "$pins") <(printf '%s\n' "$have") | sed '/^$/d')
  fi

  # The load must find what an installed Vela finds: this directory and the
  # machine's own libraries. A library path or a preload in the builder's
  # shell is neither. -I keeps Python from importing anything that is not its
  # own, whatever the package or the environment holds.
  status=0
  out="$(env -u LD_LIBRARY_PATH -u LD_PRELOAD python3 -I -B "$here/check-libmpv.py" "$set" \
           --expect-mpv "$mpv_pin" --expect-client-api "$CLIENT_API" 2>&1)" || status=$?
  # Its own closing line would read as this script's; this script prints one.
  printf '%s\n' "$out" | sed '/^RESULT /d'
  # Three things have to agree before the set is taken to have passed: no
  # line of it says FAIL, its exit status is 0, and its closing line says
  # PASS. Each alone fails the package. Its status is not trusted to have
  # counted every FAIL line it printed, and silence is not taken for a pass.
  if grep -q '^FAIL ' <<< "$out"; then
    failed=1
  elif [ "$status" -ne 0 ]; then
    fail "load: check-libmpv.py stopped with status $status"
  elif ! grep -qx 'RESULT PASS' <<< "$out"; then
    fail "load: check-libmpv.py ended without saying that the set passed"
  fi

  case "$package" in
    *.pkg.tar.*)
      texts=0
      if [ -f "$set/MANIFEST" ]; then
        while read -r kind component path; do
          [ "$kind" = licence ] || continue
          texts=$((texts + 1))
          cmp -s "$set/$path" "$root/$ARCH_LICENCES/$component/${path##*/}" \
            || fail "licences: $ARCH_LICENCES/$component/${path##*/} is missing or is not the set's $path"
        done < "$set/MANIFEST"
      fi
      if [ "$texts" -eq 0 ]; then
        fail "licences: the set's MANIFEST names no licence text"
      elif [ "$failed" -eq 0 ]; then
        pass "licences: all $texts licence texts are also under $ARCH_LICENCES"
      fi
      ;;
  esac
}

# The package must contain no part of the set.
check_without() {
  local root="$2" path line sha name kind looked=0 clean=1
  local -A in_set=()

  # By name: the directory the loader reads, the library itself, and the
  # private name every other library of the set carries.
  while IFS= read -r -d '' path; do
    fail "absent: the package has ${path#"$root"/}"
    clean=0
  done < <(find "$root" \( -name libmpv -o -name 'libmpv.so*' -o -name 'libvela-*' \) -print0)

  # By content: a file of the set under another name is still a file of the set.
  if [ -n "$reference" ]; then
    while read -r kind sha _ name; do
      [ "$kind" = file ] && [ -n "$sha" ] && [ -n "$name" ] || continue
      in_set["$sha"]="$name"
    done < "$reference/MANIFEST"
    if [ "${#in_set[@]}" -eq 0 ]; then
      fail "absent: the MANIFEST in $reference lists no file to compare with"
      clean=0
    fi
    while IFS= read -r -d '' line; do
      sha="${line%% *}"
      path="${line#*  }"
      looked=$((looked + 1))
      if [ -n "${in_set[$sha]:-}" ]; then
        fail "absent: ${path#"$root"/} has the contents of the set's ${in_set[$sha]}"
        clean=0
      fi
    done < <(find "$root" -type f -exec sha256sum -z {} +)
  fi

  [ "$clean" -eq 0 ] || {
    if [ -n "$reference" ]; then
      pass "absent: nothing named like the set, and none of the package's $looked files has the contents of a file of the set in $reference"
    else
      pass "absent: nothing named like the set (no set was given, so contents were not compared)"
    fi
  }
}

any_failed=0
unchecked=0
n=0
for package in "${packages[@]}"; do
  n=$((n + 1))
  failed=0
  printf '== %s\n' "$package"
  if [ ! -f "$package" ]; then
    fail "unpack: no such file"
  else
    status=0
    unpack "$package" "$tmp/$n" >"$tmp/$n.log" 2>&1 || status=$?
    if [ "$status" -eq 3 ]; then
      fail "unpack: not a .deb, .rpm, .AppImage or pacman package"
    elif [ "$status" -eq 4 ]; then
      fail "unpack: unsquashfs is not installed (squashfs-tools), and an AppImage is read with nothing else, so this package could not be checked"
      unchecked=1
    elif [ "$status" -ne 0 ]; then
      sed 's/^/     /' "$tmp/$n.log"
      fail "unpack: the package could not be unpacked"
    elif [ ! -f "$tmp/$n/root/$APP_PATH" ] || [ -L "$tmp/$n/root/$APP_PATH" ]; then
      fail "unpack: this is not a package of Vela: it has no $APP_PATH"
    elif [ "$without" -eq 1 ]; then
      check_without "$package" "$tmp/$n/root"
    else
      check_with "$package" "$tmp/$n/root"
    fi
  fi
  if [ "$failed" -eq 0 ]; then
    printf 'RESULT PASS %s\n' "$package"
  else
    printf 'RESULT FAIL %s\n' "$package"
    any_failed=1
  fi
  # One unpacked package at a time: a set is some 30 MB.
  chmod -R u+rwX "$tmp/$n" 2>/dev/null || true
  rm -rf "$tmp/$n" "$tmp/$n.log"
done
# A package that could not be read for want of a tool was not judged, and the
# run says so by its status, whatever became of the others.
[ "$unchecked" -eq 0 ] || exit 2
exit "$any_failed"
