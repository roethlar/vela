// Merged All view across two real sources (mock Jellyfin + local folder
// carrying the same title): the consolidated Movies listing dedups to ONE
// card marked "2 sources", the context menu offers "Play from" both
// backings, each backing plays from its own source (local path vs mock
// HTTP stream), and the per-title override persists in merged_overrides.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { pollUntil, makeClips } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const CLIP = 'Mock Movie (2020).mp4'; // parses to title "Mock Movie", year 2020 — matches the mock item

let mock;

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
  const { createConnection } = await import('node:net');
  return loaded;
}

export default {
  name: 'mergedview',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, [CLIP]);
    mock = await startMockJellyfin({
      runTimeTicks: 100_000_000,
      mediaFile: path.join(mediaDir, CLIP),
    });
    const configDir = path.join(configRoot, 'config', 'vela');
    fs.mkdirSync(configDir, { recursive: true });
    fs.writeFileSync(
      path.join(configDir, 'config.json'),
      JSON.stringify({
        local_folders: [{ id: 'e2e-local', name: 'E2E Media', path: mediaDir, kind: 'movie' }],
        sources: [
          {
            id: 'jf-mock',
            kind: 'jellyfin',
            name: 'Mock JF',
            base_url: `http://127.0.0.1:${mock.port}`,
            access_token: 'mock-token',
            user_id: mock.userId,
            device_id: 'e2e-device',
          },
        ],
        mpv_extra_args: '--vo=null\n--ao=null',
      }),
    );
  },

  async cleanup() {
    await mock?.close();
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
    assert.equal(view.count, 1, 'the two sources must dedup to one merged card');
    assert.ok(view.tag, 'the merged card must be marked "2 sources"');
    await screenshot('01-merged');

    // Play from each backing: the local copy plays the file path, the
    // server copy plays the mock stream — same card, routed per choice.
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

    const localPath = await playFromMenu(driver, 'Play from Local');
    assert.equal(localPath, path.join(configRoot, 'media', CLIP));
    await pollUntil(() => overrideValue() === 'local', `the override to persist as ${CANONICAL} → local`);

    const streamUrl = await playFromMenu(driver, 'Play from Mock JF');
    assert.ok(
      streamUrl.startsWith(`http://127.0.0.1:${mock.port}/Videos/m1/stream`),
      `server backing must play the mock stream, got ${streamUrl}`,
    );
    await pollUntil(() => overrideValue() === 'jf-mock', `the override to flip to ${CANONICAL} → jf-mock`);
  },
};
