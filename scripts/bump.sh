#!/usr/bin/env bash
# Bump Vela's version whenever the CODE changes — as part of making a code
# change, NOT at build time. A build is only meaningfully unique when the
# source is, so the version (shown in the window footer AND in the bundle
# filename, e.g. Vela_0.1.7_aarch64.dmg) tracks code edits, not builds.
#
# Usage:
#   scripts/bump.sh           # increment the patch (0.1.6 -> 0.1.7)
#   scripts/bump.sh 0.2.0     # set an explicit version
#
# Updates: src-tauri/Cargo.toml, package.json, package-lock.json (npm's two
# root version copies — else the next `npm install` rewrites the tracked
# lockfile), src-tauri/tauri.conf.json, the BUILD_DATE constant, the vela
# entry in src-tauri/Cargo.lock (so `cargo build --locked` / CI stays
# green), and packaging/arch/PKGBUILD's pkgver (pkgrel reset to 1). Run it
# as part of a code change.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - "${1:-}" <<'PY'
import json, re, sys, datetime, pathlib

cargo = pathlib.Path("src-tauri/Cargo.toml")
lock = pathlib.Path("src-tauri/Cargo.lock")
pkg = pathlib.Path("package.json")
conf = pathlib.Path("src-tauri/tauri.conf.json")
cmds = pathlib.Path("src-tauri/src/commands.rs")
pkgbuild = pathlib.Path("packaging/arch/PKGBUILD")

cargo_text = cargo.read_text()
cur = re.search(r'(?m)^version\s*=\s*"([^"]+)"', cargo_text).group(1)

arg = sys.argv[1] if len(sys.argv) > 1 else ""
if arg:
    new = arg
else:
    a, b, c = (cur.split(".") + ["0", "0"])[:3]
    new = f"{a}.{b}.{int(c) + 1}"

# Cargo.toml — only the first (the [package]) version key.
cargo.write_text(re.sub(r'(?m)^version\s*=\s*"[^"]+"', f'version = "{new}"', cargo_text, count=1))

# package.json / tauri.conf.json
for path in (pkg, conf):
    data = json.loads(path.read_text())
    data["version"] = new
    path.write_text(json.dumps(data, indent=2) + "\n")

# Cargo.lock — the vela package entry, so --locked builds stay valid.
if lock.exists():
    lock.write_text(
        re.sub(r'(name = "vela"\nversion = ")[^"]+(")', rf'\g<1>{new}\g<2>', lock.read_text())
    )

# package-lock.json — npm's two root version copies (top-level and
# packages[""]), anchored on the vela name so dependency versions are never
# touched.
locknpm = pathlib.Path("package-lock.json")
if locknpm.exists():
    locknpm.write_text(
        re.sub(
            r'("name": "vela",\s*\n\s*"version": ")[^"]+(")',
            rf"\g<1>{new}\g<2>",
            locknpm.read_text(),
            count=2,
        )
    )

# Arch PKGBUILD — pkgver tracks the app version; pkgrel resets to 1 on a bump.
if pkgbuild.exists():
    pb = pkgbuild.read_text()
    pb = re.sub(r'(?m)^pkgver=.*', f'pkgver={new}', pb)
    pb = re.sub(r'(?m)^pkgrel=.*', 'pkgrel=1', pb)
    pkgbuild.write_text(pb)

# BUILD_DATE (UTC) shown in the footer.
today = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
cmds.write_text(
    re.sub(r'const BUILD_DATE: &str = "[^"]*";', f'const BUILD_DATE: &str = "{today}";', cmds.read_text())
)

print(f"version {cur} -> {new}  (built {today})")
PY
