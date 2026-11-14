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
  // The player window state, as the component declares it at its top level.
  const state = sourceFile.statements.filter(
    (statement) =>
      ts.isVariableStatement(statement) &&
      statement.declarationList.declarations.some(
        (d) => ts.isIdentifier(d.name) && d.name.text.startsWith('playerWindow'),
      ),
  );
  assert.ok(state.length >= 2, 'the player window state is declared');
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
  const source = ts.transpileModule(
    [
      'let err = null;',
      'let loadSeq = 0;',
      others.length ? `let ${others.join(', ')};` : '',
      'function applyPlaybackPreferences() {}',
      ...state.map(code),
      code(load),
      code(save),
      'return {',
      '  load, savePlayerWindowMode,',
      '  get mode() { return playerWindowMode; }, set mode(value) { playerWindowMode = value; },',
      '  get err() { return err; },',
      '};',
    ].join('\n'),
    { compilerOptions: { target: ts.ScriptTarget.ES2022 } },
  ).outputText;
  return (invoke) => new Function('invoke', '$state', source)(invoke, (value) => value);
};

// A made-up backend. Each read of the playback preferences, and each save,
// can be held until the test lets it go; the mode a load gets is the one the
// backend holds when the load asks, and a held save changes the backend only
// when it is let go.
const madeUpBackend = (mode) => {
  const backend = { mode, refuse: false, holdLoads: false, holdSaves: false, held: [] };
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
