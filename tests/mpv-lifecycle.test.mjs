// Windows mpv, started the way a GUI app starts it, idles at end of file
// instead of exiting, and Vela's whole completion path waits for that exit
// (.agents/plans/windows-mpv-idle-exit.md). The Rust unit test proves the
// option is `--idle=no`; this proves the play path actually passes it, and
// passes it where no user-controlled option can override it. Since the player
// seam (.agents/plans/embedded-player.md, Slice 1) the launch is described in
// player/launch.rs and started in player/external.rs.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file) =>
  fs.readFileSync(path.join(repoRoot, 'src-tauri/src', file), 'utf8').replace(/\r\n/g, '\n');
const playback = read('playback.rs');
const launch = read('player/launch.rs');
const external = read('player/external.rs');

function functionBody(source, signature, what) {
  const start = source.indexOf(`\n${signature}`);
  assert.notEqual(start, -1, `${what} must exist`);
  const end = source.indexOf('\n}\n', start);
  assert.notEqual(end, -1, `${what} must have a closing brace`);
  return source.slice(start, end);
}

test('the play path starts exactly the launch that was described', () => {
  // The ordering below is checked on the function that describes a launch.
  // That only says something about a real play if `play` has that
  // description started, and nothing between the two alters it.
  const play = functionBody(playback, 'pub fn play(', 'playback::play');
  const describe = 'crate::player::launch::build_launch(spec, &cfg, &ipc_path)?';
  const at = play.indexOf(describe);
  assert.notEqual(at, -1, 'play must describe its launch with build_launch');
  assert.equal(play.indexOf('build_launch(', at + describe.length), -1, 'described exactly once');
  assert.match(
    play,
    /\*slot = Some\(crate::player::external::launch\(&mpv_bin, plan\)\?\);/,
    'play must start the described launch',
  );
  assert.doesNotMatch(play, /plan\.(args|env)\b/, 'play must not alter the described launch');

  const start = functionBody(external, 'pub(crate) fn launch(', 'external::launch');
  assert.match(start, /let child = command\(mpv_bin, &plan\)\.spawn\(\)/, 'launch must start the plan');
  assert.doesNotMatch(start, /\.(arg|args|env|envs)\(/, 'launch must not alter the command');

  // The command is the plan's options in the plan's order, its environment,
  // then the URL: one loop over each, and no other argument.
  const command = functionBody(external, 'pub(crate) fn command(', 'external::command');
  assert.match(
    command,
    /for arg in &plan\.args \{\n\s+cmd\.arg\(arg\.to_cli\(\)\);\n\s+\}\n\s+for \(name, value\) in &plan\.env \{\n\s+cmd\.env\(name, value\);\n\s+\}/,
  );
  assert.equal(command.match(/cmd\.arg\(/g).length, 2, 'the options, then `--` and the URL');
  assert.match(command, /cmd\.arg\("--"\)\.arg\(&plan\.url\);\n\s+cmd$/, 'nothing follows the URL');
});

test('the launch asserts the lifecycle options after every user-controlled option', () => {
  const body = functionBody(launch, 'pub(crate) fn build_launch(', 'launch::build_launch');
  const loop = 'args.extend(lifecycle_args())';
  const at = body.indexOf(loop);
  assert.notEqual(at, -1, 'the launch must apply lifecycle_args()');
  assert.equal(body.indexOf(loop, at + 1), -1, 'lifecycle_args() is applied exactly once');

  // Advanced arguments and own-config handling live in base_args; the
  // inherited window state and screen placement are the last option groups
  // before Vela's protected tail.
  for (const earlier of ['base_args(', 'window_state_args(', 'screen_name_arg(']) {
    const last = body.lastIndexOf(earlier);
    assert.notEqual(last, -1, `the launch must still call ${earlier}`);
    assert.ok(last < at, `lifecycle_args() must follow ${earlier}`);
  }
  const ipc = body.indexOf('"input-ipc-server"');
  assert.notEqual(ipc, -1, 'the launch must still set the IPC server');
  assert.ok(at < ipc, 'lifecycle_args() opens the protected tail, before the IPC option');
});

test('Advanced arguments are still applied inside base_args', () => {
  // The ordering assertion above leans on this: if the user's arguments ever
  // move out of base_args, that test must be re-anchored.
  const body = functionBody(launch, 'fn base_args(', 'launch::base_args');
  assert.match(body, /mpv_extra_args/);
  // And they come after every default of Vela's in that function.
  assert.ok(body.lastIndexOf('LaunchArg::set(') < body.indexOf('cfg.mpv_extra_args'));
});