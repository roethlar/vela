// Queue auto-advance: with B queued via the real "Play next" context-menu
// action, A reaching natural EOF must make the BACKEND spawn mpv for B with
// no UI interaction. (Accepted v1 gap, not asserted: auto-advance plays are
// not snapshotted into recents.) Playback starts via the context menu's
// direct Play — the queue mechanics are the subject here; the nav-flip
// detail route is the playback scenario's job.
import assert from 'node:assert/strict';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { pollUntil, openLibraryGrid, makeClips, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock;

export default {
  name: 'queue',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['a.mp4', 'b.mp4']);
    mock = await startMockJellyfin({
      movies: [
        { id: 'a1', name: 'Mock Clip A', year: 2020, runTimeTicks: 100_000_000, mediaFile: path.join(mediaDir, 'a.mp4') },
        { id: 'b1', name: 'Mock Clip B', year: 2021, runTimeTicks: 100_000_000, mediaFile: path.join(mediaDir, 'b.mp4') },
      ],
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    await openLibraryGrid(driver, { cardPrefix: 'Mock Clip' });

    // Play A via the context menu (direct play, no detail-page detour).
    const beforeA = mpvSocketSnapshot();
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Mock Clip A"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const playA = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu (play A)')
      .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Play']`));
    await driver.click(playA);
    const socketA = await waitForNewMpvSocket(beforeA).catch(async (err) => {
      const text = await driver.exec(`return document.body.innerText.slice(0, 400)`).catch(() => '?');
      err.message += ` — page: ${JSON.stringify(text)}; mock saw: ${JSON.stringify(mock.state.requests.slice(-8).map((r) => r.path))}`;
      throw err;
    });
    const mpvA = await MpvIpc.connect(socketA);
    // Freeze A for the whole UI window (eh-11): the clip is only 10s, and a
    // slow menu/screenshot path must not let it hit EOF before B is queued
    // and the beforeB snapshot exists.
    await pollUntil(
      () => mpvA.getProp('time-pos').then((t) => t > 0.1).catch(() => false),
      'clip A to start playing',
    );
    await mpvA.setProp('pause', true);

    // Queue B via the real context menu while A sits paused.
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Mock Clip B"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const playNext = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu (queue B)')
      .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Play next']`));
    await driver.click(playNext);
    await screenshot('01-queued');

    // Drive A to natural EOF (no quit — EOF is what triggers auto-advance).
    const beforeB = mpvSocketSnapshot();
    try {
      await mpvA.setProp('time-pos', 9.5);
      await mpvA.setProp('pause', false);
    } finally {
      mpvA.close();
    }

    // The backend must spawn mpv for B on its own.
    const socketB = await waitForNewMpvSocket(beforeB);
    assert.notEqual(socketB, socketA, 'auto-advance must be a fresh mpv session');
    const mpvB = await MpvIpc.connect(socketB);
    try {
      const loaded = await pollUntil(
        () => mpvB.getProp('path').catch(() => null),
        'auto-advanced mpv to load a stream',
      );
      assert.ok(
        loaded.startsWith(`http://127.0.0.1:${mock.port}/Videos/b1/stream`),
        `auto-advance must play the queued clip B's stream, got ${loaded}`,
      );
      mpvB.quit();
    } finally {
      mpvB.close();
    }
  },
};
