// Marker skipping end to end: the app must ask the server for marker ranges,
// hand them to the bundled vela-markers.lua, and act on them per the user's
// per-kind policy — auto-skip seeking on its own, Button offering a control
// that Space activates while it is visible and only while it is visible.
//
// The pointer leg injects a REAL click at the centre of the hitbox the script
// publishes, targeted at mpv's own window. A synthetic key press would prove
// nothing about the hitbox, which is the whole point of that leg.
import assert from 'node:assert/strict';
import path from 'node:path';
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
import { pollUntil, openLibraryGrid, mockSource, seedConfig } from '../helpers.mjs';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const TICKS_PER_MS = 10_000;
let mock;
let configRootPath;

// 30s clip, long enough that a range ending at 12s is reachable but is never
// crossed by ordinary playback inside an assertion window.
function makeClip(configRoot) {
  const mediaDir = path.join(configRoot, 'media');
  fs.mkdirSync(mediaDir, { recursive: true });
  const clip = path.join(mediaDir, 'markers.mp4');
  const ff = spawnSync('ffmpeg', [
    '-f', 'lavfi', '-i', 'testsrc=duration=30:size=320x180:rate=24',
    '-f', 'lavfi', '-i', 'sine=frequency=440:duration=30',
    '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest',
    clip,
  ], { stdio: 'ignore' });
  if (ff.status !== 0) throw new Error('ffmpeg is required to generate the marker clip');
  return clip;
}

function segment(type, startMs, endMs) {
  return {
    Type: type,
    StartTicks: startMs * TICKS_PER_MS,
    EndTicks: endMs * TICKS_PER_MS,
  };
}

// Rewrite only the policies; every play reloads config, so the next launch
// sees them.
function setPolicies(policies) {
  seedConfig(configRootPath, [mockSource(mock)], {
    // A real video output: the skip button is an OSD overlay, and mpv
    // publishes no osd-dimensions under --vo=null, so a null VO would make
    // the button untestable rather than merely invisible.
    mpv_extra_args: '--ao=null',
    ...policies,
  });
}

async function gridPlay(driver) {
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^="Marker Movie"]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu (play)');
  const play = await driver.find(
    'xpath',
    `//button[@role='menuitem' and normalize-space(.)='Play']`,
  );
  await driver.click(play);
}

async function launch(driver) {
  const before = mpvSocketSnapshot();
  await gridPlay(driver);
  const socketPath = await waitForNewMpvSocket(before);
  return MpvIpc.connect(socketPath);
}

// Click at (x, y) inside mpv's own window. Coordinates are window-relative:
// the script publishes its hitbox in OSD space, which is mpv's window pixels.
function clickMpvWindow(x, y) {
  const search = spawnSync('xdotool', ['search', '--class', 'mpv'], { encoding: 'utf8' });
  const ids = (search.stdout || '').trim().split('\n').filter(Boolean);
  assert.ok(ids.length > 0, 'xdotool must find an mpv window to click');
  const windowId = ids[ids.length - 1];
  const click = spawnSync(
    'xdotool',
    ['mousemove', '--window', windowId, String(x), String(y), 'click', '--window', windowId, '1'],
    { encoding: 'utf8' },
  );
  assert.equal(click.status, 0, `xdotool click failed: ${click.stderr}`);
}

async function endSession(mpv) {
  mpv.quit();
  mpv.close();
  await pollUntil(
    () => mock.state.checkins.some((c) => c.endpoint === '/Stopped'),
    'the Stopped check-in',
  );
  mock.state.checkins.length = 0;
}

