// Autocrop in the external player, all three modes of Settings (row 9).
//
// Auto: the app must inject the stock autocrop.lua with its own trigger
// DISABLED plus Vela's vela-autocrop.lua shim, and the shim must fire
// detection — on a FRESH play and on a RESUMED play — so mpv's video-crop
// property gets set on a letterboxed clip. Off: nothing crops, held past every
// automatic delay, and no key of mpv's window crops. Manual: nothing crops by
// itself; Shift+C in mpv's window crops and takes the crop away again (sent
// over IPC: see `testOnly`).
//
// Scope honesty (plan .agents/plans/autocrop-resume.md, Guard part B): this
// scenario is the shim-WIRING regression net. It cannot reproduce the
// owner-reported hwdec race (under --vo=null hwdec falls back to copy-back
// and even the stock trigger crops a resumed play); the defect's red→green
// lives in the plan's recorded mac-host probe. What this guards: a lost
// injection, a wrong script-binding name, a broken shim, or a resume that
// stops resuming — any of those fail a leg here.
import assert from 'node:assert/strict';
import path from 'node:path';
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
import { holdsFor, mockSource, mpvInput, openLibraryGrid, pollUntil, seedConfig } from '../helpers.mjs';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { shown } from '../proof.mjs';

let mock;

// 30s letterboxed clip: bright 320x140 content padded to 320x180 with black
// bars top+bottom — cropdetect should find ~320x140+0+20. Long enough that
// the stock script's playtime-remaining check passes on every leg.
function makePaddedClip(configRoot) {
  const mediaDir = path.join(configRoot, 'media');
  fs.mkdirSync(mediaDir, { recursive: true });
  const clip = path.join(mediaDir, 'letterbox.mp4');
  const ff = spawnSync('ffmpeg', [
    '-f', 'lavfi', '-i', 'testsrc=duration=30:size=320x140:rate=24',
    '-f', 'lavfi', '-i', 'sine=frequency=440:duration=30',
    '-vf', 'pad=320:180:0:20:black',
    '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest',
    clip,
  ], { stdio: 'ignore' });
  if (ff.status !== 0) throw new Error('ffmpeg is required to generate the letterboxed clip');
  return clip;
}

