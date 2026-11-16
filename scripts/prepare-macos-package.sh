#!/usr/bin/env bash
# Prepare the manifest-driven overlay and Tauri's scoped codesign launcher.
# No signing credentials are read here. Add .libmpv/sign-tools to PATH only
# for the Tauri build (release.yml and build-macos.sh do this).
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"
/usr/bin/python3 -I -B packaging/libmpv/write-macos-overlay.py \
  .libmpv/out/libmpv-macos-universal .libmpv/tauri.libmpv-macos.conf.json
mkdir -p .libmpv/sign-tools
# Compute the repo path at invocation time; repo names may contain spaces or
# apostrophes. The launcher's location under .libmpv/ is fixed.
cat > .libmpv/sign-tools/codesign <<'EOF'
#!/bin/bash
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
exec /usr/bin/python3 -I -B "$repo/packaging/libmpv/macos-codesign.py" "$@"
EOF
chmod +x .libmpv/sign-tools/codesign
