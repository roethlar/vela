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

function functionBody(signature, what) {
  const start = playback.indexOf(`\n${signature}`);
  assert.notEqual(start, -1, `${what} must exist`);
  const end = playback.indexOf('\n}\n', start);
  assert.notEqual(end, -1, `${what} must have a closing brace`);
  return playback.slice(start, end);
}

test('the play path launches exactly the composed command', () => {
  // The ordering below is checked on the function that composes the command.
  // That only says something about a real play if `play` starts that command
  // and adds nothing to it.
  const body = functionBody('pub fn play(', 'playback::play');
  const call = 'compose_mpv_command(spec, &cfg, &mpv_bin, &ipc_path)?';
  const at = body.indexOf(call);
  assert.notEqual(at, -1, 'play must compose its command with compose_mpv_command');
  assert.equal(body.indexOf('compose_mpv_command(', at + 1), -1, 'composed exactly once');
  assert.match(body, /let child = cmd\.spawn\(\)/, 'play must start the composed command');
  assert.doesNotMatch(body, /cmd\.(arg|args|env|envs)\(/, 'play must not alter the composed command');
});

test('the play path asserts the lifecycle options after every user-controlled option', () => {
  const body = functionBody('fn compose_mpv_command(', 'playback::compose_mpv_command');
  const loop = 'for arg in lifecycle_args()';
  const at = body.indexOf(loop);
  assert.notEqual(at, -1, 'the composed command must apply lifecycle_args()');
  assert.equal(body.indexOf(loop, at + 1), -1, 'lifecycle_args() is applied exactly once');

  // Advanced arguments and own-config handling live in base_mpv_command; the
  // inherited window state and screen placement are the last option groups
  // before Vela's protected tail.
  for (const earlier of ['base_mpv_command(', 'window_state_args(', 'screen_name_arg(']) {
    const last = body.lastIndexOf(earlier);
    assert.notEqual(last, -1, `the composed command must still call ${earlier}`);
    assert.ok(last < at, `lifecycle_args() must follow ${earlier}`);
  }
  const ipc = body.indexOf('--input-ipc-server=');
  assert.notEqual(ipc, -1, 'the composed command must still set the IPC server');
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
