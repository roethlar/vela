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
    /let \(first_of_run, _\) = config::load_config\(\)[\s\S]{0,200}config::first_window_of_run\(&cfg\)[\s\S]{0,4000}let resolved = match selection\.version_id/,
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
    /let resolved = match selection\.version_id[\s\S]*?\.map_err\(PlayFailure::unavailable\)\?;/,
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
  assert.match(
    commands,
    /if !recents_ready\.wait_succeeded\(\) \{[\s\S]{0,1800}let _ = write_play_end\(\s*\|change\| config::update\(change\),\s*&latest_window,\s*&session_id,\s*move \|cfg\| \{\s*crate::recents::finish_session\(cfg, &key, &finishing_session, position_ms, now_ms\);\s*\},\s*\);/,
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

test('a play is shown on the one screen decided before its copy is chosen', () => {
  // Review findings pw-1 and pw-2. The rules are unit-tested in commands.rs
  // (screen_for_play, rank_and_place, and the late fallback through
  // playback::play) and display.rs (profile_basis); this guards the wiring
  // they cannot reach, because select_playback_version and the play path
  // need a running app: the screen is decided once, the real ranking ranks
  // with the policy for that screen's profile, and the launch asks for that
  // screen and no other.
  const bodyOf = (source, signature) => {
    const start = source.indexOf(`\n${signature}`);
    assert.notEqual(start, -1, `${signature} exists`);
    return source.slice(start, source.indexOf('\n}\n', start));
  };
  const count = (source, text) => source.split(text).length - 1;
  const production = commands.slice(0, commands.indexOf('\n#[cfg(test)]\nmod tests {'));
  const select = bodyOf(commands, 'async fn select_playback_version(');

  // Decided once, for the player that will run the play, after the probes
  // and before either ready selection is made.
  const decided = select.search(
    /let \(screen, screens\) =\s+decide_play_screen\(state, replace_session, upcoming_player\.kind, &cfg\)\.await\?;/,
  );
  assert.ok(decided > 0, 'the screen is decided in the choice of copy');
  assert.equal(count(select, 'decide_play_screen('), 1, 'once');
  assert.ok(select.indexOf('let upcoming_player = crate::player::choose(') < decided, 'after the player is chosen');
  assert.ok(decided < select.indexOf('if versions.is_empty() {'), 'before any selection is made');

  // The real ranking: its target only for a Compatible choice, the profile
  // of the decided screen; then rank_and_place with the policy, that screen
  // and that target. Nothing else in the choice of copy ranks.
  assert.match(
    select,
    /let target = match state\.app_handle\.get\(\) \{\s+Some\(app\) if policy == crate::selection::PlaybackSourcePolicy::Compatible => \{\s+let overrides = playback_display_overrides\(&cfg\)\.map_err\(PlayFailure::fatal\)\?;\s+Some\(compatibility_target\(app, &screen, &screens, overrides\)\.await\)\s+\}\s+_ => None,\s+\};\s+let \(winner, destination\) = rank_and_place\(&mut candidates, policy, &screen, target\)\s+\.ok_or_else/,
    'the copy is ranked by the policy, for the decided screen',
  );
  assert.equal(count(select, 'rank_and_place('), 1);
  assert.doesNotMatch(select, /rank_candidates\(/, 'the choice of copy ranks only through rank_and_place');
  const place = bodyOf(commands, 'fn rank_and_place(');
  assert.match(place, /\) -> Option<[^{]*\{\s+crate::selection::rank_candidates\(candidates, policy, target\);\s+candidates\s+\.first\(\)\s+\.cloned\(\)\s+\.map\(\|winner\| \(winner, screen\.name\.clone\(\)\)\)$/);
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

  // Both ready selections carry the decided screen: the ranked one as
  // rank_and_place places it, the one with nothing to rank as decided.
  assert.match(select, /player: upcoming_player,\s+screen: destination,\s+\}\),/, 'the ranked selection carries it');
  assert.match(select, /player: upcoming_player,\s+screen: screen\.name,\s+\}\),/, 'an unranked one carries it too');
  assert.equal(count(select, 'screen: '), 2);

  // How it is decided: the screens are looked at before the record is
  // locked, and the record is held only to decide.
  const decide = bodyOf(commands, 'async fn decide_play_screen(');
  assert.match(
    decide,
    /let \(_, saved_first_screen\) = config::first_window_of_run\(cfg\)\.map_err\(PlayFailure::fatal\)\?;\s+let screens = match state\.app_handle\.get\(\) \{\s+Some\(app\) => display::look_at_screens\(app\)\.await,\s+None => display::Screens::default\(\),\s+\};\s+let screen = \{\s+let current = state\s+\.playback_window_session\s+\.lock\(\)\s+\.unwrap_or_else\(\|error\| error\.into_inner\(\)\);\s+screen_for_play\(\s+&current,\s+replace_session,\s+upcoming,\s+saved_first_screen\.as_deref\(\),\s+&screens,\s+\)\s+\};\s+Ok\(\(screen, screens\)\)$/,
  );
  assert.equal(count(production, 'screen_for_play('), 2, 'decided in one place');
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
  // The HDR state is asked for the decided output, by its name, size and place.
  assert.match(
    bodyOf(display, 'pub(crate) async fn hdr_of('),
    /\) -> HdrState \{\s+native_hdr_state\(\s+app,\s+basis\.name\.clone\(\),\s+basis\.width_px,\s+basis\.height_px,\s+basis\.position,\s+\)\s+\.await$/,
    'the HDR state is the decided output\'s',
  );

  // The launch asks for the decided screen: no second look at where Vela's
  // window is, or at the record's screen, after the copy is chosen.
  const playBody = bodyOf(commands, 'async fn play_by_key_locked(');
  assert.match(playBody, /let screen_name = selection\.screen\.clone\(\);/);
  assert.match(playBody, /\n\s+screen_name,\n/, 'and handed to the launch');
  assert.equal([...playBody.matchAll(/\bscreen_name\b/g)].length, 2, 'untouched in between');
  assert.doesNotMatch(
    playBody,
    /current_screen_name|look_at_screens|screen_for_play|decide_play_screen|detect_profile|profile_of|inherited_screen_name|display_snapshot/,
    'and nothing in the play path looks at a screen again',
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
  // playerWindowMode, and in its section one button, which saves it and is
  // disabled while a save is under way. The control is offered, so it is
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
  const identifier = (a) => {
    const tag = Array.isArray(a?.value) ? (a.value.length === 1 ? a.value[0] : undefined) : a?.value;
    return tag?.type === 'ExpressionTag' && tag.expression.type === 'Identifier'
      ? tag.expression.name
      : undefined;
  };
  const selects = elements.filter(
    ({ node }) => node.name === 'select' && text(attribute(node, 'id')) === 'player-window',
  );
  assert.equal(selects.length, 1, 'there is one select with the id player-window');
  const [{ node: select, section }] = selects;
  const binds = select.attributes.filter((a) => a.type === 'BindDirective');
  assert.equal(binds.length, 1, 'the select has one binding');
  assert.equal(binds[0].name, 'value');
  assert.equal(binds[0].expression.type, 'Identifier');
  assert.equal(binds[0].expression.name, 'playerWindowMode', 'the select shows playerWindowMode');
  assert.ok(section, 'the select is in a section');
  const buttons = elements.filter((e) => e.section === section && e.node.name === 'button');
  assert.equal(buttons.length, 1, 'the section has one button');
  const [{ node: button }] = buttons;
  assert.equal(identifier(attribute(button, 'onclick')), 'savePlayerWindowMode', 'the button saves the mode');
  assert.equal(identifier(attribute(button, 'disabled')), 'playerWindowBusy', 'and is disabled while saving');

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
      '  load, savePlayerWindowMode,',
      '  get mode() { return playerWindowMode; }, set mode(value) { playerWindowMode = value; },',
      '  get confirmed() { return playerWindowSaved; },',
      '  get err() { return err; },',
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
  await control.savePlayerWindowMode();
  assert.equal(control.mode, 'default', 'a refused save must show what the backend holds');
  assert.match(control.err, /could not be saved/, 'and say why');
  assert.equal(backend.mode, 'default');

  // The mode comes from the last save that succeeded; a refused one goes
  // back to it.
  backend.refuse = false;
  control.mode = 'fullscreen';
  await control.savePlayerWindowMode();
  assert.equal(control.mode, 'fullscreen');
  assert.equal(control.err, null);
  backend.refuse = true;
  control.mode = 'remember';
  await control.savePlayerWindowMode();
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
    await control.savePlayerWindowMode();
    assert.equal(backend.mode, 'fullscreen');
    await backend.release();
    await loading;
    assert.equal(control.mode, 'fullscreen', 'a load must not undo a save confirmed after it read');
    backend.refuse = true;
    control.mode = 'default';
    await control.savePlayerWindowMode();
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
    await control.savePlayerWindowMode();
    await backend.release();
    await loading;
    assert.equal(control.mode, 'default', 'a load after a refused save shows what it read');
    control.mode = 'fullscreen';
    await control.savePlayerWindowMode();
    assert.equal(control.mode, 'default', 'and a later refused save goes back to it');
  }

  // A save waits while a load reads the old mode and completes, then the
  // save is confirmed; and the same with the load completing after it.
  for (const loadFinishesFirst of [true, false]) {
    const backend = madeUpBackend('default');
    const control = build(backend.invoke);
    control.mode = 'fullscreen';
    backend.holdSaves = true;
    const saving = control.savePlayerWindowMode();
    backend.holdSaves = false;
    backend.holdLoads = true;
    const loading = control.load();
    backend.holdLoads = false;
    // held[0] is the save, held[1] the load.
    if (loadFinishesFirst) {
      backend.held.push(backend.held.shift());
      await backend.release();
      await loading;
      assert.equal(control.mode, 'default', 'nothing confirmed yet: what the load read');
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
    await control.savePlayerWindowMode();
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
    const saving = control.savePlayerWindowMode();
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
    await control.savePlayerWindowMode();
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
    const saving = a.savePlayerWindowMode();
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
    await b.savePlayerWindowMode();
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
    const saving = a.savePlayerWindowMode();
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
    await b.savePlayerWindowMode();
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
    const saving = a.savePlayerWindowMode();
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
      await b.savePlayerWindowMode();
      assert.equal(b.mode, 'fullscreen', 'a refused save before the load answers must show what the backend holds');
      backend.refuse = false;
      await backend.release();
      await loading;
    }
    assert.equal(b.mode, 'fullscreen');
    assert.equal(b.confirmed, 'fullscreen');
    backend.refuse = true;
    b.mode = 'remember';
    await b.savePlayerWindowMode();
    assert.equal(b.mode, 'fullscreen', 'a refused save must show what the backend holds');

    // A closed opening is no longer told of saves.
    backend.refuse = false;
    const before = [a.mode, a.confirmed];
    b.mode = 'default';
    await b.savePlayerWindowMode();
    assert.equal(backend.mode, 'default');
    assert.equal(b.mode, 'default');
    assert.deepEqual([a.mode, a.confirmed], before, 'a closed Settings must not be told of later saves');
  }
});

