// `.agents/plans/mpv-window-geometry.md`: how the play path opens and hands on
// the player window (Slices 1 to 3), and the "Player window" setting's
// control (Slice 3). Rust unit tests guard each rule; this guards the wiring
// they cannot reach, because the play path and its end callback need a
// running app. The play path's wiring is checked whether or not the control
// is offered: it holds either way, since remember is the default. Only the
// control's own wiring is checked behind it.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (rel) => fs.readFileSync(path.join(repoRoot, rel), 'utf8');

const settings = read('src/lib/Settings.svelte');
const commands = read('src-tauri/src/commands.rs');
const config = read('src-tauri/src/config.rs');
const lib = read('src-tauri/src/lib.rs');

test('the player window control talks to the registered setting commands', () => {
  const offered = settings.includes('id="player-window"');
  if (!offered) return; // Withheld is always safe.

  // The control talks to the two commands, and both are registered.
  assert.match(settings, /invoke<PlayerWindowMode>\("get_mpv_window_mode"\)/);
  assert.match(settings, /invoke<PlayerWindowMode>\("set_mpv_window_mode"/);
  assert.match(lib, /commands::get_mpv_window_mode,\s*\n\s*commands::set_mpv_window_mode,/);
});

test('the play path opens its window from the run or the setting, and hands it on', () => {

  // The play path reads the setting, before stream resolution...
  assert.match(
    commands,
    /let first_of_run = config::load_config\(\)[\s\S]{0,200}config::first_window_of_run\(&cfg\)[\s\S]{0,4000}let resolved = match selection\.version_id/,
    'the play path must read the player window setting before resolving the stream',
  );
  // ...and the launch is given what it says when the run has no window yet.
  assert.match(
    commands,
    /let \(inherited_window_state, inherited_screen_name\) = \{[\s\S]{0,300}opening_window\(current\.as_ref\(\), first_of_run\)/,
    'the launch must open its window from the run, or else from the setting',
  );
  // It samples the record only once the stream is resolved, so a player that
  // has just ended has had that long to report how it left its window.
  const playPath = commands.slice(commands.indexOf('\nasync fn play_by_key_locked('));
  const resolvedAt = playPath.search(
    /let resolved = match selection\.version_id[\s\S]*?\.map_err\(PlayFailure::unavailable\)\?;/,
  );
  const resolveEnd = playPath.indexOf('.map_err(PlayFailure::unavailable)?;', resolvedAt);
  const sampledAt = playPath.indexOf('opening_window(current.as_ref(), first_of_run)');
  assert.ok(resolvedAt > 0 && resolveEnd > resolvedAt && sampledAt > 0);
  assert.ok(
    resolveEnd < sampledAt,
    'the window record must be sampled after the stream is resolved',
  );
  // What it samples is what the launch gets: nothing binds the two names
  // again (for instance to gate them on an automatic successor once more).
  const playBody = playPath.slice(0, playPath.indexOf('\n}\n'));
  for (const name of ['inherited_window_state', 'inherited_screen_name']) {
    const bindings = [...playBody.matchAll(new RegExp(`let\\s+(?:mut\\s+)?(?:\\([^)]*)?\\b${name}\\b`, 'g'))];
    assert.equal(bindings.length, 1, `${name} must be bound once, by the sampling`);
  }
  // Likewise the setting read: read once, used once, by the sampling.
  assert.equal(
    [...playBody.matchAll(/\bfirst_of_run\b/g)].length,
    2,
    'what the setting says must reach the sampling untouched',
  );

  // Choosing a mode other than remember deletes the remembered window, in
  // the command's own write (apply_mpv_window_mode is unit-tested).
  const setMode = commands.slice(
    commands.indexOf('\npub fn set_mpv_window_mode('),
    commands.indexOf('\nfn apply_mpv_window_mode('),
  );
  assert.match(
    setMode,
    /config::update\(move \|cfg\| \{\s+apply_mpv_window_mode\(cfg, resolved\);\s+Ok\(\(\)\)\s+\}\)\?;/,
    'the setting must be saved through apply_mpv_window_mode',
  );
  assert.doesNotMatch(setMode, /cfg\.mpv_window/, 'and set nothing itself');
  // The play hands on the size it opened at until it shows one of its own.
  assert.match(
    commands,
    /let window_observation =\s+playback::WindowStateObservation::opened_at\(inherited_window_state\.windowed_size\);/,
    'the launch must start its window observation from the size it opens at',
  );
  assert.match(
    commands,
    /if window_record\(current\)\.is_some\(\) \{[\s\S]{0,200}inherited_window_state\(current\)[\s\S]{0,200}\} else \{\s*first_of_run\s*\}/,
    'what this run observed must win over the setting',
  );

  // The end of a play remembers its window, in the recents write that
  // follows the start gate, from the published record of this very play
  // (which names the player that actually ran it).
  assert.match(
    commands,
    /let latest_window = state\.playback_window_session\.clone\(\);/,
    'the end callback must see the record the launch path publishes',
  );
  assert.match(
    commands,
    /if !recents_ready\.wait_succeeded\(\) \{[\s\S]{0,1600}let ended = window_to_remember\(\s*latest_window[\s\S]{0,200}&session_id,\s*\);\s*let _ = config::update\(move \|cfg\| \{[\s\S]{0,300}if let Some\(\(player, window, screen\)\) = ended \{\s*config::remember_mpv_window\(cfg, player, window, screen\.as_deref\(\)\);/,
    'the end of a play must hand its final window to the settings',
  );
  // Quitting Vela hands the latest play's window over too: a play still
  // showing then never reaches its own end.
  assert.match(
    lib,
    /if let tauri::RunEvent::ExitRequested \{ \.\. \} \| tauri::RunEvent::Exit = event \{[\s\S]*commands::remember_window_on_exit\(&state\.playback_window_session\);/,
    'quitting Vela must hand the latest window to the settings',
  );
  // What quitting writes, and when, is unit-tested through
  // remember_window_on_exit_with; quitting must go through it with the real
  // settings read and write.
  assert.match(
    commands,
    /pub\(crate\) fn remember_window_on_exit\(latest: [^)]*\) \{\s*remember_window_on_exit_with\(latest, config::load_config, \|player, window, screen\| \{\s*let _ = config::update\(move \|cfg\| \{\s*config::remember_mpv_window\(cfg, player, window, screen\.as_deref\(\)\);\s*Ok\(\(\)\)\s*\}\);\s*\}\);\s*\}\n/,
    'quitting must decide what to write through remember_window_on_exit_with',
  );
  assert.match(
    commands,
    /let launched_window_session = PlaybackWindowSession \{[\s\S]{0,120}player: started_player,/,
    'the record the end callback reads must name the player that started',
  );
});

test('the remembered window never holds a position', () => {
  const struct = config.match(/pub struct MpvWindow \{([\s\S]*?)\n\}/);
  assert.ok(struct, 'the stored window type exists');
  const fields = [...struct[1].matchAll(/pub (\w+):/g)].map((match) => match[1]);
  assert.deepEqual(fields, ['fullscreen', 'maximized', 'screen', 'width', 'height']);
  assert.match(
    config,
    /#\[serde\(deny_unknown_fields\)\]\s*\npub struct MpvWindow/,
    'a stored position (or any unknown part) must invalidate the settings, not be ignored',
  );
});
