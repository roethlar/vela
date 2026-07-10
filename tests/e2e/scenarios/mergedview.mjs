// Merged All view across two real server sources (two mock Jellyfin
// instances carrying the same title): the consolidated Movies listing
// dedups to ONE card marked "2 sources", the context menu offers
// "Play from" both backings, each backing plays from its own server's
// stream, and the per-title override persists in merged_overrides.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { pollUntil, makeClips, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mockA;
let mockB;

async function playFromMenu(driver, menuLabel) {
  const before = mpvSocketSnapshot();
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  const item = await driver
    .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu')
    .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='${menuLabel}']`));
  await driver.click(item);
  const mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
  let loaded;
  try {
    loaded = await pollUntil(() => mpv.getProp('path').catch(() => null), `mpv to load via "${menuLabel}"`);
    mpv.quit();
  } finally {
    mpv.close();
  }
  return loaded;
}

export default {
  name: 'mergedview',

  async seed({ configRoot }) {
    // Same title+year on both servers (no provider ids in the mock), so the
    // merged view dedups them by normalized title+year.
    const mediaDir = makeClips(configRoot, ['a.mp4', 'b.mp4']);
    const movie = (file) => [{
      id: 'm1',
      name: 'Mock Movie',
      year: 2020,
      runTimeTicks: 100_000_000,
      mediaFile: path.join(mediaDir, file),
    }];
    mockA = await startMockJellyfin({ movies: movie('a.mp4') });
    mockB = await startMockJellyfin({ movies: movie('b.mp4') });
    seedConfig(configRoot, [
      mockSource(mockA, { id: 'jf-a', name: 'Mock JF A' }),
      mockSource(mockB, { id: 'jf-b', name: 'Mock JF B' }),
    ]);
  },

  async cleanup() {
    await mockA?.close();
    await mockB?.close();
  },

  async run({ driver, screenshot, configRoot }) {
    // Two sources ⇒ the sidebar consolidates into type tabs.
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Movies')`,
      'consolidated Movies tab (two sources)',
    );
    const tab = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Movies']`,
    );
    await driver.click(tab);

    // Dedup: exactly ONE merged card, marked as backed by 2 sources.
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Mock Movie"]')`,
      'merged movie card',
    );
    const view = await driver.exec(
      `const cards = [...document.querySelectorAll('button.poster[aria-label^="Mock Movie"]')];
       return { count: cards.length, tag: cards[0]?.innerText.includes('2 sources') };`,
    );
    assert.equal(view.count, 1, 'the two servers must dedup to one merged card');
    assert.ok(view.tag, 'the merged card must be marked "2 sources"');
    await screenshot('01-merged');

    // Play from each backing: each choice must stream from its own server.
    // The override must persist under the exact canonical key with the
    // chosen source id (eh-14): the backend applies it by exact key, so a
    // wrong-key/wrong-value persist silently loses the user's choice.
    const CANONICAL = 'title:mockmovie|2020'; // canonical_id_of: normalized title + year
    const overrideValue = () => {
      try {
        const cfg = JSON.parse(fs.readFileSync(path.join(configRoot, 'config', 'vela', 'config.json'), 'utf8'));
        return cfg.merged_overrides?.[CANONICAL];
      } catch {
        return undefined;
      }
    };

    const streamA = await playFromMenu(driver, 'Play from Mock JF A');
    assert.ok(
      streamA.startsWith(`http://127.0.0.1:${mockA.port}/Videos/m1/stream`),
      `backing A must play server A's stream, got ${streamA}`,
    );
    await pollUntil(() => overrideValue() === 'jf-a', `the override to persist as ${CANONICAL} → jf-a`);

    const streamB = await playFromMenu(driver, 'Play from Mock JF B');
    assert.ok(
      streamB.startsWith(`http://127.0.0.1:${mockB.port}/Videos/m1/stream`),
      `backing B must play server B's stream, got ${streamB}`,
    );
    await pollUntil(() => overrideValue() === 'jf-b', `the override to flip to ${CANONICAL} → jf-b`);
  },
};
