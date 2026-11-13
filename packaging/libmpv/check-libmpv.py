#!/usr/bin/env python3
"""Check a built libmpv set the way Vela will use it.

Loads libmpv.so.2 by absolute path with RTLD_NOW | RTLD_DEEPBIND and proves,
without a GPU:

  * the directory is exactly what its MANIFEST lists: every listed file
    (libraries, licence texts, the build patch) present and unmodified,
    nothing unlisted, and a licence text for every shipped component;
  * the library loads and reports the pinned mpv version;
  * every loaded library of the set really comes from the set's directory;
  * each given clip DECODES to what was recorded for it: the same number of
    video frames with the same contents (the decoders are bit-exact, so the
    per-frame checksums must match exactly) and the same amount of audio at
    the same level (audio decoders are not bit-exact; a tolerance applies);
  * each clip given with --broken does NOT pass that comparison, so a
    comparison that can no longer fail is itself a failure;
  * (with --https URL) a fetch over HTTPS works through the machine's GnuTLS
    and trust store.

usage: check-libmpv.py LIBMPV_DIR [--expect-mpv VERSION] [--https URL]
                       [--broken CLIP]... [--record] [CLIP...]

Recorded results live in expected.json next to the clips. --record prints
that file's contents for the given clips instead of comparing; use it, with a
set you trust, after changing a fixture.

Exit status 0 only if every check passed. Needs nothing but Python 3.
"""
import array
import ctypes
import hashlib
import json
import math
import os
import sys
import tempfile
import time
import wave


class Event(ctypes.Structure):
    _fields_ = [("event_id", ctypes.c_int), ("error", ctypes.c_int),
                ("reply_userdata", ctypes.c_uint64), ("data", ctypes.c_void_p)]


class EndFile(ctypes.Structure):
    _fields_ = [("reason", ctypes.c_int), ("error", ctypes.c_int)]


EVENT_NONE, EVENT_SHUTDOWN, EVENT_END_FILE = 0, 1, 7
REASON_EOF = 0
ERROR_UNKNOWN_FORMAT = -17
HTTPS_ATTEMPTS = 3
# The clips are a second long and decode in a fraction of that. A track that
# cannot be decoded can leave mpv waiting for it; that is a failure, not a wait.
RUN_SECONDS = 25
# Audio decoders work in floating point: the same decoder on another CPU may
# differ in the last bit. Lost or silent audio differs by far more than this.
AUDIO_SAMPLES_TOLERANCE = 0.02
AUDIO_LEVEL_TOLERANCE = 0.05


def check_manifest(libdir):
    """Compare the directory with its MANIFEST, in both directions.

    Every listed file must be present and unmodified; no file may exist that
    is not listed (libraries, licence texts, anything); and every shipped
    component must have at least one licence text listed. Returns a list of
    problems, empty when the set is exactly what was built."""
    problems = []
    listed = {}
    components = []
    try:
        with open(os.path.join(libdir, "MANIFEST")) as f:
            for line in f:
                parts = line.split(None, 3)
                if len(parts) == 4 and parts[0] == "file":
                    listed[parts[3].rstrip("\n")] = (parts[1], int(parts[2]))
                elif len(parts) >= 2 and parts[0] == "component":
                    components.append(parts[1])
    except OSError as e:
        return [f"no MANIFEST: {e}"]
    if "libmpv.so.2" not in listed:
        problems.append("MANIFEST does not list libmpv.so.2")
    if not components:
        problems.append("MANIFEST names no component")
    for name in components:
        if not any(path.startswith(f"licenses/{name}/") for path in listed):
            problems.append(f"no licence text listed for component: {name}")
    for name, (sha, size) in sorted(listed.items()):
        if os.path.isabs(name) or ".." in name.split("/"):
            problems.append(f"MANIFEST entry leaves the set: {name}")
            continue
        path = os.path.join(libdir, name)
        if not os.path.isfile(path) or os.path.islink(path):
            problems.append(f"missing: {name}")
            continue
        with open(path, "rb") as f:
            data = f.read()
        if len(data) != size or hashlib.sha256(data).hexdigest() != sha:
            problems.append(f"modified: {name}")
    for root, _dirs, files in os.walk(libdir):
        for filename in files:
            name = os.path.relpath(os.path.join(root, filename), libdir)
            if name != "MANIFEST" and name not in listed:
                problems.append(f"not in MANIFEST: {name}")
    return sorted(problems)


