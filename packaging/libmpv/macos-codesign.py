#!/usr/bin/env python3
"""Supply runtime/timestamp on Tauri 2.11.4's framework dylib sign calls.

app.rs marks dylibs is_an_executable=false; sign.rs only enables runtime for
executables. keychain.rs invokes codesign through PATH. Keep its identity,
keychain and all arguments; sign inside out BEFORE Tauri notarizes the app.
All other operations go straight to Apple's codesign, including verification.
"""
import os
from pathlib import Path
import sys


def command(arguments):
    extra = []
    if arguments:
        target = Path(arguments[-1])
        signing = "-s" in arguments or "--sign" in arguments
        if (signing and target.suffix == ".dylib" and target.parent.name == "Frameworks"
                and target.parent.parent.name == "Contents"
                and target.parent.parent.parent.suffix == ".app"):
            extra = ["--options", "runtime", "--timestamp"]
    return ["/usr/bin/codesign", *extra, *arguments]


if __name__ == "__main__":
    os.execv("/usr/bin/codesign", command(sys.argv[1:]))
