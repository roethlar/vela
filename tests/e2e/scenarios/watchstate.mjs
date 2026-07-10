// Watch-state refresh without restart (the 2026-07-04 owner-reported issue,
// fixed via the playback-ended event): play a server item to mid-clip, quit,
// and the card must show the updated progress from the server's refetched
// state — no app restart. The mock records the /Sessions/Playing* check-ins
// and reflects the Stopped position back on the next Items fetch, exactly
// like a real server.
import assert from 'node:assert/strict';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { pollUntil, makeClips, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock;

export default {
  name: 'watchstate',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['stream.mp4']);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Mock Movie',
        year: 2020,
        runTimeTicks: 100_000_000, // 10s, matching the real clip
        mediaFile: path.join(mediaDir, 'stream.mp4'),
      }],
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
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
    const labelBefore = await driver.exec(
      `return document.querySelector('button.poster[aria-label^="Mock Movie"]').getAttribute('aria-label')`,
    );
    assert.ok(!labelBefore.includes('% watched'), 'card starts with no progress');

    // Play through the mock's HTTP stream (card → info page → Play, per the
    // nav flip); seek to 6s; quit.
    const before = mpvSocketSnapshot();
    const card = await driver.find('css selector', 'button.poster[aria-label^="Mock Movie"]');
    await driver.click(card);
    await driver.waitFor(
      `return !!document.querySelector('.detail button.playwide')`,
      'info page with a Play button',
    );
    const play = await driver.find('css selector', '.detail button.playwide');
    await driver.click(play);
    const socketPath = await waitForNewMpvSocket(before);
    const mpv = await MpvIpc.connect(socketPath);
    try {
      const loaded = await pollUntil(() => mpv.getProp('path').catch(() => null), 'mpv to load the stream');
      assert.ok(
        loaded.startsWith(`http://127.0.0.1:${mock.port}/Videos/m1/stream`),
        `mpv must play the mock stream URL, got ${loaded}`,
      );
      await pollUntil(
        () => mpv.getProp('time-pos').then((t) => t > 0.5).catch(() => false),
        'stream playback to progress',
      );
      await mpv.setProp('time-pos', 6);
      await new Promise((r) => setTimeout(r, 1500)); // let the tracker observe ≥6s
      mpv.quit();
    } finally {
      mpv.close();
    }

    // Server side: Start and Stopped check-ins arrived, Stopped carries a
    // mid-clip position (ticks; 10s clip ⇒ 6s ≈ 60_000_000).
    await pollUntil(
      () => mock.state.checkins.some((c) => c.endpoint === '/Stopped'),
      'the Stopped check-in',
    );
    assert.ok(
      mock.state.checkins.some((c) => c.endpoint === '/Start'),
      'a Start check-in must precede Stopped',
    );
    const stopped = mock.state.checkins.find((c) => c.endpoint === '/Stopped').body;
    assert.equal(stopped.ItemId, 'm1');
    assert.equal(stopped.MediaSourceId, 'ms-m1');
    const stoppedMs = stopped.PositionTicks / 10_000;
    assert.ok(
      stoppedMs >= 3000 && stoppedMs <= 8000,
      `Stopped must carry the mid-clip position, got ${stoppedMs}ms`,
    );

    // UI side, WITHOUT restart: the refetch triggered by playback-ended must
    // put the server-reported progress on the card. The detail page stays
    // open after playback; navigate back to the grid via its crumb bar.
    await driver.waitFor(`return !!document.querySelector('div.crumbs button.back')`, 'detail crumb bar');
    const back = await driver.find('css selector', 'div.crumbs button.back');
    await driver.click(back);
    await driver.waitFor(
      `return (document.querySelector('button.poster[aria-label^="Mock Movie"]')?.getAttribute('aria-label') ?? '').includes('% watched')`,
      'progress on the card without a restart',
    );
    const labelAfter = await driver.exec(
      `return document.querySelector('button.poster[aria-label^="Mock Movie"]').getAttribute('aria-label')`,
    );
    const pct = Number(/(\d+)% watched/.exec(labelAfter)?.[1]);
    assert.ok(pct >= 30 && pct <= 80, `expected a mid-clip percentage, got ${pct}%`);
    await screenshot('01-progress');
  },
};
