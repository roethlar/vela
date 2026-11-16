#!/usr/bin/env python3
"""Check the Windows x86_64 set: python check-libmpv-windows.py SET_DIRECTORY.

Before loading code, check every MANIFEST file, hash, size, component version
and licence against the directory and versions.txt. Then load by absolute
path with a restricted DLL search, report the client API, play the synthetic
lavfi source to EOF through null outputs, and inspect loaded module paths.
This is a headless load/play check, not a D3D11, HDR, hardware decode or HTTPS
test. --manifest-only also works on Linux. Exit 0 means all requested checks
passed; 1 is a failed check; 2 means the check could not run.
"""
import argparse
import ctypes
import hashlib
import ntpath
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import time


def safe_name(name):
    return (bool(name) and not ntpath.isabs(name) and ":" not in name
            and "\\" not in name and all(p not in ("", ".", "..") for p in name.split("/")))


def read_manifest(directory):
    files, components, licences = {}, {}, []
    arch = platform = None
    for number, line in enumerate((Path(directory) / "MANIFEST").read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        kind = parts[0]
        if kind == "file" and len(parts) == 4:
            _, digest, size, name = parts
            if not safe_name(name) or name.casefold() == "manifest":
                raise ValueError(f"unsafe file name: {name}")
            if name.casefold() in files:
                raise ValueError(f"duplicate file: {name}")
            if not re.fullmatch(r"[0-9a-f]{64}", digest) or not size.isdecimal():
                raise ValueError(f"invalid file record: {name}")
            files[name.casefold()] = (name, digest, int(size))
        elif kind == "component" and len(parts) == 3:
            if parts[1] in components:
                raise ValueError(f"duplicate component: {parts[1]}")
            components[parts[1]] = parts[2]
        elif kind == "licence" and len(parts) == 3:
            licences.append((parts[1], parts[2]))
        elif kind == "arch" and len(parts) == 2 and arch is None:
            arch = parts[1]
        elif kind == "platform" and len(parts) == 2 and platform is None:
            platform = parts[1]
        elif kind == "system" and len(parts) == 2 and re.fullmatch(r"[\w.-]+\.dll", parts[1]):
            pass
        else:
            raise ValueError(f"invalid MANIFEST line {number}: {line}")
    if (arch, platform) != ("x86_64", "windows"):
        raise ValueError("MANIFEST must name Windows x86_64")
    if "libmpv-2.dll" not in files:
        raise ValueError("MANIFEST does not list libmpv-2.dll")
    for name, _, _ in files.values():
        if name.lower().endswith(".dll") and "/" in name:
            raise ValueError(f"DLL outside the set root: {name}")
    for component, name in licences:
        if (component not in components or name.casefold() not in files
                or not name.startswith(f"licenses/{component}/")
                or files[name.casefold()][2] == 0):
            raise ValueError(f"invalid licence record: {component} {name}")
    for component in components:
        if not any(c == component for c, _ in licences):
            raise ValueError(f"no licence text listed for component: {component}")
    return files, components


def pinned_components(pins):
    # Every pin but the macOS set's own build inputs (role build-macos).
    return {p[0]: p[1] for line in Path(pins).read_text(encoding="utf-8").splitlines()
            if (p := line.split()) and not p[0].startswith("#")
            and (len(p) < 3 or p[2] != "build-macos")}


def check_manifest(directory, pins):
    try:
        files, components = read_manifest(directory)
        problems = []
        if components != pinned_components(pins):
            problems.append("component versions differ from versions.txt")
        root = Path(directory).resolve()
        actual = {}
        for folder, dirs, names in os.walk(root, followlinks=False):
            for entry in dirs + names:
                path = Path(folder) / entry
                name = path.relative_to(root).as_posix()
                info = path.lstat()
                if (stat.S_ISLNK(info.st_mode)
                        or getattr(info, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT):
                    problems.append(f"link or reparse point in set: {name}")
                    if entry in dirs:
                        dirs.remove(entry)
                    continue
                if entry not in names:
                    continue
                if not stat.S_ISREG(info.st_mode):
                    problems.append(f"not a regular file: {name}")
                    continue
                key = name.casefold()
                if key in actual:
                    problems.append(f"case-colliding file: {name}")
                actual[key] = name
                if name == "MANIFEST":
                    continue
                if key not in files:
                    problems.append(f"not in MANIFEST: {name}")
                    continue
                _, digest, size = files[key]
                if info.st_size != size or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                    problems.append(f"modified: {name}")
        for key, (name, _, _) in files.items():
            if key not in actual:
                problems.append(f"missing: {name}")
        return sorted(set(problems))
    except (OSError, UnicodeError, ValueError) as error:
        return [str(error)]


def check_loaded(directory, listed, modules, require_mpv=False, baseline=(), system_root=r"C:\Windows"):
    """Pure Windows path comparison, also exercised by the policy tests on Linux."""
    root = ntpath.normcase(ntpath.normpath(str(directory)))
    baseline = {ntpath.normcase(ntpath.normpath(path)) for path in baseline}
    windows = ntpath.normcase(ntpath.normpath(system_root))
    system_dirs = [ntpath.join(windows, name) + "\\" for name in
                   ("system32", "syswow64", "winsxs", "driverstore")]
    dlls = {ntpath.basename(name).casefold() for name in listed if name.lower().endswith(".dll")}
    problems = []
    seen = set()
    for path in modules:
        path = ntpath.normcase(ntpath.normpath(path))
        parent, name = ntpath.split(path)
        # Include subdirectories, though DLLs may only be listed in the root.
        inside = path.startswith(root.rstrip("\\") + "\\")
        if inside and name.endswith(".dll") and (name not in dlls or parent != root):
            problems.append(f"loaded unlisted DLL: {path}")
        if path not in baseline and not inside and not any(path.startswith(d) for d in system_dirs):
            problems.append(f"new module loaded from outside the set and Windows: {path}")
        if name in dlls:
            if parent != root:
                problems.append(f"set DLL loaded from elsewhere: {path}")
            else:
                seen.add(name)
    if require_mpv and "libmpv-2.dll" not in seen:
        problems.append("libmpv-2.dll is not mapped from the set")
    return sorted(set(problems))


def windows_api():
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    kernel.K32EnumProcessModules.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.HMODULE),
                                             wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
    kernel.K32EnumProcessModules.restype = wintypes.BOOL
    kernel.GetModuleFileNameW.argtypes = [wintypes.HMODULE, wintypes.LPWSTR, wintypes.DWORD]
    kernel.GetModuleFileNameW.restype = wintypes.DWORD
    kernel.SetDefaultDllDirectories.argtypes = [wintypes.DWORD]
    kernel.SetDefaultDllDirectories.restype = wintypes.BOOL
    return kernel


def loaded_modules(kernel):
    from ctypes import wintypes
    count = 256
    for _ in range(8):
        handles = (wintypes.HMODULE * count)()
        needed = wintypes.DWORD()
        if not kernel.K32EnumProcessModules(kernel.GetCurrentProcess(), handles,
                                            ctypes.sizeof(handles), ctypes.byref(needed)):
            raise ctypes.WinError(ctypes.get_last_error())
        if needed.value > ctypes.sizeof(handles):
            count = needed.value // ctypes.sizeof(wintypes.HMODULE) + 32
            continue
        paths = []
        for handle in handles[:needed.value // ctypes.sizeof(wintypes.HMODULE)]:
            buffer = ctypes.create_unicode_buffer(32768)
            length = kernel.GetModuleFileNameW(handle, buffer, len(buffer))
            if not length or length >= len(buffer):
                raise OSError("cannot obtain complete loaded module path")
            paths.append(os.path.realpath(buffer.value))
        return paths
    raise OSError("loaded module list kept growing")


class Event(ctypes.Structure):
    _fields_ = [("event_id", ctypes.c_int), ("error", ctypes.c_int),
                ("reply_userdata", ctypes.c_uint64), ("data", ctypes.c_void_p)]


class EndFile(ctypes.Structure):
    _fields_ = [("reason", ctypes.c_int), ("error", ctypes.c_int)]


def play(directory):
    files, components = read_manifest(directory)
    kernel = windows_api()
    # Remove PATH and the current directory even for subsequent LoadLibrary
    # calls made by mpv/dependencies. Keep only system32 and the added set.
    if not kernel.SetDefaultDllDirectories(0x00000800 | 0x00000400):
        raise ctypes.WinError(ctypes.get_last_error())
    baseline = frozenset(loaded_modules(kernel))
    snapshots = set(baseline)
    system_root = os.environ["SystemRoot"]

    def audit(require_mpv=False):
        snapshots.update(loaded_modules(kernel))
        problems = check_loaded(directory, files, snapshots, require_mpv, baseline, system_root)
        if problems:
            raise RuntimeError("; ".join(problems))

    audit()
    with os.add_dll_directory(str(directory)):
        # LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32.
        # This is stricter than DEFAULT_DIRS: no Python application directory.
        lib = ctypes.CDLL(str(directory / "libmpv-2.dll"), winmode=0x00000100 | 0x00000800)
        audit(True)
        signatures = {
            "mpv_client_api_version": (ctypes.c_ulong, []),
            "mpv_create": (ctypes.c_void_p, []),
            "mpv_set_option_string": (ctypes.c_int, [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]),
            "mpv_initialize": (ctypes.c_int, [ctypes.c_void_p]),
            "mpv_command": (ctypes.c_int, [ctypes.c_void_p, ctypes.POINTER(ctypes.c_char_p)]),
            "mpv_wait_event": (ctypes.POINTER(Event), [ctypes.c_void_p, ctypes.c_double]),
            "mpv_get_property_string": (ctypes.c_void_p, [ctypes.c_void_p, ctypes.c_char_p]),
            "mpv_free": (None, [ctypes.c_void_p]),
            "mpv_terminate_destroy": (None, [ctypes.c_void_p]),
        }
        for name, (result, arguments) in signatures.items():
            fn = getattr(lib, name)
            fn.restype, fn.argtypes = result, arguments
        version = lib.mpv_client_api_version()
        print(f"mpv_client_api_version: {version >> 16}.{version & 0xffff}", flush=True)
        if version >> 16 != 2:
            raise RuntimeError("expected libmpv client API major 2")
        handle = lib.mpv_create()
        if not handle:
            raise RuntimeError("mpv_create failed")
        try:
            for key, value in (("config", "no"), ("load-scripts", "no"), ("terminal", "no"),
                               ("vo", "null"), ("ao", "null"), ("hwdec", "no"), ("idle", "yes")):
                if lib.mpv_set_option_string(handle, key.encode(), value.encode()) < 0:
                    raise RuntimeError(f"option rejected: {key}={value}")
            if lib.mpv_initialize(handle) < 0:
                raise RuntimeError("mpv_initialize failed")
            value = lib.mpv_get_property_string(handle, b"mpv-version")
            if not value:
                raise RuntimeError("mpv-version unavailable")
            try:
                actual = ctypes.string_at(value).decode("utf-8")
            finally:
                lib.mpv_free(value)
            if not re.match(r"^mpv v?" + re.escape(components["mpv"]) + r"(?:$|[ -])", actual):
                raise RuntimeError(f"unexpected mpv-version: {actual}")
            print(actual, flush=True)
            command = (ctypes.c_char_p * 3)(b"loadfile", b"av://lavfi:testsrc2=duration=2", None)
            if lib.mpv_command(handle, command) < 0:
                raise RuntimeError("loadfile failed")
            deadline, loaded = time.monotonic() + 25, False
            while time.monotonic() < deadline:
                event = lib.mpv_wait_event(handle, 0.1).contents
                audit(True)
                if event.event_id == 8:  # MPV_EVENT_FILE_LOADED
                    loaded = True
                elif event.event_id == 7:  # MPV_EVENT_END_FILE
                    end = ctypes.cast(event.data, ctypes.POINTER(EndFile)).contents
                    if end.reason != 0 or end.error != 0 or not loaded:
                        raise RuntimeError(f"not clean EOF: loaded={loaded}, reason={end.reason}, error={end.error}")
                    print("ok: lavfi testsrc2 played to EOF with vo=null, ao=null", flush=True)
                    break
                elif event.event_id == 1:
                    raise RuntimeError("shutdown before EOF")
            else:
                raise RuntimeError("no EOF within 25 seconds")
            audit(True)
        finally:
            lib.mpv_terminate_destroy(handle)
        audit(True)
    print("ok: set DLLs mapped from the manifest directory; new modules only from the set or Windows", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--pins", type=Path, default=Path(__file__).with_name("versions.txt"))
    parser.add_argument("--manifest-only", action="store_true")
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    directory = args.directory.resolve()
    problems = check_manifest(directory, args.pins)
    if problems:
        for problem in problems:
            print(f"FAIL manifest: {problem}")
        print("skip load: manifest check failed")
        return 1
    if args.manifest_only:
        print("ok: MANIFEST files, hashes, licences and pinned versions")
        return 0
    if sys.platform != "win32" or ctypes.sizeof(ctypes.c_void_p) != 8:
        print("check requires 64-bit Python on Windows", file=sys.stderr)
        return 2
    if not args.worker:
        # Bound loading, initialization and destruction too, not just wait_event.
        try:
            return subprocess.run([sys.executable, "-B", str(Path(__file__).resolve()),
                                   str(directory), "--pins", str(args.pins.resolve()), "--worker"],
                                  timeout=60, check=False).returncode
        except subprocess.TimeoutExpired:
            print("FAIL: Windows load/play check exceeded 60 seconds")
            return 1
    try:
        play(directory)
        return 0
    except (OSError, RuntimeError, ValueError) as error:
        print(f"FAIL: {error}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
