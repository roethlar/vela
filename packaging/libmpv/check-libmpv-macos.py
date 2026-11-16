#!/usr/bin/env python3
"""Verify the universal set, then load/play/audit it in both CPU architectures.

Use /usr/bin/python3 (Apple's universal interpreter). The controller starts
fresh arm64 and Rosetta x86_64 workers with every DYLD_* variable removed.
The strict audit exempts only the running interpreter's own installation
(sys.base_prefix: on a hosted runner, Xcode's Python3.framework under
/Applications); a Homebrew library or any other foreign image still fails.
With SET --app Vela.app, prove the signed app against SET first, then load
the untouched dylibs from its Contents/Frameworks in both workers. The
original MANIFEST remains in Resources/libmpv; signing changes dylib hashes.
"""
import argparse
import ctypes
import importlib.util
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time


spec = importlib.util.spec_from_file_location("macho", Path(__file__).with_name("macos") / "macho.py")
macho = importlib.util.module_from_spec(spec)
spec.loader.exec_module(macho)
package_spec = importlib.util.spec_from_file_location(
    "macos_package", Path(__file__).with_name("check-package-macos.py"))
package = importlib.util.module_from_spec(package_spec)
package_spec.loader.exec_module(package)


def clean_environment(environment):
    return {k: v for k, v in environment.items()
            if not k.startswith(("DYLD_", "LD_", "PYTHON"))}


def parse_vmmap(text):
    paths = set()
    for line in text.splitlines():
        if re.match(r"^__TEXT(?:_EXEC)?\s", line):
            match = re.search(r"\s(/.+)$", line)
            if match:
                paths.add(match[1].strip())
    if not paths:
        raise ValueError("vmmap returned no image paths")
    return paths


def check_loaded(directory, listed, modules, executable, require_mpv=True, interpreter=None):
    root = Path(directory).resolve()
    files = {name for name in listed if name.endswith(".dylib")}
    seen, problems = set(), []
    for module in modules:
        path = Path(module).resolve()
        # The executable is not a library, and the check's own interpreter
        # (its framework, its extension modules: sys.base_prefix) is not the
        # set's business: on a hosted runner /usr/bin/python3 is Xcode's
        # Python3.framework under /Applications. Every other image, a
        # Homebrew library the harness might have pulled in included, obeys
        # the same rule as libmpv.
        if str(path) == str(Path(executable).resolve()) or macho.system_path(str(path)):
            continue
        if interpreter is not None and path.is_relative_to(Path(interpreter).resolve()):
            continue
        if path.parent != root:
            problems.append(f"library loaded from outside set/system: {path}")
        elif path.name not in files:
            problems.append(f"loaded library not in MANIFEST: {path}")
        else:
            seen.add(path.name)
    if require_mpv and "libmpv.2.dylib" not in seen:
        problems.append("libmpv.2.dylib is not mapped from the set")
    return sorted(set(problems))


def loaded_images():
    # Supplement vmmap with dyld snapshots during playback, so a temporary
    # image disappearing before the final vmmap cannot conceal a leak.
    process = ctypes.CDLL(None)
    process._dyld_image_count.restype = ctypes.c_uint32
    process._dyld_get_image_name.argtypes = [ctypes.c_uint32]
    process._dyld_get_image_name.restype = ctypes.c_char_p
    images = []
    for index in range(process._dyld_image_count()):
        name = process._dyld_get_image_name(index)
        if not name:
            raise RuntimeError("dyld returned an unnamed image")
        images.append(os.fsdecode(name))
    if not images:
        raise RuntimeError("dyld returned no images")
    return images


class Event(ctypes.Structure):
    _fields_ = [("event_id", ctypes.c_int), ("error", ctypes.c_int),
                ("reply_userdata", ctypes.c_uint64), ("data", ctypes.c_void_p)]


class EndFile(ctypes.Structure):
    _fields_ = [("reason", ctypes.c_int), ("error", ctypes.c_int)]


