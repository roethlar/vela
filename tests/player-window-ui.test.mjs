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
    /let \(inherited_window_state, inherited_screen_name\) = \{[\s\S]{0,300}opening_window\(&current, first_of_run\)/,
    'the launch must open its window from the run, or else from the setting',
  );
  // It samples the record only once the stream is resolved, so a player that
  // has just ended has had that long to report how it left its window.
  const playPath = commands.slice(commands.indexOf('\nasync fn play_by_key_locked('));
  const resolvedAt = playPath.search(
    /let resolved = match selection\.version_id[\s\S]*?\.map_err\(PlayFailure::unavailable\)\?;/,
  );
  const resolveEnd = playPath.indexOf('.map_err(PlayFailure::unavailable)?;', resolvedAt);
  const sampledAt = playPath.indexOf('opening_window(&current, first_of_run)');
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
    /if record\.last_external\(\)\.is_some\(\) \{[\s\S]{0,200}inherited_window_state\(record\)[\s\S]{0,200}\} else \{\s*first_of_run\s*\}/,
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
    /if !recents_ready\.wait_succeeded\(\) \{[\s\S]{0,1800}let _ = write_play_end\(\s*\|change\| config::update\(change\),\s*&latest_window,\s*&session_id,\s*move \|cfg\| \{\s*crate::recents::finish_session\(cfg, &key, &finishing_session, position_ms, now_ms\);\s*\},\s*\);/,
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
    /pub\(crate\) fn remember_window_on_exit\(latest: [^{]*\) \{\s*let latest = latest\.clone\(\);\s*hand_over_within\(EXIT_WINDOW_HANDOVER_LIMIT, move \|\| \{\s*remember_window_on_exit_with\(&latest, config::load_config, \|change\| config::update\(change\)\);\s*\}\);\s*\}\n/,
    'quitting must decide what to write through remember_window_on_exit_with',
  );
  // ...on a thread of its own, waited for a short time and no longer
  // (review finding pw-4; hand_over_within is unit-tested in commands.rs).
  assert.match(commands, /\nconst EXIT_WINDOW_HANDOVER_LIMIT: Duration = Duration::from_secs\(2\);\n/);
  const handOver = commands.slice(
    commands.indexOf('\nfn hand_over_within('),
    commands.indexOf('\n}\n', commands.indexOf('\nfn hand_over_within(')),
  );
  assert.match(
    handOver,
    /let started = std::thread::Builder::new\(\)[\s\S]*?\.spawn\(move \|\| \{\s*hand_over\(\);\s*let _ = done\.send\(\(\)\);\s*\}\);\s*started\.is_ok\(\) && finished\.recv_timeout\(limit\)\.is_ok\(\)$/,
    'the exit waits for the hand-over with a limit',
  );
  assert.match(
    commands,
    /let launched_window_session = PlaybackWindowSession \{[\s\S]{0,120}player: started_player,/,
    'the record the end callback reads must name the player that started',
  );
});

test('a play opens on the screen its Compatible copy was ranked for', () => {
  // Review finding pw-1. The rule is unit-tested in commands.rs
  // (a_play_opens_on_the_screen_its_copy_was_ranked_for); this guards that
  // the screen looked at for the ranking is the one carried to the launch.
  const target = commands.slice(
    commands.indexOf('\nasync fn playback_compatibility_target('),
    commands.indexOf('\nfn playback_source_choices('),
  );
  assert.match(
    target,
    /compatibility_display\(&current, replace_session, upcoming\)\s+\};\s+let ranked_screen = observed\.as_ref\(\)\.map\(RankedScreen::of\);\s+let detected = display::detect_profile\(app, observed\)\.await;/,
    'the screen carried is the one the ranking looks at, taken once',
  );
  assert.match(target, /Ok\(Some\(\(\s+crate::selection::CompatibilityTarget \{[\s\S]{0,200}\},\s+ranked_screen,\s+\)\)\)/);
  // The screen carried and the screen the ranking ranks for are named by
  // one rule, ObservedDisplay::screen_name, also when an observation names
  // several screens.
  const rankedOf = commands.slice(
    commands.indexOf('\nimpl RankedScreen {'),
    commands.indexOf('\nfn screen_to_open_on('),
  );
  assert.match(rankedOf, /observed\s+\.screen_name\(\)\s+\.map_or\(Self::VelasScreen,/, 'the screen carried is named by screen_name');
  assert.doesNotMatch(rankedOf, /\.names\b/, 'and by nothing else');
  const display = read('src-tauri/src/display.rs');
  const detectAt = display.indexOf('\npub(crate) async fn detect_profile(');
  assert.notEqual(detectAt, -1, 'detect_profile exists');
  const detect = display.slice(detectAt, display.indexOf('\n}\n', detectAt));
  assert.match(
    detect,
    /let name = observed\s+\.as_ref\(\)\s+\.and_then\(\|value\| value\.screen_name\(\)\.cloned\(\)\)/,
    'the ranking names the observed screen by screen_name',
  );
  assert.doesNotMatch(detect, /\.names\.(first|last|get|iter|into_iter)\(/, 'and by nothing else');
  const select = commands.slice(
    commands.indexOf('\nasync fn select_playback_version('),
    commands.indexOf('\nstruct PlayLaunchRequest<'),
  );
  assert.match(
    select,
    /let \(target, ranked_screen\) = if policy == crate::selection::PlaybackSourcePolicy::Compatible \{\s+match playback_compatibility_target\([^)]*\)\.await\? \{\s+Some\(\(target, ranked_screen\)\) => \(Some\(target\), ranked_screen\),\s+None => \(None, None\),\s+\}\s+\} else \{\s+\(None, None\)\s+\};/,
  );
  assert.match(select, /player: upcoming_player,\s+ranked_screen,\s+\}\),/, 'the ranked selection carries it');
  assert.match(select, /player: upcoming_player,\s+ranked_screen: None,\s+\}\),/, 'an unranked one carries none');
  // The launch opens on it, after the window state is sampled.
  const playPath = commands.slice(commands.indexOf('\nasync fn play_by_key_locked('));
  const playBody = playPath.slice(0, playPath.indexOf('\n}\n'));
  const sampledAt = playBody.indexOf('opening_window(&current, first_of_run)');
  const screenAt = playBody.search(
    /let screen_name = match screen_to_open_on\(selection\.ranked_screen\.clone\(\), inherited_screen_name\) \{\s+Some\(name\) => Some\(name\),\s+None => match state\.app_handle\.get\(\) \{\s+Some\(app\) => display::current_screen_name\(app\)\.await,\s+None => None,\s+\},\s+\};/,
  );
  assert.ok(sampledAt > 0 && screenAt > sampledAt, 'the screen is decided from the ranking after the sampling');
  assert.match(playBody, /\n\s+screen_name,\n/, 'and handed to the launch');
  assert.equal([...playBody.matchAll(/\bscreen_name\b/g)].length, 2, 'untouched in between');
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
