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
const SECTION = "Movies";
const TARGET = "12 Years a Slave";
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
const targetState = (driver) =>
  driver.exec(
    `const cards = [...document.querySelectorAll('main.grid button.poster')];
     const matches = cards.filter((card) => card.title === ${JSON.stringify(TARGET)});
     return {
       gridCount: cards.length,
       matches: matches.length,
       label: matches[0]?.getAttribute('aria-label') ?? null,
     }`,
  );

async function targetMenuItem(driver, label) {
  await driver.exec(
    `const matches = [...document.querySelectorAll('main.grid button.poster')]
       .filter((card) => card.title === ${JSON.stringify(TARGET)});
     if (matches.length !== 1) throw new Error('expected one exact target card');
     const el = matches[0];
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  const item = await driver
    .waitFor(`return !!document.querySelector('.ctxmenu')`, "context menu")
    .then(() =>
      driver.find("xpath", `//button[@role='menuitem' and normalize-space(.)='${label}']`),
    );
  return item;
}

async function closeContextMenu(driver) {
  const backdrop = await driver.find("css selector", ".menubackdrop");
  await driver.click(backdrop);
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
      `const b = [...document.querySelectorAll('button.sideitem')]
         .find((x) => x.textContent.trim() === ${JSON.stringify(SECTION)});
       if (b) b.click();
       return b ? b.textContent.trim() : null`,
    );
    assert.equal(section, SECTION, `the real Plex server must offer the ${SECTION} library`);
    await pollUntil(
      async () => {
        const target = await targetState(driver);
        return target.matches === 1 ? target : null;
      },
      `${TARGET} from the real Plex ${SECTION} library`,
    );
    const initialTarget = await targetState(driver);
    assert.equal(initialTarget.matches, 1, `exactly one ${TARGET} card is visible`);
    assert.ok(initialTarget.label, `${TARGET} has a stable accessible identity`);
    const before = initialTarget.gridCount;
    assert.equal(await banner(driver), null, "a healthy Plex server produces no banner");
    await targetMenuItem(driver, "Mark watched");
    await closeContextMenu(driver);

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
    const markWatched = await targetMenuItem(driver, "Mark watched");
    await driver.click(markWatched);
    const said = await pollUntil(
      async () => {
        const target = await targetState(driver);
        assert.equal(target.matches, 1, `${TARGET} must never disappear while the edit fails`);
        assert.equal(
          target.label,
          initialTarget.label,
          `${TARGET}'s identity must remain unchanged while the edit fails`,
        );
        return (await editLine(driver)) || null;
      },
      "the edit's failure, on the edit's own line",
      { timeoutMs: 40000, intervalMs: 200 }, // dead Plex is a CONNECT timeout
    );

    assert.ok(
      said.includes("Couldn't mark"),
      `the line must NAME the action — against Plex this is the exact defect the owner hit: the edit's line and the grid's banner were the SAME sentence, twice — got ${JSON.stringify(said)}`,
    );
    assert.ok(
      !/https?:\/\//.test(said) && !/token/i.test(said),
      `no url and nothing token-shaped may reach the screen — a Plex url carries ?X-Plex-Token= — got ${JSON.stringify(said)}`,
    );
    assert.equal(
      await banner(driver),
      null,
      "the failed edit makes no listing request and therefore no view failure",
    );

    // ── 4. ...and the library is STILL THERE ───────────────────────────────
    const afterFailure = await targetState(driver);
    assert.equal(afterFailure.matches, 1, `${TARGET} remains after the named failure`);
    assert.equal(afterFailure.label, initialTarget.label, `${TARGET}'s identity remains exact`);
    assert.equal(
      afterFailure.gridCount,
      before,
      "the failed edit must not change the loaded grid's cardinality",
    );

    // ── 5. Plex comes back, and Refresh recovers ───────────────────────────
    await control("/plex/start");
    const beforeRefresh = await targetState(driver);
    assert.equal(beforeRefresh.matches, 1, `${TARGET} remains cached while Plex restarts`);
    assert.equal(beforeRefresh.label, initialTarget.label, `${TARGET} remains exact before Refresh`);
    let refreshAttempts = 0;
    await pollUntil(
      async () => {
        const refreshReady = await driver.exec(
          `const b = document.querySelector('button.refreshbtn'); return !!b && !b.disabled`,
        );
        if (refreshReady) {
          const refresh = await driver.find("css selector", "button.refreshbtn");
          await driver.click(refresh);
          refreshAttempts++;
          await new Promise((r) => setTimeout(r, 2000));
        }
        const target = await targetState(driver);
        const settled = await driver.exec(
          `const b = document.querySelector('button.refreshbtn'); return !!b && !b.disabled`,
        );
        return refreshAttempts > 0 &&
          settled &&
          target.matches === 1 &&
          target.label === initialTarget.label &&
          (await banner(driver)) === null
          ? true
          : null;
      },
      `${TARGET} remains unwatched after Plex returns and Refresh succeeds`,
      { timeoutMs: 90000 }, // Plex takes a while to start serving after systemd says active
    );
    // The EDIT's line is not the refresh's to clear: the action still failed.
    assert.ok(
      await editLine(driver),
      "a refresh repairs the VIEW; it does not un-fail the user's edit",
    );
    await targetMenuItem(driver, "Mark watched");
    await closeContextMenu(driver);
  },
};
