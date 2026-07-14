#!/usr/bin/env bash
# Run the LIVE e2e suite: Vela driven against the owner's REAL servers, on the Linux VM
# (the only host that can drive the app — tauri-driver has no macOS support).
#
# Not hermetic. Never part of the gating suite. It exists because the owner's manual
# playtest kept finding defects that 18 mock scenarios and 24 review rounds did not — the
# error text, the leaked url, the emptied library. Those needed a real server.
#
# CREDENTIALS: extracted from the local Vela config at run time, written 0600 to the VM's
# /tmp, and DELETED afterwards. They are never printed, never logged, never committed.
set -euo pipefail

VM="${VELA_E2E_VM:-michael@192.168.64.5}"
VM_REPO="${VELA_E2E_VM_REPO:-~/dev/vela}"
CFG="$HOME/Library/Application Support/com.vela.vela/config.json"
CREDS_REMOTE="/tmp/vela-live-creds.json"

[ -f "$CFG" ] || { echo "live: no Vela config at $CFG — configure a server in the app first" >&2; exit 1; }

TMP="$(mktemp -t vela-live)"
# `mktemp` is 0600 already; be explicit, and make sure it dies with us however we exit.
chmod 600 "$TMP"
cleanup() {
  rm -f "$TMP"
  ssh "$VM" "rm -f $CREDS_REMOTE" 2>/dev/null || true
}
trap cleanup EXIT

# Reshape the real config into just what the live scenario needs. The Jellyfin source is
# copied whole (it carries its own access_token / api_key) with the base_url rewritten by
# the scenario to point at its proxy.
python3 - "$CFG" "$TMP" <<'PY'
import json, sys
cfg, out = sys.argv[1], sys.argv[2]
c = json.load(open(cfg))
jf = next((s for s in c.get("sources", []) if (s.get("kind") or s.get("type")) == "jellyfin"), None)
if not jf:
    sys.exit("live: no Jellyfin source in the Vela config — the live suite needs one "
             "(Plex cannot be proxied: it is HTTPS behind a plex.direct certificate)")
base = jf.get("base_url") or jf.get("url")
if not base:
    sys.exit("live: the Jellyfin source has no base_url")
json.dump({"jellyfin": {"baseUrl": base, "source": jf}}, open(out, "w"))
PY

# The VM reaches this Mac at the UTM gateway, not at "localhost".
python3 - "$TMP" <<'PY'
import json, sys
p = sys.argv[1]
d = json.load(open(p))
u = d["jellyfin"]["baseUrl"]
for host in ("localhost", "127.0.0.1"):
    u = u.replace(f"//{host}:", "//192.168.64.1:")
d["jellyfin"]["baseUrl"] = u
json.dump(d, open(p, "w"))
PY

scp -q "$TMP" "$VM:$CREDS_REMOTE"
ssh "$VM" "chmod 600 $CREDS_REMOTE"

# shellcheck disable=SC2029
ssh "$VM" "bash -lc 'cd $VM_REPO && npm run e2e -- --live ${*:-}'"
