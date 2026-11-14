// The skip button of the player view, end to end in the embedded player
// (.agents/plans/embedded-player.md, Slice 5).
//
// Runs only in a run of the embedded player (`npm run e2e -- --embedded`).
// There the marker script draws no button of its own (the embedded launch
// passes `vela-markers-draw=no`); the page shows one while the script reports
// an active marker, and pressing it asks the script to skip.
//
// The marker here ends past the end of the clip. A skip of it while playing
// ends the play, as it does in the external player, which would hide whether
// the skip went through the script or not. So the play is paused through the
// page first: held there, the script's own skip clears the active marker,
// while anything else that seeks past the end leaves it set.
import assert from 'node:assert/strict';
import path from 'node:path';
import { createConnection } from 'node:net';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  holdsFor,
  makeClips,
  mockSource,
  openDetailAndPlay,
  openLibraryGrid,
  pollUntil,
  seedConfig,
  showPlayerBar,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const TICKS_PER_MS = 10_000;
let mock;

const accepting = (socketPath) =>
  new Promise((resolve) => {
    const probe = createConnection(socketPath);
    probe.once('connect', () => {
      probe.destroy();
      resolve(true);
    });
    probe.once('error', () => resolve(false));
  });

const active = (mpv) => mpv.getProp('user-data/vela-markers/active');

export default {
  name: 'embeddedskip',
  onlyPlayer: 'embedded',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['skip.mp4']);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Mock Movie',
        year: 2020,
        runTimeTicks: 100_000_000,
        mediaFile: path.join(mediaDir, 'skip.mp4'),
      }],
    });
    // An intro from 2 s to 20 s, in a 10 s clip.
    mock.state.setMediaSegments('m1', [
      { Type: 'Intro', StartTicks: 2_000 * TICKS_PER_MS, EndTicks: 20_000 * TICKS_PER_MS },
    ]);
    seedConfig(configRoot, [mockSource(mock)], {
      skip_intros: 'button',
      skip_credits: 'off',
      skip_commercials: 'off',
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    const before = mpvSocketSnapshot();
    await openLibraryGrid(driver);
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
    const socketPath = await waitForNewMpvSocket(before);
    const mpv = await MpvIpc.connect(socketPath);
    try {
      await pollUntil(
        () => mpv.getProp('user-data/vela-markers/loaded').catch(() => null),
        'the marker script loaded with its payload',
        { timeoutMs: 10000 },
      );

      // 1. Inside the intro the script arms the marker, draws nothing of its
      //    own, and the page shows its button.
      await pollUntil(
        () => active(mpv).then((a) => a === 'intro').catch(() => false),
        'the script to arm the intro',
        { timeoutMs: 8000 },
      );
      // An empty Lua table: mpv hands it over as [] (or {}). A drawn button
      // publishes its rectangle here.
      const bounds = await mpv.getProp('user-data/vela-markers/button-bounds');
      assert.equal(
        Object.keys(bounds).length,
        0,
        `the script draws no button of its own in the embedded player (bounds ${JSON.stringify(bounds)})`,
      );
      await driver.waitFor(
        `return document.querySelector('.player .over button.skip')?.textContent.trim() === 'Skip Intro'`,
        "the page's Skip Intro button",
        { timeoutMs: 5000 },
      );

      // 2. Paused through the page, so that nothing ends the play at the
      //    clamp; mpv itself says it is paused.
      await showPlayerBar(driver);
      await driver.click(await driver.find('css selector', '.player button.playpause'));
      await pollUntil(() => mpv.getProp('pause'), 'the page to pause the play', { timeoutMs: 5000 });
      assert.equal(await active(mpv), 'intro', 'still inside the intro while paused');
      await screenshot('01-skip-button');

      // 3. The page's button skips through the script.
      await driver.click(await driver.find('css selector', '.player .over button.skip'));
      await pollUntil(
        () => active(mpv).then((a) => a === '').catch(() => false),
        'the skip to clear the active marker',
        { timeoutMs: 5000 },
      );
      // The page hears of it with the next state, up to a quarter second on.
      await driver.waitFor(
        `return !document.querySelector('.player .over button.skip')`,
        'the skip button to go once the marker is cleared',
        { timeoutMs: 3000, intervalMs: 50 },
      );
      await holdsFor(
        async () => {
          if (await driver.exec(`return !!document.querySelector('.player .over button.skip')`)) {
            return 'the skip button came back';
          }
          if (!(await accepting(socketPath))) return 'the play ended';
          return null;
        },
        2000,
        'after the skip the button stays away and the play stays up',
      );
      assert.equal(await mpv.getProp('pause'), true, 'the play is still paused at the clamp');
      assert.equal(await mpv.getProp('eof-reached'), true, 'the skip went to the end of the intro, the end of the clip');

      await driver.pressKeys('q');
    } finally {
      mpv.close();
    }
    await pollUntil(async () => !(await accepting(socketPath)), 'the player to stop after q', { timeoutMs: 5000 });
    await driver.waitFor(`return !document.querySelector('.player')`, 'the player view to go away');
  },
};