export default {
  name: 'markers',

  async seed({ configRoot }) {
    configRootPath = configRoot;
    const clip = makeClip(configRoot);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Marker Movie',
        year: 2021,
        runTimeTicks: 300_000_000,
        mediaFile: clip,
      }],
    });
    setPolicies({ skip_intros: 'autoskip', skip_credits: 'off', skip_commercials: 'off' });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    await openLibraryGrid(driver, { cardPrefix: 'Marker Movie' });

    // Leg 1 — auto-skip. The range ends far enough ahead that ordinary
    // playback cannot reach it inside the deadline, so crossing it is only
    // explicable as a seek the script performed.
    mock.state.setMediaSegments('m1', [segment('Intro', 2_000, 12_000)]);
    let mpv = await launch(driver);
    try {
      await pollUntil(
        () => mpv.getProp('user-data/vela-markers/loaded').catch(() => null),
        'auto-skip: the vela-markers load marker',
        { timeoutMs: 8000 },
      );
      await pollUntil(
        () => mpv.getProp('time-pos').catch(() => null).then((t) => (t != null && t >= 12 ? t : null)),
        'auto-skip: time-pos past the end of the intro range',
        { timeoutMs: 12000 },
      );
    } finally {
      await endSession(mpv);
    }
    await screenshot('01-autoskip');

    // Leg 5 — commercial ranges travel the same path. Asserted with auto-skip
    // so it needs no owner media and no on-screen interaction.
    setPolicies({ skip_intros: 'off', skip_credits: 'off', skip_commercials: 'autoskip' });
    mock.state.setMediaSegments('m1', [segment('Commercial', 2_000, 12_000)]);
    mpv = await launch(driver);
    try {
      await pollUntil(
        () => mpv.getProp('time-pos').catch(() => null).then((t) => (t != null && t >= 12 ? t : null)),
        'commercial: time-pos past the end of the commercial range',
        { timeoutMs: 14000 },
      );
    } finally {
      await endSession(mpv);
    }
    await screenshot('02-commercial-autoskip');

    // Leg 3 — Button mode: the control appears with a real hitbox, Space
    // activates it while it is shown, and Space returns to mpv afterwards.
    setPolicies({ skip_intros: 'button', skip_credits: 'off', skip_commercials: 'off' });
    mock.state.setMediaSegments('m1', [segment('Intro', 2_000, 12_000)]);
    mpv = await launch(driver);
    try {
      await pollUntil(
        () => mpv.getProp('user-data/vela-markers/active').catch(() => null)
          .then((v) => (v === 'intro' ? v : null)),
        'button: the active marker property',
        { timeoutMs: 14000 },
      );
      const bounds = await mpv.getProp('user-data/vela-markers/button-bounds');
      assert.ok(
        bounds && bounds.x2 > bounds.x1 && bounds.y2 > bounds.y1,
        `button: a real hitbox must be published, got ${JSON.stringify(bounds)}`,
      );
      await screenshot('03-button-visible');

      // Space while the button is up performs the skip, not a pause.
      await mpv.cmd('keypress', 'SPACE');
      await pollUntil(
        () => mpv.getProp('time-pos').catch(() => null).then((t) => (t != null && t >= 12 ? t : null)),
        'button: Space activated the skip',
        { timeoutMs: 8000 },
      );
      await pollUntil(
        () => mpv.getProp('user-data/vela-markers/active').catch(() => null)
          .then((v) => (v === '' ? 'cleared' : null)),
        'button: the active property clears after the skip',
        { timeoutMs: 6000 },
      );
      assert.equal(
        await mpv.getProp('pause'),
        false,
        'button: Space must have skipped, not paused',
      );

      // Outside every range Space is mpv's again.
      await mpv.cmd('keypress', 'SPACE');
      await pollUntil(
        () => mpv.getProp('pause').catch(() => null).then((p) => (p === true ? p : null)),
        'button: Space pauses normally once the button is gone',
        { timeoutMs: 6000 },
      );
    } finally {
      await endSession(mpv);
    }

    // Leg 2 — the button is genuinely clickable at the hitbox it publishes.
    // A real pointer event, not a key press: this is the only leg that proves
    // the drawn rectangle and the clickable area are the same rectangle.
    mock.state.setMediaSegments('m1', [segment('Intro', 2_000, 12_000)]);
    mpv = await launch(driver);
    try {
      await pollUntil(
        () => mpv.getProp('user-data/vela-markers/active').catch(() => null)
          .then((v) => (v === 'intro' ? v : null)),
        'click: the active marker property',
        { timeoutMs: 14000 },
      );
      const box = await mpv.getProp('user-data/vela-markers/button-bounds');
      clickMpvWindow(
        Math.round((box.x1 + box.x2) / 2),
        Math.round((box.y1 + box.y2) / 2),
      );
      await pollUntil(
        () => mpv.getProp('time-pos').catch(() => null).then((t) => (t != null && t >= 12 ? t : null)),
        'click: the pointer click activated the skip',
        { timeoutMs: 8000 },
      );
      await pollUntil(
        () => mpv.getProp('user-data/vela-markers/active').catch(() => null)
          .then((v) => (v === '' ? 'cleared' : null)),
        'click: the active property clears after the skip',
        { timeoutMs: 6000 },
      );
      await screenshot('04-clicked');
    } finally {
      await endSession(mpv);
    }

    // Leg 4 — injection polarity: with every policy Off the server must never
    // be asked for markers at all.
    setPolicies({ skip_intros: 'off', skip_credits: 'off', skip_commercials: 'off' });
    mock.state.requests.length = 0;
    mpv = await launch(driver);
    try {
      await pollUntil(
        () => mpv.getProp('time-pos').catch(() => null),
        'policies off: playback still starts',
        { timeoutMs: 10000 },
      );
      assert.equal(
        await mpv.getProp('user-data/vela-markers/loaded').catch(() => null),
        null,
        'policies off: the marker script must not be injected at all',
      );
    } finally {
      await endSession(mpv);
    }
    assert.equal(
      mock.state.requests.filter((r) => r.path.startsWith('/MediaSegments/')).length,
      0,
      'policies off: no MediaSegments request may be made',
    );
    assert.deepEqual(
      mock.state.contractViolations,
      [],
      'every MediaSegments request must carry all three includeSegmentTypes filters',
    );
  },
};
