// Probe M2: the end-to-end suite's mock Jellyfin server with one ten-second
// clip, and a Vela configuration in a throwaway home that names it and
// chooses Vela's own window for video. Usage: node serve.mjs <root> <home>.
// Runs until killed.
import fs from 'node:fs';
import path from 'node:path';
import { makeClips, mockSource } from '../../tests/e2e/helpers.mjs';
import { startMockJellyfin } from '../../tests/e2e/mockjf.mjs';

const [root, home] = process.argv.slice(2);
if (!root || !home) throw new Error('usage: node serve.mjs <root> <home>');
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
// directories::ProjectDirs::from("com", "vela", "vela") on macOS.
const configDir = path.join(home, 'Library', 'Application Support', 'com.vela.vela');
fs.mkdirSync(configDir, { recursive: true, mode: 0o700 });
fs.writeFileSync(
  path.join(configDir, 'config.json'),
  JSON.stringify({ sources: [mockSource(mock)], player: 'embedded' }),
  { mode: 0o600 },
);
console.log(`mock Jellyfin on port ${mock.port}; configuration in ${configDir}`);
setInterval(() => {}, 1 << 30);
