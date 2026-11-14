// The skip button of the external player, end to end (row 8b of the parity
// list in .agents/plans/embedded-player.md). `embeddedskip` is the embedded
// player's: there the page draws the button; here the bundled marker script
// draws it on mpv's own on-screen display.
//
// The external player only: it is mpv's window the button is drawn in. The
// play here has a real video output on the run's private X display (no
// `--vo=null`, which publishes no OSD size and so draws no button; that is
// why `markers` covers only the automatic skip), and the script draws its
// button, publishes its rectangle (`user-data/vela-markers/button-bounds`, in
// OSD pixels) and binds the left click inside it and Space while it shows.
//
// Pressing it is a TEST-ONLY route: the click and the key are given to mpv's
// own input layer over its IPC socket (mpv's `mouse` and `keypress`
// commands), the layer a click or a key in mpv's window reaches; there is no
// window manager or input tool in this venue to press them in the window.
// What that proves and does not is declared below and listed by the parity
// test.
//
// The play is paused while the button is pressed, so that a position past
// the intro can only be the skip's.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { holdsFor, mockSource, mpvInput, openLibraryGrid, pollUntil, seedConfig, stopPlayAsUser } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { shown } from '../proof.mjs';

const TICKS_PER_MS = 10_000;
const INTRO_END = 12;
let mock;

// 30 s, so the intro (2 to 12 s) is well inside the film.
function makeClip(configRoot) {
  const mediaDir = path.join(configRoot, 'media');
  fs.mkdirSync(mediaDir, { recursive: true });
  const clip = path.join(mediaDir, 'button.mp4');
  const ff = spawnSync('ffmpeg', [
    '-f', 'lavfi', '-i', 'testsrc=duration=30:size=320x180:rate=24',
    '-f', 'lavfi', '-i', 'sine=frequency=440:duration=30',
    '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest',
    clip,
  ], { stdio: 'ignore' });
  if (ff.status !== 0) throw new Error('ffmpeg is required to generate the clip');
  return clip;
}

const active = (mpv) => mpv.getProp('user-data/vela-markers/active');
const bounds = (mpv) => mpv.getProp('user-data/vela-markers/button-bounds');

// Seeks into the intro, holds the play there and waits for the button.
async function buttonShown(mpv, what) {
  await mpv.setProp('pause', true);
  await mpv.setProp('time-pos', 3);
  await pollUntil(() => active(mpv).then((kind) => kind === 'intro').catch(() => false), `${what}: the button to show`);
  const rect = await bounds(mpv);
  const osd = await mpv.getProp('osd-dimensions');
  assert.ok(osd.w > 0 && osd.h > 0, `${what}: mpv has an OSD to draw on (${JSON.stringify(osd)})`);
  assert.ok(
    rect.x1 >= 0 && rect.y1 >= 0 && rect.x1 < rect.x2 && rect.y1 < rect.y2 && rect.x2 <= osd.w && rect.y2 <= osd.h,
    `${what}: the button lies inside the OSD (${JSON.stringify(rect)} in ${osd.w}x${osd.h})`,
  );
  return rect;
}

async function skipped(mpv, what) {
  const position = await pollUntil(
    () => mpv.getProp('time-pos').then((t) => (t >= INTRO_END - 0.05 ? t : null)).catch(() => null),
    `${what}: the play to be past the intro`,
    { timeoutMs: 5000 },
  );
  assert.ok(position < INTRO_END + 1, `${what}: the skip lands at the end of the intro, got ${position}`);
  await pollUntil(() => active(mpv).then((kind) => kind === '').catch(() => false), `${what}: the button to go`);
  // The script sets an empty Lua table, which mpv hands on as an empty list.
  const left = await bounds(mpv);
  assert.equal(Object.keys(left).length, 0, `${what}: no button rectangle once it is gone, got ${JSON.stringify(left)}`);
}

