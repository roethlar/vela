#!/usr/bin/env python3
"""Unpack and check EVERY NSIS/MSI in a Windows Tauri bundle directory.

python -B packaging/libmpv/check-package-windows.py --set DIR --bundle-dir DIR
--preflight checks the reference set and tools without building/unpacking.
Requires 64-bit Windows Python, 7-Zip and msiexec (no lessmsi). NSIS is read
by 7-Zip, never executed; MSI uses /a to make a temporary administrative
image, not a product installation. Neither check launches Vela.
Exit 0: all checks passed; 1: a prerequisite or package failed; 2: bad usage.
"""
import argparse
import ctypes
import importlib.util
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile


HERE = Path(__file__).resolve().parent
CHECKER = HERE / "check-libmpv-windows.py"
PINS = HERE / "versions.txt"
spec = importlib.util.spec_from_file_location("windows_libmpv", CHECKER)
libmpv = importlib.util.module_from_spec(spec)
spec.loader.exec_module(libmpv)


class PackageError(Exception):
    pass


def regular_tree(root):
    """Never discover the executable or set through a link/reparse point."""
    files = []
    for folder, dirs, names in os.walk(root, followlinks=False):
        for entry in [Path(folder), *(Path(folder) / name for name in dirs + names)]:
            info = entry.lstat()
            if (stat.S_ISLNK(info.st_mode)
                    or getattr(info, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT):
                raise PackageError(f"link or reparse point in payload: {entry}")
            if not (stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode)):
                raise PackageError(f"not a regular file or directory: {entry}")
        files.extend(Path(folder) / name for name in names)
    return files


def verify_tree(root, reference_manifest, pins=PINS):
    # Both bundlers can put installation directories below their extraction
    # root. Locate the app, not a MANIFEST that might live somewhere unrelated.
    # Tauri 2.11.5 -> tauri-utils 2.9.3 platform::resource_dir_from returns
    # exe_dir on Windows; Vela appends libmpv/libmpv-2.dll.
    apps = [p for p in regular_tree(root) if p.name.casefold() == "vela.exe"]
    if len(apps) != 1:
        raise PackageError(f"expected one Vela.exe in payload; found {len(apps)}")
    shipped = apps[0].parent / "libmpv"
    if (shipped / "MANIFEST").read_bytes() != reference_manifest:
        raise PackageError("packaged MANIFEST differs from the input set")
    problems = libmpv.check_manifest(shipped, pins)
    if problems:
        raise PackageError("packaged set: " + "; ".join(problems))
    return shipped


def installers(bundle_dir):
    packages = []
    for folder, pattern in (("nsis", "*-setup.exe"), ("msi", "*.msi")):
        found = sorted((bundle_dir / folder).glob(pattern))
        if not found:
            raise PackageError(f"required Windows installer missing: {folder}/{pattern}")
        for package in found:
            if not package.is_file() or package.is_symlink():
                raise PackageError(f"not an installer file: {package}")
        packages.extend(found)
    return packages


def preflight(reference):
    if sys.platform != "win32" or ctypes.sizeof(ctypes.c_void_p) != 8:
        raise PackageError("requires 64-bit Python on Windows")
    problems = libmpv.check_manifest(reference, PINS)
    if problems:
        raise PackageError("input set: " + "; ".join(problems))
    sevenzip = shutil.which("7z")
    if not sevenzip:
        candidate = Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "7-Zip/7z.exe"
        if candidate.is_file():
            sevenzip = str(candidate)
    msiexec = shutil.which("msiexec.exe")
    if not sevenzip or not msiexec:
        raise PackageError("requires 7-Zip (7z on PATH or in Program Files/7-Zip) and msiexec.exe")
    return sevenzip, msiexec


def unpack(package, destination, sevenzip, msiexec):
    if package.suffix.lower() == ".msi":
        log = destination.parent / "msiexec.log"
        # msiexec documents PROPERTY="value with spaces". Quote the value,
        # not the whole assignment as list2cmdline would. This string goes
        # directly to CreateProcess on Windows; no command shell is involved.
        command = (subprocess.list2cmdline(
            [msiexec, "/a", str(package), "/qn", "/norestart", "/L*V", str(log)])
            + f' TARGETDIR="{destination}"')
    else:
        log = None
        command = [sevenzip, "x", "-y", f"-o{destination}", str(package)]
    # Only status 0 is accepted (7-Zip warnings and MSI reboot-required
    # statuses also fail).
    try:
        subprocess.run(command, check=True, timeout=300)
    except (subprocess.SubprocessError, OSError):
        if log is not None and log.is_file():
            print(log.read_text(encoding="utf-16", errors="replace"), flush=True)
        raise


def check_package(package, reference_manifest, sevenzip, msiexec):
    with tempfile.TemporaryDirectory(prefix="vela-windows-package-") as scratch:
        destination = Path(scratch) / "payload"
        destination.mkdir()
        unpack(package, destination, sevenzip, msiexec)
        shipped = verify_tree(destination, reference_manifest)
        print(f"ok: {package.name}: input MANIFEST and every packaged file match", flush=True)
        # A fresh process per package: a DLL mapped from the NSIS payload must
        # never answer the MSI's check. The existing checker bounds playback,
        # restricts DLL search, checks pinned versions, and audits module paths.
        subprocess.run([sys.executable, "-I", "-B", str(CHECKER), str(shipped)],
                       check=True, timeout=90)
        print(f"ok: {package.name}: packaged set loads and plays to EOF", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--set", dest="reference", type=Path, required=True)
    parser.add_argument("--bundle-dir", type=Path)
    parser.add_argument("--preflight", action="store_true")
    args = parser.parse_args()
    if args.preflight == (args.bundle_dir is not None):
        parser.error("choose either --preflight or --bundle-dir DIR")
    try:
        reference = args.reference.resolve(strict=True)
        sevenzip, msiexec = preflight(reference)
        reference_manifest = (reference / "MANIFEST").read_bytes()
        if args.preflight:
            print("ok: Windows package prerequisites and input MANIFEST")
            return 0
        for package in installers(args.bundle_dir.resolve(strict=True)):
            print(f"checking {package}", flush=True)
            check_package(package, reference_manifest, sevenzip, msiexec)
        return 0
    except (PackageError, OSError, subprocess.SubprocessError) as error:
        print(f"FAIL Windows package: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
