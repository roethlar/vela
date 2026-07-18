// Mark-watched / mark-unwatched round-trip against the hermetic mock
// Jellyfin server: each context-menu action must hit PlayedItems with the
// right method (POST / DELETE) for the right user/item, flip the server
// state, and the card's watched badge must follow an authoritative refetch
// without the mounted grid disappearing while that refetch is delayed.
import assert from 'node:assert/strict';
import { pollUntil, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock; // seed → run → cleanup share the module instance

export default {
  name: 'markwatched',

  async seed({ configRoot }) {
    mock = await startMockJellyfin(); // default single unwatched movie, no stream
    seedConfig(configRoot, [mockSource(mock)]);
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
    // The fully-watched marker is the .watchedbadge check inside the card
    // (played === true and no mid-resume progress).
    const watchedBefore = await driver.exec(
      `return !!document.querySelector('button.poster[aria-label^="Mock Movie"] .watchedbadge')`,
    );
    assert.equal(watchedBefore, false, 'seeded item starts unwatched');

    // eh-15 still requires POST-REFETCH server authority, but wsp-1 forbids the
    // old `items = []` gap. Delay each listing response, require the card and
    // its confirmed local state continuously during that delay, then gate the
    // final assertion on the response being served.
    const itemsRefetches = () =>
      mock.state.requests.filter(
        (r) => r.method === 'GET' && r.path === `/Users/${mock.userId}/Items`,
      ).length;
    const itemsServed = () =>
      mock.state.served.filter(
        (r) => r.method === 'GET' && r.path === `/Users/${mock.userId}/Items`,
      ).length;
    const holdPresent = async (watched, what) => {
      const deadline = Date.now() + 600;
      while (Date.now() < deadline) {
        const state = await driver.exec(
          `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
           return { present: !!el, watched: !!el?.querySelector('.watchedbadge') };`,
        );
        assert.equal(state.present, true, `${what}: card must remain mounted`);
        assert.equal(state.watched, watched, `${what}: confirmed local badge must remain visible`);
        await new Promise((r) => setTimeout(r, 75));
      }
    };

    // Mark watched via the real context menu.
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const markBtn = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu')
      .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Mark watched']`));
    const itemsBeforeWatch = itemsRefetches();
    const servedBeforeWatch = itemsServed();
    mock.state.itemsDelayMs = 1000;
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
    assert.equal(mock.state.userData.m1.played, true, 'mock server watch state must flip');

    // The refetch must start at the mock, but its delayed response may not cost
    // the user their card/grid. Only after the response is served is the final
    // badge authoritative across a possible merged/server transformation.
    await pollUntil(
      () => itemsRefetches() > itemsBeforeWatch,
      'a server Items refetch after mark-watched',
    );
    await holdPresent(true, 'delayed mark-watched revalidation');
    await pollUntil(
      () => itemsServed() > servedBeforeWatch,
      'the delayed mark-watched Items response',
    );
    await driver.waitFor(
      `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]'); return !!el && !!el.querySelector('.watchedbadge');`,
      'watched badge on the refetched card',
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
    const itemsBeforeUnwatch = itemsRefetches();
    const servedBeforeUnwatch = itemsServed();
    mock.state.itemsDelayMs = 1000;
    await driver.click(unmarkBtn);
    await pollUntil(
      () => mock.state.requests.some((r) => r.method === 'DELETE' && r.path === `/Users/${mock.userId}/PlayedItems/m1`),
      'the PlayedItems DELETE',
    );
    assert.equal(mock.state.userData.m1.played, false, 'mock server watch state must flip back');
    // Mirror of the watched leg. The OLD `!card?...watched` wait was vacuously
    // true while the card was missing during the refresh gap; gate on the
    // refetch, then assert the card is PRESENT and lacks the badge — catching a
    // refetch that resurrected the watched state, dropped the card, or never
    // fired.
    await pollUntil(
      () => itemsRefetches() > itemsBeforeUnwatch,
      'a server Items refetch after mark-unwatched',
    );
    await holdPresent(false, 'delayed mark-unwatched revalidation');
    await pollUntil(
      () => itemsServed() > servedBeforeUnwatch,
      'the delayed mark-unwatched Items response',
    );
    await driver.waitFor(
      `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]'); return !!el && !el.querySelector('.watchedbadge');`,
      'watched badge cleared on the refetched card',
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
