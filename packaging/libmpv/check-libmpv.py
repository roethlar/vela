#!/usr/bin/env python3
"""Check a built libmpv set the way Vela will use it.

Loads libmpv.so.2 by absolute path with RTLD_NOW | RTLD_DEEPBIND and proves,
without a GPU:

  * every file the MANIFEST lists is present and unmodified, and the
    directory holds no library the MANIFEST does not list;
  * the library loads and reports the pinned mpv version;
  * every loaded library of the set really comes from the set's directory;
  * the given clips decode in software to end of file;
  * (with --https URL) a fetch over HTTPS works through the machine's GnuTLS
    and trust store.

usage: check-libmpv.py LIBMPV_DIR [--expect-mpv VERSION] [--https URL] [CLIP...]
Exit status 0 only if every check passed. Needs nothing but Python 3.
"""
import ctypes
import hashlib
import os
import sys


class Event(ctypes.Structure):
    _fields_ = [("event_id", ctypes.c_int), ("error", ctypes.c_int),
                ("reply_userdata", ctypes.c_uint64), ("data", ctypes.c_void_p)]


class EndFile(ctypes.Structure):
    _fields_ = [("reason", ctypes.c_int), ("error", ctypes.c_int)]


EVENT_NONE, EVENT_SHUTDOWN, EVENT_END_FILE, EVENT_FILE_LOADED = 0, 1, 7, 8
REASON_EOF = 0
ERROR_UNKNOWN_FORMAT = -17
HTTPS_ATTEMPTS = 3


def check_manifest(libdir):
    """Compare the directory with its MANIFEST. Returns a list of problems."""
    problems = []
    listed = {}
    try:
        with open(os.path.join(libdir, "MANIFEST")) as f:
            for line in f:
                parts = line.split()
                if parts and parts[0] == "file":
                    listed[parts[3]] = (parts[1], int(parts[2]))
    except OSError as e:
        return [f"no MANIFEST: {e}"]
    if "libmpv.so.2" not in listed:
        problems.append("MANIFEST does not list libmpv.so.2")
    for name, (sha, size) in sorted(listed.items()):
        path = os.path.join(libdir, name)
        if not os.path.isfile(path):
            problems.append(f"missing: {name}")
            continue
        with open(path, "rb") as f:
            data = f.read()
        if len(data) != size or hashlib.sha256(data).hexdigest() != sha:
            problems.append(f"modified: {name}")
    for name in sorted(os.listdir(libdir)):
        if ".so" in name and name not in listed:
            problems.append(f"not in MANIFEST: {name}")
    return problems


def main():
    args = sys.argv[1:]
    if not args:
        print(__doc__)
        return 2
    libdir = os.path.realpath(args.pop(0))
    expect, https, clips = None, None, []
    while args:
        a = args.pop(0)
        if a == "--expect-mpv":
            expect = args.pop(0)
        elif a == "--https":
            https = args.pop(0)
        else:
            clips.append(os.path.abspath(a))
    ok = True

    problems = check_manifest(libdir)
    for p in problems:
        print(f"FAIL manifest: {p}")
    if problems:
        ok = False
    else:
        print("ok   manifest: every listed file present and unmodified, nothing unlisted")

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

    def play(url):
        """Play one item with null outputs. Returns (reason, error, video codec)."""
        h = lib.mpv_create()
        for k, val in (("config", "no"), ("vo", "null"), ("ao", "null"), ("hwdec", "no"),
                       ("untimed", "yes"), ("idle", "once"), ("terminal", "no"),
                       ("network-timeout", "20")):
            lib.mpv_set_option_string(h, k.encode(), val.encode())
        if lib.mpv_initialize(h) < 0:
            return None
        cmd = (ctypes.c_char_p * 3)(b"loadfile", url.encode(), None)
        lib.mpv_command(h, cmd)
        reason = error = codec = None
        while True:
            ev = lib.mpv_wait_event(h, 60.0).contents
            if ev.event_id in (EVENT_NONE, EVENT_SHUTDOWN):
                break
            if ev.event_id == EVENT_FILE_LOADED:
                codec = prop(h, "video-codec")
            if ev.event_id == EVENT_END_FILE:
                ef = ctypes.cast(ev.data, ctypes.POINTER(EndFile)).contents
                reason, error = ef.reason, ef.error
        lib.mpv_terminate_destroy(h)
        return reason, error, codec

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
    expected = {os.path.join(libdir, n) for n in os.listdir(libdir) if ".so" in n}
    not_loaded = sorted(os.path.basename(p) for p in expected - mapped)
    good = not outside and not not_loaded
    print(f"{'ok  ' if good else 'FAIL'} isolation: {len(mapped)} libraries mapped from the set"
          + (f"; from elsewhere: {outside}" if outside else "")
          + (f"; never loaded: {not_loaded}" if not_loaded else ""))
    ok = ok and good

    for clip in clips:
        r = play(clip)
        good = r is not None and r[0] == REASON_EOF and r[2] is not None
        print(f"{'ok  ' if good else 'FAIL'} decode {os.path.basename(clip)}: "
              f"end-file reason={r and r[0]} error={r and r[1]} codec={r and r[2]!r}")
        ok = ok and good

    if https:
        good, r = False, None
        for _ in range(HTTPS_ATTEMPTS):
            r = play(https)
            # A page that is not media still proves the handshake and the fetch.
            good = r is not None and (r[0] == REASON_EOF or r[1] == ERROR_UNKNOWN_FORMAT)
            if good:
                break
        detail = lib.mpv_error_string(r[1]).decode() if r and r[1] is not None else "?"
        print(f"{'ok  ' if good else 'FAIL'} https {https}: end-file reason={r and r[0]} "
              f"error={r and r[1]} ({detail})")
        ok = ok and good

    print("RESULT", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
