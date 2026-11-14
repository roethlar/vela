// What the bundled crop script `vela-crop.lua` does in a real mpv.
//
// Like `markerpayload`, this drives the REAL script in a REAL mpv directly,
// without the app: what is under test is the script's contract (measure the
// bars, crop to the picture, refuse what it must not touch, keep playback
// going). The app-side chain (the script chosen and its mode passed for a
// play) is held by the launch tests in Rust, and the crop control by the
// scenarios that drive the player view.
//
// mpv's `bbox` filter, which the script measures with, is in the system mpv
// this venue uses. The settle delay is shortened the way a user would, with
// `--script-opts-append=vela-crop-delay=1`.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { holdsFor, pollUntil } from '../helpers.mjs';
import { MpvIpc } from '../mpv.mjs';

// A 1280x720 frame whose picture is 1280x544, with black bars of 88 rows above
// and below: the crop the script must find.
const LETTERBOXED = '1280x544+0+88';
// Long enough that a crop late in a leg still has well over two seconds left.
const SECONDS = 30;

function makeClip(dir, name, { bars, tenBit = false, pq = false }) {
  const file = path.join(dir, name);
  const picture = bars ? '1280x544' : '1280x720';
  const args = ['-y', '-f', 'lavfi', '-i', `testsrc2=size=${picture}:rate=24:duration=${SECONDS}`];
  const filters = [];
  if (bars) filters.push('pad=1280:720:0:88:black');
  // Tagged in the stream itself, which is where mpv reads the transfer from.
  if (pq) filters.push('setparams=color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc');
  if (filters.length) args.push('-vf', filters.join(','));
  args.push('-c:v', 'libx264', '-preset', 'ultrafast', '-pix_fmt', tenBit ? 'yuv420p10le' : 'yuv420p');
  args.push(file);
  const ff = spawnSync('ffmpeg', args, { stdio: 'ignore' });
  if (ff.status !== 0) throw new Error(`ffmpeg is required to generate ${name}`);
  return file;
}

async function startMpv({ script, dir, name, clip, auto }) {
  const socket = path.join(dir, `${name}.sock`);
  const args = [
    // Copy-back decoding where the machine has a hardware decoder: there a
    // crop with nothing after it halts playback every time, so the checks
    // that playback goes on after a crop have something to catch. (Software
    // decoding is used where there is none, and halts only now and then.)
    '--no-config', '--vo=null', '--ao=null', '--hwdec=auto-copy', '--keep-open=yes',
    // With no window, what mpv would show on screen goes to its terminal,
    // which is how a message from the script is read below.
    '--term-osd=force',
    `--input-ipc-server=${socket}`,
    `--script=${script}`,
    '--script-opts-append=vela-crop-delay=1',
  ];
  // `undefined` passes no mode at all: the script's own default must hold.
  if (auto !== undefined) args.push(`--script-opts-append=vela-crop-auto=${auto ? 'yes' : 'no'}`);
  args.push(clip);
  const mpv = spawn('mpv', args, { stdio: ['ignore', 'pipe', 'ignore'] });
  let terminal = '';
  mpv.stdout.setEncoding('utf8');
  mpv.stdout.on('data', (chunk) => {
    terminal += chunk;
  });
  let ipc;
  try {
    ipc = await MpvIpc.connect(socket);
  } catch (err) {
    mpv.kill('SIGKILL');
    throw err;
  }
  const stop = () => {
    ipc.quit();
    ipc.close();
    mpv.kill('SIGKILL');
  };
  try {
    await pollUntil(
      async () => (await ipc.getProp('time-pos').catch(() => null)) > 0.2,
      `${name}: playback to start`,
    );
  } catch (err) {
    stop();
    throw err;
  }
  return { ipc, stop, terminal: () => terminal };
}

const state = (ipc) => ipc.getProp('user-data/vela-crop/state').catch(() => null);
const crop = (ipc) => ipc.getProp('video-crop');
const toggle = (ipc) => ipc.cmd('script-binding', 'vela_crop/toggle');

async function waitForCrop(ipc, expected, what) {
  await pollUntil(
    async () => (await state(ipc)) === 'on' && (await crop(ipc)) === expected,
    `${what}: video-crop ${expected} with the state "on"`,
    { timeoutMs: 12000 },
  );
  assert.deepEqual(await ipc.getProp('vf'), [], `${what}: the measuring filter is removed`);
}

// A crop can halt playback in mpv (the position stops while mpv says it is not
// idle); the script follows every crop with what gets it going again.
async function stillPlaying(ipc, what) {
  const before = await ipc.getProp('time-pos');
  await new Promise((r) => setTimeout(r, 2000));
  const after = await ipc.getProp('time-pos');
  assert.ok(
    after - before > 1,
    `${what}: playback must still advance two seconds after the crop (${before} -> ${after})`,
  );
}

// Nothing measured and nothing cropped for `ms`, with the state "off" throughout.
async function staysUncropped(ipc, ms, what, { measuring = false } = {}) {
  await holdsFor(async () => {
    const current = await crop(ipc);
    if (current !== '') return `video-crop became ${current}`;
    const now = await state(ipc);
    if (now === 'on' || (!measuring && now !== 'off')) return `the state became ${now}`;
    return null;
  }, ms, what);
}

