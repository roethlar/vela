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
# The venue is the current-main worktree; the older clone beside it is kept for
# its stash and is NOT updated (see .agents/machines.md).
VM_REPO="${VELA_E2E_VM_REPO:-~/dev/vela-main}"
STATE="$HOME/Library/Application Support/com.vela.vela"
CFG="$STATE/config.json"
# Active connections moved out of config.json into a private connections.json
# when the config split landed (slice 1 of config-integrity-recovery). Both
# layouts are read below: a pre-split config keeps its connections in
# `config.json.sources`, a post-split one in `connections.json.sources`.
CONNS="$STATE/connections.json"
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
python3 - "$CFG" "$CONNS" "$TMP" "http://$CONTROL_ADDR/$SECRET" <<'PY'
import json, os, sys
from urllib.parse import urlsplit
cfg, conns, out, control = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
c = json.load(open(cfg))
d = {"control": control}

# Connections live in connections.json after the config split and in
# config.json before it. Prefer the split file when present; a pre-split
# install has none. Either way the record shape is the same SourceConfig.
sources = []
if os.path.exists(conns):
    try:
        sources = json.load(open(conns)).get("sources") or []
    except Exception as e:
        sys.exit(f"live: connections.json is present but unreadable ({e})")
if not sources:
    sources = c.get("sources") or []

jf = next((s for s in sources if (s.get("kind") or s.get("type")) == "jellyfin"), None)
if jf and (jf.get("base_url") or jf.get("url")):
    d["jellyfin"] = {"baseUrl": jf.get("base_url") or jf.get("url"), "source": jf}

# Plex cannot be proxied — it is HTTPS behind a plex.direct certificate — so the
# live Plex scenario stops the REAL server instead, which is what the control
# endpoint is for. Host/port/scheme come from the connection's own base_url;
# older configs kept the same facts in top-level last_server_* fields, which are
# still honoured so a pre-split install keeps working.
# Fail closed on more than one. live-control stops and starts ONE fixed host, so
# seeding Vela against a different Plex would time out the outage assertions and
# briefly disrupt an unrelated server for no reason.
plex_all = [s for s in sources if (s.get("kind") or s.get("type")) == "plex"]
if len(plex_all) > 1:
    wanted = os.environ.get("VELA_LIVE_PLEX_ID")
    plex_all = [s for s in plex_all if s.get("id") == wanted] if wanted else []
    if len(plex_all) != 1:
        sys.exit(
            "live: several Plex connections are configured and live-control manages "
            "exactly one host — set VELA_LIVE_PLEX_ID to the connection id of the "
            "server it controls"
        )
plex = plex_all[0] if plex_all else None
if plex and plex.get("access_token") and plex.get("base_url"):
    parts = urlsplit(plex["base_url"])
    if parts.scheme == "https" and parts.hostname:
        d["plex"] = {
            "auth_token": plex["access_token"],
            "client_identifier": plex.get("device_id"),
            "last_server_host": parts.hostname,
            "last_server_port": parts.port or 32400,
            "last_server_scheme": parts.scheme,
        }
if "plex" not in d and c.get("auth_token") and c.get("last_server_host") \
        and c.get("last_server_scheme") == "https":
    d["plex"] = {
        "auth_token": c["auth_token"],
        "client_identifier": c.get("client_identifier"),
        "last_server_host": c["last_server_host"],
        "last_server_port": c.get("last_server_port"),
        "last_server_scheme": c["last_server_scheme"],
    }

if not d.get("jellyfin") and not d.get("plex"):
    sys.exit(
        "live: no Jellyfin source and no https Plex connection found in "
        "connections.json or config.json — connect a server in the app first"
    )
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
