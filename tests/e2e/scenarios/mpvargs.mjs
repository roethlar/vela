// The user's Advanced mpv arguments reach the player, in both players, and
// with "use my own mpv configuration" off the user's mpv.conf is not read.
// Row 13a of the parity list in .agents/plans/embedded-player.md (13b, the own
// configuration, is `embeddedownconfig`). This was the second leg of
// `embeddedownconfig` until review finding atk-1 of the parity guard: there it
// ran after the own-configuration leg, which fails in the external player on
// a product defect, so in that player it was never reached, and row 13 was
// counted as shown there by a scenario that never got this far.
//
// A user's mpv.conf (found through MPV_HOME, where mpv looks for it) sets a
// marker, `sub-pos=77`. The Advanced arguments set a volume. The volume must
// be in effect and the marker must not be.
//
// Where the two players legitimately differ, and nowhere else, the scenario
// branches: each player's idle setting is its own (the embedded player waits
// for its first file, `once`; the external one never idles, `no`, which mpv's
// IPC answers as `false`), and which player played is checked by where mpv
// runs (inside Vela, or a program of its own), plus, in the embedded player,
// that it draws through the library.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  DISPLAYLESS_MPV_ARGS,
  makeClips,
  mockSource,
  openDetailAndPlay,
  openLibraryGrid,
  pollUntil,
  runPlayer,
  seedConfig,
  stopPlayAsUser,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { shown } from '../proof.mjs';

let mock;

const VELA_IDLE = runPlayer === 'embedded' ? 'once' : false;

export default {
  name: 'mpvargs',
  rows: ['13a'],

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
    const mpvHome = path.join(configRoot, 'mpv-home');
    fs.mkdirSync(mpvHome);
    fs.writeFileSync(path.join(mpvHome, 'mpv.conf'), 'volume=37\nsub-pos=77\n');
    seedConfig(configRoot, [mockSource(mock)], {
      mpv_extra_args: `${DISPLAYLESS_MPV_ARGS}\n--volume=42`,
      mpv_use_own_config: false,
    });
  },

  environment({ configRoot }) {
    // Where mpv would look for the user's mpv.conf, were it to look.
    return { MPV_HOME: path.join(configRoot, 'mpv-home') };
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver }) {
    const before = mpvSocketSnapshot();
    await openLibraryGrid(driver);
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
    const socketPath = await waitForNewMpvSocket(before);
    const mpv = await MpvIpc.connect(socketPath);
    try {
      await pollUntil(() => mpv.getProp('path').catch(() => null), 'mpv to load the stream');
      await pollUntil(
        () => mpv.getProp('time-pos').then((t) => t > 0.3).catch(() => false),
        'the play to progress',
      );
      // The play is in the player this run is about.
      const pid = await mpv.getProp('pid');
      const program = fs.readFileSync(`/proc/${pid}/comm`, 'utf8').trim();
      if (runPlayer === 'embedded') {
        assert.equal(program, 'vela', 'the embedded player plays inside Vela');
        assert.equal(
          await pollUntil(() => mpv.getProp('current-vo').catch(() => null), 'mpv to set up its video output'),
          'libmpv',
          'the embedded player draws through the library',
        );
      } else {
        assert.equal(program, 'mpv', 'the external player is a program of its own');
      }
      assert.equal(await mpv.getProp('options/volume'), 42, 'the Advanced arguments were applied');
      assert.notEqual(
        await mpv.getProp('options/sub-pos'),
        77,
        "with own configuration off, the user's mpv.conf is not read",
      );
      assert.notEqual(await mpv.getProp('options/volume'), 37, "the user's mpv.conf did not set the volume");
      assert.equal(await mpv.getProp('options/idle'), VELA_IDLE, "Vela's own idle setting");
      await stopPlayAsUser(driver, mpv, socketPath, 'the Advanced-arguments play');
    } finally {
      mpv.close();
    }
    // The end of its proof of its rows in this player (proof.mjs).
    shown('13a');
  },
};
