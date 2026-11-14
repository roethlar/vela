// Cropping in the embedded player, end to end (.agents/plans/embedded-player.md,
// Slice 5, and "Autocrop in the embedded player").
//
// Runs only in a run of the embedded player (`npm run e2e -- --embedded`).
// The embedded launch loads Vela's own `vela-crop.lua` (not mpv's stock
// autocrop.lua, which the external player uses and `autocrop` covers) with the
// mode from Settings, and the player view's Crop control and `C` key reach it
// through the backend. This is `autocrop`'s counterpart for the embedded
// player: automatic cropping on a fresh and on a resumed play, manual
// cropping that waits for the user, and Off, which crops nothing.
//
// The settle delay is shortened the way a user would, through the user's mpv
// options. What is read back is mpv's own `video-crop`; a WebDriver screenshot
// shows only the page, never the picture under it.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { createConnection } from 'node:net';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { PLAYER_VIEW_UP, holdsFor, mockSource, openLibraryGrid, pollUntil, seedConfig, showPlayerBar } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { shown } from '../proof.mjs';

// A 1280x720 frame whose picture is 1280x544, with black bars of 88 rows above
// and below: the crop the script must find.
const LETTERBOXED = '1280x544+0+88';
const SECONDS = 30;
let mock;
let configRootPath;

function makeClip(configRoot) {
  const mediaDir = path.join(configRoot, 'media');
  fs.mkdirSync(mediaDir, { recursive: true });
  const clip = path.join(mediaDir, 'letterbox.mp4');
  const ff = spawnSync('ffmpeg', [
    '-f', 'lavfi', '-i', `testsrc2=size=1280x544:rate=24:duration=${SECONDS}`,
    '-f', 'lavfi', '-i', `sine=frequency=440:duration=${SECONDS}`,
    '-vf', 'pad=1280:720:0:88:black',
    '-c:v', 'libx264', '-preset', 'ultrafast', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest',
    clip,
  ], { stdio: 'ignore' });
  if (ff.status !== 0) throw new Error('ffmpeg is required to generate the letterboxed clip');
  return clip;
}

// Every play reads the configuration again, so the next play has this mode.
function setMode(mode) {
  seedConfig(configRootPath, [mockSource(mock)], {
    mpv_autocrop: mode,
    mpv_extra_args: '--vo=null\n--ao=null\n--script-opts-append=vela-crop-delay=1',
  });
}

const accepting = (socketPath) =>
  new Promise((resolve) => {
    const probe = createConnection(socketPath);
    probe.once('connect', () => {
      probe.destroy();
      resolve(true);
    });
    probe.once('error', () => resolve(false));
  });

const cropState = (mpv) => mpv.getProp('user-data/vela-crop/state').catch(() => null);
const videoCrop = (mpv) => mpv.getProp('video-crop');