async function gridCtxPlay(driver, verb) {
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
  const play = await driver
    .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu (play)')
    .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='${verb}']`));
  await driver.click(play);
}

// Play the clip, assert the start position, wait for the shim-triggered crop,
// optionally park the head at ~6s for the next leg, then quit.
async function playAndAssertCrop(driver, label, { minStart, maxStart, park }) {
  const before = mpvSocketSnapshot();
  await gridCtxPlay(driver, label === 'resume' ? 'Resume' : 'Play');
  const socketPath = await waitForNewMpvSocket(before);
  const mpv = await MpvIpc.connect(socketPath);
  try {
    // The shim must be PRESENT, not merely "cropping happened": a lost or
    // misresolved vela-autocrop.lua degrades to the stock trigger, which
    // also crops under --vo=null — this marker is what distinguishes the
    // two (plan-review ac-r2). The shim publishes it at load.
    await pollUntil(
      () => mpv.getProp('user-data/vela-autocrop/loaded').catch(() => null),
      `${label}: the vela-autocrop shim load marker`,
      { timeoutMs: 6000 },
    );
    const firstPos = await pollUntil(
      () => mpv.getProp('time-pos').catch(() => null).then((t) => (t == null ? null : t)),
      `${label}: first time-pos sample`,
    );
    if (minStart !== undefined) {
      assert.ok(firstPos >= minStart, `${label} must resume (time-pos ${firstPos} < ${minStart})`);
    }
    if (maxStart !== undefined) {
      assert.ok(firstPos < maxStart, `${label} must start fresh (time-pos ${firstPos} >= ${maxStart})`);
    }
    // vela-autocrop-delay=1 + detect_seconds=1 + startup slack.
    const crop = await pollUntil(
      () => mpv.getProp('video-crop').catch(() => '').then((c) => (c ? c : null)),
      `${label}: video-crop from the shim-triggered detection`,
      { timeoutMs: 12000 },
    );
    const m = /^(\d+)x(\d+)\+(\d+)\+(\d+)$/.exec(crop);
    assert.ok(m, `${label}: video-crop should be WxH+X+Y, got ${JSON.stringify(crop)}`);
    assert.ok(
      Number(m[2]) < 180,
      `${label}: detected crop must remove the letterbox bars (h=${m[2]})`,
    );
    if (park !== undefined) {
      await mpv.setProp('time-pos', park);
      await new Promise((r) => setTimeout(r, 1500)); // let Vela observe the position
    }
    mpv.quit();
  } finally {
    mpv.close();
  }
  // Wait for the session to actually end so the Stopped check-in lands
  // before the next leg reads the resume point.
  await pollUntil(
    () => mock.state.checkins.some((c) => c.endpoint === '/Stopped'),
    `${label}: the Stopped check-in`,
  );
}

// Every play reads the configuration again, so the next play has this mode.
function setMode(configRoot, mode) {
  seedConfig(configRoot, [mockSource(mock)], {
    mpv_autocrop: mode,
    // One option per line (Vela's mpv_extra_args parsing). Short shim delay
    // to keep the legs fast — the delay VALUE is not the behavior under
    // test, the trigger firing at all is.
    mpv_extra_args: '--vo=null\n--ao=null\n--script-opts-append=vela-autocrop-delay=1',
  });
}

// Plays the clip from the beginning, runs `body(mpv)` on the running play,
// then quits it and waits for its Stopped check-in.
async function playFromBeginning(driver, label, body) {
  mock.state.checkins.length = 0;
  const before = mpvSocketSnapshot();
  await gridCtxPlay(driver, 'Play from Beginning');
  const mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
  try {
    const firstPos = await pollUntil(
      () => mpv.getProp('time-pos').catch(() => null).then((t) => (t == null ? null : t)),
      `${label}: first time-pos sample`,
    );
    assert.ok(firstPos < 2, `${label} must start fresh (time-pos ${firstPos})`);
    await body(mpv);
    mpv.quit();
  } finally {
    mpv.close();
  }
  await pollUntil(
    () => mock.state.checkins.some((c) => c.endpoint === '/Stopped'),
    `${label}: the Stopped check-in`,
  );
}

// The key mpv's window binds to the stock script's toggle, and to what.
const cropKeyBindings = async (mpv) =>
  (await mpv.getProp('input-bindings'))
    .filter((b) => /\bautocrop\/toggle_crop\b/.test(b.cmd ?? ''))
    .map((b) => b.key);

// No crop may appear for `ms`, checked all the way through.
const noCropFor = (mpv, ms, what) =>
  holdsFor(
    async () => {
      const crop = await mpv.getProp('video-crop').catch(() => null);
      return crop === '' ? null : `video-crop became ${JSON.stringify(crop)}`;
    },
    ms,
    what,
  );

// The Off leg holds for longer than any automatic detection could take to
// crop: the shim's delay here (1 s) or its default (5 s), plus the stock
// script's own 1 s of detection, or the stock script's own trigger (auto_delay
// 4 s plus 1 s) were it loaded with automatic cropping left on.
const OFF_HOLD_MS = 8000;
// The manual leg holds, before the user's toggle, past the same 1 + 1 s of
// this scenario's automatic mode and past the stock trigger's 4 + 1 s.
const MANUAL_HOLD_MS = 6000;

export default {
  name: 'autocrop',
  // The stock autocrop.lua and its shim belong to the external player. The
  // embedded player crops with Vela's own vela-crop.lua instead, and
  // `embeddedcrop` is this scenario's counterpart there.
  onlyPlayer: 'external',
  onlyPlayerWhy:
    "the stock autocrop.lua and Vela's shim are loaded only by the external launch; the embedded player crops with vela-crop.lua, which embeddedcrop shows",
  rows: ['9'],
  testOnly: [
    {
      row: '9',
      player: 'external',
      via: 'mpvInput',
      route: "mpv's `keypress C` command (Shift+C) over the IPC socket, twice, in the Manual leg",
      proves: "that in Manual mode the stock script binds Shift+C in mpv's input layer (read from mpv's input-bindings), that the key crops the letterbox bars away, and that pressing it again takes the crop away",
      unproven: "that a key pressed in mpv's window reaches mpv's input layer (mpv's own window system code; no venue here can type into the window)",
    },
  ],

  async seed({ configRoot }) {
    const clip = makePaddedClip(configRoot);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Letterbox Movie',
        year: 2020,
        runTimeTicks: 300_000_000, // 30s in 100ns ticks, matching the clip
        mediaFile: clip,
      }],
    });
    setMode(configRoot, 'auto');
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot }) {
    await openLibraryGrid(driver, { cardPrefix: 'Letterbox Movie' });

    // Leg 1 — fresh play: shim triggers detection ~1s in; park at 6s so the
    // server stores a resume point for leg 2.
    await playAndAssertCrop(driver, 'fresh', { maxStart: 2, park: 6 });
    await pollUntil(
      () => mock.state.userData.m1.positionTicks > 0,
      'the server-side resume point',
    );
    await screenshot('01-fresh-cropped');

    // Leg 2 — resumed play (--start from the server position): the shim must
    // trigger detection identically. Asserting the start position keeps this
    // leg honest — a stale resume point would silently rerun the fresh case.
    mock.state.checkins.length = 0; // fresh Stopped gate for this leg
    await playAndAssertCrop(driver, 'resume', { minStart: 4 });
    await screenshot('02-resume-cropped');

    // Leg 3 — Off (review finding pa-2): nothing is loaded, so nothing crops
    // by itself, and mpv's window has no crop key.
    setMode(configRoot, 'off');
    await playFromBeginning(driver, 'off', async (mpv) => {
      await noCropFor(mpv, OFF_HOLD_MS, `off: nothing is cropped in ${OFF_HOLD_MS} ms of playback`);
      assert.equal(
        await mpv.getProp('user-data/vela-autocrop/loaded').catch(() => null),
        null,
        'off: the automatic-mode shim is not loaded',
      );
      assert.deepEqual(await cropKeyBindings(mpv), [], "off: no key of mpv's window crops");
    });

    // Leg 4 — Manual (review finding pa-2): nothing crops by itself; the
    // user's Shift+C in mpv's window crops, and pressing it again takes the
    // crop away. The key is sent into mpv's own input layer over IPC
    // (mpvInput, a test-only route: no venue here can type into mpv's
    // window); that it is the key the stock script binds is read from mpv.
    setMode(configRoot, 'manual');
    await playFromBeginning(driver, 'manual', async (mpv) => {
      assert.deepEqual(await cropKeyBindings(mpv), ['C'], "manual: Shift+C in mpv's window is the crop toggle");
      assert.equal(
        await mpv.getProp('user-data/vela-autocrop/loaded').catch(() => null),
        null,
        'manual: the automatic-mode shim is not loaded',
      );
      await noCropFor(mpv, MANUAL_HOLD_MS, `manual: nothing is cropped in ${MANUAL_HOLD_MS} ms without the key`);
      await mpvInput(mpv, 'keypress', 'C');
      const crop = await pollUntil(
        () => mpv.getProp('video-crop').catch(() => '').then((c) => (c ? c : null)),
        'manual: video-crop after Shift+C',
        { timeoutMs: 8000 },
      );
      const m = /^(\d+)x(\d+)\+(\d+)\+(\d+)$/.exec(crop);
      assert.ok(m && Number(m[2]) < 180, `manual: Shift+C crops the letterbox bars away, got ${JSON.stringify(crop)}`);
      await mpvInput(mpv, 'keypress', 'C');
      await pollUntil(
        () => mpv.getProp('video-crop').catch(() => null).then((c) => c === ''),
        'manual: Shift+C again to take the crop away',
        { timeoutMs: 5000 },
      );
    });
    // The end of its proof of its rows in this player (proof.mjs).
    shown('9');
  },
};
