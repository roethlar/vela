#!/usr/bin/env bash
# Run check-libmpv.py against a built set inside a clean distribution image that
# has only what a desktop install has: runtime libraries, no -dev packages, no
# GPU. This is the proof that the set built on the baseline loads on the
# baseline and on newer distributions.
#
# usage: check-in-container.sh IMAGE [SET_DIR]
#   IMAGE    e.g. docker.io/library/ubuntu:22.04, docker.io/library/debian:12,
#            registry.fedoraproject.org/fedora:latest
#   SET_DIR  default: .libmpv/out/libmpv in the repo root
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
image="${1:?usage: check-in-container.sh IMAGE [SET_DIR]}"
set_dir="$(cd "${2:-$repo/.libmpv/out/libmpv}" && pwd)"
engine="$(command -v podman || command -v docker || true)"
[ -n "$engine" ] || { echo "check-in-container: needs podman or docker" >&2; exit 2; }

case "$image" in
  *ubuntu*|*debian*)
    setup='export DEBIAN_FRONTEND=noninteractive
           apt-get update -qq >/dev/null
           apt-get install -y -qq --no-install-recommends python3 ca-certificates libegl1 libva2 libva-drm2 libdrm2 libpulse0 libasound2 libfontconfig1 libgnutls30 zlib1g >/dev/null' ;;
  *fedora*)
    setup='dnf install -y -q python3 ca-certificates libglvnd-egl libva libdrm pulseaudio-libs alsa-lib fontconfig gnutls zlib >/dev/null' ;;
  *) echo "check-in-container: no package recipe for $image" >&2; exit 2 ;;
esac

# The pinned mpv version comes from the one place it is written.
expect="$(awk '$1 == "mpv" { print $2 }' "$here/versions.txt")"

"$engine" run --rm --security-opt label=disable \
  -v "$set_dir":/set:ro -v "$here":/recipe:ro "$image" bash -ec "
    $setup
    . /etc/os-release
    echo \"== \$PRETTY_NAME, glibc \$(ldd --version | head -1 | awk '{print \$NF}')\"
    python3 /recipe/check-libmpv.py /set --expect-mpv $expect --https https://example.com/ /recipe/fixtures/*
  "
