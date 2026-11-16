#!/usr/bin/env bash
# Regenerate the clips check-libmpv.py decodes. Needs ffmpeg (with libx264,
# libx265 and libsvtav1), ffprobe and python3; none of that is needed to RUN
# the check, only to remake its inputs.
#
# The clips are tiny synthetic patterns (FFmpeg's testsrc2 and a sine tone),
# one second each, so nothing here is anyone's content. After regenerating,
# record what a trusted set decodes them to:
#
#   python3 packaging/libmpv/check-libmpv.py <set dir> --record \
#       packaging/libmpv/fixtures/*.mp4 packaging/libmpv/fixtures/*.mkv
#
# and put the printed object under "clips" in fixtures/expected.json. An
# encoder of another version produces other bytes, so the recorded results
# change with the clips.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="$here/fixtures"
mkdir -p "$out/broken"

src=(-f lavfi -i "testsrc2=d=1:s=160x90:r=12" -f lavfi -i "sine=d=1")
ffmpeg -v error -y "${src[@]}" -c:v libx264 -preset veryslow -crf 30 -pix_fmt yuv420p \
  -c:a aac -b:a 32k "$out/h264-aac.mp4"
ffmpeg -v error -y "${src[@]}" -c:v libx265 -x265-params log-level=error -crf 32 \
  -pix_fmt yuv420p10le -c:a ac3 -b:a 64k "$out/hevc10-ac3.mkv"
ffmpeg -v error -y "${src[@]}" -c:v libsvtav1 -svtav1-params lp=1 -crf 45 \
  -c:a libopus -b:a 24k "$out/av1-opus.mkv"

# Damaged copies: the same file with every packet of one track overwritten
# with zeroes. The container still opens and the other track still plays to
# the end, which is exactly the case the check must not accept.
zero_track() { # SOURCE DEST v|a
  python3 - "$1" "$2" "$3" <<'PY'
import json, subprocess, sys
source, dest, stream = sys.argv[1:4]
packets = json.loads(subprocess.run(
    ["ffprobe", "-v", "error", "-select_streams", stream, "-show_entries",
     "packet=pos,size", "-of", "json", source],
    capture_output=True, text=True, check=True).stdout)["packets"]
data = bytearray(open(source, "rb").read())
for packet in packets:
    position, size = int(packet["pos"]), int(packet["size"])
    data[position:position + size] = bytes(size)
open(dest, "wb").write(data)
PY
}
zero_track "$out/h264-aac.mp4" "$out/broken/video-zeroed.mp4" v
zero_track "$out/h264-aac.mp4" "$out/broken/audio-zeroed.mp4" a
ls -l "$out" "$out/broken"
