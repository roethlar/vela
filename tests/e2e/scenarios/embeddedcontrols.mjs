// The player view's controls and keys, end to end in the embedded player
// (.agents/plans/embedded-player.md, Slice 5).
//
// Runs only in a run of the embedded player (`npm run e2e -- --embedded`).
// The player takes no input of its own there: every key and every control
// goes from the page to the backend's `player_command`, and the backend's
// `player-state` events are all the page knows about the play. This scenario
// drives both ends for real: the controls are clicked with WebDriver, the keys
// are pressed with WebDriver's Actions (trusted key events, as a user's), and
// what happened is read from mpv itself over its IPC socket, never from what
// the page claims.
//
// Covered: the state arrives; pause, the seek bar, seeking by key in both
// directions, mute, volume, fullscreen on and off (the page's real height and
// mpv's drawing size, not Tauri's answer) and stop, through the controls and
// through the keys.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { createConnection } from 'node:net';
import { Driver } from '../driver.mjs';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  mockSource,
  openDetailAndPlay,
  openLibraryGrid,
  pollUntil,
  recordPlayerEvents,
  seedConfig,
  showPlayerBar,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const { KEY } = Driver;
// Long enough for a +60 s key seek from the middle, with a key frame every
// second so that mpv's key-frame seeks by key land within a second of the
// target.
const SECONDS = 180;
let mock;

function makeClip(configRoot) {
  const mediaDir = path.join(configRoot, 'media');
  fs.mkdirSync(mediaDir, { recursive: true });
  const clip = path.join(mediaDir, 'controls.mp4');
  const ff = spawnSync('ffmpeg', [
    '-f', 'lavfi', '-i', `testsrc=duration=${SECONDS}:size=320x180:rate=24`,
    '-f', 'lavfi', '-i', `sine=frequency=440:duration=${SECONDS}`,
    '-c:v', 'libx264', '-preset', 'ultrafast', '-g', '24', '-pix_fmt', 'yuv420p',
    '-c:a', 'aac', '-shortest',
    clip,
  ], { stdio: 'ignore' });
  if (ff.status !== 0) throw new Error('ffmpeg is required to generate the controls clip');
  return clip;
}

const stopReports = () =>
  mock.state.served.filter((r) => r.method === 'POST' && r.path === '/Sessions/Playing/Stopped').length;

const accepting = (socketPath) =>
  new Promise((resolve) => {
    const probe = createConnection(socketPath);
    probe.once('connect', () => {
      probe.destroy();
      resolve(true);
    });
    probe.once('error', () => resolve(false));
  });

// The play is over for Vela after its last check-in and the page's reload.
async function settled(stopReportsBefore) {
  await pollUntil(() => stopReports() > stopReportsBefore, 'the stop report of the play that ended');
  let asked = mock.state.requests.length;
  let quietSince = Date.now();
  await pollUntil(
    () => {
      if (mock.state.requests.length !== asked) {
        asked = mock.state.requests.length;
        quietSince = Date.now();
        return false;
      }
      return Date.now() - quietSince >= 700;
    },
    'the page to finish reloading after the play',
    { intervalMs: 100 },
  );
}

// Starts a play and returns once the view has its first state.
async function startPlay(driver) {
  const before = mpvSocketSnapshot();
  await openLibraryGrid(driver);
  await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
  const socketPath = await waitForNewMpvSocket(before);
  const mpv = await MpvIpc.connect(socketPath);
  await pollUntil(
    () => mpv.getProp('time-pos').then((t) => t > 0.5).catch(() => false),
    'playback to progress',
  );
  await driver.waitFor(
    `const b = document.querySelector('.player button.playpause'); return !!b && !b.disabled`,
    'the player view to have its first state',
  );
  return { mpv, socketPath };
}

async function stopped(driver, socketPath, what) {
  await pollUntil(async () => !(await accepting(socketPath)), `the player to stop after ${what}`, {
    timeoutMs: 5000,
  });
  await driver.waitFor(
    `return !document.documentElement.classList.contains('player-open') && !document.querySelector('.player')`,
    `the player view to go away after ${what}`,
  );
}

const click = async (driver, selector) => driver.click(await driver.find('css selector', selector));

const attr = (driver, selector, name) =>
  driver.exec(`return document.querySelector(arguments[0])?.getAttribute(arguments[1]) ?? null`, [
    selector,
    name,
  ]);