def judge_decode(expected, measured):
    """Compare what a clip decoded to with what was recorded for it.

    `expected` has video_frames, frame_bytes, video_sha256, audio_samples and
    audio_level; `measured` has those and how each track's decoding ended. Returns a list of problems, each starting
    with "video" or "audio"; empty when the clip decoded as recorded."""
    problems = []
    if measured.get("video_ended") != "eof":
        problems.append(f"video: decoding did not reach end of file ({measured.get('video_ended')})")
    if measured.get("audio_ended") != "eof":
        problems.append(f"audio: decoding did not reach end of file ({measured.get('audio_ended')})")
    if measured.get("video_frames") != expected["video_frames"]:
        problems.append(f"video: {measured.get('video_frames')} frames decoded, {expected['video_frames']} recorded")
    elif measured.get("frame_bytes") != expected["frame_bytes"]:
        problems.append(f"video: frames of {measured.get('frame_bytes')} bytes, {expected['frame_bytes']} recorded")
    elif measured.get("video_sha256") != expected["video_sha256"]:
        problems.append("video: decoded frames differ from the recorded ones")
    samples, want = measured.get("audio_samples") or 0, expected["audio_samples"]
    level, want_level = measured.get("audio_level") or 0.0, expected["audio_level"]
    if abs(samples - want) > want * AUDIO_SAMPLES_TOLERANCE:
        problems.append(f"audio: {samples} samples decoded, {want} recorded")
    elif abs(level - want_level) > want_level * AUDIO_LEVEL_TOLERANCE:
        problems.append(f"audio: level {level:.1f}, {want_level:.1f} recorded")
    return problems


def judge_broken(must_fail, problems):
    """A deliberately damaged clip must fail the comparison on the track that
    was damaged. Returns a problem string, or None when it failed as it must."""
    if any(p.startswith(must_fail) for p in problems):
        return None
    return f"the {must_fail} of a damaged clip was not noticed (problems found: {problems or 'none'})"


