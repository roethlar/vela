// Marker skipping, app side: the parts of the chain this venue can actually
// prove — that Vela asks the server for marker ranges with the right query,
// hands them to the bundled script, and that the script acts on them; and that
// with every policy Off the server is never asked and nothing is injected.
//
// Scope, deliberately: the venue runs mpv with `--vo=null` and never with a
// real video output, so mpv publishes no `osd-dimensions` and the on-screen
// skip button cannot be drawn here. The button's appearance, its hitbox, the
// pointer click, and the temporary Space binding are therefore NOT tested in
// this suite — they are verified directly against real mpv with a real video
// output on a desktop host, recorded in
// `.agents/plans/skip-credits-intros-v2.md` (Slice 4 evidence). Auto-skip needs
// no OSD, so it is the behaviour this scenario asserts.
import assert from 'node:assert/strict';
import path from 'node:path';
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
import { pollUntil, openLibraryGrid, mockSource, seedConfig } from '../helpers.mjs';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { shown } from '../proof.mjs';
import { skipJumpVerdict } from '../judge.mjs';

const TICKS_PER_MS = 10_000;
let mock;
let configRootPath;

// 30s clip: a range ending at 12s is reachable by a seek but is never crossed
// by ordinary playback inside an assertion window.
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
  return { Type: type, StartTicks: startMs * TICKS_PER_MS, EndTicks: endMs * TICKS_PER_MS };
}

// Every play reloads config, so the next launch sees these.
function setPolicies(policies) {
  seedConfig(configRootPath, [mockSource(mock)], policies);
}

async function gridPlay(driver) {
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^="Marker Movie"]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu (play)');
  // Leg 1 leaves the item in progress, so from leg 2 on the menu offers
  // "Resume" / "Play from Beginning" and there is no bare "Play" to find.
  // Every leg must start at zero — the ranges under test sit at 2-12s, ahead
  // of any resume point — so take the explicit from-the-top entry when it is
  // the one on offer.
  const play = await driver.find(
    'xpath',
    `//button[@role='menuitem' and (normalize-space(.)='Play'` +
      ` or normalize-space(.)='Play from Beginning')]`,
  );
  await driver.click(play);
}

async function launch(driver) {
  const before = mpvSocketSnapshot();
  await gridPlay(driver);
  return MpvIpc.connect(await waitForNewMpvSocket(before));
}