async function play(driver, verb) {
  // The grid can be re-rendering when a play has just ended (its watch state
  // refreshes): wait for the card, and open its menu in the same step.
  await driver.waitFor(
    `const el = document.querySelector('button.poster[aria-label^="Letterbox Movie"]');
     if (!el) return false;
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));
     return true;`,
    'the movie card, to open its menu',
  );
  await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu (play)');
  const before = mpvSocketSnapshot();
  await driver.click(
    await driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='${verb}']`),
  );
  const socketPath = await waitForNewMpvSocket(before);
  const mpv = await MpvIpc.connect(socketPath);
  const first = await pollUntil(
    () => mpv.getProp('time-pos').catch(() => null),
    `${verb}: the first position`,
  );
  return { mpv, socketPath, first };
}

async function cropped(driver, mpv, what) {
  await pollUntil(
    async () => (await cropState(mpv)) === 'on' && (await videoCrop(mpv)) === LETTERBOXED,
    `${what}: video-crop ${LETTERBOXED} with the state "on"`,
    { timeoutMs: 12000 },
  );
  assert.deepEqual(await mpv.getProp('vf'), [], `${what}: the measuring filter is removed`);
  // The measurement turns hardware decoding off where frames stay on the GPU
  // (as in this venue); it is the launch's `auto` again afterwards. (mpv
  // reports the option as a list.)
  assert.deepEqual(await mpv.getProp('hwdec'), ['auto'], `${what}: hardware decoding is restored`);
  await driver.waitFor(
    `return document.querySelector('.player button.crop')?.getAttribute('aria-pressed') === 'true'`,
    `${what}: the Crop control to show the crop`,
    { timeoutMs: 5000 },
  );
  // A crop can halt playback in mpv; the script follows it with what keeps
  // playback going.
  const before = await mpv.getProp('time-pos');
  await new Promise((r) => setTimeout(r, 2000));
  const after = await mpv.getProp('time-pos');
  assert.ok(after - before > 1, `${what}: playback goes on after the crop (${before} -> ${after})`);
}

// Ends the play with q and waits for its last check-in.
async function stop(driver, mpv, socketPath, what) {
  try {
    await driver.pressKeys('q');
  } finally {
    mpv.close();
  }
  await pollUntil(async () => !(await accepting(socketPath)), `${what}: the player to stop after q`, {
    timeoutMs: 5000,
  });
  await pollUntil(
    () => mock.state.checkins.some((c) => c.endpoint === '/Stopped'),
    `${what}: the Stopped check-in`,
  );
  mock.state.checkins.length = 0;
  await driver.waitFor(`return !document.querySelector('.player')`, `${what}: the player view to go away`);
}

export default {
  name: 'embeddedcrop',
  onlyPlayer: 'embedded',
  onlyPlayerWhy:
    "vela-crop.lua and the view's Crop control are the embedded player's; autocrop shows rows 9a, 9b and 9c in the external player",
  // Row 9 split by mode: 9a Off, 9b Manual, 9c Auto, each shown at the end
  // of its legs.
  rows: ['9a', '9b', '9c'],

  async seed({ configRoot }) {
    configRootPath = configRoot;
    const clip = makeClip(configRoot);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Letterbox Movie',
        year: 2020,
        runTimeTicks: SECONDS * 10_000_000,
        mediaFile: clip,
      }],
    });
    setMode('auto');
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, log }) {
    const say = (...a) => (log ? log(...a) : console.log(...a));
    await openLibraryGrid(driver, { cardPrefix: 'Letterbox Movie' });

    // 1. Automatic, fresh: cropped after the delay with nothing pressed.
    let { mpv, socketPath, first } = await play(driver, 'Play');
    try {
      assert.ok(first < 2, `a fresh play starts at the beginning (time-pos ${first})`);
      say('hwdec-current', await mpv.getProp('hwdec-current').catch(() => null));
      await cropped(driver, mpv, 'automatic, fresh');
      await screenshot('01-auto-fresh');
      // A position for the resumed play to start from.
      await mpv.setProp('time-pos', 6);
      await new Promise((r) => setTimeout(r, 1500));
    } catch (err) {
      mpv.close();
      throw err;
    }
    await stop(driver, mpv, socketPath, 'automatic, fresh');
    await pollUntil(() => mock.state.userData.m1.positionTicks > 0, 'the server-side resume point');

    // 2. Automatic, resumed: the same, from the resume point.
    ({ mpv, socketPath, first } = await play(driver, 'Resume'));
    try {
      assert.ok(first >= 4, `the play resumes (time-pos ${first})`);
      await cropped(driver, mpv, 'automatic, resumed');
    } catch (err) {
      mpv.close();
      throw err;
    }
    await stop(driver, mpv, socketPath, 'automatic, resumed');
    // The end of its proof of Auto (row 9c) in this player (proof.mjs).
    shown('9c');

    // 3. Manual: nothing by itself past the delay; the Crop control crops and
    //    the C key takes the crop away again.
    setMode('manual');
    ({ mpv, socketPath, first } = await play(driver, 'Play from Beginning'));
    try {
      await driver.waitFor(
        `return document.querySelector('.player button.crop')?.getAttribute('aria-pressed') === 'false'`,
        'manual: the Crop control, not pressed',
      );
      await holdsFor(
        async () => {
          const crop = await videoCrop(mpv);
          if (crop !== '') return `video-crop became ${crop}`;
          const state = await cropState(mpv);
          if (state !== 'off') return `the state became ${state}`;
          return null;
        },
        4000,
        'manual: nothing is cropped without the control',
      );
      await showPlayerBar(driver);
      await driver.click(await driver.find('css selector', '.player button.crop'));
      await cropped(driver, mpv, 'manual, after the Crop control');
      await screenshot('02-manual-cropped');
      await driver.pressKeys('C');
      await pollUntil(
        async () => (await cropState(mpv)) === 'off' && (await videoCrop(mpv)) === '',
        'manual: C to take the crop away',
        { timeoutMs: 5000 },
      );
      await driver.waitFor(
        `return document.querySelector('.player button.crop')?.getAttribute('aria-pressed') === 'false'`,
        'manual: the Crop control to show no crop',
        { timeoutMs: 5000 },
      );
    } catch (err) {
      mpv.close();
      throw err;
    }
    await stop(driver, mpv, socketPath, 'manual');
    // The end of its proof of Manual (row 9b).
    shown('9b');

    // 4. Off (review finding pa-2): no crop script, so the view offers no Crop
    //    control, its C key crops nothing, and nothing crops by itself, held
    //    past the delay of this scenario's automatic mode (1 s) and the
    //    script's default (5 s).
    setMode('off');
    ({ mpv, socketPath, first } = await play(driver, 'Play from Beginning'));
    try {
      assert.ok(first < 2, `off: a play from the beginning starts there (time-pos ${first})`);
      await driver.waitFor(
        PLAYER_VIEW_UP,
        'off: the player view to be up',
      );
      const offHold = async () => {
        const crop = await videoCrop(mpv);
        if (crop !== '') return `video-crop became ${crop}`;
        const state = await cropState(mpv);
        if (state !== null) return `the crop script published the state ${state}`;
        const control = await driver.exec(`return !!document.querySelector('.player button.crop')`);
        if (control) return 'the view shows a Crop control';
        return null;
      };
      await holdsFor(offHold, 3000, 'off: nothing is cropped and no Crop control is offered');
      await driver.pressKeys('C');
      await holdsFor(offHold, 5000, 'off: the C key crops nothing');
    } catch (err) {
      mpv.close();
      throw err;
    }
    await stop(driver, mpv, socketPath, 'off');
    // The end of its proof of Off (row 9a).
    shown('9a');
  },
};