export default {
  name: 'cropscript',
  rows: [],
  noRows: "the contract of vela-crop.lua, run in the system's mpv without the app; cropping in the app (row 9) is autocrop's and embeddedcrop's",

  async run({ repoRoot, configRoot }) {
    const script = path.join(repoRoot, 'src-tauri/resources/mpv-scripts/vela-crop.lua');
    assert.ok(fs.existsSync(script), 'the bundled crop script must exist');
    const dir = path.join(configRoot, 'crop');
    fs.mkdirSync(dir, { recursive: true });
    const lb8 = makeClip(dir, 'letterboxed-8.mp4', { bars: true });
    const lb10 = makeClip(dir, 'letterboxed-10.mkv', { bars: true, tenBit: true });
    const noBars = makeClip(dir, 'no-bars.mp4', { bars: false });
    const pq = makeClip(dir, 'letterboxed-pq.mkv', { bars: true, tenBit: true, pq: true });

    // 1. No mode given: the default is to crop only on request. Past the
    //    delay plus the measurement, nothing has happened; then a toggle crops.
    {
      const { ipc, stop } = await startMpv({ script, dir, name: 'default', clip: lb8 });
      try {
        assert.equal(await state(ipc), 'off');
        await staysUncropped(ipc, 4000, 'with no mode given nothing may crop by itself');
      } finally {
        stop();
      }
    }

    // 2. Manual, 8-bit: nothing by itself; a toggle crops, a toggle during the
    //    measurement is ignored, a second toggle removes the crop.
    {
      const { ipc, stop } = await startMpv({ script, dir, name: 'manual', clip: lb8, auto: false });
      try {
        await staysUncropped(ipc, 4000, 'manual mode must not crop by itself');
        await toggle(ipc);
        await pollUntil(async () => (await state(ipc)) === 'measuring', 'manual: the measurement to start');
        // Ignored: it must neither cancel the measurement nor remove anything.
        await toggle(ipc);
        await waitForCrop(ipc, LETTERBOXED, 'manual');
        await stillPlaying(ipc, 'manual');

        await toggle(ipc);
        await pollUntil(
          async () => (await state(ipc)) === 'off' && (await crop(ipc)) === '',
          'manual: a second toggle to remove the crop',
        );
        await stillPlaying(ipc, 'manual, after the crop was removed');

        // And it crops again on request.
        await toggle(ipc);
        await waitForCrop(ipc, LETTERBOXED, 'manual, cropped again');
        await stillPlaying(ipc, 'manual, cropped again');
      } finally {
        stop();
      }
    }

    // 3. Automatic, 8-bit and 10-bit: the crop arrives by itself after the delay.
    for (const [name, clip] of [['auto-8', lb8], ['auto-10', lb10]]) {
      const { ipc, stop } = await startMpv({ script, dir, name, clip, auto: true });
      try {
        await waitForCrop(ipc, LETTERBOXED, name);
        await stillPlaying(ipc, name);
      } finally {
        stop();
      }
    }

    // 4. No bars: measured, and nothing cropped.
    {
      const { ipc, stop } = await startMpv({ script, dir, name: 'no-bars', clip: noBars, auto: true });
      try {
        await pollUntil(async () => (await state(ipc)) === 'measuring', 'no bars: the measurement to start');
        await pollUntil(async () => (await state(ipc)) === 'off', 'no bars: the measurement to end');
        assert.equal(await crop(ipc), '', 'a frame with no bars must not be cropped');
        assert.deepEqual(await ipc.getProp('vf'), [], 'no bars: the measuring filter is removed');
        await stillPlaying(ipc, 'no bars');
      } finally {
        stop();
      }
    }

    // 5. HDR (PQ): refused, automatically and on request, never measured.
    {
      const { ipc, stop, terminal } = await startMpv({ script, dir, name: 'pq', clip: pq, auto: true });
      const told = () => terminal().split('Cropping is not available for HDR video').length - 1;
      try {
        assert.equal(await ipc.getProp('video-params/gamma'), 'pq', 'the clip must be tagged PQ');
        // Past the delay and the three seconds the message stays up.
        await staysUncropped(ipc, 5500, 'an HDR clip must not be measured by itself');
        assert.ok(told() > 0, 'the automatic refusal must say why on screen');
        const before = told();
        await toggle(ipc);
        await staysUncropped(ipc, 3000, 'an HDR clip must not be measured on request');
        assert.ok(told() > before, 'a refused request must say why on screen');
        assert.deepEqual(await ipc.getProp('vf'), [], 'no filter was inserted for HDR');
      } finally {
        stop();
      }
    }

    // 6. A new file in the same mpv starts uncropped, and automatic mode
    //    measures it on its own: the first file's rectangle is not its picture.
    {
      const { ipc, stop } = await startMpv({ script, dir, name: 'next-file', clip: lb8, auto: true });
      try {
        await waitForCrop(ipc, LETTERBOXED, 'next file, the first');
        await ipc.cmd('loadfile', noBars, 'replace');
        await pollUntil(
          async () => (await crop(ipc)) === '' && (await state(ipc)) === 'off',
          'next file: the crop of the first file to be removed',
        );
        await pollUntil(async () => (await state(ipc)) === 'measuring', 'next file: its own measurement');
        await pollUntil(async () => (await state(ipc)) === 'off', 'next file: its measurement to end');
        assert.equal(await crop(ipc), '', 'the second file has no bars and stays uncropped');
        await stillPlaying(ipc, 'next file');
      } finally {
        stop();
      }
    }
  },
};
