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
  },
};
