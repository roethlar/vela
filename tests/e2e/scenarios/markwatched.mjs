// Mark-watched / mark-unwatched round-trip against the hermetic mock
// Jellyfin server: each context-menu action must hit PlayedItems with the
// right method (POST / DELETE) for the right user/item, flip the server
// state, and the card's watched badge must follow — surviving the refetch
// that re-reads server state.
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

    // eh-15: prove the badge reflects POST-REFETCH server state. setWatched
    // (src/routes/+page.svelte) does an optimistic item.played mutation, then
    // refreshWatchState() → resetAndLoad() which empties the grid (items = [])
    // and refetches the server listing. The OLD unwatch wait `!card?...watched`
    // was vacuously true while the card was missing in that refresh gap, and
    // neither leg proved a refetch actually fired (a skipped refetch would leave
    // the optimistic mutation as the only evidence). Gate each assertion on a
    // LATER /Users/{u}/Items listing refetch reaching the mock (distinct from
    // the single-item /Users/{u}/Items/m1) — which both proves the refetch ran
    // and, by then, has cleared the pre-refetch grid — then assert the card is
    // PRESENT with the expected badge.
    const itemsRefetches = () =>
      mock.state.requests.filter(
        (r) => r.method === 'GET' && r.path === `/Users/${mock.userId}/Items`,
      ).length;

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

    // UI side: prove the badge came from the server refetch, not a skipped
    // refetch leaving the optimistic mutation behind. Wait for the mark action's
    // /Users/{u}/Items refetch to reach the mock, then assert the card is PRESENT
    // and watched — a refetch that served stale Played:false, dropped the card,
    // or never fired fails here (eh-15).
    await pollUntil(
      () => itemsRefetches() > itemsBeforeWatch,
      'a server Items refetch after mark-watched',
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