export default {
  name: 'markerbutton',
  onlyPlayer: 'external',
  onlyPlayerWhy:
    "the button is drawn by the marker script on mpv's own OSD, in mpv's window; the embedded player's button is the page's, which embeddedskip shows",
  rows: ['8b'],
  testOnly: [
    {
      row: '8b',
      player: 'external',
      via: 'mpvInput',
      route: "mpv's `mouse` (a left click at the button's centre, and one outside it) and `keypress SPACE` commands over the IPC socket",
      proves: 'that a play with a button policy draws the button on the OSD with its rectangle, that a left click inside it and Space while it shows skip the intro, that a click outside it does not, and that Space is pause again once it is gone',
      unproven: "that a click or key in mpv's window reaches mpv's input layer (mpv's own window system code; no venue here can click into the window)",
    },
  ],

  async seed({ configRoot }) {
    const clip = makeClip(configRoot);
    mock = await startMockJellyfin({
      movies: [{ id: 'm1', name: 'Marker Movie', year: 2021, runTimeTicks: 300_000_000, mediaFile: clip }],
    });
    mock.state.setMediaSegments('m1', [
      { Type: 'Intro', StartTicks: 2_000 * TICKS_PER_MS, EndTicks: INTRO_END * 1000 * TICKS_PER_MS },
    ]);
    // A real video output on the run's private display, and no sound. mpv's
    // default output (gpu-next) did not come up within 15 s on that display
    // (Xvfb, no GPU); `x11` draws the same OSD in software.
    seedConfig(configRoot, [mockSource(mock)], {
      mpv_extra_args: '--ao=null\n--vo=x11',
      skip_intros: 'button',
      skip_credits: 'off',
      skip_commercials: 'off',
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver }) {
    await openLibraryGrid(driver, { cardPrefix: 'Marker Movie' });
    const before = mpvSocketSnapshot();
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Marker Movie"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, 'the context menu');
    await driver.click(await driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Play']`));
    const socketPath = await waitForNewMpvSocket(before);
    const mpv = await MpvIpc.connect(socketPath);
    try {
      await pollUntil(() => mpv.getProp('user-data/vela-markers/loaded').catch(() => null), 'the script to load its markers');
      const vo = await pollUntil(() => mpv.getProp('current-vo').catch(() => null), 'mpv to set up its video output');
      assert.notEqual(vo, 'null', 'the play draws (a real video output)');
      console.log(`markerbutton: video output ${vo}`);

      // 1. The click: outside the button nothing happens; inside, the skip.
      let rect = await buttonShown(mpv, 'click');
      await mpvInput(mpv, 'mouse', 5, 5, 0, 'single');
      // Held over a window, not sampled once after a sleep: a skip that came
      // late would have passed a single look (venue finding atk-7). The play
      // is paused, so its position must not move at all.
      const outsideAt = await mpv.getProp('time-pos');
      await holdsFor(
        async () => {
          const kind = await active(mpv);
          if (kind !== 'intro') return `the button is ${JSON.stringify(kind)} after a click outside it`;
          const position = await mpv.getProp('time-pos');
          if (Math.abs(position - outsideAt) > 0.05) return `the play moved from ${outsideAt} to ${position} after a click outside the button`;
          return false;
        },
        1500,
        'a click outside the button neither skips nor clears it',
      );
      assert.ok(outsideAt < INTRO_END - 1, 'the click outside the button was made in the intro');
      await mpvInput(mpv, 'mouse', Math.floor((rect.x1 + rect.x2) / 2), Math.floor((rect.y1 + rect.y2) / 2), 0, 'single');
      await skipped(mpv, 'click');
      // Space is the player's pause again once the button has gone.
      assert.equal(await mpv.getProp('pause'), true);
      await mpvInput(mpv, 'keypress', 'SPACE');
      await pollUntil(() => mpv.getProp('pause').then((p) => p === false), 'Space to resume the play once the button is gone');

      // 2. Space while the button shows skips instead of pausing. Back into
      //    the intro (outside it first, which the script needs to show the
      //    button for the same intro again).
      rect = await buttonShown(mpv, 'Space');
      assert.ok(rect.x2 > rect.x1);
      await mpvInput(mpv, 'keypress', 'SPACE');
      await skipped(mpv, 'Space');
      assert.equal(await mpv.getProp('pause'), true, 'Space skipped instead of toggling the pause');

      await stopPlayAsUser(driver, mpv, socketPath, 'the play');
    } finally {
      mpv.close();
    }
    // The end of its proof of its rows in this player (proof.mjs).
    shown('8b');
  },
};
