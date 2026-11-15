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
  // The control is there, however its id is quoted; without it this test
  // fails, it does not pass by default (review finding pw-7: an early return
  // on a text search let a broken control through). The control itself is
  // checked from the parsed component in the tests of a failed save, below.
  assert.match(settings, /\bid=(["'])player-window\1/, 'Settings has the Player window control');

  // The control talks to the two commands, and both are registered.
  assert.match(settings, /invoke<PlayerWindowMode>\("get_mpv_window_mode"\)/);
  assert.match(settings, /invoke<PlayerWindowMode>\("set_mpv_window_mode"/);
  assert.match(lib, /commands::get_mpv_window_mode,\s*\n\s*commands::set_mpv_window_mode,/);
});

test('the play path opens its window from the run or the setting, and hands it on', () => {

  // The play path reads the setting, before stream resolution...
  assert.match(
    commands,
    /let \(first_of_run, saved_first_screen\) = config::load_config\(\)[\s\S]{0,200}config::first_window_of_run\(&cfg\)[\s\S]{0,4000}let resolved = resolve_selected\(/,
    'the play path must read the player window setting before resolving the stream',
  );
  // ...and the launch is given what it says when the run has no window yet.
  assert.match(
    commands,
    /let inherited_window_state = \{[\s\S]{0,300}opening_window\(&current, first_of_run\)/,
    'the launch must open its window from the run, or else from the setting',
  );
  // It samples the record only once the stream is resolved, so a player that
  // has just ended has had that long to report how it left its window.
  const playPath = commands.slice(commands.indexOf('\nasync fn play_by_key_locked('));
  const resolvedAt = playPath.search(
    /let resolved = resolve_selected\(src\.as_ref\(\), &selection, include_markers, &quality\)\s+\.await\s+\.map_err\(PlayFailure::unavailable\)\?;/,
  );
  const resolveEnd = playPath.indexOf('.map_err(PlayFailure::unavailable)?;', resolvedAt);
  const sampledAt = playPath.indexOf('opening_window(&current, first_of_run)');
  assert.ok(resolvedAt > 0 && resolveEnd > resolvedAt && sampledAt > 0);
  assert.ok(
    resolveEnd < sampledAt,
    'the window record must be sampled after the stream is resolved',
  );
  // What it samples is what the launch gets: nothing binds the name again
  // (for instance to gate it on an automatic successor once more). The
  // screen is no longer sampled here at all: it comes with the copy (the
  // next test).
  const playBody = playPath.slice(0, playPath.indexOf('\n}\n'));
  for (const name of ['inherited_window_state']) {
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
  // Read without whole-line comments, so the comment above the write can say
  // what it must (review finding pw-8) without widening the code allowed
  // between the gate and the write (370 characters of code; the bound was
  // 1800 with about 1170 characters of comment in it).
  assert.match(
    commands.replace(/^[ \t]*\/\/[^\n]*\n/gm, ''),
    /if !recents_ready\.wait_succeeded\(\) \{[\s\S]{0,600}let _ = write_play_end\(\s*\|change\| config::update\(change\),\s*&latest_window,\s*&session_id,\s*move \|cfg\| \{\s*crate::recents::finish_session\(cfg, &key, &finishing_session, position_ms, now_ms\);\s*\},\s*\);/,
    'the end of a play must hand its final window to the settings',
  );
  // Quitting Vela hands the last external play's window over too: a play still
  // showing then never reaches its own end.
  assert.match(
    lib,
    /if let tauri::RunEvent::ExitRequested \{ \.\. \} \| tauri::RunEvent::Exit = event \{[\s\S]*commands::remember_window_on_exit\(&state\.playback_window_session\);/,
    'quitting Vela must hand the latest window to the settings',
  );
  // What quitting writes, and when, is unit-tested through
  // remember_window_on_exit_with and hand_over_window_on_exit; quitting must
  // go through them with the real limits and the real settings read and
  // write.
  assert.match(
    commands,
    /pub\(crate\) fn remember_window_on_exit\(latest: [^{]*\) \{\s*hand_over_window_on_exit\(\s*latest\.clone\(\),\s*EXIT_WINDOW_HANDOVER_LIMIT,\s*EXIT_READER_DRAIN_LIMIT,\s*config::load_config,\s*\|change\| config::update\(change\),\s*\);\s*\}\n/,
    'quitting must go through hand_over_window_on_exit with the real limits and settings',
  );
  // ...which, on the hand-over's own thread and within its limit, first
  // waits for the last external play's reader (review finding pw-8), then
  // decides what to write.
  const exitHandOverStart = commands.indexOf('\nfn hand_over_window_on_exit(');
  assert.notEqual(exitHandOverStart, -1, 'the exit hand-over exists');
  const exitHandOver = commands
    .slice(exitHandOverStart, commands.indexOf('\n}\n', exitHandOverStart))
    .replace(/\/\/[^\n]*/g, '');
  assert.match(
    exitHandOver,
    /\) -> bool \{\s*hand_over_within\(limit, move \|\| \{\s*wait_for_last_external_reader\(&latest, reader_limit\);\s*remember_window_on_exit_with\(&latest, load, update\);\s*\}\)$/,
    'the exit waits for the reader inside the hand-over, before it looks at the window',
  );
  // ...on a thread of its own, waited for a short time and no longer
  // (review finding pw-4; hand_over_within is unit-tested in commands.rs).
  assert.match(commands, /\nconst EXIT_WINDOW_HANDOVER_LIMIT: Duration = Duration::from_secs\(2\);\n/);
  assert.match(commands, /\nconst EXIT_READER_DRAIN_LIMIT: Duration = Duration::from_millis\(500\);\n/);
  // Vela's exit kills the player first, hands the window over second, and
  // only then tells the tracking to stop: a connected reader is not stopped
  // with lines unread, it reads to the end of the killed mpv's connection.
  const exitStart = lib.indexOf('if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {');
  assert.notEqual(exitStart, -1, 'the exit handler exists');
  const exit = lib
    .slice(exitStart, lib.indexOf('commands::take_any_active_transcode', exitStart))
    .replace(/\/\/[^\n]*/g, '');
  const kill = exit.search(/if let Some\(mut child\) = slot\.take\(\) \{\s*let _ = child\.remove_consumed_header_include\(\);\s*let _ = child\.kill\(\);/);
  const handOverAt = exit.indexOf('commands::remember_window_on_exit(&state.playback_window_session);');
  const stopAt = exit.search(/let stop = state\s*\.tracking_stop[\s\S]*?\.take\(\);\s*if let Some\(stop\) = stop \{\s*stop\.store\(true, Ordering::Relaxed\);/);
  assert.ok(kill !== -1 && handOverAt !== -1 && stopAt !== -1, 'the exit kills, hands over and stops');
  assert.ok(kill < handOverAt, 'the exit kills the player before it hands the window over');
  assert.ok(handOverAt < stopAt, 'the exit hands the window over before it stops the tracking');
  assert.equal(exit.split('tracking_stop').length, 2, 'the tracking is stopped in one place only');
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

test('the real launch seeds and shares the complete opening window state', () => {
  const start = commands.indexOf('\nasync fn play_by_key_locked(');
  assert.notEqual(start, -1, 'the launch path exists');
  // Ignore comments so a disconnected copy of the old wiring cannot pass.
  const play = commands.slice(start, commands.indexOf('\n}\n', start)).replace(/\/\/[^\n]*/g, '');
  assert.match(
    play,
    /let window_observation =\s+playback::WindowStateObservation::opened_at\(inherited_window_state\);/,
    'the launch must seed both flags and size from the complete opening state',
  );
  assert.equal(
    [...play.matchAll(/let\s+(?:mut\s+)?window_observation\b/g)].length, 1,
    'the seeded observation must not be replaced before launch or publication',
  );
  assert.match(
    play,
    /let spec = playback::PlaySpec \{[^}]*\binherited_window_state,\s+screen_name,\s+window_observation: window_observation\.clone\(\),/,
    'the IPC reader must receive the observation seeded from the actual launch state',
  );
  assert.match(
    play,
    /let launched_window_session = PlaybackWindowSession \{\s+session_id: session_id\.to_string\(\),\s+player: started_player,\s+observation: window_observation,\s+\};\s+let window_slot = &state\.playback_window_session;\s+let played = publish_started_play\(played, window_slot, launched_window_session,/,
    'the published play must share that seeded observation with its IPC reader',
  );
});

test('a Compatible play is shown on the screen its copy was ranked for, any other on the one its launch decides', () => {
  // Review findings pw-1 and pw-2, and the third review of pw-1 (comments 5
  // and 6). The rules are unit-tested in commands.rs (screen_for_play,
  // rank_and_place, screen_at_launch, compatibility_target_from, and the
  // late fallback through playback::play) and display.rs (profile_basis,
  // screens_from); this guards the wiring they cannot reach, because
  // select_playback_version and the play path need a running app. Only a
  // Compatible choice decides the screen, once, before the copy is chosen,
  // ranks for it, and hands it to the launch, which then looks at no screen
  // again. For any other the choice of copy looks at no screen, and the
  // launch decides it once, after the stream is resolved, through
  // decide_play_screen and nothing else.
  const bodyOf = (source, signature) => {
    const start = source.indexOf(`\n${signature}`);
    assert.notEqual(start, -1, `${signature} exists`);
    return source.slice(start, source.indexOf('\n}\n', start));
  };
  const count = (source, text) => source.split(text).length - 1;
  const production = commands.slice(0, commands.indexOf('\n#[cfg(test)]\nmod tests {'));
  const select = bodyOf(commands, 'async fn select_playback_version(');

  // The choice of copy: a Compatible choice decides the screen for the
  // player that will run the play, with the setting's saved first screen,
  // ranks for that screen's profile with the settings' overrides, and
  // rank_and_place carries the screen. Every other ranks by its policy and
  // looks at no screen.
  const ranking = select.search(
    /let ranking = match state\.app_handle\.get\(\) \{\s+Some\(app\) if policy == crate::selection::PlaybackSourcePolicy::Compatible => \{\s+let overrides = playback_display_overrides\(&cfg\)\.map_err\(PlayFailure::fatal\)\?;\s+let \(_, saved_first_screen\) =\s+config::first_window_of_run\(&cfg\)\.map_err\(PlayFailure::fatal\)\?;\s+let \(screen, screens\) = decide_play_screen\(\s+state,\s+replace_session,\s+upcoming_player\.kind,\s+saved_first_screen\.as_deref\(\),\s+\)\s+\.await;\s+let target = compatibility_target\(app, &screen, &screens, overrides\)\.await;\s+Ranking::ForScreen \{ screen, target \}\s+\}\s+_ => Ranking::ByPolicy\(policy\),\s+\};\s+let \(winner, destination\) = rank_and_place\(&mut candidates, ranking\)\s+\.ok_or_else/,
  );
  assert.ok(ranking > 0, 'only a Compatible choice decides the screen, and ranks for it');
  assert.equal(count(select, 'decide_play_screen('), 1, 'once, in that arm');
  // The names that arm and the launch hand decide_play_screen are the
  // play's own: nothing in either function binds them again (the attack on
  // the fourth repair, finding 5: a `let replace_session = None;` before the
  // arm passed every pin). In the choice of copy the play's session is the
  // parameter, read only by that arm; in the launch it is taken from the
  // request once, and the saved first screen read from the setting once.
  const code = (body) => body.replace(/\/\/[^\n]*/g, '');
  const bindings = (body, name) =>
    (code(body).match(new RegExp(`\\blet\\s[^=;]*\\b${name}\\b[^=;]*=`, 'g')) ?? []).length;
  assert.equal(count(code(select), 'replace_session'), 2, "the choice of copy's session is its parameter, read by that arm only");
  assert.equal(bindings(select, 'replace_session'), 0, 'never bound again in the choice of copy');
  assert.equal(bindings(select, 'saved_first_screen'), 1, 'the saved first screen is read once, in that arm');
  const launch = bodyOf(commands, 'async fn play_by_key_locked(');
  assert.match(launch, /let PlayLaunchRequest \{[^}]*\breplace_session,[^}]*\} = request;/, "the launch's session is the request's");
  assert.equal(bindings(launch, 'replace_session'), 1, 'and never bound again in the launch');
  assert.equal(bindings(launch, 'saved_first_screen'), 1, 'the saved first screen is read once in the launch');
  assert.ok(select.indexOf('let upcoming_player = crate::player::choose(') < ranking, 'after the player is chosen');
  assert.doesNotMatch(
    select,
    /look_at_screens|screen_for_play|profile_basis|hdr_of|detect_profile|current_monitor|Screens::default/,
    'the choice of copy looks at the screens through that arm and nothing else',
  );
  assert.equal(count(select, 'rank_and_place('), 1);
  assert.doesNotMatch(select, /rank_candidates\(/, 'the choice of copy ranks only through rank_and_place');
  const place = bodyOf(commands, 'fn rank_and_place(');
  assert.match(
    place,
    /\) -> Option<[^{]*\{\s+let \(policy, target, destination\) = match ranking \{\s+Ranking::ForScreen \{ screen, target \} => \(\s+crate::selection::PlaybackSourcePolicy::Compatible,\s+Some\(target\),\s+PlayDestination::RankedFor\(screen\.name\),\s+\),\s+Ranking::ByPolicy\(policy\) => \(policy, None, PlayDestination::DecidedAtLaunch\),\s+\};\s+crate::selection::rank_candidates\(candidates, policy, target\);\s+candidates\s+\.first\(\)\s+\.cloned\(\)\s+\.map\(\|winner\| \(winner, destination\)\)$/,
  );
  // Nothing else makes either: a ranking for a screen is made only in the
  // Compatible arm, a screen ranked for only by rank_and_place.
  assert.equal(count(production, 'Ranking::ForScreen {'), 2, 'rank_and_place reads it, the Compatible arm makes it');
  assert.equal(count(production, 'PlayDestination::RankedFor('), 2, 'rank_and_place makes it, screen_at_launch reads it');
  // The only other ranking is the Ask prompt's, Best within each source.
  assert.equal(count(production, 'rank_candidates('), 2, 'rank_and_place and the Ask prompt rank, nothing else');
  assert.match(
    production,
    /crate::selection::rank_candidates\(\s+&mut source_candidates,\s+crate::selection::PlaybackSourcePolicy::Best,\s+None,\s+\);/,
  );
  // The target's construction is compatibility_target_from, which the Rust
  // tests run with made-up screens and HDR answers
  // (a_compatible_target_is_made_for_the_decided_screen); the production
  // wrapper is that one call with the system's HDR answer, and nothing else.
  const target = bodyOf(commands, 'async fn compatibility_target(');
  assert.match(
    target,
    /\) -> crate::selection::CompatibilityTarget \{\s+compatibility_target_from\(screen, screens, overrides, async \|basis\| display::hdr_of\(app, basis\)\.await\)\.await$/,
    'the wrapper only hands the construction the system',
  );
  assert.equal(count(production, 'compatibility_target_from('), 2, 'defined, and called by the wrapper only');
  assert.equal(count(production, 'compatibility_target('), 2, 'defined, and called by the choice of copy only');

  // The ranked selection is the winner's (selection_for_winner, run by the
  // Rust tests from the ranking to the stream resolved) and carries what
  // rank_and_place gives; the one with nothing to rank leaves the screen to
  // the launch.
  assert.match(
    select,
    /\.ok_or_else\(\|\| PlayFailure::unavailable\("no playable copy was found"\.to_string\(\)\)\)\?;\s+let selection = selection_for_winner\(\s+versions,\s+&mut probed_sources,\s+&winner,\s+item,\s+upcoming_player,\s+destination,\s+\)\?;\s+Ok\(PlaybackSelectionOutcome::Ready \{\s+selection: Box::new\(selection\),/,
    'the ranked selection is the winner\'s, and carries it',
  );
  assert.equal(count(production, 'selection_for_winner('), 2, 'defined, and called by the choice of copy only');
  assert.match(
    select,
    /player: upcoming_player,\s+screen: PlayDestination::DecidedAtLaunch,\s+\}\),/,
    'one with nothing to rank is decided at the launch',
  );
  assert.equal(count(select.replace(/\/\/[^\n]*/g, ''), 'screen: '), 1);

  // How it is decided, by either: the screens are looked at before the
  // record is locked, and the record is held only to decide.
  const decide = bodyOf(commands, 'async fn decide_play_screen(');
  assert.match(
    decide,
    /\) -> \(PlayScreen, display::Screens\) \{\s+let screens = match state\.app_handle\.get\(\) \{\s+Some\(app\) => display::look_at_screens\(app\)\.await,\s+None => display::Screens::default\(\),\s+\};\s+let screen = \{\s+let current = state\s+\.playback_window_session\s+\.lock\(\)\s+\.unwrap_or_else\(\|error\| error\.into_inner\(\)\);\s+screen_for_play\(\s+&current,\s+replace_session,\s+upcoming,\s+saved_first_screen,\s+&screens,\s+\)\s+\};\s+\(screen, screens\)$/,
  );
  assert.equal(count(production, 'screen_for_play('), 2, 'decided in one place');
  assert.equal(
    count(production, 'decide_play_screen('),
    3,
    'defined, and called by a Compatible choice of copy and by the launch only',
  );
  assert.equal(count(production, 'profile_basis('), 1, 'ranked for in one place');
  const display = read('src-tauri/src/display.rs');
  // The screens are asked of the system once, on a blocking thread and never
  // on an async worker: the first thing look_at_screens does is that thread,
  // and both Vela's monitor and the list come from its one answer, which
  // screens_from puts together with the system's own list of outputs.
  const look = bodyOf(display, 'pub(crate) async fn look_at_screens(');
  assert.match(
    look,
    /\) -> Screens \{\s+let asked = \{\s+let app = app\.clone\(\);\s+tauri::async_runtime::spawn_blocking\(move \|\| \{\s+use tauri::Manager;\s+let window = app\.get_webview_window\("main"\)\?;\s+let velas = window\.current_monitor\(\)\.ok\(\)\.flatten\(\)\.map\(\|monitor\| descriptor\(&monitor\)\);\s+let all = window\s+\.available_monitors\(\)/,
    'the system is asked once, on a blocking thread',
  );
  assert.doesNotMatch(look, /current_monitor\(app|current_screen_name/, 'and not again on the async worker');
  assert.equal(count(look, 'current_monitor('), 1);
  assert.match(look, /screens_from\(velas, monitors, platform_outputs\(\)\.await\)\s*$/, 'what was seen is put together');
  assert.equal(count(display, 'current_screen_name'), 0, "Vela's screen is looked up only by look_at_screens");
  assert.equal(count(display.slice(0, display.indexOf('\n#[cfg(test)]\nmod tests {')), 'screens_from('), 2);
  // What the compositor's list says reaches screens_from as it is: a list
  // that could not be read, or a join that failed, is incomplete, so nothing
  // is known to be connected (third review of pw-1, comment 1; screens_from
  // and wayland_hdr::listed are unit-tested), and the transform the geometry
  // event carries reaches the list (comment 4).
  assert.match(
    bodyOf(display, 'async fn platform_outputs() -> PlatformOutputs {'),
    /if !wayland_session\(\) \|\| !names_a_display\(std::env::var_os\("WAYLAND_DISPLAY"\)\.as_deref\(\)\) \{\s+return PlatformOutputs::NoneToAdd;\s+\}\s+wayland_hdr::outputs\(\)\.unwrap_or\(PlatformOutputs::Incomplete\(Vec::new\(\)\)\)\s+\}\)\s+\.await\s+\.unwrap_or\(PlatformOutputs::Incomplete\(Vec::new\(\)\)\)$/,
  );
  assert.match(display, /wl_output::Event::Geometry \{\s+model, transform, \.\.\s+\} => \{\s+output\.model = Some\(model\);\s+output\.transform = Some\(transform\);\s+\}/);
  assert.match(display, /Ok\(listed\(\s+state\s+\.outputs\s+\.iter\(\)\s+\.map\(\|output\| Described \{\s+name: output\.name\.clone\(\),\s+model: output\.model\.clone\(\),\s+width: output\.width,\s+height: output\.height,\s+transform: output\.transform,\s+\}\)/);
  // The HDR state is asked for the decided output, by its name, size and place.
  assert.match(
    bodyOf(display, 'pub(crate) async fn hdr_of('),
    /\) -> HdrState \{\s+native_hdr_state\(\s+app,\s+basis\.name\.clone\(\),\s+basis\.width_px,\s+basis\.height_px,\s+basis\.position,\s+\)\s+\.await$/,
    'the HDR state is the decided output\'s',
  );

  // The launch: a Compatible copy's screen as it was decided, with nothing
  // looked at; any other decided once, after the stream is resolved, as for
  // mpv's own window (the only player that uses it), with the setting's
  // saved first screen, through decide_play_screen and nothing else.
  assert.match(
    bodyOf(commands, 'async fn screen_at_launch('),
    /\) -> Option<String> \{\s+match destination \{\s+PlayDestination::RankedFor\(name\) => name,\s+PlayDestination::DecidedAtLaunch => decide\(\)\.await\.name,\s+\}$/,
  );
  assert.equal(count(production, 'screen_at_launch('), 2, 'defined, and called by the launch only');
  const playBody = bodyOf(commands, 'async fn play_by_key_locked(');
  const decidedAt = playBody.search(
    /let screen_name = screen_at_launch\(selection\.screen, async \|\| \{\s+decide_play_screen\(\s+state,\s+replace_session,\s+crate::player::PlayerKind::External,\s+saved_first_screen\.as_deref\(\),\s+\)\s+\.await\s+\.0\s+\}\)\s+\.await;/,
  );
  assert.ok(decidedAt > 0, 'the launch takes the screen through screen_at_launch');
  const resolvedEnd = playBody.indexOf(
    '.map_err(PlayFailure::unavailable)?;',
    playBody.search(/let resolved = /),
  );
  assert.ok(resolvedEnd > 0 && resolvedEnd < decidedAt, 'after the stream is resolved');
  // The stream resolved is the selection's (resolve_selected, run by the
  // Rust tests), and it is what the launch plays (second review of pw-2).
  assert.match(playBody, /let resolved = resolve_selected\(src\.as_ref\(\), &selection, include_markers, &quality\)/);
  assert.equal(count(production, 'resolve_selected('), 2, 'defined, and called by the launch only');
  assert.match(playBody, /let spec = playback::PlaySpec \{[^}]*\burl: resolved\.url,/, 'the launch plays it');
  assert.match(
    playBody,
    /let \(first_of_run, saved_first_screen\) = config::load_config\(\)[\s\S]{0,200}config::first_window_of_run\(&cfg\)/,
    'the saved first screen is the setting the launch reads',
  );
  assert.equal([...playBody.matchAll(/\bsaved_first_screen\b/g)].length, 2, 'read once, used once');
  assert.equal(count(playBody, 'selection.screen'), 1, 'what the choice of copy handed on is read once');
  assert.equal(count(playBody, 'decide_play_screen('), 1);
  assert.match(playBody, /\n\s+screen_name,\n/, 'and handed to the launch');
  assert.equal([...playBody.matchAll(/\bscreen_name\b/g)].length, 2, 'untouched in between');
  assert.doesNotMatch(
    playBody,
    /current_screen_name|look_at_screens|screen_for_play|detect_profile|profile_of|profile_basis|hdr_of|inherited_screen_name|display_snapshot|PlayDestination::/,
    'and nothing else in the play path looks at a screen',
  );
  // A play the embedded player cannot start is described again for mpv's
  // own window from the same play, so its window asks for the same screen.
  const playback = read('src-tauri/src/playback.rs');
  assert.match(
    playback,
    /let plan = crate::player::launch::build_launch_for\(\s+spec,\s+crate::player::PlayerKind::External,\s+&cfg,\s+&path,\s+\)\?;/,
  );
});

// Review finding pw-7: the control shows the mode the backend holds. The
// component's own load and save functions are run, with their types
// stripped, against a made-up backend. What is run and what is looked at are
// found by parsing, not by searching the text, so that quoting, attribute
// order or a copy left in a comment cannot change what is checked.
const playerWindowParts = async () => {
  const { parse } = await import('svelte/compiler');
  const ts = (await import('typescript')).default;
  const ast = parse(settings, { modern: true });

  // The control: one select with the id player-window, bound to
  // playerWindow.mode, and saves on change without disabling the select.
  // The control is offered, so it is
  // required.
  const elements = [];
  const seen = new Set();
  const walk = (node, section) => {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    if (node.type === 'RegularElement') {
      elements.push({ node, section });
      if (node.name === 'section') section = node;
    }
    for (const key of Object.keys(node)) if (key !== 'parent') walk(node[key], section);
  };
  walk(ast.fragment, null);
  const attribute = (node, name) => {
    const found = node.attributes.filter((a) => a.type === 'Attribute' && a.name === name);
    assert.ok(found.length <= 1, `<${node.name}> has one ${name}`);
    return found[0];
  };
  const text = (a) =>
    Array.isArray(a?.value) && a.value.every((part) => part.type === 'Text')
      ? a.value.map((part) => part.data).join('')
      : undefined;
  const selects = elements.filter(
    ({ node }) => node.name === 'select' && text(attribute(node, 'id')) === 'player-window',
  );
  assert.equal(selects.length, 1, 'there is one select with the id player-window');
  const [{ node: select, section }] = selects;
  const binds = select.attributes.filter((a) => a.type === 'BindDirective');
  assert.equal(binds.length, 1, 'the select has one binding');
  assert.equal(binds[0].name, 'value');
  assert.equal(settings.slice(binds[0].expression.start, binds[0].expression.end), 'playerWindow.mode');
  assert.ok(section, 'the select is in a section');
  const changeAttribute = attribute(select, 'onchange');
  const changeTag = Array.isArray(changeAttribute?.value) ? changeAttribute.value[0] : changeAttribute?.value;
  assert.ok(changeTag?.expression, 'changing the select saves the mode');
  const change = settings.slice(changeTag.expression.start, changeTag.expression.end);
  assert.equal(attribute(select, 'disabled'), undefined, 'the user can change their mind while saving');

  // The script, from the parser, read by the TypeScript compiler. Only
  // declarations are taken from it: a copy in a comment is not one.
  const program = ast.instance.content;
  const sourceFile = ts.createSourceFile(
    'Settings.ts',
    settings.slice(program.start, program.end),
    ts.ScriptTarget.ES2022,
    true,
  );
  const functions = new Map();
  const visit = (node) => {
    if (ts.isFunctionDeclaration(node) && node.name) {
      const name = node.name.text;
      functions.set(name, [...(functions.get(name) ?? []), node]);
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  const declaration = (name) => {
    const found = functions.get(name) ?? [];
    assert.equal(found.length, 1, `${name} is declared once`);
    return found[0];
  };
  const load = declaration('load');
  const save = declaration('savePlayerWindowMode');
  // The component's other functions for the player window (how it shows a
  // confirmed mode), each declared once.
  const helpers = [...functions.keys()]
    .filter((name) => name.includes('PlayerWindow') && name !== 'savePlayerWindowMode')
    .map(declaration);
  // The player window state, as the component declares it at its top level.
  const state = sourceFile.statements.filter(
    (statement) =>
      ts.isVariableStatement(statement) &&
      statement.declarationList.declarations.some(
        (d) => ts.isIdentifier(d.name) && d.name.text.startsWith('playerWindow'),
      ),
  );
  assert.ok(state.length >= 2, 'the player window state is declared');
  // What the component does about the player window when it is closed.
  const teardown = sourceFile.statements.filter(
    (statement) =>
      ts.isExpressionStatement(statement) &&
      ts.isCallExpression(statement.expression) &&
      ts.isIdentifier(statement.expression.expression) &&
      statement.expression.expression.text === 'onDestroy' &&
      /\bplayerWindow/.test(statement.expression.getText(sourceFile)),
  );
  // The module script runs once and is shared by every mounting of the
  // component (review finding pw-7: what the backend confirmed outlives one
  // opening of Settings). It is taken whole, as the compiler takes it.
  const moduleScript = ast.module
    ? settings.slice(ast.module.content.start, ast.module.content.end)
    : '';
  // Everything else load assigns is declared here as a plain variable.
  const assigned = new Set();
  const assignments = (node) => {
    if (
      ts.isBinaryExpression(node) &&
      node.operatorToken.kind === ts.SyntaxKind.EqualsToken &&
      ts.isIdentifier(node.left)
    ) {
      assigned.add(node.left.text);
    }
    ts.forEachChild(node, assignments);
  };
  assignments(load);
  const others = [...assigned].filter((name) => !name.startsWith('playerWindow') && name !== 'err');
  const code = (node) => node.getText(sourceFile);
  // One app: the module script, run once; each call of `open` is one
  // mounting of the component, with its own instance state, sharing the
  // module's. `close` runs what the mounting asked to run on destroy.
  const source = ts.transpileModule(
    [
      moduleScript,
      'return () => {',
      'const destroyed = [];',
      'const onDestroy = (fn) => { destroyed.push(fn); };',
      'let err = null;',
      'let loadSeq = 0;',
      others.length ? `let ${others.join(', ')};` : '',
      'function applyPlaybackPreferences() {}',
      ...state.map(code),
      ...teardown.map(code),
      ...helpers.map(code),
      code(load),
      code(save),
      'return {',
      '  load,',
      `  change() { return (${change})({ currentTarget: { value: playerWindow.mode } }); },`,
      '  get mode() { return playerWindow.mode; }, set mode(value) { playerWindowDraft.edit("mode"); playerWindow.mode = value; },',
      '  get confirmed() { return playerWindowSetting.confirmed.value.mode; },',
      '  get sharedConfirmed() { return playerWindowSetting.confirmed.value.mode; },',
      '  get saving() { return playerWindowSetting.status.pending > 0; },',
      '  get err() { return playerWindowDraft.feedback.error ?? err; },',
      '  close() { for (const fn of destroyed.splice(0)) fn(); },',
      '};',
      '};',
    ].join('\n'),
    { compilerOptions: { target: ts.ScriptTarget.ES2022 } },
  ).outputText;
  const app = (invoke) => ({ open: new Function('invoke', '$state', source)(invoke, (value) => value) });
  // A fresh app with one mounting, for the tests of one opening.
  const build = (invoke) => app(invoke).open();
  build.app = app;
  return build;
};

// A made-up backend. Each read of the playback preferences, and each save,
// can be held until the test lets it go; the mode a load gets is the one the
// backend holds when the load asks, and a held save changes the backend only
// when it is let go.
const madeUpBackend = (mode) => {
  const backend = { mode, refuse: false, failLoads: false, holdLoads: false, holdSaves: false, held: [] };
  const hold = (finish) =>
    new Promise((resolve, reject) => {
      backend.held.push(() => {
        try {
          resolve(finish());
        } catch (e) {
          reject(e);
        }
      });
    });
  const set = (args) => {
    if (backend.refuse) throw new Error('the settings could not be saved');
    backend.mode = args.mode;
    return backend.mode;
  };
  backend.invoke = async (command, args) => {
    if (command === 'get_mpv_window_mode') return backend.mode;
    if (command === 'set_mpv_window_mode') return backend.holdSaves ? hold(() => set(args)) : set(args);
    if (command === 'get_playback_preferences' && backend.failLoads) throw new Error('the settings could not be read');
    if (command === 'get_playback_preferences' && backend.holdLoads) return hold(() => ({}));
    return {};
  };
  // Lets the oldest held answer go, and waits until whoever waited on it has
  // gone on.
  backend.release = async () => {
    backend.held.shift()();
    for (let turn = 0; turn < 20; turn += 1) await Promise.resolve();
  };
  return backend;
};

test('a failed save of the player window setting puts the control back to the saved mode', async () => {
  const build = await playerWindowParts();
  const backend = madeUpBackend('default');
  const control = build(backend.invoke);

  // The mode comes from loading; a refused save goes back to it.
  await control.load();
  assert.equal(control.mode, 'default');
  control.mode = 'fullscreen';
  backend.refuse = true;
  await control.change();
  assert.equal(control.mode, 'default', 'a refused save must show what the backend holds');
  assert.match(control.err, /could not be saved/, 'and say why');
  assert.equal(backend.mode, 'default');

  // The mode comes from the last save that succeeded; a refused one goes
  // back to it.
  backend.refuse = false;
  control.mode = 'fullscreen';
  await control.change();
  assert.equal(control.mode, 'fullscreen');
  assert.equal(control.err, null);
  backend.refuse = true;
  control.mode = 'remember';
  await control.change();
  assert.equal(control.mode, 'fullscreen', 'a refused save must show what the backend holds');
  assert.equal(backend.mode, 'fullscreen');
});

test('a load that read the mode before a save was confirmed does not put the older mode back', async () => {
  const build = await playerWindowParts();

  // A load reads remember and waits; a save of fullscreen is confirmed; the
  // load completes; a second save is refused. The backend holds fullscreen,
  // and so must the control, both times.
  {
    const backend = madeUpBackend('remember');
    const control = build(backend.invoke);
    backend.holdLoads = true;
    const loading = control.load();
    backend.holdLoads = false;
    control.mode = 'fullscreen';
    await control.change();
    assert.equal(backend.mode, 'fullscreen');
    await backend.release();
    await loading;
    assert.equal(control.mode, 'fullscreen', 'a load must not undo a save confirmed after it read');
    backend.refuse = true;
    control.mode = 'default';
    await control.change();
    assert.equal(backend.mode, 'fullscreen');
    assert.equal(control.mode, 'fullscreen', 'a refused save must show what the backend holds');
  }

  // A load waits while a save is refused: what the load read is still what
  // the backend holds.
  {
    const backend = madeUpBackend('default');
    const control = build(backend.invoke);
    backend.holdLoads = true;
    const loading = control.load();
    backend.holdLoads = false;
    control.mode = 'fullscreen';
    backend.refuse = true;
    await control.change();
    await backend.release();
    await loading;
    assert.equal(control.mode, 'default', 'a load after a refused save shows what it read');
    control.mode = 'fullscreen';
    await control.change();
    assert.equal(control.mode, 'default', 'and a later refused save goes back to it');
  }

  // A save waits while a load reads the old mode and completes, then the
  // save is confirmed; and the same with the load completing after it.
  for (const loadFinishesFirst of [true, false]) {
    const backend = madeUpBackend('default');
    const control = build(backend.invoke);
    control.mode = 'fullscreen';
    backend.holdSaves = true;
    const saving = control.change();
    backend.holdSaves = false;
    backend.holdLoads = true;
    const loading = control.load();
    backend.holdLoads = false;
    // held[0] is the save, held[1] the load.
    if (loadFinishesFirst) {
      backend.held.push(backend.held.shift());
      await backend.release();
      await loading;
      assert.equal(control.mode, 'fullscreen', 'the slow load keeps the newer choice while it is saving');
      await backend.release();
      await saving;
    } else {
      await backend.release();
      await saving;
      await backend.release();
      await loading;
    }
    assert.equal(backend.mode, 'fullscreen');
    assert.equal(control.mode, 'fullscreen', 'the confirmed save is what the control shows');
    backend.refuse = true;
    control.mode = 'remember';
    await control.change();
    assert.equal(control.mode, 'fullscreen', 'a refused save must show what the backend holds');
  }

  // A save waits while a load reads the old mode, and is refused; the load
  // completes after it. The backend never changed.
  {
    const backend = madeUpBackend('default');
    const control = build(backend.invoke);
    control.mode = 'fullscreen';
    backend.holdSaves = true;
    backend.refuse = true;
    const saving = control.change();
    backend.holdSaves = false;
    backend.holdLoads = true;
    const loading = control.load();
    backend.holdLoads = false;
    await backend.release();
    await saving;
    await backend.release();
    await loading;
    assert.equal(backend.mode, 'default');
    assert.equal(control.mode, 'default', 'the control shows what the backend holds');
  }

  // Two loads: the older completes last and changes nothing.
  {
    const backend = madeUpBackend('default');
    const control = build(backend.invoke);
    backend.holdLoads = true;
    const older = control.load();
    backend.mode = 'fullscreen';
    const newer = control.load();
    backend.holdLoads = false;
    backend.held.push(backend.held.shift());
    await backend.release();
    await newer;
    await backend.release();
    await older;
    assert.equal(control.mode, 'fullscreen', 'an overtaken load changes nothing');
    backend.refuse = true;
    control.mode = 'remember';
    await control.change();
    assert.equal(control.mode, 'fullscreen', 'a refused save must show what the backend holds');
  }
});

// Settings is mounted afresh each time it is opened, and can be closed while
// a save is out, so one opening's save can be confirmed while another is
// loading or open (review finding pw-7, round 2). Each test below runs two
// mountings of the component that share its module script, as the app does.

test('a save confirmed while a reopened Settings is loading is what it shows', async () => {
  const build = await playerWindowParts();

  // As the review found it: A sends fullscreen; Settings is reopened as B,
  // which reads remember; A's save is confirmed; B's load completes. B must
  // show fullscreen, and a refused retry in B must go back to it. Run with A
  // closed, as reported, and with A still open.
  for (const closeA of [true, false]) {
    const backend = madeUpBackend('remember');
    const app = build.app(backend.invoke);
    const a = app.open();
    await a.load();
    a.mode = 'fullscreen';
    backend.holdSaves = true;
    const saving = a.change();
    backend.holdSaves = false;
    if (closeA) a.close();
    const b = app.open();
    backend.holdLoads = true;
    const loading = b.load();
    backend.holdLoads = false;
    // held[0] is A's save, held[1] B's load, which has read remember.
    await backend.release();
    await saving;
    assert.equal(backend.mode, 'fullscreen');
    await backend.release();
    await loading;
    assert.equal(b.mode, 'fullscreen', 'a load must not undo a save confirmed after it read, from any opening');
    assert.equal(b.confirmed, 'fullscreen');
    if (!closeA) {
      assert.equal(a.mode, 'fullscreen');
      assert.equal(a.confirmed, 'fullscreen');
    }
    backend.refuse = true;
    b.mode = 'default';
    await b.change();
    assert.equal(backend.mode, 'fullscreen');
    assert.equal(b.mode, 'fullscreen', 'a refused save must show what the backend holds');
  }

  // The same, but B's load completes before A's save is confirmed: B shows
  // what it read, which the backend held when B asked, and then what A's
  // save confirmed, without the user doing anything.
  for (const closeA of [true, false]) {
    const backend = madeUpBackend('remember');
    const app = build.app(backend.invoke);
    const a = app.open();
    await a.load();
    a.mode = 'fullscreen';
    backend.holdSaves = true;
    const saving = a.change();
    backend.holdSaves = false;
    if (closeA) a.close();
    const b = app.open();
    backend.holdLoads = true;
    const loading = b.load();
    backend.holdLoads = false;
    backend.held.push(backend.held.shift());
    await backend.release();
    await loading;
    assert.equal(b.mode, 'remember', 'nothing confirmed yet: what the load read');
    await backend.release();
    await saving;
    assert.equal(b.mode, 'fullscreen', 'a save confirmed from another opening must reach this one');
    assert.equal(b.confirmed, 'fullscreen');
    backend.refuse = true;
    b.mode = 'default';
    await b.change();
    assert.equal(b.mode, 'fullscreen', 'a refused save must show what the backend holds');
  }
});

test('a Settings opened after one that closed with its save out shows what that save confirmed', async () => {
  const build = await playerWindowParts();

  // A sends fullscreen and is closed; the save is confirmed with no Settings
  // open. B is opened: before its load answers, and if its load fails, it
  // shows what the app knows the backend holds.
  for (const loadFails of [false, true]) {
    const backend = madeUpBackend('remember');
    const app = build.app(backend.invoke);
    const a = app.open();
    await a.load();
    a.mode = 'fullscreen';
    backend.holdSaves = true;
    const saving = a.change();
    backend.holdSaves = false;
    a.close();
    await backend.release();
    await saving;
    assert.equal(backend.mode, 'fullscreen');

    const b = app.open();
    assert.equal(b.mode, 'fullscreen', 'a new opening starts on what the backend last confirmed');
    if (loadFails) {
      backend.failLoads = true;
      await b.load();
      assert.match(b.err, /could not be read/);
    } else {
      backend.holdLoads = true;
      const loading = b.load();
      backend.holdLoads = false;
      backend.refuse = true;
      b.mode = 'default';
      await b.change();
      assert.equal(b.mode, 'fullscreen', 'a refused save before the load answers must show what the backend holds');
      backend.refuse = false;
      await backend.release();
      await loading;
    }
    assert.equal(b.mode, 'fullscreen');
    assert.equal(b.confirmed, 'fullscreen');
    backend.refuse = true;
    b.mode = 'remember';
    await b.change();
    assert.equal(b.mode, 'fullscreen', 'a refused save must show what the backend holds');

    // A closed opening is no longer told of saves.
    backend.refuse = false;
    const before = a.mode;
    b.mode = 'default';
    await b.change();
    assert.equal(backend.mode, 'default');
    assert.equal(b.mode, 'default');
    assert.equal(a.mode, before, 'a closed Settings must not be told of later saves');
  }
});

test('a save confirmed in one open Settings reaches another, keeping a newer draft', async () => {
  const build = await playerWindowParts();
  const backend = madeUpBackend('remember');
  const app = build.app(backend.invoke);
  const a = app.open();
  const b = app.open();
  await a.load();
  await b.load();

  // B has loaded and is open; A's save is confirmed: B shows it.
  a.mode = 'fullscreen';
  await a.change();
  assert.equal(b.mode, 'fullscreen', 'a save confirmed from another opening must reach this one');
  assert.equal(b.confirmed, 'fullscreen');
  backend.refuse = true;
  b.mode = 'default';
  await b.change();
  assert.equal(b.mode, 'fullscreen', 'a refused save must show what the backend holds');
  backend.refuse = false;

  // B's user picks default and has not saved. A save that confirms what the
  // backend already held, and a load that reads it, leave that choice.
  b.mode = 'default';
  await a.change();
  assert.equal(b.mode, 'default', 'an unsaved choice stays while the backend has not changed');
  await b.load();
  assert.equal(b.mode, 'default', 'and a load that reads the same mode keeps it');
  assert.equal(b.confirmed, 'fullscreen');
  // A save from another opening still leaves the newer draft alone.
  a.mode = 'remember';
  await a.change();
  assert.equal(b.mode, 'default', 'a changed backend leaves the newer draft alone');
  assert.equal(b.confirmed, 'remember');

  // Two save requests, one from each: both complete, and both openings
  // show what the backend holds. Keep holding writes until both have been
  // sent, including when the second waits for the first one's reply.
  a.mode = 'fullscreen';
  b.mode = 'default';
  backend.holdSaves = true;
  const savingA = a.change();
  const savingB = b.change();
  await backend.release();
  await savingA;
  await backend.release();
  await savingB;
  backend.holdSaves = false;
  assert.equal(backend.mode, 'default');
  for (const [name, control] of [['A', a], ['B', b]]) {
    assert.equal(control.mode, 'default', `${name} shows what the backend holds`);
    assert.equal(control.confirmed, 'default');
  }
});

test('a refused save in one open Settings goes back to what the backend holds and leaves the other alone', async () => {
  const build = await playerWindowParts();
  const backend = madeUpBackend('fullscreen');
  const app = build.app(backend.invoke);
  const a = app.open();
  const b = app.open();
  await a.load();
  await b.load();
  b.mode = 'remember';
  backend.refuse = true;
  a.mode = 'default';
  await a.change();
  assert.equal(backend.mode, 'fullscreen');
  assert.equal(a.mode, 'fullscreen', 'a refused save must show what the backend holds');
  assert.equal(a.confirmed, 'fullscreen');
  assert.equal(b.mode, 'remember', "the other's unsaved choice is left alone");
  assert.equal(b.confirmed, 'fullscreen');
  b.mode = 'default';
  await b.change();
  assert.equal(b.mode, 'fullscreen', 'a refused save must show what the backend holds');
});

// Round 3: applying a write and delivering its reply are different events.
// No write in this backend applies or answers automatically. A successful
// reply carries THAT write's applied value, even if a later write has since
// changed the backend. Tests may refuse a write before or after applying it.
// This permits the exact bad reply order when the serialization is removed;
// with serialization in place, the later request must not even be sent yet.
const splitWriteBackend = (mode) => {
  const backend = { mode, writes: [], applied: [], delivered: [] };
  const request = (index) => {
    assert.ok(backend.writes[index], `write ${index} has not been sent`);
    return backend.writes[index];
  };
  backend.invoke = (command, args) => {
    if (command === 'get_mpv_window_mode') return Promise.resolve(backend.mode);
    if (command !== 'set_mpv_window_mode') return Promise.resolve({});
    return new Promise((resolve, reject) => {
      backend.writes.push({ mode: args.mode, resolve, reject, applied: false, delivered: false });
    });
  };
  backend.apply = (index) => {
    const write = request(index);
    assert.equal(write.applied, false, `write ${index} was already applied`);
    assert.equal(write.delivered, false, `write ${index} already replied`);
    backend.mode = write.mode;
    write.applied = true;
    backend.applied.push(index);
  };
  backend.deliver = (index, refused = false) => {
    const write = request(index);
    assert.equal(write.delivered, false, `write ${index} already replied`);
    assert.ok(refused || write.applied, `write ${index} must apply before a successful reply`);
    write.delivered = true;
    backend.delivered.push(index);
    if (refused) write.reject(new Error('the settings could not be saved'));
    else write.resolve(write.mode);
  };
  return backend;
};

// Give promise continuations (including the next queued send) a full event
// loop turn. There is no clock delay and an unanswered save is never awaited.
const nextTurn = () => new Promise((resolve) => setImmediate(resolve));
const assertBackendMode = (control, backend, because) => {
  assert.equal(control.mode, backend.mode, `${because}: selection`);
  assert.equal(control.confirmed, backend.mode, `${because}: opening's confirmed mode`);
  assert.equal(control.sharedConfirmed, backend.mode, `${because}: app's confirmed mode`);
};

test('an applied save from closed A cannot reply after reopened B is confirmed', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const app = build.app(backend.invoke);
  const a = app.open();
  await a.load();
  a.mode = 'fullscreen';
  const savingA = a.change();
  await nextTurn();
  backend.apply(0);
  a.close();

  const b = app.open();
  await b.load();
  assertBackendMode(b, backend, 'B reads the applied fullscreen before A replies');
  b.mode = 'default';
  const savingB = b.change();
  await nextTurn();

  // Attempt the reviewer's exact order: B applies and is confirmed, then
  // A's old reply arrives. Under the fix B cannot reach the backend yet.
  // If that gate regresses, do deliver this order so the final assertion
  // diagnoses the original stale selection AND stale confirmed value.
  const overlapped = backend.writes.length > 1;
  if (overlapped) {
    backend.apply(1);
    backend.deliver(1);
    await savingB;
    assertBackendMode(b, backend, 'B has confirmed default before A replies');
  }
  backend.deliver(0);
  await savingA;
  await nextTurn();
  if (!overlapped) {
    assert.equal(backend.writes.length, 2, 'B is sent when A answers');
    backend.apply(1);
    backend.deliver(1);
    await savingB;
  }
  assert.equal(backend.mode, 'default');
  assertBackendMode(b, backend, "A's delayed reply must not undo B's applied default");
  assert.equal(overlapped, false, 'B must wait for A, not guess backend order from replies');
});

// Separate guards for a normal refusal and a rejection after application.
for (const applyA of [false, true]) {
  test(`a delayed refusal from closed A ${applyA ? 'after applying' : 'without applying'} releases reopened B`, async () => {
    const build = await playerWindowParts();
    const backend = splitWriteBackend('remember');
    const app = build.app(backend.invoke);
    const a = app.open();
    await a.load();
    a.mode = 'fullscreen';
    const savingA = a.change();
    await nextTurn();
    if (applyA) backend.apply(0);
    a.close();
    const b = app.open();
    await b.load();
    b.mode = 'default';
    const savingB = b.change();
    await nextTurn();
    assert.equal(backend.writes.length, 1, 'B must wait even if A will refuse');
    backend.deliver(0, true);
    await savingA;
    await nextTurn();
    assert.match(a.err, /could not be saved/, 'the refusal is reported to its caller');
    assert.equal(backend.writes.length, 2, 'a refused predecessor must release B');
    assert.equal(b.sharedConfirmed, backend.mode, 'a refusal must not confirm the attempted mode');
    backend.apply(1);
    backend.deliver(1);
    await savingB;
    assert.equal(backend.mode, 'default');
    assertBackendMode(b, backend, 'B confirms default after the refusal');
    assert.equal(b.err, null, 'A does not give B its error');
  });
}

test('a refused retry after reopened B saves default restores default', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const app = build.app(backend.invoke);
  const a = app.open();
  await a.load();
  a.mode = 'fullscreen';
  const savingA = a.change();
  await nextTurn();
  backend.apply(0);
  a.close();
  const b = app.open();
  await b.load();
  b.mode = 'default';
  const savingB = b.change();
  await nextTurn();
  const overlapped = backend.writes.length > 1;
  if (overlapped) {
    backend.apply(1);
    backend.deliver(1);
    await savingB;
  }
  backend.deliver(0);
  await savingA;
  await nextTurn();
  if (!overlapped) {
    backend.apply(1);
    backend.deliver(1);
    await savingB;
  }
  b.mode = 'remember';
  const retry = b.change();
  await nextTurn();
  backend.deliver(2, true);
  await retry;
  assert.equal(backend.mode, 'default');
  assertBackendMode(b, backend, 'a refused retry must restore B\'s applied default');
  assert.match(b.err, /could not be saved/);

  // The refusal must leave the page able to retry successfully too.
  b.mode = 'fullscreen';
  const success = b.change();
  await nextTurn();
  backend.apply(3);
  backend.deliver(3);
  await success;
  assertBackendMode(b, backend, 'the retry after a refusal is confirmed');
  assert.equal(b.err, null);
  assert.equal(b.saving, false);
});

test('one page prevents replies in reverse application order', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const control = build(backend.invoke);
  await control.load();
  control.mode = 'fullscreen';
  const first = control.change();
  await nextTurn();
  backend.apply(0);
  control.mode = 'default';
  const second = control.change();
  control.mode = 'remember';
  const last = control.change();
  await nextTurn();
  assert.equal(backend.writes.length, 1, 'later changes wait for the first reply');
  assert.throws(() => backend.deliver(1), /write 1 has not been sent/);
  backend.deliver(0);
  await first;
  await nextTurn();
  assert.equal(backend.writes.length, 2, 'the second change is sent after the first reply');
  assert.equal(control.mode, 'remember', 'the first reply keeps the latest choice visible');
  assert.equal(backend.writes[1].mode, 'default');
  backend.apply(1);
  backend.deliver(1);
  await second;
  await nextTurn();
  assert.equal(backend.writes.length, 3, 'the last change is sent after the second reply');
  assert.equal(control.mode, 'remember', 'the second reply keeps the latest choice visible');
  backend.apply(2);
  backend.deliver(2);
  await last;
  assert.equal(backend.mode, 'remember');
  assertBackendMode(control, backend, 'the last change is stored and shown');
  assert.deepEqual(backend.applied, [0, 1, 2]);
  assert.deepEqual(backend.delivered, [0, 1, 2]);
});

test('two open pages prevent replies in reverse application order', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const app = build.app(backend.invoke);
  const a = app.open();
  const b = app.open();
  await a.load();
  await b.load();
  a.mode = 'fullscreen';
  const savingA = a.change();
  await nextTurn();
  backend.apply(0);
  b.mode = 'default';
  const savingB = b.change();
  await nextTurn();
  const overlapped = backend.writes.length > 1;
  if (overlapped) {
    backend.apply(1);
    backend.deliver(1);
    await savingB;
    assertBackendMode(a, backend, 'B reaches the other open page');
  }
  backend.deliver(0);
  await savingA;
  await nextTurn();
  if (!overlapped) {
    backend.apply(1);
    backend.deliver(1);
    await savingB;
  }
  assert.deepEqual(backend.applied, [0, 1]);
  assert.equal(backend.mode, 'default');
  for (const [name, control] of [['A', a], ['B', b]]) {
    assertBackendMode(control, backend, `${name} must follow application order, not reversed replies`);
  }
  assert.deepEqual(backend.delivered, [0, 1], 'serialization makes reverse replies unreachable');
});

test('queued choices survive several closings and openings', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const app = build.app(backend.invoke);
  const choices = ['fullscreen', 'default', 'remember', 'fullscreen'];
  const pending = [];
  for (const choice of choices) {
    const control = app.open();
    await control.load();
    control.mode = choice;
    pending.push(control.change());
    control.close();
  }
  await nextTurn();
  assert.equal(backend.writes.length, 1, 'closing a page must not let later writes overtake it');
  // Drain with NO page open: unmounting must not discard queued choices.
  for (const [index, choice] of choices.entries()) {
    assert.equal(backend.writes.length, index + 1, 'only the next queued write is sent');
    assert.equal(backend.writes[index].mode, choice, 'every closed page keeps its changed choice');
    backend.apply(index);
    backend.deliver(index);
    await pending[index];
    await nextTurn();
  }
  const reopened = app.open();
  assertBackendMode(reopened, backend, 'a new opening starts on the last queued confirmation');
  await reopened.load();
  assertBackendMode(reopened, backend, 'loading agrees with the last queued confirmation');
});

test('a queued save keeps its changed choice through an earlier confirmation', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const app = build.app(backend.invoke);
  const a = app.open();
  const b = app.open();
  await a.load();
  await b.load();
  a.mode = 'fullscreen';
  const savingA = a.change();
  b.mode = 'default';
  const savingB = b.change();
  await nextTurn();
  backend.apply(0);
  backend.deliver(0);
  await savingA;
  await nextTurn();
  assert.equal(b.mode, 'default', 'the earlier confirmation does not replace B while B is waiting');
  assert.equal(backend.writes[1]?.mode, 'default', 'the queued write must keep the changed default');
  // Neither a load nor another change alters the already captured request.
  await b.load();
  b.mode = 'remember';
  const newer = b.change();
  backend.apply(1);
  backend.deliver(1);
  await savingB;
  assert.equal(backend.mode, 'default');
  assert.equal(b.mode, 'remember', 'the reply keeps the newer choice visible');
  await nextTurn();
  backend.apply(2);
  backend.deliver(2);
  await newer;
  assertBackendMode(b, backend, 'the newer choice follows the captured default');
});

test('a refusal in the middle of the queue does not discard the following save', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const app = build.app(backend.invoke);
  const controls = ['fullscreen', 'default', 'remember'].map((mode) => {
    const control = app.open();
    control.mode = mode;
    return control;
  });
  const pending = controls.map((control) => control.change());
  await nextTurn();
  backend.apply(0);
  backend.deliver(0);
  await pending[0];
  await nextTurn();
  assert.equal(backend.writes.length, 2, 'only the second write may run before its reply');
  backend.deliver(1, true);
  await pending[1];
  await nextTurn();
  assertBackendMode(controls[1], backend, 'the refused middle save restores fullscreen');
  assert.match(controls[1].err, /could not be saved/);
  assert.equal(backend.writes.length, 3, 'a queued refusal must release the following write');
  backend.apply(2);
  backend.deliver(2);
  await pending[2];
  assert.equal(backend.mode, 'remember');
  for (const control of controls) {
    assertBackendMode(control, backend, 'the final queued save reaches every open page');
    assert.equal(control.saving, false);
  }
});

test('a fresh page sends and confirms its first save', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('default');
  const control = build(backend.invoke);
  await control.load();
  control.mode = 'fullscreen';
  const saving = control.change();
  await nextTurn();
  assert.equal(backend.writes.length, 1, 'a fresh page must send its first save');
  assert.equal(backend.writes[0].mode, 'fullscreen');
  assert.equal(control.saving, true);
  assert.equal(backend.mode, 'default', 'sending alone does not apply a write');
  assert.equal(control.sharedConfirmed, 'default', 'sending alone does not confirm a write');
  backend.apply(0);
  assert.equal(control.sharedConfirmed, 'default', 'applying alone does not deliver a reply');
  backend.deliver(0);
  await saving;
  assertBackendMode(control, backend, 'the fresh page confirms its save');
  assert.equal(control.saving, false, 'a confirmed save clears the pending status');
  assert.equal(control.err, null);
});

test('a new page can save after a queue has drained', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const app = build.app(backend.invoke);
  const first = app.open();
  first.mode = 'fullscreen';
  const savingFirst = first.change();
  await nextTurn();
  backend.apply(0);
  backend.deliver(0);
  await savingFirst;
  first.close();
  await nextTurn();
  const fresh = app.open();
  await fresh.load();
  fresh.mode = 'default';
  const savingFresh = fresh.change();
  await nextTurn();
  assert.equal(backend.writes.length, 2, 'a drained queue must accept a new page\'s save');
  backend.apply(1);
  backend.deliver(1);
  await savingFresh;
  assertBackendMode(fresh, backend, 'the new page confirms default');
});

test('an earlier completion cannot release the queue while a later write is unanswered', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const app = build.app(backend.invoke);
  const a = app.open();
  const b = app.open();
  a.mode = 'fullscreen';
  const savingA = a.change();
  b.mode = 'default';
  const savingB = b.change();
  await nextTurn();
  backend.apply(0);
  backend.deliver(0);
  await savingA;
  await nextTurn();
  assert.equal(backend.writes.length, 2);
  a.close();
  b.close();
  const c = app.open();
  await c.load();
  c.mode = 'remember';
  const savingC = c.change();
  await nextTurn();
  assert.equal(backend.writes.length, 2, 'finishing A must not let C bypass unanswered B');
  backend.apply(1);
  backend.deliver(1);
  await savingB;
  await nextTurn();
  assert.equal(backend.writes.length, 3, 'C is sent only after B answers');
  backend.apply(2);
  backend.deliver(2);
  await savingC;
  assertBackendMode(c, backend, 'C becomes the final confirmed write');
});

test('an unanswered save keeps later writes queued while pages can load and close', async () => {
  const build = await playerWindowParts();
  const backend = splitWriteBackend('remember');
  const app = build.app(backend.invoke);
  const a = app.open();
  await a.load();
  a.mode = 'fullscreen';
  let finished = 0;
  void a.change().then(() => { finished += 1; });
  await nextTurn();
  backend.apply(0);
  a.close();
  for (const choice of ['default', 'remember', 'default']) {
    const control = app.open();
    let loaded = false;
    const loading = control.load().then(() => { loaded = true; });
    await nextTurn();
    assert.equal(loaded, true, 'an unanswered save must not block a new page load');
    await loading;
    assertBackendMode(control, backend, 'a pending save does not block loading a new page');
    assert.equal(control.saving, true, 'a new page reports the shared pending save without disabling the control');
    control.mode = choice;
    void control.change().then(() => { finished += 1; });
    assert.equal(control.saving, true, 'this page reports its queued save as pending');
    control.close();
    await nextTurn();
  }
  assert.equal(backend.writes.length, 1, 'no later write may escape an unanswered predecessor');
  assert.equal(finished, 0, 'no queued save may be reported complete or discarded');
  assert.equal(backend.mode, 'fullscreen');
  assert.deepEqual(backend.delivered, [], 'the test never answers the first write');
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
