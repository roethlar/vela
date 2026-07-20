// Merged All view across two real server sources (two mock Jellyfin
// instances carrying the same title): the consolidated Movies listing
// dedups to ONE card marked "2 sources", the context menu offers
// "Play Version" exposes both backings, each backing plays from its own server's
// stream, title-level watch edits reach both servers, and clean EOF fans out once.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { holdsFor, pollUntil, makeClips, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mockA;
let mockB;

async function playFromMenu(driver, menuLabel, { finish = false } = {}) {
  const before = mpvSocketSnapshot();
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu');
  await driver.click(
    await driver.find(
      'xpath',
      `//button[@role='menuitem' and normalize-space(.)='Play Version']`,
    ),
  );
  await driver.waitFor(
    `return !!document.querySelector('[role="group"][aria-label="Play Version"]')`,
    'Play Version submenu',
  );
  const item = await driver.find(
      'xpath',
      `//*[@role='group' and @aria-label='Play Version']//button[@role='menuitem' and (` +
        `normalize-space(.)='${menuLabel}' or normalize-space(.)='Resume on ${menuLabel}')]`,
  );
  await driver.click(item);
  const mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
  let loaded;
  try {
    loaded = await pollUntil(() => mpv.getProp('path').catch(() => null), `mpv to load via "${menuLabel}"`);
    if (finish) {
      await mpv.setProp('pause', true);
      await mpv.setProp('time-pos', 9.2);
      await mpv.setProp('pause', false);
    } else {
      mpv.quit();
    }
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
    // Both copies start watched so one title-level unwatch can prove that the
    // command reaches both physical servers.
    mockA.state.userData.m1.played = true;
    mockB.state.userData.m1.played = true;
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
    assert.equal(
      await driver.exec(
        `return !!document.querySelector('button.poster[aria-label^="Mock Movie"] .watchedbadge')`,
      ),
      true,
      'either watched backing makes the merged card watched',
    );
    await screenshot('01-merged');

    // Title-level Mark unwatched must independently reach both backings.
    const listingArrivals = (mock) =>
      mock.state.requests.filter(
        (r) => r.method === 'GET' && r.path === `/Users/${mock.userId}/Items`,
      ).length;
    const listingServed = (mock) =>
      mock.state.served.filter(
        (r) => r.method === 'GET' && r.path === `/Users/${mock.userId}/Items`,
      ).length;
    const beforeA = { arrived: listingArrivals(mockA), served: listingServed(mockA) };
    const beforeB = { arrived: listingArrivals(mockB), served: listingServed(mockB) };
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const unwatch = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'merged context menu')
      .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Mark unwatched']`));
    await driver.click(unwatch);
    await pollUntil(
      () =>
        mockA.state.requests.some(
          (r) => r.method === 'DELETE' && r.path === `/Users/${mockA.userId}/PlayedItems/m1`,
        ) && mockB.state.requests.some(
          (r) => r.method === 'DELETE' && r.path === `/Users/${mockB.userId}/PlayedItems/m1`,
        ),
      'both merged backings to receive Mark unwatched',
    );
    assert.equal(mockA.state.userData.m1.played, false, 'backing A must become unwatched');
    assert.equal(mockB.state.userData.m1.played, false, 'backing B must become unwatched');
    await pollUntil(
      () =>
        listingArrivals(mockA) > beforeA.arrived && listingArrivals(mockB) > beforeB.arrived
          ? true
          : null,
      'fresh merged listing requests after Mark unwatched',
    );
    await pollUntil(
      () =>
        listingServed(mockA) > beforeA.served && listingServed(mockB) > beforeB.served
          ? true
          : null,
      'fresh merged listing responses after Mark unwatched',
    );
    await driver.waitFor(
      `return !document.querySelector('button.poster[aria-label^="Mock Movie"] .watchedbadge')`,
      'merged watched badge cleared after both backings settle',
    );
    await screenshot('02-title-level-unwatched');

    // One offline backing must not undo the healthy server. The action-owned
    // warning names only the failed source and the authoritative merged card
    // retains the successful watched state.
    mockB.state.unauthNextPlayed = true;
    const beforePartialA = { arrived: listingArrivals(mockA), served: listingServed(mockA) };
    const beforePartialB = { arrived: listingArrivals(mockB), served: listingServed(mockB) };
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const watch = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'merged context menu for partial watch')
      .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Mark watched']`));
    await driver.click(watch);
    await driver.waitFor(
      `const warning = document.querySelector('.editwarning');
       return warning?.textContent.includes('1 of 2 sources') && warning.textContent.includes('Mock JF B');`,
      'non-destructive partial-success warning naming Mock JF B',
    );
    assert.equal(mockA.state.userData.m1.played, true, 'healthy backing success must be retained');
    assert.equal(mockB.state.userData.m1.played, false, 'offline backing keeps its older state');
    await pollUntil(
      () =>
        listingServed(mockA) > beforePartialA.served && listingServed(mockB) > beforePartialB.served &&
        listingArrivals(mockA) > beforePartialA.arrived && listingArrivals(mockB) > beforePartialB.arrived,
      'authoritative refresh after partial watched success',
    );
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Mock Movie"] .watchedbadge')`,
      'successful backing keeps the merged watched badge',
    );
    await screenshot('03-partial-watch-warning');

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

    const streamA = await playFromMenu(driver, 'Mock JF A');
    assert.ok(
      streamA.startsWith(`http://127.0.0.1:${mockA.port}/Videos/m1/stream`),
      `backing A must play server A's stream, got ${streamA}`,
    );
    await pollUntil(() => overrideValue() === 'jf-a', `the override to persist as ${CANONICAL} → jf-a`);

    const streamB = await playFromMenu(driver, 'Mock JF B');
    assert.ok(
      streamB.startsWith(`http://127.0.0.1:${mockB.port}/Videos/m1/stream`),
      `backing B must play server B's stream, got ${streamB}`,
    );
    await pollUntil(() => overrideValue() === 'jf-b', `the override to flip to ${CANONICAL} → jf-b`);

    // Natural completion carries the immutable backing set captured at launch
    // and marks both servers exactly once, independent of the chosen stream.
    const postCount = (mock) => mock.state.requests.filter(
      (request) => request.method === 'POST' && request.path === `/Users/${mock.userId}/PlayedItems/m1`,
    ).length;
    const beforeCompletionA = postCount(mockA);
    const beforeCompletionB = postCount(mockB);
    const completedStream = await playFromMenu(driver, 'Mock JF B', { finish: true });
    assert.ok(completedStream.startsWith(`${mockB.baseUrl}/Videos/m1/stream`));
    await pollUntil(
      () => postCount(mockA) === beforeCompletionA + 1 && postCount(mockB) === beforeCompletionB + 1,
      'clean EOF to mark both title backings played once',
      { timeoutMs: 20_000 },
    );
    await holdsFor(
      () =>
        postCount(mockA) > beforeCompletionA + 1 || postCount(mockB) > beforeCompletionB + 1
          ? 'clean EOF repeated a backing mutation'
          : null,
      1_000,
      'clean EOF all-backing mutation to remain single-shot',
    );
    assert.equal(mockA.state.userData.m1.played, true);
    assert.equal(mockB.state.userData.m1.played, true);
  },
};
