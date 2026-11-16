#!/usr/bin/env python3
"""Prove the dylibs inside Vela.app are the input set, allowing re-signing.

Used by check-libmpv-macos.py SET --app APP before loading that app's files.
Only temporary thin copies lose signatures; neither app nor set is modified.
"""
import importlib.util
from pathlib import Path
import plistlib
import re
import stat
import subprocess
import tempfile


HERE = Path(__file__).resolve().parent
PINS = HERE / "versions.txt"
spec = importlib.util.spec_from_file_location("macho", HERE / "macos/macho.py")
macho = importlib.util.module_from_spec(spec)
spec.loader.exec_module(macho)


def run(*args):
    result = subprocess.run([str(a) for a in args], check=True, text=True,
                            capture_output=True, timeout=60)
    # codesign's display output goes to stderr, even on success.
    return result.stdout + result.stderr


def inside(root, relative, directory=False):
    path = root
    for component in Path(relative).parts:
        path = path / component
        if path.is_symlink():
            raise ValueError(f"symbolic link in app set: {path}")
    mode = path.stat().st_mode
    if not (stat.S_ISDIR(mode) if directory else stat.S_ISREG(mode)):
        raise ValueError(f"not a regular {'directory' if directory else 'file'}: {path}")
    return path


def inventory(root):
    files = set()
    for path in root.rglob("*"):
        if path.is_symlink():
            raise ValueError(f"symbolic link in app set: {path}")
        mode = path.stat().st_mode
        if stat.S_ISREG(mode):
            files.add(path.relative_to(root).as_posix())
        elif not stat.S_ISDIR(mode):
            raise ValueError(f"not a regular file: {path}")
    return files


def signature_team(details, expected=None):
    team = re.search(r"(?m)^TeamIdentifier=([A-Z0-9]{10})$", details)
    flags = re.search(r"\bflags=0x([0-9a-fA-F]+)\b", details)
    if not team or (expected is not None and team[1] != expected):
        raise ValueError("signature Team ID differs from the app or is missing")
    if not flags or not int(flags[1], 16) & 0x10000:
        raise ValueError("signature lacks hardened runtime")
    if not re.search(r"(?m)^Authority=Developer ID Application:", details):
        raise ValueError("signature lacks Developer ID Application authority")
    if not re.search(r"(?m)^Timestamp=.+$", details):
        raise ValueError("signature lacks secure timestamp")
    return team[1]


def signed(path, expected=None, deep=False):
    run("/usr/bin/codesign", "--verify", "--strict", "--all-architectures",
        *(["--deep"] if deep else []), path)
    for arch in sorted(macho.ARCHES):
        details = run("/usr/bin/codesign", "--display", "--verbose=4", "--architecture", arch, path)
        expected = signature_team(details, expected)
    return expected


def same_code(reference, shipped):
    # Fat offsets/padding can move when a CMS signature replaces an ad-hoc
    # one. Compare each architecture separately after Apple's tool removes
    # BOTH signatures: all remaining Mach-O code, data and load commands must
    # match byte for byte. CDHash itself changes with signing flags/identity.
    with tempfile.TemporaryDirectory(prefix="vela-dylib-code-") as temporary:
        for arch in sorted(macho.ARCHES):
            copies = []
            for label, source in (("input", reference), ("app", shipped)):
                copy = Path(temporary) / f"{label}-{arch}.dylib"
                run("/usr/bin/xcrun", "lipo", source, "-thin", arch, "-output", copy)
                run("/usr/bin/codesign", "--remove-signature", copy)
                copies.append(copy.read_bytes())
            if copies[0] != copies[1]:
                raise ValueError(f"code differs from input set: {shipped.name} ({arch})")


def verify_app(app, reference, pins=PINS, require_signed=True):
    problems = macho.check_manifest(reference, pins)
    if problems:
        raise ValueError("input set: " + "; ".join(problems))
    files, _ = macho.read_manifest(reference)
    frameworks = inside(app, "Contents/Frameworks", directory=True)
    resources = inside(app, "Contents/Resources/libmpv", directory=True)
    info = plistlib.loads(inside(app, "Contents/Info.plist").read_bytes())
    if info.get("LSMinimumSystemVersion") != "11.0":
        raise ValueError("app must declare macOS minimum 11.0")
    executable = info.get("CFBundleExecutable")
    if not isinstance(executable, str) or not macho.safe_name(executable) or "/" in executable:
        raise ValueError("invalid CFBundleExecutable")
    binary = inside(app, "Contents/MacOS/" + executable)
    macho.architectures(run("/usr/bin/xcrun", "lipo", "-archs", binary))
    dylibs = {name for name in files if name.endswith(".dylib")}
    if inventory(frameworks) != dylibs:
        raise ValueError("Frameworks inventory differs from the set's dylibs")
    notices = set(files) - dylibs
    if inventory(resources) != notices | {"MANIFEST"}:
        raise ValueError("Resources/libmpv inventory differs from the set's notices")
    for name in sorted(notices | {"MANIFEST"}):
        if (resources / name).read_bytes() != (reference / name).read_bytes():
            raise ValueError(f"packaged resource differs from input set: {name}")
    team = signed(app, deep=True) if require_signed else None
    for name in sorted(dylibs):
        shipped = frameworks / name
        macho.architectures(run("/usr/bin/xcrun", "lipo", "-archs", shipped))
        if require_signed:
            signed(shipped, team)
        same_code(reference / name, shipped)
    # Includes both slices' @rpath install names, @loader_path run path,
    # dependency closure and macOS 11 deployment floor, at the SHIPPED path.
    macho.validate(frameworks)
    print("ok: app set matches input code and notices" +
          (f"; Developer ID team {team}, runtime, timestamps on both slices" if team else
           "; unsigned app requested (no signing/notarization claim)"), flush=True)
    return frameworks
