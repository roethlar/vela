// LIVE — drives Vela against the owner's REAL Plex server. `npm run e2e:live`.
//
// The Plex path has been the most dangerous code in this repo and the least testable. Eight
// review rounds went into stopping a scan reaching the WRONG server, and NONE of it could
// ever be exercised: the mock is Jellyfin, which has GUID ids and never rebinds, while a
// Plex section key is a server-LOCAL number, so "2" on one server is a different library on
// another. Everything guarding that — `sameSection`, the section binding, provenance — was
// verified by inspection and the owner's playtest, and by nothing else.
//
// This does not close all of that (a rebind needs a SECOND Plex server, which does not
// exist here). What it does close is the part that was pure faith: that the real Plex path
// works end to end against a real server — real section keys, real provenance, a real scan
// — and that it fails the way it is supposed to when the server goes away.
//
// Plex CANNOT be proxied (HTTPS behind a plex.direct certificate), so "the server goes
// away" means the real service is stopped, through a control endpoint on the Mac. It is
// restored on every exit path, including a crash. See scripts/live-control.mjs.
import assert from "node:assert/strict";
import fs from "node:fs";
import { pollUntil, seedConfig } from "../helpers.mjs";

const CREDS = "/tmp/vela-live-creds.json";
let creds;

const control = async (path) => {
  const res = await fetch(`${creds.control}${path}`, { method: "GET" });
  if (!res.ok) throw new Error(`live-control ${path}: ${res.status} ${await res.text()}`);
  return res.json();
};

const banner = (driver) =>
  driver.exec(`return document.querySelector('div.error')?.textContent ?? null`);
const editLine = (driver) =>
  driver.exec(
    `return [...document.querySelectorAll('div.scanerror')].map((e) => e.textContent).join(' | ') || null`,
  );
const notice = (driver) =>
  driver.exec(`return document.querySelector('div.notice')?.textContent ?? null`);
const cardCount = (driver) =>
  driver.exec(`return document.querySelectorAll('main.grid button.poster').length`);

async function ctxMenuOnCard(driver, index, label) {
  await driver.exec(
    `const el = document.querySelectorAll('main.grid button.poster')[${index}];
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  const item = await driver
    .waitFor(`return !!document.querySelector('.ctxmenu')`, "context menu")
    .then(() =>
      driver.find("xpath", `//button[@role='menuitem' and normalize-space(.)='${label}']`),
    );
  await driver.click(item);
}

export default {
  name: "live-plex",

  async seed({ configRoot }) {
    if (!fs.existsSync(CREDS)) throw new Error(`live: no credentials at ${CREDS}`);
    creds = JSON.parse(fs.readFileSync(CREDS, "utf8"));
    if (!creds.plex) throw new Error("live: no saved https Plex server in the Vela config");
    // Plex is restored from TOP-LEVEL config, not from `sources` (lib.rs).
    seedConfig(configRoot, [], creds.plex);
  },

  async cleanup() {
    // Belt and braces: the control server restores Plex on its own exit, but a scenario
    // that dies mid-test must not depend on that.
    try {
      await control("/plex/start");
    } catch {
      /* the launcher's trap is the backstop */
    }
  },

  async run({ driver }) {
    // ── 1. A real Plex server, real libraries, real section keys ────────────
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].length > 1`,
      "the real Plex libraries in the sidebar",
    );
    const section = await driver.exec(
      `const b = [...document.querySelectorAll('button.sideitem')].find((x) => x.textContent.trim() !== 'Home');
       if (b) b.click();
       return b ? b.textContent.trim() : null`,
    );
    assert.ok(section, "the real Plex server must offer at least one library");
    await pollUntil(
      async () => ((await cardCount(driver)) > 0 ? true : null),
      `real items from the real Plex server (${section})`,
    );
    const before = await cardCount(driver);
    assert.equal(await banner(driver), null, "a healthy Plex server produces no banner");

    // ── 2. Scan Library reaches the REAL server ─────────────────────────────
    // The whole provenance/binding apparatus exists to make sure a scan reaches the server
    // the section key came FROM. Until now nothing exercised it against a real Plex at all
    // — the key had never been a real Plex section number in a test. A refusal here
    // ("Scan Library" absent, or a failure notice) means the provenance plumbing has
    // rejected a key its own server just issued.
    await driver.exec(
      `const b = [...document.querySelectorAll('button.sideitem')].find((x) => x.textContent.trim() === ${JSON.stringify(section)});
       const r = b.getBoundingClientRect();
       b.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const scan = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, "the library's context menu")
      .then(() =>
        driver.find("xpath", `//button[@role='menuitem' and normalize-space(.)='Scan Library']`),
      );
    await driver.click(scan);
    await pollUntil(
      async () => {
        const n = await notice(driver);
        return n && n.startsWith("Scan started") ? true : null;
      },
      "the real Plex server accepted a scan of a key it issued itself — the provenance check must not refuse its own server",
    );
    assert.equal(
      await banner(driver),
      null,
      "a scan that succeeded must not leave a failure on the view's banner",
    );

    // ── 3. Plex goes away, and a watch-state edit fails ─────────────────────
    // The owner's playtest, against the server it actually happened on.
    await control("/plex/stop");
    await ctxMenuOnCard(driver, 0, "Mark watched");
    await pollUntil(
      async () => ((await editLine(driver)) ? true : null),
      "the edit's failure, on the edit's own line",
      { timeoutMs: 40000 }, // a dead Plex is a CONNECT timeout, not an instant refusal
    );
    const said = await editLine(driver);

    assert.ok(
      said.includes("Couldn't mark"),
      `the line must NAME the action — against Plex this is the exact defect the owner hit: the edit's line and the grid's banner were the SAME sentence, twice — got ${JSON.stringify(said)}`,
    );
    assert.ok(
      !/https?:\/\//.test(said) && !/token/i.test(said),
      `no url and nothing token-shaped may reach the screen — a Plex url carries ?X-Plex-Token= — got ${JSON.stringify(said)}`,
    );

    // ── 4. ...and the library is STILL THERE ───────────────────────────────
    assert.equal(
      await cardCount(driver),
      before,
      "a failed watch-state repaint must not empty the library — the owner lost every library to this, and with the server down had nothing left to retry with",
    );

    // ── 5. Plex comes back, and Refresh recovers ───────────────────────────
    await control("/plex/start");
    await pollUntil(
      async () => {
        const refresh = await driver.find("css selector", "button.refreshbtn");
        await driver.click(refresh);
        await new Promise((r) => setTimeout(r, 2000));
        return (await cardCount(driver)) > 0 && (await banner(driver)) === null ? true : null;
      },
      "the library recovers once the real server is back",
      { timeoutMs: 90000 }, // Plex takes a while to start serving after systemd says active
    );
    // The EDIT's line is not the refresh's to clear: the action still failed.
    assert.ok(
      await editLine(driver),
      "a refresh repairs the VIEW; it does not un-fail the user's edit",
    );
  },
};
