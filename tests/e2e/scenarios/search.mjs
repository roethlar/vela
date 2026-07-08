// Library search: the too-short query is rejected with the real error, a
// valid query filters to matching items only, and a result card plays.
import assert from 'node:assert/strict';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { pollUntil, seedLocalMedia } from '../helpers.mjs';

const CLIP_HIT = 'Alpha Voyage.mp4';
const CLIP_MISS = 'Beta Horizon.mp4';
const ENTER = '';

export default {
  name: 'search',

  async seed({ configRoot }) {
    seedLocalMedia(configRoot, [CLIP_HIT, CLIP_MISS]);
  },

  async run({ driver, screenshot, configRoot }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && !!document.querySelector('input[aria-label="Search your libraries"]')`,
      'search box (authenticated view)',
    );
    const box = await driver.find('css selector', 'input[aria-label="Search your libraries"]');

    // Too short: rejected with the real validation error, nothing searched.
    await driver.type(box, `a${ENTER}`);
    await driver.waitFor(
      `return document.body.innerText.includes('Search needs at least 2 characters.')`,
      'short-query validation error',
    );

    // Valid query: only the matching clip shows.
    await driver.exec(
      `const i = document.querySelector('input[aria-label="Search your libraries"]'); i.value = ''; i.dispatchEvent(new Event('input', { bubbles: true }));`,
    );
    await driver.type(box, `alpha${ENTER}`);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Alpha Voyage"]')`,
      'search hit in the results grid',
    );
    const missVisible = await driver.exec(
      `return !!document.querySelector('button.poster[aria-label^="Beta Horizon"]')`,
    );
    assert.equal(missVisible, false, 'non-matching clip must not be in search results');
    await screenshot('01-results');

    // A result card plays the right file.
    const before = mpvSocketSnapshot();
    const hit = await driver.find('css selector', 'button.poster[aria-label^="Alpha Voyage"]');
    await driver.click(hit);
    const mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
    try {
      const loaded = await pollUntil(() => mpv.getProp('path').catch(() => null), 'mpv to load the search hit');
      assert.equal(loaded, path.join(configRoot, 'media', CLIP_HIT));
      mpv.quit();
    } finally {
      mpv.close();
    }
  },
};