def play(directory, arch, log_dir, manifest_directory=None):
    if platform.machine() != arch:
        raise RuntimeError(f"worker is {platform.machine()}, expected {arch}")
    if any(key.startswith("DYLD_") for key in os.environ):
        raise RuntimeError("worker inherited DYLD_* variables")
    files, components = macho.read_manifest(manifest_directory or directory)
    images = loaded_images()
    executable = images[0]
    snapshots = set(images)

    def audit(label, require_mpv=True, maps=False):
        snapshots.update(loaded_images())
        if maps:
            result = subprocess.run(["/usr/bin/vmmap", "-w", str(os.getpid())],
                                    text=True, capture_output=True, timeout=15, check=False)
            if log_dir:
                (log_dir / f"{arch}-{label}-vmmap.txt").write_text(result.stdout + result.stderr)
            if result.returncode:
                raise RuntimeError(f"vmmap failed ({result.returncode}): {result.stderr}")
            mapped = parse_vmmap(result.stdout)
            if require_mpv and str(directory / "libmpv.2.dylib") not in {os.path.realpath(p) for p in mapped}:
                raise RuntimeError("vmmap did not see libmpv.2.dylib")
            snapshots.update(mapped)
        problems = check_loaded(directory, files, snapshots, executable, require_mpv,
                                interpreter=sys.base_prefix)
        if log_dir:
            (log_dir / f"{arch}-loaded-images.txt").write_text("\n".join(sorted(snapshots)) + "\n")
        if problems:
            raise RuntimeError("; ".join(problems))

    # Audit the harness too; no arbitrary baseline subtraction that could hide
    # a Homebrew library already loaded before mpv.
    audit("startup", require_mpv=False)
    lib = ctypes.CDLL(str(directory / "libmpv.2.dylib"), mode=os.RTLD_NOW | os.RTLD_LOCAL)
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
    print(f"{arch}: mpv_client_api_version: {version >> 16}.{version & 0xffff}", flush=True)
    if version >> 16 != 2:
        raise RuntimeError("expected libmpv client API major 2")
    audit("load", maps=True)
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
            actual = ctypes.string_at(value).decode()
        finally:
            lib.mpv_free(value)
        if not re.match(r"^mpv v?" + re.escape(components["mpv"]) + r"(?:$|[ -])", actual):
            raise RuntimeError(f"unexpected mpv-version: {actual}")
        print(f"{arch}: {actual}", flush=True)
        command = (ctypes.c_char_p * 3)(b"loadfile", b"av://lavfi:testsrc2=duration=2", None)
        if lib.mpv_command(handle, command) < 0:
            raise RuntimeError("loadfile failed")
        deadline, loaded = time.monotonic() + 25, False
        while time.monotonic() < deadline:
            event = lib.mpv_wait_event(handle, 0.1).contents
            audit("play")
            if event.event_id == 8:
                loaded = True
            elif event.event_id == 7:
                end = ctypes.cast(event.data, ctypes.POINTER(EndFile)).contents
                if end.reason != 0 or end.error != 0 or not loaded:
                    raise RuntimeError(f"not clean EOF: loaded={loaded}, reason={end.reason}, error={end.error}")
                print(f"{arch}: ok: lavfi testsrc2 played to EOF with vo=null, ao=null", flush=True)
                break
            elif event.event_id == 1:
                raise RuntimeError("shutdown before EOF")
        else:
            raise RuntimeError("no EOF within 25 seconds")
        audit("eof", maps=True)
    finally:
        lib.mpv_terminate_destroy(handle)
    audit("destroy")
    print(f"{arch}: ok: every loaded library is in the set's MANIFEST or /usr/lib or /System", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--pins", type=Path, default=Path(__file__).with_name("versions.txt"))
    parser.add_argument("--log-dir", type=Path)
    parser.add_argument("--manifest-only", action="store_true")
    parser.add_argument("--app", type=Path, help="check and play the set INSIDE this app")
    parser.add_argument("--unsigned-app", action="store_true",
                        help="local/secretless app only: omit Developer ID requirements")
    parser.add_argument("--worker", choices=["arm64", "x86_64"], help=argparse.SUPPRESS)
    args = parser.parse_args()
    if (args.unsigned_app and not args.app) or (args.app and args.manifest_only):
        parser.error("--unsigned-app requires --app; --app cannot use --manifest-only")
    directory = args.directory.resolve()
    problems = macho.check_manifest(directory, args.pins)
    if problems:
        for problem in problems:
            print(f"FAIL manifest: {problem}")
        print("skip load: manifest check failed")
        return 1
    if args.manifest_only:
        print("ok: MANIFEST files, hashes, licences, patch and pinned versions")
        return 0
    if sys.platform != "darwin":
        print("check requires macOS and Apple's universal /usr/bin/python3", file=sys.stderr)
        return 2
    if args.log_dir:
        args.log_dir = args.log_dir.resolve()
        args.log_dir.mkdir(parents=True, exist_ok=True)
    try:
        manifest_directory = directory
        if args.app:
            args.app = args.app.resolve(strict=True)
            directory = package.verify_app(args.app, manifest_directory, args.pins,
                                           require_signed=not args.unsigned_app)
        if args.worker:
            play(directory, args.worker, args.log_dir, manifest_directory)
            return 0
        macho.validate(directory)  # lipo + install names + deployment + closure, BOTH slices
        environment = clean_environment(os.environ)
        environment["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
        failures = []
        for arch in ("arm64", "x86_64"):
            command = ["/usr/bin/arch", f"-{arch}", "/usr/bin/python3", "-I", "-B",
                       str(Path(__file__).resolve()), str(manifest_directory), "--pins",
                       str(args.pins.resolve()), "--worker", arch]
            if args.app:
                command += ["--app", str(args.app)]
            if args.unsigned_app:
                command += ["--unsigned-app"]
            if args.log_dir:
                command += ["--log-dir", str(args.log_dir)]
            try:
                result = subprocess.run(command, env=environment, timeout=90, check=False)
                if result.returncode:
                    failures.append(f"{arch}: load/play audit failed ({result.returncode})")
            except subprocess.TimeoutExpired:
                failures.append(f"{arch}: load/play audit exceeded 90 seconds")
        if failures:
            raise RuntimeError("; ".join(failures))
        return 0
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        print(f"FAIL: {error}\n{getattr(error, 'stderr', '')}", flush=True)
        return 1


if __name__ == "__main__":
    sys.exit(main())