// A range control moved to `value` and let go: what the page sees from a
// drag, without depending on where the thumb is drawn.
const setRange = (driver, selector, value) =>
  driver.exec(
    `const el = document.querySelector(arguments[0]);
     el.value = String(arguments[1]);
     el.dispatchEvent(new Event('input', { bubbles: true }));
     el.dispatchEvent(new Event('change', { bubbles: true }));
     return true;`,
    [selector, value],
  );

const position = (mpv) => mpv.getProp('time-pos');

// Waits for mpv's position to settle inside [low, high].
const positionBetween = (mpv, low, high, what) =>
  pollUntil(
    () => position(mpv).then((t) => (t >= low && t <= high ? t : null)).catch(() => null),
    `${what} (time-pos within ${low}..${high})`,
    { timeoutMs: 6000 },
  );

const pageSize = (driver) =>
  driver.exec(`return [window.innerWidth, window.innerHeight, window.devicePixelRatio]`);

// mpv draws at the size of the page, in pixels.
async function drawnAtPageSize(driver, mpv, what) {
  let last = null;
  let page = null;
  try {
    await pollUntil(
      async () => {
        page = await pageSize(driver);
        const [width, height, ratio] = page;
        last = await mpv.getProp('osd-dimensions').catch(() => null);
        return last && last.w === Math.round(width * ratio) && last.h === Math.round(height * ratio);
      },
      `${what}: mpv to draw at the page's size`,
      { timeoutMs: 5000 },
    );
  } catch (err) {
    err.message += ` (page ${JSON.stringify(page)}, mpv ${last ? `${last.w}x${last.h}` : 'none'})`;
    throw err;
  }
  console.log(`${what}: page ${page[0]}x${page[1]}, mpv ${last.w}x${last.h}`);
  return page[1];
}

