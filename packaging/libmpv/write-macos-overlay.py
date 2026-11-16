#!/usr/bin/env python3
"""Write a Tauri overlay from a checked universal set, never a dylib name list.

Paths are absolute: Tauri resolves resource/framework paths from src-tauri,
regardless of where the generated overlay lives. Keep it under .libmpv/.
"""
import argparse
import importlib.util
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("macho", HERE / "macos/macho.py")
macho = importlib.util.module_from_spec(spec)
spec.loader.exec_module(macho)


def overlay(directory, pins=HERE / "versions.txt"):
    directory = directory.resolve(strict=True)
    problems = macho.check_manifest(directory, pins)
    if problems:
        raise ValueError("input set: " + "; ".join(problems))
    files, _ = macho.read_manifest(directory)
    return {"bundle": {
        "license": "MIT AND GPL-2.0-or-later AND LGPL-2.1-or-later AND ISC AND FTL AND MIT-Modern-Variant AND BSD-2-Clause AND MPL-1.1 AND Apache-2.0",
        "macOS": {
            "minimumSystemVersion": "11.0",
            "hardenedRuntime": True,
            "frameworks": [str(directory / name) for name in sorted(files) if name.endswith(".dylib")],
        },
        # Merge into the base resource map, preserving mpv-scripts/. Dylibs
        # must be framework sign targets, never unsigned resource copies.
        "resources": {
            str(directory / "licenses") + "/": "libmpv/licenses/",
            str(directory / "MANIFEST"): "libmpv/MANIFEST",
        },
    }}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        config = overlay(args.directory)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(config, indent=2) + "\n", encoding="utf-8")
    except (OSError, ValueError) as error:
        parser.exit(1, f"FAIL macOS overlay: {error}\n")


if __name__ == "__main__":
    main()
