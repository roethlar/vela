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
# `mktemp` is 0600 already; be explicit.
chmod 600 "$TMP"

# A control endpoint, on THIS host, that the VM scenarios can call to stop/start the real
# Plex. The Plex box only trusts this Mac; the alternative would be giving the VM its own
# SSH key on the owner's server, which is persistent access granted for a test. This dies
# with the run, and RESTORES PLEX on every exit path.
SECRET="$(openssl rand -hex 16)"
CONTROL_LOG="$(mktemp -t vela-live-control)"
node scripts/live-control.mjs 192.168.64.1 "$SECRET" >"$CONTROL_LOG" 2>&1 &
CONTROL_PID=$!
cleanup() {
  # Stop the control server FIRST: its own exit handler is what puts Plex back.
  if [ -n "${CONTROL_PID:-}" ]; then kill "$CONTROL_PID" 2>/dev/null || true; wait "$CONTROL_PID" 2>/dev/null || true; fi
  [ -n "${CONTROL_LOG:-}" ] && { grep -q "FAILED TO RESTORE" "$CONTROL_LOG" 2>/dev/null && cat "$CONTROL_LOG" >&2; rm -f "$CONTROL_LOG"; }
  rm -f "$TMP"
  ssh "$VM" "rm -f $CREDS_REMOTE" 2>/dev/null || true
}
trap cleanup EXIT

for _ in $(seq 1 40); do grep -q "^live-control: " "$CONTROL_LOG" 2>/dev/null && break; sleep 0.1; done
CONTROL_ADDR="$(sed -n 's/^live-control: //p' "$CONTROL_LOG" | head -1)"
[ -n "$CONTROL_ADDR" ] || { echo "live: the control server did not start:" >&2; cat "$CONTROL_LOG" >&2; exit 1; }

# Reshape the real config into just what the live scenarios need. Each source is copied
# whole (it carries its own credentials); the scenarios rewrite the endpoints they proxy.
python3 - "$CFG" "$TMP" "http://$CONTROL_ADDR/$SECRET" <<'PY'
import json, sys
cfg, out, control = sys.argv[1], sys.argv[2], sys.argv[3]
c = json.load(open(cfg))
d = {"control": control}

jf = next((s for s in c.get("sources", []) if (s.get("kind") or s.get("type")) == "jellyfin"), None)
if jf and (jf.get("base_url") or jf.get("url")):
    d["jellyfin"] = {"baseUrl": jf.get("base_url") or jf.get("url"), "source": jf}

# Plex is restored from TOP-LEVEL config (auth_token + last_server_*), not from `sources`,
# and only when the scheme is https (lib.rs). It cannot be proxied — it is HTTPS behind a
# plex.direct certificate — so the live Plex scenario stops the REAL server instead, which
# is what the control endpoint is for.
if c.get("auth_token") and c.get("last_server_host") and c.get("last_server_scheme") == "https":
    d["plex"] = {
        "auth_token": c["auth_token"],
        "client_identifier": c.get("client_identifier"),
        "last_server_host": c["last_server_host"],
        "last_server_port": c.get("last_server_port"),
        "last_server_scheme": c["last_server_scheme"],
    }

if not d.get("jellyfin") and not d.get("plex"):
    sys.exit("live: the Vela config has neither a Jellyfin source nor a saved https Plex server")
json.dump(d, open(out, "w"))
PY

# The VM reaches this Mac at the UTM gateway, not at "localhost".
python3 - "$TMP" <<'PY'
import json, sys
p = sys.argv[1]
d = json.load(open(p))
if "jellyfin" in d:
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
