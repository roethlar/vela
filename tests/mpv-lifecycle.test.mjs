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
  // One place starts the player, from the one described launch, with the
  // launcher of the player the play was given: the external program, or the
  // step the embedded launcher prepared.
  // The external player, chosen: under the lock on the player slot, after
  // the check that Vela is not exiting, the described launch is started and
  // published in one statement.
  assert.match(
    play,
    /let started_player = match \(embedded_start, mpv_bin\) \{\s+\(None, None\) => return Err\(NO_EMBEDDED_PLAYER\.to_string\(\)\),\s+\(None, Some\(mpv_bin\)\) => \{\s+let mut slot = player_slot\.lock\(\)\.unwrap_or_else\(\|e\| e\.into_inner\(\)\);\s+if shutting_down\.load\(Ordering::SeqCst\) \{\s+plan\.discard_marker_payload\(\);\s+return Err\(SHUTTING_DOWN\.to_string\(\)\);\s+\}\s+\*slot = Some\(crate::player::external::launch\(&mpv_bin, plan\)\?\);\s+crate::player::PlayerKind::External\s+\}/,
    'play must start the described launch',
  );
  // The embedded player: the same lock, the same check, then the step the
  // embedded launcher prepared, given the same described launch.
  assert.match(
    play,
    /let player = match prepared \{\s+Ok\(start\) => start\(plan, own_config, &ipc_path\),/,
  );
  assert.match(play, /let own_config = cfg\.mpv_use_own_config\.unwrap_or\(false\);/);
  // The described launch is handed over whole, to one player or the other;
  // all play itself ever does with one is throw its marker payload away
  // when it will not be started.
  assert.doesNotMatch(play, /\bplan\.(?!discard_marker_payload\(\);)/, 'play must not alter a described launch');
  assert.equal(play.split('external::launch(&mpv_bin, plan)').length - 1, 2, 'the external player, chosen or fallen back to');
  assert.equal(play.split('start(plan, ').length - 1, 1, 'the embedded player');
  assert.match(playback, /^const SHUTTING_DOWN: &str = "Vela is shutting down\.";$/m);

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
  // A launch is described for the player the play was given...
  assert.match(
    functionBody(launch, 'pub(crate) fn build_launch(', 'launch::build_launch'),
    /\) -> Result<LaunchPlan, String> \{\n    build_launch_for\(spec, spec\.player, cfg, ipc_path\)$/,
  );
  // ...by the one function that describes a launch for either.
  const body = functionBody(launch, 'pub(crate) fn build_launch_for(', 'launch::build_launch_for');
  assert.doesNotMatch(body, /spec\.player/, 'the player is the one named, nothing else decides');
  // One call serves both players: the external one gets `idle=no`, the
  // embedded one `idle=once` (the Rust unit tests hold the two values).
  const loop = 'args.extend(lifecycle_args(player))';
  const at = body.indexOf(loop);
  assert.notEqual(at, -1, 'the launch must apply lifecycle_args()');
  assert.equal(body.indexOf(loop, at + 1), -1, 'lifecycle_args() is applied exactly once');
  assert.equal(body.split('lifecycle_args(').length - 1, 1, 'and in no other form');

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
  const user = functionBody(launch, 'fn user_args(', 'launch::user_args');
  assert.match(user, /cfg\s*\.mpv_extra_args/, "user_args is where the user's arguments come from");
  // Each player's list takes them once, after every default of Vela's.
  const taken = [...body.matchAll(/args\.extend\(user_args\(cfg\)\);/g)].map((m) => m.index);
  assert.equal(taken.length, 2, 'once for the embedded player and once for the external one');
  assert.match(
    body,
    /let mut args = embedded_default_args\(\);\n\s+args\.extend\(user_args\(cfg\)\);\n\s+return \(args, Vec::new\(\)\);/,
    "the embedded list is Vela's defaults, then the user's arguments, and nothing after",
  );
  assert.ok(body.lastIndexOf('LaunchArg::set(') < taken[1], 'no external default follows them');
  assert.ok(body.lastIndexOf('LaunchArg::flag(') < taken[1]);
});

test("the embedded player's required options follow everything the user controls", () => {
  // `vo=libmpv` is what makes an embedded play draw into Vela's surface. A
  // user option that came later would win and break the player. So would a
  // user's `force-window`: mpv would open its video output before the file,
  // and the player's thread hands over the file with a call that waits
  // (review finding hd-3).
  const body = functionBody(launch, 'pub(crate) fn build_launch_for(', 'launch::build_launch_for');
  const required = 'args.extend(embedded_required_args())';
  const at = body.indexOf(required);
  assert.notEqual(at, -1, 'the launch must apply embedded_required_args()');
  assert.equal(body.indexOf(required, at + 1), -1, 'exactly once');
  for (const earlier of ['base_args(', 'markers_args(', 'args.extend(lifecycle_args(player))']) {
    const last = body.lastIndexOf(earlier);
    assert.notEqual(last, -1, `the launch must still call ${earlier}`);
    assert.ok(last < at, `embedded_required_args() must follow ${earlier}`);
  }
  assert.match(
    functionBody(launch, 'fn embedded_required_args(', 'launch::embedded_required_args'),
    /\[\s+LaunchArg::set\("vo", "libmpv"\),\s+LaunchArg::set\("force-window", "no"\),\s+\]/,
  );
});