// The proof that the script SKIPPED a range (2-12 s), not that the play ran
// past it (review finding pa-1). The play is paused and put at a known
// position, 1 s, just ahead of the range (playing into the range is what
// triggers a skip; a position seen outside it also re-arms a range already
// skipped once). Then it plays on, and its position must be past the range's
// end within JUMP_WINDOW_MS. From 1 s, ordinary playback reaches at most
// about 1 + JUMP_WINDOW_MS / 1000 s in that time, far short of 12 s, so only
// the script's seek explains it. The crossing must also be one jump from the
// range's start to its end: a sample seen well inside the range (from
// rangeStart + IN_RANGE_MARGIN_S on) fails at once, so a skip that lands short
// of the end and lets playback cover the rest does not pass. The margin
// allows for the sample that sees the play just past the start, before the
// script's seek lands. The samples either side of the jump are logged.
// Exported for tests/e2e-judge.test.mjs, which drives it with a stand-in for
// mpv to see that its verdict is the one it acts on.
const JUMP_WINDOW_MS = 5000;
const IN_RANGE_MARGIN_S = 1;
export async function proveSkippedPast(mpv, what, rangeStart, rangeEnd) {
  await pollUntil(
    () => mpv.getProp('time-pos').catch(() => null).then((t) => t != null),
    `${what}: the play to have a position`,
    { timeoutMs: 12000 },
  );
  await mpv.setProp('pause', true);
  await mpv.setProp('time-pos', 1);
  const start = await pollUntil(
    async () => {
      const paused = await mpv.getProp('pause').catch(() => null);
      const t = await mpv.getProp('time-pos').catch(() => null);
      if (paused === true && t != null && t >= 0.9 && t < 1.5) return { t };
      // A skip the play made on its own before the pause can land after this
      // seek and move it on again: put it back.
      await mpv.setProp('time-pos', 1).catch(() => {});
      return null;
    },
    `${what}: the paused play at 1 s, ahead of the range`,
    { timeoutMs: 5000, intervalMs: 100 },
  );
  await mpv.setProp('pause', false);
  const unpausedAt = Date.now();
  // The judgement is skipJumpVerdict's (judge.mjs), held by unit tests; this
  // loop only samples, and asks it after each sample.
  const samples = [];
  const rule = {
    startS: start.t,
    rangeStart,
    rangeEnd,
    windowMs: JUMP_WINDOW_MS,
    marginS: IN_RANGE_MARGIN_S,
  };
  for (;;) {
    const t = await mpv.getProp('time-pos').catch(() => null);
    samples.push({ t, ms: Date.now() - unpausedAt });
    const verdict = skipJumpVerdict(samples, rule);
    if (verdict?.ok === true) {
      console.log(
        `${what}: from ${start.t.toFixed(2)} s, last sample before the jump ${verdict.before.toFixed(2)} s, ` +
          `first after ${verdict.after.toFixed(2)} s, ${verdict.ms} ms after the play resumed`,
      );
      return;
    }
    if (verdict) throw new Error(`${what}: ${verdict.why}`);
    await new Promise((r) => setTimeout(r, 100));
  }
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
  rows: ['8a'],

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

    // Leg 1 — the whole chain: the app must request the ranges, write the
    // payload, inject the script, and the script must seek. proveSkippedPast
    // gives the play too little time to cross the range by playing, so
    // crossing it is only explicable as that seek.
    mock.state.setMediaSegments('m1', [segment('Intro', 2_000, 12_000)]);
    let mpv = await launch(driver);
    try {
      await pollUntil(
        () => mpv.getProp('user-data/vela-markers/loaded').catch(() => null),
        'auto-skip: the script loaded with a payload',
        { timeoutMs: 10000 },
      );
      await proveSkippedPast(mpv, 'auto-skip: the intro range', 2, 12);
    } finally {
      await endSession(mpv);
    }
    await screenshot('01-autoskip');

    const segmentRequests = mock.state.requests.filter((r) => r.path.startsWith('/MediaSegments/'));
    assert.ok(segmentRequests.length > 0, 'the app must ask the server for marker ranges');
    assert.deepEqual(
      mock.state.contractViolations,
      [],
      'every MediaSegments request must carry all three includeSegmentTypes filters',
    );

    // Leg 2 — commercial ranges travel the same path, with no owner media.
    setPolicies({ skip_intros: 'off', skip_credits: 'off', skip_commercials: 'autoskip' });
    mock.state.setMediaSegments('m1', [segment('Commercial', 2_000, 12_000)]);
    mpv = await launch(driver);
    try {
      await pollUntil(
        () => mpv.getProp('user-data/vela-markers/loaded').catch(() => null),
        'commercial: the script loaded with a payload',
        { timeoutMs: 10000 },
      );
      await proveSkippedPast(mpv, 'commercial: the commercial range', 2, 12);
    } finally {
      await endSession(mpv);
    }
    await screenshot('02-commercial-autoskip');

    // Leg 3 — a marker endpoint that fails must cost markers, never playback.
    setPolicies({ skip_intros: 'autoskip', skip_credits: 'off', skip_commercials: 'off' });
    mock.state.mediaSegmentsStatus = 500;
    mpv = await launch(driver);
    try {
      await pollUntil(
        () => mpv.getProp('time-pos').catch(() => null),
        'endpoint failure: playback still starts',
        { timeoutMs: 12000 },
      );
    } finally {
      await endSession(mpv);
    }
    mock.state.mediaSegmentsStatus = null;

    // Leg 4 — injection polarity: every policy Off means the server is never
    // asked and the script is never injected.
    setPolicies({ skip_intros: 'off', skip_credits: 'off', skip_commercials: 'off' });
    mock.state.requests.length = 0;
    mpv = await launch(driver);
    try {
      await pollUntil(
        () => mpv.getProp('time-pos').catch(() => null),
        'policies off: playback still starts',
        { timeoutMs: 12000 },
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
    // The end of its proof of its rows in this player (proof.mjs).
    shown('8a');
  },
};
