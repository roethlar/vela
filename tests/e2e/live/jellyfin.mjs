// LIVE — drives Vela against the owner's REAL Jellyfin server. Not hermetic, not part of
// the gating suite: `npm run e2e:live`.
//
// It exists because the owner's manual playtest found THREE defects in two sessions that
// 18 mock scenarios and 24 rounds of code review all missed:
//
//   * the failure line did not say WHAT failed (with Plex, two identical sentences)
//   * the error carried the request url — a user GUID, an item key, and on the Plex side
//     a `?X-Plex-Token=…` waiting to happen
//   * a failed mark-watched EMPTIED the library, so there was nothing left to retry with
//
// Every one of those needed a real server saying real things. The mocks could not.
//
// HOW THE SERVER "GOES AWAY": a TCP proxy in this process forwards to the real Jellyfin,
// and Vela is pointed at the proxy. Killing the proxy is an instant, deterministic
// connection failure — and it never touches the owner's server. (Stopping the real
// Jellyfin would work too, but it is slow, it is rude, and Plex on the other box has a
// watchdog that would restart it mid-test anyway.)
//
// CREDENTIALS: read at run time from a 0600 file the launcher drops on this host, never
// from the repo. Nothing here is committed, printed, or logged.
import assert from "node:assert/strict";
import fs from "node:fs";
import net from "node:net";
import { pollUntil, seedConfig } from "../helpers.mjs";

const CREDS = "/tmp/vela-live-creds.json";

let proxy;
let sockets = new Set();
let creds;
let proxyPort;

// Forward 127.0.0.1:proxyPort -> the real Jellyfin. `stop()` makes the server vanish.
function startProxy(target, port = 0) {
  return new Promise((resolve) => {
    proxy = net.createServer((client) => {
      sockets.add(client);
      const upstream = net.connect(target.port, target.host);
      sockets.add(upstream);
      client.pipe(upstream);
      upstream.pipe(client);
      const bye = () => {
        sockets.delete(client);
        sockets.delete(upstream);
        client.destroy();
        upstream.destroy();
      };
      client.on("error", bye);
      upstream.on("error", bye);
      client.on("close", bye);
      upstream.on("close", bye);
    });
    // The SAME port on a restart: the app is pinned to the base_url it was seeded with,
    // so the server "coming back" means coming back where it was.
    proxy.listen(port, "127.0.0.1", () => resolve(proxy.address().port));
  });
}
// The server "goes offline": stop listening AND drop every live connection, or an
// in-flight keep-alive would sail straight through and the test would prove nothing.
function killProxy() {
  return new Promise((resolve) => {
    for (const s of sockets) s.destroy();
    sockets.clear();
    if (!proxy) return resolve();
    proxy.close(() => {
      proxy = null;
      resolve();
    });
  });
}

const banner = (driver) =>
  driver.exec(`return document.querySelector('div.error')?.textContent ?? null`);
const editLine = (driver) =>
  driver.exec(
    `return [...document.querySelectorAll('div.scanerror')].map((e) => e.textContent).join(' | ') || null`,
  );
const cardCount = (driver) =>
  driver.exec(`return document.querySelectorAll('main.grid button.poster').length`);

async function watchToggle(driver, index, label) {
  await driver.exec(
    `const el = document.querySelectorAll('main.grid button.poster')[${index}];
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  const item = await driver
    .waitFor(`return !!document.querySelector('.ctxmenu')`, "context menu")
    .then(() =>
      driver.find(
        "xpath",
        `//button[@role='menuitem' and normalize-space(.)='${label}']`,
      ),
    );
  await driver.click(item);
}