def load_expectations(directory):
    """The recorded results kept beside the clips, or {} when there are none."""
    try:
        with open(os.path.join(directory, "expected.json")) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def main():
    args = sys.argv[1:]
    if not args:
        print(__doc__)
        return 2
    libdir = os.path.realpath(args.pop(0))
    expect, https, clips, broken, record = None, None, [], [], False
    while args:
        a = args.pop(0)
        if a == "--expect-mpv":
            expect = args.pop(0)
        elif a == "--https":
            https = args.pop(0)
        elif a == "--broken":
            broken.append(os.path.abspath(args.pop(0)))
        elif a == "--record":
            record = True
        else:
            clips.append(os.path.abspath(a))
    ok = True

    problems = check_manifest(libdir)
    for p in problems:
        print(f"FAIL manifest: {p}")
    if problems:
        ok = False
    else:
        print("ok   manifest: every listed file present and unmodified, nothing unlisted, "
              "a licence text for every component")

    path = os.path.join(libdir, "libmpv.so.2")
    try:
        lib = ctypes.CDLL(path, mode=os.RTLD_NOW | os.RTLD_LOCAL | os.RTLD_DEEPBIND)
    except OSError as e:
        print(f"FAIL load: {e}")
        print("RESULT FAIL")
        return 1
    lib.mpv_client_api_version.restype = ctypes.c_ulong
    lib.mpv_create.restype = ctypes.c_void_p
    lib.mpv_get_property_string.restype = ctypes.c_void_p
    lib.mpv_get_property_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
    lib.mpv_set_option_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
    lib.mpv_initialize.argtypes = [ctypes.c_void_p]
    lib.mpv_command.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_char_p)]
    lib.mpv_wait_event.restype = ctypes.POINTER(Event)
    lib.mpv_wait_event.argtypes = [ctypes.c_void_p, ctypes.c_double]
    lib.mpv_terminate_destroy.argtypes = [ctypes.c_void_p]
    lib.mpv_free.argtypes = [ctypes.c_void_p]
    lib.mpv_error_string.restype = ctypes.c_char_p
    lib.mpv_error_string.argtypes = [ctypes.c_int]
    v = lib.mpv_client_api_version()
    print(f"ok   load: {path} (client API {v >> 16}.{v & 0xffff})")

    def prop(h, name):
        p = lib.mpv_get_property_string(h, name.encode())
        if not p:
            return None
        s = ctypes.string_at(p).decode()
        lib.mpv_free(p)
        return s

    def run(url, options):
        """Play one item to its end with the given options. Returns (reason, error)."""
        h = lib.mpv_create()
        for k, val in (("config", "no"), ("terminal", "no"), ("hwdec", "no"), ("idle", "once"),
                       ("network-timeout", "20")) + tuple(options):
            if lib.mpv_set_option_string(h, k.encode(), val.encode()) < 0:
                lib.mpv_terminate_destroy(h)
                return None, None
        if lib.mpv_initialize(h) < 0:
            lib.mpv_terminate_destroy(h)
            return None, None
        cmd = (ctypes.c_char_p * 3)(b"loadfile", url.encode(), None)
        lib.mpv_command(h, cmd)
        reason = error = None
        deadline = time.monotonic() + RUN_SECONDS
        while time.monotonic() < deadline:
            ev = lib.mpv_wait_event(h, 0.5).contents
            if ev.event_id == EVENT_SHUTDOWN:
                break
            if ev.event_id == EVENT_END_FILE:
                ef = ctypes.cast(ev.data, ctypes.POINTER(EndFile)).contents
                reason, error = ef.reason, ef.error
        lib.mpv_terminate_destroy(h)
        return reason, error

    def ended(reason):
        return "eof" if reason == REASON_EOF else ("no end within the time allowed" if reason is None
                                                   else f"reason {reason}")

    def measure(clip):
        """Decode a clip in software, one track at a time, and describe what came out."""
        out = {"video_ended": None, "video_frames": 0, "frame_bytes": None, "video_sha256": None,
               "audio_ended": None, "audio_samples": 0, "audio_level": 0.0}
        with tempfile.TemporaryDirectory() as tmp:
            # Video alone: every decoded frame, unconverted, as one checksum
            # line (FFmpeg's framecrc writer).
            crc = os.path.join(tmp, "frames.crc")
            reason, _ = run(clip, (("o", crc), ("of", "framecrc"), ("ovc", "rawvideo"), ("aid", "no")))
            out["video_ended"] = ended(reason)
            frames = []
            if os.path.exists(crc):
                with open(crc) as f:
                    for line in f:
                        fields = [x.strip() for x in line.split(",")]
                        if len(fields) == 6 and fields[0] == "0":
                            frames.append((fields[4], fields[5]))
            out["video_frames"] = len(frames)
            sizes = {size for size, _ in frames}
            out["frame_bytes"] = int(sizes.pop()) if len(sizes) == 1 else None
            out["video_sha256"] = hashlib.sha256(
                "\n".join(c for _, c in frames).encode()).hexdigest() if frames else None
            # Audio alone: the decoded sound as PCM, to count it and measure its level.
            wav = os.path.join(tmp, "audio.wav")
            reason, _ = run(clip, (("o", wav), ("of", "wav"), ("oac", "pcm_s16le"), ("vid", "no")))
            out["audio_ended"] = ended(reason)
            try:
                with wave.open(wav, "rb") as w:
                    pcm = array.array("h")
                    pcm.frombytes(w.readframes(w.getnframes()))
                    out["audio_samples"] = len(pcm)
                    if pcm:
                        out["audio_level"] = math.sqrt(sum(x * x for x in pcm) / len(pcm))
            except (OSError, EOFError, wave.Error):
                pass
        return out
    h = lib.mpv_create()
    for k, val in (("config", "no"), ("vo", "null"), ("terminal", "no")):
        lib.mpv_set_option_string(h, k.encode(), val.encode())
    if lib.mpv_initialize(h) < 0:
        print("FAIL mpv_initialize")
        print("RESULT FAIL")
        return 1
    mpv_version, ffmpeg_version = prop(h, "mpv-version"), prop(h, "ffmpeg-version")
    lib.mpv_terminate_destroy(h)
    good = expect is None or mpv_version == f"mpv v{expect}"
    print(f"{'ok  ' if good else 'FAIL'} version: {mpv_version!r}, FFmpeg {ffmpeg_version!r}"
          + ("" if good else f" (expected 'mpv v{expect}')"))
    ok = ok and good

    # Every mapped file whose name marks it as part of the set must come
    # from the set's directory, and the set's libraries must all be mapped.
    mapped = set()
    with open("/proc/self/maps") as f:
        for line in f:
            p = line.split()[-1]
            base = os.path.basename(p)
            if base.startswith("libvela-") or base == "libmpv.so.2":
                mapped.add(os.path.realpath(p))
    outside = sorted(p for p in mapped if os.path.dirname(p) != libdir)
    expected_libs = {os.path.join(libdir, n) for n in os.listdir(libdir) if ".so" in n}
    not_loaded = sorted(os.path.basename(p) for p in expected_libs - mapped)
    good = not outside and not not_loaded
    print(f"{'ok  ' if good else 'FAIL'} isolation: {len(mapped)} libraries mapped from the set"
          + (f"; from elsewhere: {outside}" if outside else "")
          + (f"; never loaded: {not_loaded}" if not_loaded else ""))
    ok = ok and good

    if record:
        recorded = {os.path.basename(clip): {k: v for k, v in measure(clip).items() if not k.endswith("_ended")}
                    for clip in clips}
        print(json.dumps(recorded, indent=2, sort_keys=True))
        return 0 if ok else 1

    for clip in clips:
        name = os.path.basename(clip)
        expected = load_expectations(os.path.dirname(clip)).get("clips", {}).get(name)
        if expected is None:
            print(f"FAIL decode {name}: nothing is recorded for it in expected.json")
            ok = False
            continue
        problems = judge_decode(expected, measure(clip))
        for p in problems:
            print(f"FAIL decode {name}: {p}")
        if not problems:
            print(f"ok   decode {name}: {expected['video_frames']} video frames identical to the "
                  f"recorded ones, {expected['audio_samples']} audio samples at the recorded level")
        ok = ok and not problems

    for clip in broken:
        name = os.path.basename(clip)
        # Damaged clips live in a subdirectory of the clips they were made from.
        recorded = load_expectations(os.path.dirname(os.path.dirname(clip)))
        entry = recorded.get("broken", {}).get(name)
        expected = recorded.get("clips", {}).get(entry["of"]) if entry else None
        if not entry or not expected:
            print(f"FAIL broken {name}: nothing is recorded for it in expected.json")
            ok = False
            continue
        problem = judge_broken(entry["must_fail"], judge_decode(expected, measure(clip)))
        print(f"FAIL broken {name}: {problem}" if problem
              else f"ok   broken {name}: its damaged {entry['must_fail']} fails the comparison, as it must")
        ok = ok and problem is None

    if https:
        good, r = False, (None, None)
        for _ in range(HTTPS_ATTEMPTS):
            r = run(https, (("vo", "null"), ("ao", "null"), ("untimed", "yes")))
            # A page that is not media still proves the handshake and the fetch.
            good = r[0] == REASON_EOF or r[1] == ERROR_UNKNOWN_FORMAT
            if good:
                break
        detail = lib.mpv_error_string(r[1]).decode() if r[1] is not None else "?"
        print(f"{'ok  ' if good else 'FAIL'} https {https}: end-file reason={r[0]} error={r[1]} ({detail})")
        ok = ok and good

    print("RESULT", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