test('a save confirmed in one open Settings reaches another, keeping an unsaved choice the save does not change', async () => {
  const build = await playerWindowParts();
  const backend = madeUpBackend('remember');
  const app = build.app(backend.invoke);
  const a = app.open();
  const b = app.open();
  await a.load();
  await b.load();

  // B has loaded and is open; A's save is confirmed: B shows it.
  a.mode = 'fullscreen';
  await a.savePlayerWindowMode();
  assert.equal(b.mode, 'fullscreen', 'a save confirmed from another opening must reach this one');
  assert.equal(b.confirmed, 'fullscreen');
  backend.refuse = true;
  b.mode = 'default';
  await b.savePlayerWindowMode();
  assert.equal(b.mode, 'fullscreen', 'a refused save must show what the backend holds');
  backend.refuse = false;

  // B's user picks default and has not saved. A save that confirms what the
  // backend already held, and a load that reads it, leave that choice.
  b.mode = 'default';
  await a.savePlayerWindowMode();
  assert.equal(b.mode, 'default', 'an unsaved choice stays while the backend has not changed');
  await b.load();
  assert.equal(b.mode, 'default', 'and a load that reads the same mode keeps it');
  assert.equal(b.confirmed, 'fullscreen');
  // A save that changes the backend under it replaces it.
  a.mode = 'remember';
  await a.savePlayerWindowMode();
  assert.equal(b.mode, 'remember', 'a changed backend replaces an unsaved choice');
  assert.equal(b.confirmed, 'remember');

  // Two saves out at once, one from each: the one confirmed last is what
  // the backend holds, and what both show.
  a.mode = 'fullscreen';
  b.mode = 'default';
  backend.holdSaves = true;
  const savingA = a.savePlayerWindowMode();
  const savingB = b.savePlayerWindowMode();
  backend.holdSaves = false;
  await backend.release();
  await savingA;
  await backend.release();
  await savingB;
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
  await a.savePlayerWindowMode();
  assert.equal(backend.mode, 'fullscreen');
  assert.equal(a.mode, 'fullscreen', 'a refused save must show what the backend holds');
  assert.equal(a.confirmed, 'fullscreen');
  assert.equal(b.mode, 'remember', "the other's unsaved choice is left alone");
  assert.equal(b.confirmed, 'fullscreen');
  b.mode = 'default';
  await b.savePlayerWindowMode();
  assert.equal(b.mode, 'fullscreen', 'a refused save must show what the backend holds');
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