export default {
  name: "live-jellyfin",

  async seed({ configRoot }) {
    if (!fs.existsSync(CREDS)) {
      throw new Error(
        `live: no credentials at ${CREDS} — run this through \`npm run e2e:live\`, which extracts them from the local Vela config and drops them here 0600`,
      );
    }
    creds = JSON.parse(fs.readFileSync(CREDS, "utf8"));
    const target = new URL(creds.jellyfin.baseUrl);
    proxyPort = await startProxy({
      host: target.hostname,
      port: Number(target.port || 8096),
    });
    // Vela talks to the proxy; the proxy talks to the real server.
    seedConfig(configRoot, [
      {
        ...creds.jellyfin.source,
        base_url: `http://127.0.0.1:${proxyPort}`,
      },
    ]);
  },

  async cleanup() {
    await killProxy();
  },

  async run({ driver }) {
    // ── 1. A real server, a real library ───────────────────────────────────
    await driver.waitFor(
      `return document.readyState === 'complete' && document.querySelectorAll('button.sideitem').length > 0`,
      "the real server's libraries in the sidebar",
    );
    const section = await driver.exec(
      `const b = [...document.querySelectorAll('button.sideitem')].find((x) => x.textContent.trim() !== 'Home');
       if (b) b.click();
       return b ? b.textContent.trim() : null`,
    );
    assert.ok(section, "the real server must offer at least one library");
    await pollUntil(
      async () => ((await cardCount(driver)) > 0 ? true : null),
      `real items from the real server (${section})`,
    );
    const before = await cardCount(driver);

    // ── 2. The server goes away, and the user marks something watched ───────
    // THE OWNER'S PLAYTEST, AUTOMATED. Everything below is a defect a real server found
    // and every mock missed.
    await killProxy();
    await watchToggle(driver, 0, "Mark watched");

    await pollUntil(
      async () => ((await editLine(driver)) ? true : null),
      "the edit's failure, on the edit's own line",
    );
    const said = await editLine(driver);

    assert.ok(
      said.includes("Couldn't mark"),
      `the line must NAME the action — an unlabelled line is indistinguishable from the grid's, and against Plex the two were the SAME sentence twice — got ${JSON.stringify(said)}`,
    );
    assert.ok(
      !/https?:\/\//.test(said),
      `no request url may reach the screen: it carries a user GUID and an item key, and on the Plex path a token — got ${JSON.stringify(said)}`,
    );
    assert.ok(
      !/token/i.test(said),
      `nothing that looks like a token may reach the screen — got ${JSON.stringify(said)}`,
    );

    // ── 3. ...and the library is STILL THERE ───────────────────────────────
    // The user asked to change one item, not to lose their view. Losing it also meant
    // losing any way to retry — there was nothing left to right-click.
    assert.equal(
      await cardCount(driver),
      before,
      "a failed watch-state repaint must not empty the library",
    );

    // ── 4. A second edit REPLACES the first outcome ─────────────────────────
    await watchToggle(driver, 1, "Mark watched");
    await pollUntil(
      async () => {
        const now = await editLine(driver);
        return now && now !== said ? true : null;
      },
      "the newer edit's outcome supersedes the older one — a stale outcome is not an outcome",
    );
    assert.ok(
      !(await editLine(driver)).includes(" | "),
      "one edit, one line: outcomes must not stack",
    );

    // ── 5. The server comes back, and Refresh recovers ─────────────────────
    // Back on the SAME port — the app is pinned to the base_url it was seeded with.
    const back = new URL(creds.jellyfin.baseUrl);
    await startProxy(
      { host: back.hostname, port: Number(back.port || 8096) },
      proxyPort,
    );
    const refresh = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh);
    await pollUntil(
      async () => ((await banner(driver)) === null ? true : null),
      "the view's banner clears once the server is back",
    );
    await pollUntil(
      async () => ((await cardCount(driver)) > 0 ? true : null),
      "and the library reloads from the real server",
    );
    // The EDIT's line is NOT the refresh's to clear — it is an action's outcome, and the
    // action still failed. Only a newer edit supersedes it. (Scan Library has behaved this
    // way since r15; this is the same rule.)
    assert.ok(
      await editLine(driver),
      "a refresh repairs the VIEW; it does not un-fail the user's edit",
    );
  },
};
