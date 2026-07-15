// Autocrop trigger wiring (auto mode): the app must inject the stock
// autocrop.lua with its own trigger DISABLED plus Vela's vela-autocrop.lua
// shim, and the shim must fire detection — on a FRESH play and on a RESUMED
// play — so mpv's video-crop property gets set on a letterboxed clip.
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
import { pollUntil, openLibraryGrid, mockSource, seedConfig } from '../helpers.mjs';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

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
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^="Letterbox Movie"]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
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

export default {
  name: 'autocrop',

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
    seedConfig(configRoot, [mockSource(mock)], {
      mpv_autocrop: 'auto',
      // One option per line (Vela's mpv_extra_args parsing). Short shim delay
      // to keep the legs fast — the delay VALUE is not the behavior under
      // test, the trigger firing at all is.
      mpv_extra_args: '--vo=null\n--ao=null\n--script-opts-append=vela-autocrop-delay=1',
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
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
  },
};
