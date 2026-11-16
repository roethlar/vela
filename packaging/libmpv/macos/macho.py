#!/usr/bin/env python3
"""Collect/rewrite Mach-O dependencies, join the two slices, and inventory them.

All subprocesses here are Apple tools. Parsers/judges are pure so the policy
tests can exercise their rejection paths on a host without a macOS SDK.
"""
import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess


ARCHES = {"arm64", "x86_64"}
MINIMUM = "11.0"


def system_path(value):
    # Component boundaries matter; /usr/library and /System-elsewhere aren't OS.
    value = os.path.normpath(value)
    return value.startswith(("/usr/lib/", "/System/"))


def run(*args):
    return subprocess.run([str(a) for a in args], check=True, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout


def apple(tool, *args):
    return run("/usr/bin/xcrun", tool, *args)


def parse_load_commands(text):
    info = {"id": [], "needed": [], "rpaths": [], "minimum": []}
    for block in re.split(r"(?m)^Load command \d+\s*$", text)[1:]:
        cmd = re.search(r"(?m)^\s*cmd (LC_\w+)\s*$", block)
        if not cmd:
            raise ValueError("otool load command has no type")
        cmd = cmd[1]
        if cmd in {"LC_ID_DYLIB", "LC_LOAD_DYLIB", "LC_LOAD_WEAK_DYLIB",
                   "LC_REEXPORT_DYLIB", "LC_LOAD_UPWARD_DYLIB", "LC_LAZY_LOAD_DYLIB"}:
            name = re.search(r"(?m)^\s*name (.+) \(offset \d+\)\s*$", block)
            if not name:
                raise ValueError(f"otool {cmd} has no name")
            info["id" if cmd == "LC_ID_DYLIB" else "needed"].append(name[1])
        elif cmd == "LC_RPATH":
            name = re.search(r"(?m)^\s*path (.+) \(offset \d+\)\s*$", block)
            if not name:
                raise ValueError("otool LC_RPATH has no path")
            info["rpaths"].append(name[1])
        elif cmd in {"LC_BUILD_VERSION", "LC_VERSION_MIN_MACOSX"}:
            key = "minos" if cmd == "LC_BUILD_VERSION" else "version"
            version = re.search(r"(?m)^\s*" + key + r" (\d+(?:\.\d+){1,2})\s*$", block)
            if not version:
                raise ValueError("otool has no macOS deployment version")
            if cmd == "LC_BUILD_VERSION" and not re.search(r"(?m)^\s*platform (?:1|macos)\s*$", block):
                raise ValueError("not a macOS Mach-O slice")
            info["minimum"].append(version[1])
    if len(info["id"]) != 1 or len(info["minimum"]) != 1:
        raise ValueError("expected a dylib with one install name and one deployment version")
    return info


def inspect(file, arch):
    return parse_load_commands(apple("otool", "-arch", arch, "-l", file))


def architectures(text, expected=ARCHES):
    actual = text.split()
    if len(actual) != len(set(actual)) or set(actual) != set(expected):
        raise ValueError(f"wrong architectures: {actual}, expected {sorted(expected)}")


def judge(file, info, names):
    problems = []
    if info["id"] != [f"@rpath/{file}"]:
        problems.append(f"{file}: wrong install name")
    if info["rpaths"] != ["@loader_path"]:
        problems.append(f"{file}: rpaths must be exactly @loader_path")
    if len(info["minimum"]) != 1:
        problems.append(f"{file}: missing deployment version")
    elif tuple(map(int, (info["minimum"][0] + ".0.0").split(".")[:3])) > (11, 0, 0):
        problems.append(f"{file}: requires macOS {info['minimum'][0]}, above {MINIMUM}")
    for dep in info["needed"]:
        if system_path(dep):
            continue
        if not dep.startswith("@rpath/") or dep[7:] not in names or "/" in dep[7:]:
            problems.append(f"{file}: dependency outside the set/system: {dep}")
    return problems


def validate(directory, expected=ARCHES):
    directory = Path(directory)
    names = {p.name for p in directory.glob("*.dylib")}
    if "libmpv.2.dylib" not in names:
        raise ValueError("no libmpv.2.dylib")
    system = set()
    for name in sorted(names):
        file = directory / name
        architectures(apple("lipo", "-archs", file), expected)
        for arch in sorted(expected):
            info = inspect(file, arch)
            problems = judge(name, info, names)
            if problems:
                raise ValueError("; ".join(problems))
            system.update(dep for dep in info["needed"] if system_path(dep))
            # Keep the human-readable dependency evidence too, from both slices.
            print(apple("otool", "-arch", arch, "-L", file), end="", flush=True)
    return system


def collect(prefix, dest, arch):
    prefix, dest = Path(prefix).resolve(), Path(dest)
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    queue, seen = ["libmpv.2.dylib"], set()
    while queue:
        name = queue.pop(0)
        if name in seen:
            continue
        seen.add(name)
        if Path(name).name != name:
            raise ValueError(f"not a flat dylib name: {name}")
        source = prefix / "lib" / name
        if not source.is_file() or source.resolve().parent != prefix / "lib":
            raise ValueError(f"missing dependency inside prefix: {name}")
        architectures(apple("lipo", "-archs", source), {arch})
        info = inspect(source, arch)
        target = dest / name
        shutil.copyfile(source, target)  # dereference SONAME links, never ship links
        target.chmod(0o644)
        edits = ["-id", f"@rpath/{name}"]
        for dep in info["needed"]:
            if system_path(dep):
                continue
            if dep.startswith("@rpath/") and "/" not in dep[7:]:
                child = dep[7:]
            elif Path(dep).is_absolute() and Path(dep).parent == prefix / "lib":
                child = Path(dep).name
            else:
                raise ValueError(f"{name}: dependency outside the set/system: {dep}")
            queue.append(child)
            edits.extend(["-change", dep, f"@rpath/{child}"])
        # Remove build-tree search paths; every image can find its siblings.
        for rpath in info["rpaths"]:
            edits.extend(["-delete_rpath", rpath])
        edits.extend(["-add_rpath", "@loader_path"])
        apple("install_name_tool", *edits, target)
    validate(dest, {arch})


def pins_for_macos(file):
    pins = {}
    for line in Path(file).read_text().splitlines():
        fields = line.split()
        if fields and not fields[0].startswith("#") and (fields[2] == "ship" or fields[0] == "vulkan-headers"):
            pins[fields[0]] = fields[1]
    return pins


def merge(work, dest, recipe):
    work, dest, recipe = Path(work), Path(dest), Path(recipe)
    arm, intel = work / "arm64/stage", work / "x86_64/stage"
    names = {p.name for p in arm.glob("*.dylib")}
    if names != {p.name for p in intel.glob("*.dylib")} or "libmpv.2.dylib" not in names:
        raise ValueError("architecture dependency closures differ or omit libmpv")
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    for name in sorted(names):
        apple("lipo", "-create", arm / name, intel / name, "-output", dest / name)
        # install_name_tool invalidates arm64's linker signature. Renew only
        # after the final lipo/edit; release packaging replaces this ad-hoc sig.
        run("/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", dest / name)
        run("/usr/bin/codesign", "--verify", "--strict", dest / name)
    system = validate(dest)
    texts = {
        "mpv": ["LICENSE.LGPL", "Copyright"],
        "ffmpeg": ["COPYING.LGPLv2.1", "LICENSE.md"],
        "libplacebo": ["LICENSE"], "libass": ["COPYING"],
        "freetype": ["LICENSE.TXT", "docs/FTL.TXT"],
        "harfbuzz": ["COPYING"], "fribidi": ["COPYING"],
        "dav1d": ["COPYING"], "uchardet": ["COPYING"],
        "vulkan-headers": ["LICENSE.md", "LICENSES/Apache-2.0.txt", "LICENSES/MIT.txt"],
    }
    roles = []
    for component, files in texts.items():
        for file in files:
            name = f"licenses/{component}/{file}"
            (dest / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(work / "arm64/src" / component / file, dest / name)
            roles.append(f"licence {component} {name}")
    lua = (work / "arm64/src/lua/src/lua.h").read_text()
    licence = re.search(r"\* Copyright \(C\)[\s\S]+?\*{5,}", lua)
    if not licence:
        raise ValueError("could not extract Lua's licence text")
    (dest / "licenses/lua").mkdir()
    (dest / "licenses/lua/LICENSE").write_text(licence[0] + "\n")
    roles.append("licence lua licenses/lua/LICENSE")
    patch = "licenses/mpv/mpv-sdk-only.patch"
    shutil.copyfile(recipe / "macos/mpv-sdk-only.patch", dest / patch)
    roles.append(f"patch mpv {patch}")
    shutil.copyfile(recipe / "macos/NOTES.txt", dest / "licenses/NOTES.txt")
    pins = pins_for_macos(recipe / "versions.txt")
    if {line.split()[1] for line in roles if line.startswith("licence ")} != set(pins):
        raise ValueError("no licence text collected for a component")
    lines = ["# libmpv set for Vela. Generated by scripts/build-libmpv-macos.sh; do not edit.",
             "arch universal", "platform macos", f"minimum-macos {MINIMUM}"]
    lines += [f"component {k} {v}" for k, v in pins.items()]
    lines += sorted(roles) + [f"system {s}" for s in sorted(system)]
    for file in sorted(dest.rglob("*")):
        if file.is_file():
            content = file.read_bytes()
            lines.append(f"file {hashlib.sha256(content).hexdigest()} {len(content)} {file.relative_to(dest).as_posix()}")
    (dest / "MANIFEST").write_text("\n".join(lines) + "\n")
    print((dest / "MANIFEST").read_text(), end="")


def safe_name(name):
    return (bool(name) and not name.startswith("/") and "\\" not in name and ":" not in name
            and all(p not in ("", ".", "..") for p in name.split("/")))


def read_manifest(directory):
    files, components, licences, patches = {}, {}, [], []
    attributes = {}
    for line in (Path(directory) / "MANIFEST").read_text().splitlines():
        fields = line.split()
        if not fields or fields[0].startswith("#"):
            continue
        kind = fields[0]
        if kind == "file" and len(fields) == 4:
            _, digest, size, name = fields
            if not safe_name(name) or name.lower() == "manifest":
                raise ValueError(f"unsafe file name: {name}")
            if name.casefold() in {n.casefold() for n in files}:
                raise ValueError(f"duplicate file: {name}")
            if not re.fullmatch(r"[a-f0-9]{64}", digest) or not size.isdecimal():
                raise ValueError(f"invalid file record: {name}")
            if not (name.startswith("licenses/") or (name.endswith(".dylib") and "/" not in name)):
                raise ValueError(f"unexpected set file: {name}")
            files[name] = (digest, int(size))
        elif kind == "component" and len(fields) == 3 and fields[1] not in components:
            components[fields[1]] = fields[2]
        elif kind in {"licence", "patch"} and len(fields) == 3:
            (licences if kind == "licence" else patches).append(fields[1:])
        elif kind in {"arch", "platform", "minimum-macos"} and len(fields) == 2 and kind not in attributes:
            attributes[kind] = fields[1]
        elif kind == "system" and len(fields) == 2 and system_path(fields[1]):
            pass
        else:
            raise ValueError(f"invalid MANIFEST record: {line}")
    if attributes != {"arch": "universal", "platform": "macos", "minimum-macos": MINIMUM}:
        raise ValueError("MANIFEST must name macOS universal with minimum 11.0")
    if "libmpv.2.dylib" not in files:
        raise ValueError("MANIFEST does not list libmpv.2.dylib")
    for component, name in licences + patches:
        if (component not in components or name not in files or files[name][1] == 0
                or not name.startswith(f"licenses/{component}/")):
            raise ValueError(f"invalid licence/patch record: {component} {name}")
    for component in components:
        if not any(c == component and not name.endswith(".patch") for c, name in licences):
            raise ValueError(f"no licence text listed for component: {component}")
    if ["mpv", "licenses/mpv/mpv-sdk-only.patch"] not in patches:
        raise ValueError("MANIFEST omits the mpv build patch")
    return files, components


def check_manifest(directory, pins):
    try:
        directory = Path(directory).resolve()
        if (directory / "MANIFEST").is_symlink():
            raise ValueError("symbolic link in set: MANIFEST")
        files, components = read_manifest(directory)
        problems, actual = [], set()
        if components != pins_for_macos(pins):
            problems.append("component versions differ from versions.txt")
        for folder, dirs, names in os.walk(directory, followlinks=False):
            for entry in dirs[:] + names:
                file = Path(folder) / entry
                name = file.relative_to(directory).as_posix()
                mode = file.lstat().st_mode
                if stat.S_ISLNK(mode):
                    problems.append(f"symbolic link in set: {name}")
                    if entry in dirs:
                        dirs.remove(entry)
                    continue
                if entry in dirs:
                    continue
                if not stat.S_ISREG(mode):
                    problems.append(f"not a regular file: {name}")
                    continue
                actual.add(name)
                if name == "MANIFEST":
                    continue
                if name not in files:
                    problems.append(f"not in MANIFEST: {name}")
                else:
                    content = file.read_bytes()
                    if files[name] != (hashlib.sha256(content).hexdigest(), len(content)):
                        problems.append(f"modified: {name}")
        problems += [f"missing: {name}" for name in files.keys() - actual]
        return sorted(problems)
    except (OSError, ValueError, UnicodeError) as error:
        return [str(error)]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["collect", "merge"])
    parser.add_argument("arguments", nargs=3)
    args = parser.parse_args()
    try:
        (collect if args.operation == "collect" else merge)(*args.arguments)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"FAIL: {error}\n{getattr(error, 'stderr', '')}")
