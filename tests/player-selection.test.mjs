// Which player a play uses (.agents/plans/embedded-player.md, "Selection",
// decisions D1 and D3). The Rust unit tests prove the choice itself
// (src-tauri/src/player/select.rs). These prove the play path actually makes
// that choice, carries it to the launch, and keeps it across a relaunch; and
// that the setting stays out of sight until the owner has done the checks the
// plan lists.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file) => fs.readFileSync(path.join(repoRoot, file), 'utf8').replace(/\r\n/g, '\n');
const commands = read('src-tauri/src/commands.rs');
const playback = read('src-tauri/src/playback.rs');
const lib = read('src-tauri/src/lib.rs');
const config = read('src-tauri/src/config.rs');

function functionBody(source, signature, what) {
  const start = source.indexOf(`\n${signature}`);
  assert.notEqual(start, -1, `${what} must exist`);
  const end = source.indexOf('\n}\n', start);
  assert.notEqual(end, -1, `${what} must have a closing brace`);
  return source.slice(start, end);
}

test('every play chooses its player from the setting and what can run', () => {
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  // What is wanted: the launch's own player when it replaces a running play,
  // otherwise the saved setting.
  assert.match(
    body,
    /let wanted_player = match player \{\s+Some\(kind\) => kind,\s+None => crate::player::PlayerKind::from_setting\(\s+config::load_config\(\)[\s\S]{0,160}\.player\s+\.as_deref\(\),\s+\),\s+\};/,
  );
  // The choice: one call, with the real availability, made once.
  const choose = 'crate::player::choose(wanted_player, &crate::player::embedded_availability())';
  const at = body.indexOf(choose);
  assert.notEqual(at, -1, 'the play path must choose its player with player::choose');
  assert.equal(body.indexOf('player::choose(', at + choose.length), -1, 'chosen exactly once');
  // ...and it reaches the launch, together with the one-line explanation.
  const spec = body.slice(body.indexOf('let spec = playback::PlaySpec {'));
  assert.match(spec, /^let spec = playback::PlaySpec \{\n\s+player: player_choice\.kind,/);
  assert.match(spec, /osd_notice: osd_notice\.or\(fallback_notice\),/);
  assert.ok(at < body.indexOf('let spec = playback::PlaySpec {'), 'chosen before the launch is described');
});

test('once a play has installed its session, only the launch-failure path ends it early', () => {
  // Installing the session, moving the playlist position, stopping the
  // previous player and registering a transcode are undone in one place: the
  // launch-failure arm at the end of the function. An early return between
  // the installation and that arm skips all of it (review finding sel-1).
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  const installed = body.indexOf(
    'install_playback_session(state, session_id, item, run_kind, next_affinity).await;',
  );
  const undone = body.indexOf('let stop = match played {');
  assert.notEqual(installed, -1, 'the play must still install its session');
  assert.ok(undone > installed, 'and must still end in the launch-failure arm');
  const stretch = body
    .slice(installed, undone)
    .split('\n')
    .filter((line) => !/^\s*\/\//.test(line));
  const earlyReturns = stretch
    .filter((line) => /\?\s*(;|$)|\)\?\./.test(line) || /^ {4}(return\b|.*\breturn (Err|Ok)\()/.test(line))
    .map((line) => line.trim());
  assert.deepEqual(earlyReturns, [
    // Known, older than the embedded player and recorded for the owner in
    // .agents/state.md (Blockers): the settings read for Vela's own resume
    // position. Nothing may be added to this list.
    '.map_err(|_| PlayFailure::fatal("could not read Vela settings".to_string()))?;',
  ]);
  // The player setting is read before anything is resolved or installed.
  const read = body.indexOf('let wanted_player = match player {');
  const resolved = body.indexOf('let resolved = match selection.version_id.as_deref() {');
  assert.notEqual(read, -1, 'the play must read which player is wanted');
  assert.notEqual(resolved, -1, 'the play must still resolve its stream');
  assert.ok(read < resolved, 'the player setting is read before the stream is resolved');
  assert.ok(read < installed, 'and before the session is installed');
  assert.equal(body.indexOf('let wanted_player', read + 1), -1, 'and only once');
});

test('a fallback is explained once, and only by a play that started', () => {
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  // Asked for only when the launch carries no notice of its own.
  assert.match(
    body,
    /let fallback_notice = if osd_notice\.is_none\(\) \{\s+state\s+\.embedded_fallback_notices[\s\S]{0,120}\.due\(player_choice\.fallback\.as_ref\(\)\)\s+\} else \{\s+None\s+\};/,
  );
  // Recorded inside the success continuation, never before the launch.
  const success = body.slice(body.indexOf('let played = after_successful_play(played, || {'));
  assert.match(
    success,
    /^let played = after_successful_play\(played, \|\| \{[\s\S]{0,400}if fallback_told \|\| played_fallback\.is_none\(\) \{\s+state\s+\.embedded_fallback_notices[\s\S]{0,120}\.record\(played_fallback\.as_ref\(\)\);/,
  );
  // The notice state is touched in exactly two places in the whole command
  // layer: asked before the launch, recorded after it succeeded. Anything else
  // (an early record, a second ask) would tell the user at the wrong time.
  const uses = [...commands.matchAll(/embedded_fallback_notices\s*\.lock\(\)\s*\.unwrap_or_else\(\|e\| e\.into_inner\(\)\)\s*\.(\w+)\(/g)];
  assert.deepEqual(uses.map((use) => use[1]), ['due', 'record']);
  assert.equal(
    commands.split('embedded_fallback_notices').length - 1,
    2,
    'no other code reads or writes the notice state',
  );
  const successAt = commands.indexOf('let played = after_successful_play(played, || {');
  const launchAt = commands.indexOf('playback::play(\n');
  assert.ok(uses[0].index < launchAt, 'the notice is asked for before the launch');
  assert.ok(uses[1].index > launchAt && uses[1].index > successAt, 'and recorded only after it succeeded');
});

test('a relaunch stays in the player it replaces', () => {
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  assert.match(
    body,
    /\*state\.active_playback_player\.lock\(\)\.await = Some\(\(session_id\.to_string\(\), player_choice\.kind\)\);/,
    'the session records the player it runs in',
  );
  const stepDown = functionBody(commands, 'pub(crate) async fn apply_step_down(', 'apply_step_down');
  assert.match(
    stepDown,
    /let replaced_player = state\s+\.active_playback_player\s+\.lock\(\)\s+\.await\s+\.as_ref\(\)\s+\.filter\(\|\(session, _\)\| session == &request\.session_id\)\s+\.map\(\|\(_, kind\)\| \*kind\);/,
    'a step-down reads the player of the exact session it replaces',
  );
  assert.match(stepDown, /continues_automatic: true,\n\s+player: replaced_player,/);
  // Every other launch is the user's own and chooses from the setting.
  const others = commands.split('player: replaced_player,').join('');
  const launches = others.split('continues_automatic: false,').length - 1;
  assert.ok(launches >= 4, 'the user-started launches must still be found');
  assert.equal(
    others.split('continues_automatic: false,\n            player: None,').length - 1,
    launches,
    'a launch the user starts never names a player',
  );
  // The record does not outlive its session.
  const clear = functionBody(commands, 'async fn clear_playback_session_if(', 'clear_playback_session_if');
  assert.match(
    clear,
    /let mut player = state\.active_playback_player\.lock\(\)\.await;\s+if player\s+\.as_ref\(\)\s+\.is_some_and\(\|\(session, _\)\| session == expected_session\)\s+\{\s+\*player = None;/,
  );
});

test('play refuses a player it cannot start before it starts anything', () => {
  const body = functionBody(playback, 'pub fn play(', 'playback::play');
  const guard = body.indexOf('if spec.player != crate::player::PlayerKind::External {');
  assert.notEqual(guard, -1, 'play must check which player it was asked for');
  for (const later of ['new_ipc_path()', 'resolve_mpv()', 'build_launch(', 'external::launch(']) {
    const at = body.indexOf(later);
    assert.notEqual(at, -1, `play must still call ${later}`);
    assert.ok(guard < at, `the player check must come before ${later}`);
  }
});

test('the player setting is validated, and not yet shown to users', () => {
  assert.match(
    config,
    /validate_optional_closed\("player", self\.player\.as_deref\(\), &\["external", "embedded"\]\)\?;/,
  );
  // Owner, 2026-11-12: until the by-eye checks are done there is no Settings
  // control for the embedded player. It can be tried only by editing the
  // config file. Slice 6 of the plan removes this assertion on purpose.
  assert.doesNotMatch(lib, /commands::(get|set)_player\b/, 'no command exposes the player setting');
  assert.doesNotMatch(commands, /fn (get|set)_player\b/);
  for (const file of ['src/lib/Settings.svelte', 'src/routes/+page.svelte', 'src/lib/types.ts']) {
    assert.doesNotMatch(read(file), /embedded/i, `${file} must not mention the embedded player yet`);
  }
  for (const file of ['README.md', 'docs/usage.md', 'RELEASE_NOTES.md']) {
    assert.doesNotMatch(
      read(file),
      /embedded player|"player"\s*:/i,
      `${file} must not announce the embedded player yet`,
    );
  }
});
