// The user's own mpv configuration against Vela's protected options, in the
// embedded player (.agents/plans/embedded-player.md; review finding ec-4).
//
// Runs only in a run of the embedded player (`npm run e2e -- --embedded`).
//
// The embedded player applies a launch's options to mpv's library one after
// another. mpv applies an `include` it meets inside an options file after
// the rest of that file, and a profile can contain an include. So if a
// profile is selected in one file with options that follow it, whatever the
// profile includes is applied after them.
//
// The case here is the one that got past the first repair of that finding:
// nothing in the Advanced options at all, "use my own mpv configuration" on,
// and a user's mpv.conf that redefines `gpu-hq`, the profile Vela itself
// selects, to include a file that says `idle=yes`. Vela's line selecting the
// profile and Vela's own `idle=once` are both Vela's; with the two in one
// file the user's `idle=yes` won, and a player that idles never ends at the
// end of a film, so the play is never finished off.
//
// (`embeddedplay` covers the other way in: a profile the user selects.)
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createConnection } from 'node:net';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { makeClips, mockSource, openDetailAndPlay, openLibraryGrid, pollUntil, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

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

export default {
  name: 'embeddedownconfig',
  onlyPlayer: 'embedded',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['stream.mp4']);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Mock Movie',
        year: 2020,
        runTimeTicks: 100_000_000,
        mediaFile: path.join(mediaDir, 'stream.mp4'),
      }],
    });
    const override = path.join(configRoot, 'idle-yes.conf');
    fs.writeFileSync(override, 'idle=yes\n');
    const mpvHome = path.join(configRoot, 'mpv-home');
    fs.mkdirSync(mpvHome);
    // Silence comes from here too: there must be nothing in the Advanced
    // options, or the case is not the one described above.
    fs.writeFileSync(path.join(mpvHome, 'mpv.conf'), `ao=null\nvolume=37\n\n[gpu-hq]\ninclude=${override}\n`);
    seedConfig(configRoot, [mockSource(mock)], {
      mpv_extra_args: '',
      mpv_use_own_config: true,
    });
  },

  environment({ configRoot }) {
    // Where mpv, and so Vela, looks for the user's mpv.conf.
    return { MPV_HOME: path.join(configRoot, 'mpv-home') };
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock Library')`,
      'seeded library in the sidebar',
    );
    const before = mpvSocketSnapshot();
    await openLibraryGrid(driver);
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
    const socketPath = await waitForNewMpvSocket(before);
    const mpv = await MpvIpc.connect(socketPath);
    try {
      await pollUntil(() => mpv.getProp('path').catch(() => null), 'mpv to load the stream');
      assert.equal(
        await pollUntil(() => mpv.getProp('current-vo').catch(() => null), 'mpv to set up its video output'),
        'libmpv',
        'this is the embedded player',
      );
      // The user's configuration was read: a setting of theirs is in effect.
      assert.equal(await mpv.getProp('options/volume'), 37, "the user's own mpv.conf was applied");
      // And its redefined profile did not get past Vela's own options.
      assert.equal(
        await mpv.getProp('options/idle'),
        'once',
        "Vela's own idle setting stands, whatever the profile Vela selects has been made to include",
      );
      await pollUntil(
        () => mpv.getProp('time-pos').then((t) => t > 0.3).catch(() => false),
        'the play to progress',
      );
      await mpv.setProp('time-pos', 9);
    } finally {
      mpv.close();
    }
    // What that setting is for: at the end of its file the player ends.
    await pollUntil(async () => !(await accepting(socketPath)), 'the player to end at the end of its file', { timeoutMs: 8000 });
    await driver.waitFor(
      `return !document.documentElement.classList.contains('player-open') && !document.querySelector('.player')`,
      'the player view to go away at the end of the file',
    );
  },
};
