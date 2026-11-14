// The embedded player, end to end (.agents/plans/embedded-player.md, Slice 3).
//
// Runs only in a run of the embedded player (`npm run e2e -- --embedded`;
// tests/e2e/README.md, "The embedded player"). In an ordinary run it is
// skipped, and says so.
//
// The other scenarios, run in that same venue, prove that playback through
// the embedded player does what playback does. This one proves the things
// they cannot see: that the play really is inside Vela's process and drawn by
// mpv's library, that the page turns into the player view and back, and that
// a play can be closed from the page and ends by itself at the end of its
// file.
//
// Every play here is also made to wait two seconds between starting mpv and
// giving it its file (a development build honours the variable set below).
// The embedded player is created first and handed its file once its surface
// exists; mpv's `idle` option decides what happens in between and after. With
// `no` the library shuts down during the wait and nothing plays; with `yes`
// it never shuts down at the end. Both halves are checked here (plan review
// ep-1).
//
// The user's configuration here also tries to take `idle` away from Vela by
// the route that once worked: a profile that contains an include (review
// finding ec-4). mpv applies such an include after the rest of the options
// file the profile was selected in, so a profile must never share a file
// with the options after it. Here it is a profile the user selects in the
// Advanced options; `embeddedownconfig` has the other case, the profile Vela
// itself selects, redefined in the user's own mpv.conf. Only the real
// library can show whether either gets through.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createConnection } from 'node:net';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { makeClips, mockSource, openDetailAndPlay, openLibraryGrid, pollUntil, seedConfig, showPlayerBar } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock;

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

// What the page looks like from inside: is the player view up, and is
// everything else out of the picture's way?
const pageState = (driver) =>
  driver.exec(`
    const app = document.querySelector('.app');
    return {
      open: document.documentElement.classList.contains('player-open'),
      view: !!document.querySelector('.player'),
      close: !!document.querySelector('.player button.close'),
      body: getComputedStyle(document.body).backgroundColor,
      bodyImage: getComputedStyle(document.body).backgroundImage,
      html: getComputedStyle(document.documentElement).backgroundColor,
      app: app ? getComputedStyle(app).visibility : null,
      size: [window.innerWidth, window.innerHeight, window.devicePixelRatio],
    };
  `);

async function startPlay(driver) {
  const before = mpvSocketSnapshot();
  await openLibraryGrid(driver);
  await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
  const socketPath = await waitForNewMpvSocket(before);
  const mpv = await MpvIpc.connect(socketPath);
  // The socket is up as soon as mpv is; the file arrives after the wait. In
  // between, mpv is alive and has nothing loaded.
  const waitedFrom = Date.now();
  assert.equal(
    await mpv.getProp('path').catch(() => null),
    null,
    'mpv has no file yet during the wait',
  );
  await pollUntil(() => mpv.getProp('path').catch(() => null), 'mpv to load the stream after the wait');
  assert.ok(
    Date.now() - waitedFrom >= 1000,
    'the file arrived only after a real wait, which the player survived',
  );
  return { mpv, socketPath };
}

// The play is over for Vela after its last check-in, and the page reloads
// what it shows; see the same wait in playerfallback.mjs.
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

