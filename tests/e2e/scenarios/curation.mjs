// Continue Watching curation: "Remove from Continue Watching" (hero context
// menu) tombstones the item and empties the hero; the tombstone survives an
// app restart; replaying the item clears it and the hero returns.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import playback from './playback.mjs';

const HERO_CLIP = `[aria-label="Continue watching"] [aria-label^="Play E2E Clip"]`;
const EMPTY_HOME = 'Nothing on your home screen yet';

async function pollUntil(fn, what, { timeoutMs = 15000, intervalMs = 250 } = {}) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await fn();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, intervalMs));
  }
}

// Lean play-and-quit: get an unfinished session into recents. The playback
// scenario owns the detailed IPC assertions; here mpv is just a means.
async function playClipAndQuit(driver) {
  const before = mpvSocketSnapshot();
  const card = await driver.find('css selector', 'button.poster[aria-label^="E2E Clip"]');
  await driver.click(card);
  const mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
  try {
    await pollUntil(
      () => mpv.getProp('time-pos').then((t) => t > 0.5).catch(() => false),
      'playback to progress',
    );
    await mpv.setProp('time-pos', 6);
    await new Promise((r) => setTimeout(r, 1500)); // let Vela observe ≥6s
    mpv.quit();
  } finally {
    mpv.close();
  }
}

async function openLibraryGrid(driver) {
  await driver.waitFor(
    `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'E2E Media')`,
    'seeded source in the sidebar',
  );
  const section = await driver.find(
    'xpath',
    `//button[contains(@class,'sideitem') and normalize-space(.)='E2E Media']`,
  );
  await driver.click(section);
  await driver.waitFor(
    `return !!document.querySelector('button.poster[aria-label^="E2E Clip"]')`,
    'clip card in the grid',
  );
}

async function goHome(driver) {
  const home = await driver.find(
    'xpath',
    `//button[contains(@class,'sideitem') and normalize-space(.)='Home']`,
  );
  await driver.click(home);
}

export default {
  name: 'curation',
  seed: playback.seed, // same local folder + generated clip + displayless mpv

  async run({ driver, screenshot, configRoot, restart }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const readCfg = () => JSON.parse(fs.readFileSync(configFile, 'utf8'));
    const stampedRecent = () => {
      try {
        return (readCfg().recents?.[0]?.item?.viewOffsetMs ?? 0) > 0;
      } catch {
        return false;
      }
    };

    // An unfinished play lands the clip in the hero.
    await openLibraryGrid(driver);
    await playClipAndQuit(driver);
    await pollUntil(stampedRecent, 'the recents position stamp');
    await goHome(driver);
    await driver.waitFor(`return !!document.querySelector('${HERO_CLIP}')`, 'clip in the hero');

    // Remove from Continue Watching via the hero's real context menu.
    await driver.exec(
      `const el = document.querySelector('${HERO_CLIP}');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const removeBtn = await driver.waitFor(
      `return !!document.querySelector('.ctxmenu')`,
      'hero context menu',
    ).then(() =>
      driver.find(
        'xpath',
        `//button[@role='menuitem' and normalize-space(.)='Remove from Continue Watching']`,
      ),
    );
    await driver.click(removeBtn);

    // Hero empties (hub-less home falls back to the empty state) and the
    // tombstone is persisted.
    await driver.waitFor(
      `return !document.querySelector('${HERO_CLIP}') && document.body.innerText.includes('${EMPTY_HOME}')`,
      'hero to empty after removal',
    );
    await pollUntil(
      () => (readCfg().hidden_from_continue?.length ?? 0) > 0,
      'the tombstone in config.json',
    );
    await screenshot('01-removed');

    // The removal survives an app restart (recents entry still exists, so
    // only the tombstone keeps it out of the hero).
    const { execSync } = await import('node:child_process');
    const appPid = () => execSync('pgrep -x vela || true').toString().trim();
    const pidBefore = appPid();
    assert.ok(pidBefore, 'app process should be findable before restart');
    await restart();
    const pidAfter = appPid();
    assert.ok(
      pidAfter && pidAfter !== pidBefore,
      `restart must relaunch the app (pid ${pidBefore} → ${pidAfter})`,
    );
    await driver.waitFor(
      `return document.readyState === 'complete' && document.body.innerText.includes('${EMPTY_HOME}')`,
      'empty home after restart',
    );
    const heroAfterRestart = await driver.exec(`return !!document.querySelector('${HERO_CLIP}')`);
    assert.equal(heroAfterRestart, false, 'removed item must stay out of the hero across restart');
    assert.ok((readCfg().hidden_from_continue?.length ?? 0) > 0, 'tombstone must survive restart');

    // Replaying the item clears the tombstone and the hero returns.
    await openLibraryGrid(driver);
    await playClipAndQuit(driver);
    await pollUntil(
      () => (readCfg().hidden_from_continue?.length ?? 0) === 0,
      'the tombstone to clear on replay',
    );
    await goHome(driver);
    await driver.waitFor(`return !!document.querySelector('${HERO_CLIP}')`, 'clip back in the hero');
    await screenshot('02-restored');
  },
};
