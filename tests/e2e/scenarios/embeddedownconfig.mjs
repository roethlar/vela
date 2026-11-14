// The user's own mpv configuration ("use my own mpv configuration") reaches
// the player, in both players, without getting past Vela's own idle setting.
// Rows 13b and 14 of the parity list in .agents/plans/embedded-player.md (13a,
// the Advanced arguments, is `mpvargs`). The name is from when this ran only
// in the embedded player (review finding ec-4); it now runs in both.
//
// KNOWN TO FAIL IN THE EXTERNAL PLAYER, on a product defect: there the
// profile's `idle=yes` wins over Vela's `--idle=no`, because Vela's command
// line ends with an `--include` (the stream-header file) after it. It is
// recorded in tests/e2e/parity.mjs (KNOWN_FAILURES), with its evidence, and
// rows 13b and 14 are NOT counted as shown in that player by this scenario.
// The assertion stays as it is: it is the product that has to change.
//
// Nothing in the Advanced options, "use my own mpv configuration" on, and a
// user's mpv.conf (found through MPV_HOME, where mpv looks for it) that sets
// a volume and redefines `gpu-hq`, the profile Vela itself selects, to
// include a file that says `idle=yes`. In both players the user's volume must
// be in effect, and Vela's own idle setting must stand: a player that idles
// never ends at the end of a film, so the play is never finished off.
//
// Why this case matters in the embedded player: it applies a launch's options
// to mpv's library one after another, mpv applies an `include` it meets
// inside an options file after the rest of that file, and a profile can
// contain an include. With Vela's line selecting the profile and Vela's own
// `idle=once` in one file, the user's `idle=yes` won; this is the case that
// got past the first repair of ec-4. (`embeddedplay` covers the other way in:
// a profile the user selects.)
//
// Where the two players legitimately differ, and nowhere else, the scenario
// branches: each player's idle setting is its own (the embedded player waits
// for its first file, `once`; the external one never idles, `no`), and which
// player played is checked by where mpv runs (inside Vela, or a program of its
// own), plus, in the embedded player, that it draws through the library.
//
// In the external player this play has no `--vo=null` (there
// must be nothing in the Advanced options), so its mpv opens a window on the
// run's private display (Xvfb), never on a real desktop.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  makeClips,
  mockSource,
  openDetailAndPlay,
  openLibraryGrid,
  pollUntil,
  runPlayer,
  seedConfig,
  socketAccepting,
  waitForPageUncovered,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { shown } from '../proof.mjs';

let mock;

// What each player sets `idle` to, whatever the user's configuration says:
// `once` in the embedded player, `no` in the external one (which mpv's IPC
// answers as `false`; `yes` would be `true`).
const VELA_IDLE = runPlayer === 'embedded' ? 'once' : false;

// Starts a play from the detail page; returns its mpv once it plays.
async function startPlay(driver) {
  const before = mpvSocketSnapshot();
  await openLibraryGrid(driver);
  await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
  const socketPath = await waitForNewMpvSocket(before);
  const mpv = await MpvIpc.connect(socketPath);
  await pollUntil(() => mpv.getProp('path').catch(() => null), 'mpv to load the stream');
  await pollUntil(
    () => mpv.getProp('time-pos').then((t) => t > 0.3).catch(() => false),
    'the play to progress',
  );
  return { mpv, socketPath };
}

// The play is in the player this run is about.
async function assertPlayer(mpv) {
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
}

export default {
  name: 'embeddedownconfig',
  rows: ['13b', '14'],

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
    // options, or the case is not the one described above. `sub-pos` is only
    // a marker that the file was read (its default is 100).
    fs.writeFileSync(
      path.join(mpvHome, 'mpv.conf'),
      `ao=null\nvolume=37\nsub-pos=77\n\n[gpu-hq]\ninclude=${override}\n`,
    );
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

    const { mpv, socketPath } = await startPlay(driver);
    try {
      await assertPlayer(mpv);
      // The user's configuration was read: settings of theirs are in effect.
      assert.equal(await mpv.getProp('options/volume'), 37, "the user's own mpv.conf was applied");
      assert.equal(await mpv.getProp('options/sub-pos'), 77, "the user's own mpv.conf was applied");
      // And its redefined profile did not get past Vela's own options.
      assert.equal(
        await mpv.getProp('options/idle'),
        VELA_IDLE,
        "Vela's own idle setting stands, whatever the profile Vela selects has been made to include",
      );
      await mpv.setProp('time-pos', 9);
    } finally {
      mpv.close();
    }
    // What that setting is for: at the end of its file the player ends.
    await pollUntil(async () => !(await socketAccepting(socketPath)), 'the player to end at the end of its file', {
      timeoutMs: 8000,
    });
    await waitForPageUncovered(driver, 'at the end of the file');
    // The end of its proof of its rows in this player (proof.mjs).
    shown('13b', '14');
  },
};
