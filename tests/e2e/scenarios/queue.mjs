// Queue auto-advance: with B queued via the real "Play next" context-menu
// action, A reaching natural EOF must make the BACKEND spawn mpv for B with
// no UI interaction. (Accepted v1 gap, not asserted: auto-advance plays are
// not snapshotted into recents.)
import assert from 'node:assert/strict';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { pollUntil, openLibraryGrid, seedLocalMedia } from '../helpers.mjs';

const CLIP_A = 'E2E Clip A.mp4';
const CLIP_B = 'E2E Clip B.mp4';

export default {
  name: 'queue',

  async seed({ configRoot }) {
    seedLocalMedia(configRoot, [CLIP_A, CLIP_B]);
  },

  async run({ driver, screenshot, configRoot }) {
    await openLibraryGrid(driver);

    // Play A.
    const beforeA = mpvSocketSnapshot();
    const cardA = await driver.find('css selector', `button.poster[aria-label^="E2E Clip A"]`);
    await driver.click(cardA);
    const socketA = await waitForNewMpvSocket(beforeA);
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
      `const el = document.querySelector('button.poster[aria-label^="E2E Clip B"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const playNext = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu')
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
        'auto-advanced mpv to load a file',
      );
      assert.equal(
        loaded,
        path.join(configRoot, 'media', CLIP_B),
        'auto-advance must play the queued clip B',
      );
      mpvB.quit();
    } finally {
      mpvB.close();
    }
  },
};
