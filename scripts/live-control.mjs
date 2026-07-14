// A control endpoint for the LIVE e2e suite, run ON THE MAC by scripts/e2e-live.sh.
//
// The scenarios run on the Linux VM (the only host that can drive the app), but only this
// Mac has SSH to the Plex box. So the VM asks, and this answers.
//
// Deliberately NOT solved by giving the VM its own SSH key on the Plex box: that would be
// persistent access to the owner's server, granted for a test. This lives for the length
// of one run and dies with it.
//
// It is as small as it can be and still be safe:
//   * bound to the UTM host-only address, never 0.0.0.0
//   * an ephemeral port, handed to the scenario through the 0600 creds file
//   * a per-run secret in the path, so nothing else on that network can drive it
//   * exactly two verbs: stop plex, start plex. Nothing takes an argument.
//   * Plex is RESTORED on every exit path, including a crash or a Ctrl-C. A test that
//     dies must never leave the owner's server down.
import http from "node:http";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const run = promisify(execFile);
const HOST = process.argv[2] ?? "192.168.64.1";
const SECRET = process.argv[3];
const PLEX = process.env.VELA_PLEX_SSH ?? "michael@altiera";

if (!SECRET) {
  console.error("live-control: refusing to start without a per-run secret");
  process.exit(1);
}

// The watchdog timer restarts Plex every 5 minutes. It has to be down while a test expects
// Plex to be down, or the test is racing a robot — and it has to come back afterwards.
const sshPlex = (...args) =>
  run("ssh", ["-o", "BatchMode=yes", PLEX, "sudo", "-n", "/usr/bin/systemctl", ...args]);

async function stopPlex() {
  await sshPlex("stop", "plex-watchdog.timer");
  await sshPlex("stop", "plexmediaserver.service");
}
async function startPlex() {
  await sshPlex("start", "plexmediaserver.service");
  await sshPlex("start", "plex-watchdog.timer");
}

let restored = true;
async function restore() {
  if (restored) return;
  try {
    await startPlex();
    restored = true;
    console.error("live-control: plex restored");
  } catch (e) {
    console.error(`live-control: FAILED TO RESTORE PLEX — do it by hand: ${e.message}`);
  }
}

const server = http.createServer(async (req, res) => {
  const ok = (body) => {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify(body));
  };
  try {
    if (req.url === `/${SECRET}/plex/stop`) {
      await stopPlex();
      restored = false;
      return ok({ plex: "stopped" });
    }
    if (req.url === `/${SECRET}/plex/start`) {
      await startPlex();
      restored = true;
      return ok({ plex: "started" });
    }
    res.writeHead(404).end();
  } catch (e) {
    res.writeHead(500, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: String(e.message ?? e) }));
  }
});

server.listen(0, HOST, () => {
  // The launcher reads this line to learn the port.
  console.log(`live-control: ${HOST}:${server.address().port}`);
});

for (const sig of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.on(sig, async () => {
    await restore();
    process.exit(0);
  });
}
process.on("exit", () => {
  // Best effort: the async restore above handles the real cases; this catches the rest.
  if (!restored) {
    try {
      require("node:child_process").execFileSync(
        "ssh",
        ["-o", "BatchMode=yes", PLEX, "sudo", "-n", "/usr/bin/systemctl", "start", "plexmediaserver.service"],
        { stdio: "ignore" },
      );
    } catch {
      /* nothing more we can do from an exit handler */
    }
  }
});
