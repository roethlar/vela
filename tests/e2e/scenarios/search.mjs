// Library search: the too-short query is rejected with the real error, a
// valid query filters to matching items only, and a result card routes
// through the info page (nav flip) to play the right stream.
import assert from 'node:assert/strict';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { pollUntil, makeClips, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import path from 'node:path';

const ENTER = '';

let mock;

export default {
  name: 'search',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['alpha.mp4']);
    mock = await startMockJellyfin({
      movies: [
        { id: 'a1', name: 'Alpha Voyage', year: 2020, runTimeTicks: 100_000_000, mediaFile: path.join(mediaDir, 'alpha.mp4') },
        { id: 'b1', name: 'Beta Horizon', year: 2021, runTimeTicks: 100_000_000 },
      ],
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
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

    // Valid query: only the matching movie shows.
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
    assert.equal(missVisible, false, 'non-matching movie must not be in search results');
    await screenshot('01-results');

    // A result card routes to the info page and plays the right stream.
    const before = mpvSocketSnapshot();
    const hit = await driver.find('css selector', 'button.poster[aria-label^="Alpha Voyage"]');
    await driver.click(hit);
    await driver.waitFor(
      `return !!document.querySelector('.detail button.playwide')`,
      'info page from the search result',
    );
    const play = await driver.find('css selector', '.detail button.playwide');
    await driver.click(play);
    const mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
    try {
      const loaded = await pollUntil(() => mpv.getProp('path').catch(() => null), 'mpv to load the search hit');
      assert.ok(
        loaded.startsWith(`http://127.0.0.1:${mock.port}/Videos/a1/stream`),
        `the search hit must play its own stream, got ${loaded}`,
      );
      mpv.quit();
    } finally {
      mpv.close();
    }
  },
};