export default {
  name: 'embeddedplay',
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
    // A profile whose include sets `idle=yes`, defined in a file the
    // Advanced options include, and then selected.
    const override = path.join(configRoot, 'idle-yes.conf');
    fs.writeFileSync(override, 'idle=yes\n');
    const profiles = path.join(configRoot, 'profiles.conf');
    fs.writeFileSync(profiles, `[trouble]\ninclude=${override}\n`);
    seedConfig(configRoot, [mockSource(mock)], {
      mpv_extra_args: `--vo=null\n--ao=null\n--include=${profiles}\n--profile=trouble`,
    });
  },

  environment() {
    return { VELA_EMBEDDED_LOAD_DELAY_MS: '2000' };
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock Library')`,
      'seeded library in the sidebar',
    );
    const idle = await pageState(driver);
    assert.equal(idle.open, false, 'no player view before a play');
    assert.equal(idle.app, 'visible');

    // 1. A play starts, and it is the embedded player that runs it.
    let stopsBefore = stopReports();
    let { mpv, socketPath } = await startPlay(driver);
    try {
      // mpv names its video output once the first frame is decoded.
      const output = await pollUntil(
        () => mpv.getProp('current-vo').catch(() => null),
        'mpv to set up its video output',
      );
      assert.equal(output, 'libmpv', "mpv draws through the library's renderer");
      // mpv reports the process it runs in. An external player is a process
      // called mpv; this one is Vela itself.
      const pid = await mpv.getProp('pid');
      assert.equal(
        fs.readFileSync(`/proc/${pid}/comm`, 'utf8').trim(),
        'vela',
        'the player runs inside Vela, not as a program of its own',
      );
      // The seeded --vo=null is a user option; it must not have won. Nor has
      // the seeded profile, whose include says `idle=yes`: with that mpv
      // would never end at the end of a file.
      assert.equal(
        await mpv.getProp('options/idle'),
        'once',
        "Vela's own idle setting stands after everything the user's options bring in",
      );
      // The venue's compositor offers no colour management, so nothing can
      // be agreed with it about HDR: the surface stays as it is and mpv is
      // left to choose what it draws for (the plan, "Colour"). On a real
      // desktop the display decides, so there is nothing to assert there.
      if (!process.env.VELA_E2E_HEADED) {
        for (const option of ['target-trc', 'target-prim', 'target-peak']) {
          assert.equal(await mpv.getProp(option), 'auto', `${option} is left to mpv`);
        }
      }

      // The page has become the player view, and nothing of the app is
      // painted over the picture.
      await driver.waitFor(
        `return document.documentElement.classList.contains('player-open') && !!document.querySelector('.player button.close')`,
        'the player view',
      );
      const playing = await pageState(driver);
      assert.equal(playing.body, 'rgba(0, 0, 0, 0)', 'the page paints no background over the picture');
      assert.equal(playing.html, 'rgba(0, 0, 0, 0)');
      assert.equal(playing.app, 'hidden', 'the app behind the player view is out of the way');

      // Frames are drawn, at the size of the page: mpv's own measure of what
      // it draws into is the page's size in pixels.
      await pollUntil(
        () => mpv.getProp('time-pos').then((t) => t > 0.5).catch(() => false),
        'playback to progress',
      );
      const [width, height, ratio] = playing.size;
      const drawn = await pollUntil(
        () =>
          mpv.getProp('osd-dimensions').then((d) =>
            d.w === Math.round(width * ratio) && d.h === Math.round(height * ratio) ? d : null,
          ).catch(() => null),
        `the picture to be the size of the page (${width}x${height} at ${ratio})`,
      );
      assert.ok(drawn.w > 0 && drawn.h > 0);
      await screenshot('01-playing');

      // 2. Close, from the page. The controls hide after a few seconds of
      //    playing without a pointer move, and a hidden control takes no
      //    click; the pointer brings them back, as it does for a user.
      await showPlayerBar(driver);
      const close = await driver.find('css selector', '.player button.close');
      await driver.click(close);
    } finally {
      mpv.close();
    }
    await pollUntil(async () => !(await accepting(socketPath)), 'the player to stop after Close', { timeoutMs: 5000 });
    await driver.waitFor(
      `return !document.documentElement.classList.contains('player-open') && !document.querySelector('.player')`,
      'the player view to go away after Close',
    );
    const closed = await pageState(driver);
    assert.equal(closed.app, 'visible', 'the app is back');
    assert.notEqual(closed.body, 'rgba(0, 0, 0, 0)', 'and paints its background again');
    await settled(stopsBefore);

    // 3. A second play in the same run, left to reach the end of its file:
    //    the player shuts down by itself and the view goes with it.
    stopsBefore = stopReports();
    ({ mpv, socketPath } = await startPlay(driver));
    try {
      await driver.waitFor(
        `return document.documentElement.classList.contains('player-open')`,
        'the player view for the second play',
      );
      await pollUntil(
        () => mpv.getProp('time-pos').then((t) => t > 0.3).catch(() => false),
        'the second play to progress',
      );
      await mpv.setProp('time-pos', 9);
    } finally {
      mpv.close();
    }
    await pollUntil(async () => !(await accepting(socketPath)), 'the player to end at the end of its file', { timeoutMs: 8000 });
    await driver.waitFor(
      `return !document.documentElement.classList.contains('player-open') && !document.querySelector('.player')`,
      'the player view to go away at the end of the file',
    );
    await settled(stopsBefore);
    assert.equal((await pageState(driver)).app, 'visible');
  },
};
