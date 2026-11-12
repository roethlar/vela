// Windows mpv, started the way a GUI app starts it, idles at end of file
// instead of exiting, and Vela's whole completion path waits for that exit
// (.agents/plans/windows-mpv-idle-exit.md). The Rust unit test proves the
// option is `--idle=no`; this proves the play path actually passes it, and
// passes it where no user-controlled option can override it.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const playback = fs
  .readFileSync(path.join(repoRoot, 'src-tauri/src/playback.rs'), 'utf8')
  .replace(/\r\n/g, '\n');

function playBody() {
  const start = playback.indexOf('\npub fn play(');
  assert.notEqual(start, -1, 'playback::play must exist');
  const end = playback.indexOf('\n}\n', start);
  assert.notEqual(end, -1, 'playback::play must have a closing brace');
  return playback.slice(start, end);
}

test('the play path asserts the lifecycle options after every user-controlled option', () => {
  const body = playBody();
  const loop = 'for arg in lifecycle_args()';
  const at = body.indexOf(loop);
  assert.notEqual(at, -1, 'play must apply lifecycle_args()');
  assert.equal(body.indexOf(loop, at + 1), -1, 'lifecycle_args() is applied exactly once');

  // Advanced arguments and own-config handling live in base_mpv_command; the
  // inherited window state and screen placement are the last option groups
  // before Vela's protected tail.
  for (const earlier of ['base_mpv_command(', 'window_state_args(', 'screen_name_arg(']) {
    const last = body.lastIndexOf(earlier);
    assert.notEqual(last, -1, `play must still call ${earlier}`);
    assert.ok(last < at, `lifecycle_args() must follow ${earlier}`);
  }
  const ipc = body.indexOf('--input-ipc-server=');
  assert.notEqual(ipc, -1, 'play must still set the IPC server');
  assert.ok(at < ipc, 'lifecycle_args() opens the protected tail, before the IPC option');
});

test('Advanced arguments are still applied inside base_mpv_command', () => {
  // The ordering assertion above leans on this: if the user's arguments ever
  // move out of base_mpv_command, that test must be re-anchored.
  const start = playback.indexOf('\nfn base_mpv_command(');
  assert.notEqual(start, -1);
  const body = playback.slice(start, playback.indexOf('\n}\n', start));
  assert.match(body, /mpv_extra_args/);
});
