// Mark-watched / mark-unwatched round-trip against the hermetic mock
// Jellyfin server: each context-menu action must hit PlayedItems with the
// right method (POST / DELETE) for the right user/item, flip the server
// state, and the card's watched badge must follow — surviving the refetch
// that re-reads server state.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pollUntil } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock; // seed → run → cleanup share the module instance

export default {
  name: 'markwatched',

  async seed({ configRoot }) {
    mock = await startMockJellyfin();
    const configDir = path.join(configRoot, 'config', 'vela');
    fs.mkdirSync(configDir, { recursive: true });
    fs.writeFileSync(
      path.join(configDir, 'config.json'),
      JSON.stringify({
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
      }),
    );
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    // Seeded server source ⇒ authenticated view with its library section.
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock Library')`,
      'mock library in the sidebar',
    );
    const section = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Mock Library']`,
    );
    await driver.click(section);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Mock Movie"]')`,
      'movie card in the grid',
    );
    const watchedBefore = await driver.exec(
      `return document.querySelector('button.poster[aria-label^="Mock Movie"]').classList.contains('watched')`,
    );
    assert.equal(watchedBefore, false, 'seeded item starts unwatched');

    // Mark watched via the real context menu.
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const markBtn = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu')
      .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Mark watched']`));
    await driver.click(markBtn);

    // Server side: the app must have posted PlayedItems for this user/item.
    const posted = await (async () => {
      const deadline = Date.now() + 10000;
      while (Date.now() < deadline) {
        if (mock.state.requests.some((r) => r.method === 'POST' && r.path === `/Users/${mock.userId}/PlayedItems/m1`)) {
          return true;
        }
        await new Promise((r) => setTimeout(r, 200));
      }
      return false;
    })();
    assert.ok(posted, 'app must POST /Users/{u}/PlayedItems/m1');
    assert.equal(mock.state.played, true, 'mock server watch state must flip');

    // UI side: the watched badge appears and survives the refetch (which
    // re-reads the mock's now-Played item state).
    await driver.waitFor(
      `return document.querySelector('button.poster[aria-label^="Mock Movie"]')?.classList.contains('watched')`,
      'watched badge on the card',
    );
    await screenshot('01-watched');

    // And back: mark unwatched must DELETE PlayedItems, flip the server
    // state, and clear the badge.
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const unmarkBtn = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu (unwatch)')
      .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Mark unwatched']`));
    await driver.click(unmarkBtn);
    await pollUntil(
      () => mock.state.requests.some((r) => r.method === 'DELETE' && r.path === `/Users/${mock.userId}/PlayedItems/m1`),
      'the PlayedItems DELETE',
    );
    assert.equal(mock.state.played, false, 'mock server watch state must flip back');
    await driver.waitFor(
      `return !document.querySelector('button.poster[aria-label^="Mock Movie"]')?.classList.contains('watched')`,
      'watched badge to clear',
    );
    await screenshot('02-unwatched');

    // The client must never have broken the Items query contract (eh-12).
    assert.deepEqual(
      mock.state.contractViolations,
      [],
      'client sent an Items request violating the Jellyfin query contract',
    );
  },
};