export default {
  name: 'embeddedcontrols',
  onlyPlayer: 'embedded',
  onlyPlayerWhy:
    "the player view's controls and keys exist only in the embedded player (the plan's inventory: embedded-only by nature)",
  rows: [],
  noRows: "the player view's controls; the stop they make is the embedded user's stop, which the scenarios of rows 1 to 6 use through stopPlayAsUser",

  async seed({ configRoot }) {
    const clip = makeClip(configRoot);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Mock Movie',
        year: 2020,
        runTimeTicks: SECONDS * 10_000_000,
        mediaFile: clip,
      }],
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock Library')`,
      'seeded library in the sidebar',
    );
    await recordPlayerEvents(driver);

    // 1. The state arrives, for this play, with what the controls show.
    let stopsBefore = stopReports();
    let { mpv, socketPath } = await startPlay(driver);
    try {
      const first = await driver.exec(
        `return window.__playerEvents.states.find((s) => s.position !== null) ?? null`,
      );
      assert.ok(first, 'a player-state with a position');
      assert.deepEqual(
        Object.keys(first).sort(),
        ['audioTracks', 'crop', 'duration', 'muted', 'paused', 'position', 'sessionId', 'skip', 'subtitleTracks', 'volume'],
      );
      assert.equal(first.paused, false);
      assert.equal(first.crop, null, 'no crop script, no crop control');
      assert.equal(await driver.exec(`return !!document.querySelector('.player button.crop')`), false);
      assert.equal(Math.round(first.duration), SECONDS);

      // 2. Pause, through the control.
      await showPlayerBar(driver);
      await click(driver, '.player button.playpause');
      await pollUntil(() => mpv.getProp('pause'), 'the Pause control to pause mpv', { timeoutMs: 5000 });
      await driver.waitFor(
        `return document.querySelector('.player button.playpause').getAttribute('aria-label') === 'Play'`,
        'the control to offer Play once paused',
        { timeoutMs: 5000 },
      );

      // 3. The seek bar seeks where it is let go.
      await setRange(driver, '.player input.seek', 60);
      await positionBetween(mpv, 59.5, 60.5, 'the seek bar');
      await driver.waitFor(
        `return document.querySelector('.player span.time').textContent.trim() === '1:00 / 3:00'`,
        'the time to read 1:00 / 3:00',
        { timeoutMs: 5000 },
      );

      // 4. The seek keys, each in its own direction.
      await driver.pressKeys(KEY.ArrowRight);
      await positionBetween(mpv, 63.5, 66.5, 'Right seeks five seconds forward');
      await driver.pressKeys(KEY.ArrowLeft);
      await positionBetween(mpv, 58.5, 61.5, 'Left seeks five seconds back');
      await driver.pressKeys(KEY.ArrowUp);
      await positionBetween(mpv, 118.5, 121.5, 'Up seeks a minute forward');
      await driver.pressKeys(KEY.ArrowDown);
      await positionBetween(mpv, 58.5, 61.5, 'Down seeks a minute back');

      // 5. Space plays again.
      await driver.pressKeys(KEY.Space);
      await pollUntil(
        () => mpv.getProp('pause').then((p) => p === false),
        'Space to resume playback',
        { timeoutMs: 5000 },
      );

      // 6. Mute through the control, unmute by key.
      await showPlayerBar(driver);
      await click(driver, '.player button.mute');
      await pollUntil(() => mpv.getProp('mute'), 'the Mute control to mute mpv', { timeoutMs: 5000 });
      await driver.waitFor(
        `return document.querySelector('.player button.mute').getAttribute('aria-pressed') === 'true'`,
        'the Mute control to show pressed',
        { timeoutMs: 5000 },
      );
      await driver.pressKeys('m');
      await pollUntil(() => mpv.getProp('mute').then((m) => m === false), 'm to unmute', { timeoutMs: 5000 });

      // 7. Volume, through its slider.
      await showPlayerBar(driver);
      await setRange(driver, '.player input.volume', 40);
      await pollUntil(() => mpv.getProp('volume').then((v) => v === 40), 'the slider to set volume 40', {
        timeoutMs: 5000,
      });

      // 8. Fullscreen. What counts is the window the compositor really made
      //    (the page's own size, and mpv's drawing size following it), not
      //    what Tauri says it asked for.
      const windowed = await drawnAtPageSize(driver, mpv, 'windowed');
      await showPlayerBar(driver);
      await click(driver, '.player button.fullscreen');
      await driver.waitFor(
        `return window.innerHeight > ${windowed}`,
        'the Fullscreen control to fill the screen',
        { timeoutMs: 5000 },
      );
      const full = await drawnAtPageSize(driver, mpv, 'fullscreen');
      assert.ok(full > windowed);
      assert.equal(await attr(driver, '.player button.fullscreen', 'aria-pressed'), 'true');
      assert.ok(
        await driver.exec(`return window.__playerEvents.fullscreen.some((e) => e.on === true)`),
        'the window reported fullscreen',
      );
      await screenshot('01-fullscreen');
      await driver.pressKeys('f');
      await driver.waitFor(
        `return window.innerHeight === ${windowed}`,
        'f to leave fullscreen',
        { timeoutMs: 5000 },
      );
      await drawnAtPageSize(driver, mpv, 'windowed again');
      await driver.pressKeys('f');
      await driver.waitFor(`return window.innerHeight > ${windowed}`, 'f to enter fullscreen', {
        timeoutMs: 5000,
      });
      // Escape in fullscreen leaves fullscreen and does not stop the play.
      await driver.pressKeys(KEY.Escape);
      await driver.waitFor(
        `return window.innerHeight === ${windowed}`,
        'Escape to leave fullscreen',
        { timeoutMs: 5000 },
      );
      await drawnAtPageSize(driver, mpv, 'after Escape');
      assert.equal(await accepting(socketPath), true, 'Escape in fullscreen leaves the play running');
      assert.equal(await attr(driver, '.player button.fullscreen', 'aria-pressed'), 'false');

      // 9. Space pauses as well as plays.
      await driver.pressKeys(KEY.Space);
      await pollUntil(() => mpv.getProp('pause'), 'Space to pause', { timeoutMs: 5000 });

      // 10. q stops the play.
      await driver.pressKeys('q');
    } finally {
      mpv.close();
    }
    await stopped(driver, socketPath, 'q');
    await settled(stopsBefore);

    // 11. A second play, stopped with the Close control while it plays.
    stopsBefore = stopReports();
    ({ mpv, socketPath } = await startPlay(driver));
    try {
      await showPlayerBar(driver);
      await click(driver, '.player button.close');
    } finally {
      mpv.close();
    }
    await stopped(driver, socketPath, 'Close');
    await settled(stopsBefore);

    // Nothing from mpv beyond the closed sets reached the page.
    const states = await driver.exec(`return JSON.stringify(window.__playerEvents.states)`);
    assert.ok(!states.includes('http'), 'no URL in any player-state');
    assert.ok(!states.includes('/'), 'no path in any player-state');
  },
};
