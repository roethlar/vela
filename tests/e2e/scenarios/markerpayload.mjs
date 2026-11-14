// How the bundled marker script is told where its payload is.
//
// The external player hands `vela-markers.lua` the payload path on mpv's
// environment. A player inside Vela's own process has no environment of its
// own, so the script also takes the path as a script option, and the option
// wins when both are given (.agents/plans/embedded-player.md, "The seam").
//
// This scenario drives the REAL script in a REAL mpv directly, without the
// app: what is under test is the script's contract, which both players rely
// on. The app-side chain (ranges fetched, payload written, script injected,
// auto-skip acted on) stays with the `markers` scenario.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { pollUntil } from '../helpers.mjs';
import { MpvIpc } from '../mpv.mjs';

const PAYLOAD = JSON.stringify({ markers: [{ kind: 'intro', start_ms: 1000, end_ms: 2000 }] });

// Start an idle mpv with the script, ask whether it loaded any marker, stop it.
async function scriptLoadsMarkers({ script, dir, name, option, environment }) {
  const socket = path.join(dir, `${name}.sock`);
  const env = { ...process.env };
  delete env.VELA_MARKERS_PAYLOAD;
  if (environment) env.VELA_MARKERS_PAYLOAD = environment;
  const args = [
    '--no-config', '--vo=null', '--ao=null', '--idle=yes',
    `--input-ipc-server=${socket}`,
    `--script=${script}`,
    '--script-opts-append=vela-markers-intro-policy=autoskip',
  ];
  // One appended option, exactly as a launch passes it. `--script-opts=a=b,c=d`
  // would be split at the comma in the path below.
  if (option) args.push(`--script-opts-append=vela-markers-payload=${option}`);
  const mpv = spawn('mpv', args, { env, stdio: 'ignore' });
  try {
    const ipc = await MpvIpc.connect(socket);
    // The script publishes this property as its last act of loading.
    const loaded = await pollUntil(async () => {
      try {
        const value = await ipc.getProp('user-data/vela-markers/loaded');
        return typeof value === 'boolean' ? { value } : null;
      } catch {
        return null; // property not published yet
      }
    }, `${name}: the marker script to finish loading`);
    ipc.quit();
    ipc.close();
    return loaded.value;
  } finally {
    mpv.kill('SIGKILL');
  }
}

// Play a clip with a button policy and the given `draw` option, and record
// what `user-data/vela-markers/active` and `button-bounds` show until playback
// has passed the marker (1 s to 2 s). There is no window (`vo=null`), so mpv
// never publishes an OSD size: a drawn button can never be shown, and only
// the quiet arm of `draw=no` can arm the marker.
async function armsWithoutWindow({ script, dir, name, draw }) {
  const socket = path.join(dir, `${name}.sock`);
  const payload = path.join(dir, `${name}.json`);
  fs.writeFileSync(payload, PAYLOAD, { mode: 0o600 });
  const env = { ...process.env };
  delete env.VELA_MARKERS_PAYLOAD;
  const args = [
    '--no-config', '--vo=null', '--ao=null', '--keep-open=yes',
    `--input-ipc-server=${socket}`,
    `--script=${script}`,
    '--script-opts-append=vela-markers-intro-policy=button',
    `--script-opts-append=vela-markers-payload=${payload}`,
  ];
  if (draw !== undefined) args.push(`--script-opts-append=vela-markers-draw=${draw}`);
  args.push('av://lavfi:testsrc=duration=10:size=320x180:rate=24');
  const mpv = spawn('mpv', args, { env, stdio: 'ignore' });
  try {
    const ipc = await MpvIpc.connect(socket);
    const seen = new Set();
    const bounds = new Set();
    await pollUntil(async () => {
      const active = await ipc.getProp('user-data/vela-markers/active').catch(() => null);
      if (typeof active === 'string') seen.add(active);
      const box = await ipc.getProp('user-data/vela-markers/button-bounds').catch(() => null);
      if (box !== null) bounds.add(JSON.stringify(box));
      return ((await ipc.getProp('time-pos').catch(() => 0)) ?? 0) > 2.3;
    }, `${name}: playback to pass the marker`, { intervalMs: 50 });
    ipc.quit();
    ipc.close();
    return { seen: [...seen].sort(), bounds: [...bounds] };
  } finally {
    mpv.kill('SIGKILL');
  }
}

