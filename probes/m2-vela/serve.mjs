// Probes M2 and W2: the end-to-end suite's mock Jellyfin server with one
// ten-second clip, and a Vela configuration that names it and chooses Vela's
// own window for video, written into <config dir> (Vela's ProjectDirs config
// directory on the runner: a throwaway home's on macOS, the disposable
// runner's roaming AppData on Windows). Usage: node serve.mjs <root> <config
// dir>. Runs until killed.
import fs from 'node:fs';
import path from 'node:path';
import { makeClips, mockSource } from '../../tests/e2e/helpers.mjs';
import { startMockJellyfin } from '../../tests/e2e/mockjf.mjs';

const [root, configDir] = process.argv.slice(2);
if (!root || !configDir) throw new Error('usage: node serve.mjs <root> <config dir>');
fs.mkdirSync(root, { recursive: true });
const mediaDir = makeClips(root, ['stream.mp4']);
const mock = await startMockJellyfin({
  movies: [{
    id: 'm1',
    name: 'Mock Movie',
    year: 2020,
    runTimeTicks: 100_000_000,
    mediaFile: path.join(mediaDir, 'stream.mp4'),
  }],
});
fs.mkdirSync(configDir, { recursive: true, mode: 0o700 });
fs.writeFileSync(
  path.join(configDir, 'config.json'),
  JSON.stringify({
    sources: [mockSource(mock)],
    player: 'embedded',
    // mpv's own account of the plays, as the Advanced options would give it.
    // profile=fast: hosted runners draw with a software renderer.
    mpv_extra_args: `--log-file=${path.join(root, 'mpv.log')}\n--msg-level=all=v\n--profile=fast`,
  }),
  { mode: 0o600 },
);
console.log(`mock Jellyfin on port ${mock.port}; configuration in ${configDir}`);
setInterval(() => {}, 1 << 30);