export default {
  name: 'markerpayload',

  async run({ repoRoot, configRoot }) {
    const script = path.join(repoRoot, 'src-tauri/resources/mpv-scripts/vela-markers.lua');
    assert.ok(fs.existsSync(script), 'the bundled marker script must exist');
    // A directory name with a comma and a space: the characters a comma-split
    // option list would break on.
    const dir = path.join(configRoot, 'payloads, with a comma');
    fs.mkdirSync(dir, { recursive: true });
    const write = (name) => {
      const file = path.join(dir, name);
      fs.writeFileSync(file, PAYLOAD, { mode: 0o600 });
      return file;
    };

    // 1. The script option alone: read, and consumed (the script deletes the
    //    payload once it has read it).
    const viaOption = write('option only.json');
    assert.equal(
      await scriptLoadsMarkers({ script, dir, name: 'option', option: viaOption }),
      true,
      'a payload named by the script option must be loaded',
    );
    assert.ok(!fs.existsSync(viaOption), 'the script must consume the payload it was given');

    // 2. The environment alone: the external player's route, unchanged.
    const viaEnvironment = write('environment only.json');
    assert.equal(
      await scriptLoadsMarkers({ script, dir, name: 'environment', environment: viaEnvironment }),
      true,
      'a payload named by the environment must still be loaded',
    );
    assert.ok(!fs.existsSync(viaEnvironment), 'the environment payload must be consumed');

    // 3. Both: the option wins, and the environment's file is left alone.
    const preferred = write('preferred.json');
    const ignored = write('ignored.json');
    assert.equal(
      await scriptLoadsMarkers({ script, dir, name: 'both', option: preferred, environment: ignored }),
      true,
    );
    assert.ok(!fs.existsSync(preferred), 'with both given, the option is the one read');
    assert.ok(fs.existsSync(ignored), 'with both given, the environment must not be read');

    // 4. The option pointing nowhere must not fall back to the environment:
    //    a launch that named a payload gets that payload or none.
    const fallback = write('fallback.json');
    assert.equal(
      await scriptLoadsMarkers({
        script, dir, name: 'missing',
        option: path.join(dir, 'does not exist.json'),
        environment: fallback,
      }),
      false,
      'a missing option payload must leave the script inert',
    );
    assert.ok(fs.existsSync(fallback), 'the environment is not a fallback for a missing option payload');

    // 5. Neither: inert.
    assert.equal(await scriptLoadsMarkers({ script, dir, name: 'neither' }), false);

    // 6. `draw=no`, as Vela passes it when its page shows the skip button:
    //    the marker is armed with no OSD size, and nothing is drawn.
    const quiet = await armsWithoutWindow({ script, dir, name: 'draw-no', draw: 'no' });
    assert.deepEqual(
      quiet.seen,
      ['', 'intro'],
      'with draw=no the marker must be armed (active = its kind) without any OSD size',
    );
    // (An empty Lua table reaches IPC as an empty JSON array.)
    assert.deepEqual(quiet.bounds, ['[]'], 'with draw=no nothing is drawn, so button-bounds stays empty');

    // 7. `draw=yes`, and the option left out (the external player): the
    //    button waits for an OSD size, which never comes without a window.
    for (const [name, draw] of [['draw-yes', 'yes'], ['draw-default', undefined]]) {
      const drawn = await armsWithoutWindow({ script, dir, name, draw });
      assert.deepEqual(
        drawn.seen,
        [''],
        `${name}: a button that has to be drawn must not be armed without an OSD size`,
      );
    }
  },
};
