// The backend's side of the player view's controls
// (.agents/plans/embedded-player.slice5-design.md, S1, S2, S3 and S5): the
// wiring that the Rust tests cannot reach, because there is no Tauri harness
// to call a command with. The decisions themselves are proven in Rust
// (player/control.rs, player/report.rs, player/mod.rs, playback.rs); these
// pins hold that the commands and the event do nothing but hand over to them.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file) => fs.readFileSync(path.join(repoRoot, file), 'utf8').replace(/\r\n/g, '\n');
const commands = read('src-tauri/src/commands.rs');
const lib = read('src-tauri/src/lib.rs');
const playback = read('src-tauri/src/playback.rs');
const playerMod = read('src-tauri/src/player/mod.rs');
const control = read('src-tauri/src/player/control.rs');
const report = read('src-tauri/src/player/report.rs');
const session = read('src-tauri/src/player/embedded/session.rs');
const host = read('src-tauri/src/player/embedded/host.rs');

/** A top-level item's text, from its first line to its closing brace. */
function item(source, opening, label) {
  const start = source.indexOf(`\n${opening}`);
  assert.notEqual(start, -1, `${label} must exist`);
  assert.equal(source.indexOf(`\n${opening}`, start + 1), -1, `${label} must exist once`);
  const end = source.indexOf('\n}\n', start);
  assert.notEqual(end, -1, `${label} must end`);
  return source.slice(start + 1, end + 3);
}

/** A file's shipped code: everything before its test module. */
function shipped(source) {
  const tests = source.indexOf('#[cfg(test)]\nmod tests');
  return tests === -1 ? source : source.slice(0, tests);
}

const count = (text, needle) => text.split(needle).length - 1;

/** Every shipped Rust file, by path. */
function shippedRust() {
  const files = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(path.join(repoRoot, dir), { withFileTypes: true })) {
      const relative = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(relative);
      else if (entry.name.endsWith('.rs')) files.push([relative, shipped(read(relative))]);
    }
  };
  walk('src-tauri/src');
  return files;
}

test('the player command is one call of the plain function, on the blocking pool', () => {
  assert.equal(
    item(commands, 'pub async fn player_command(', 'player_command'),
    `pub async fn player_command(
    session_id: serde_json::Value,
    name: serde_json::Value,
    args: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let slot = state.current_child.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::player::control::command(&slot, &session_id, &name, &args)
    })
    .await
    .unwrap_or_else(|_| Err(crate::player::control::NOT_RUNNING.to_string()))
}
`,
  );
});

test("stop is decided and done in one hold of the player slot's lock, and the socket only after it", () => {
  // Review finding ec-5: whose play it is, and stopping it, in one hold of
  // the lock, decided by the player itself. A check made in one hold and
  // acted on in another let a late Close stop the play that replaced it. The
  // Rust tests in player/control.rs and player/mod.rs prove what each call
  // decides; this proves the function makes no other decision and holds the
  // lock exactly here.
  assert.equal(
    item(control, 'pub(crate) fn command(', 'control::command'),
    `pub(crate) fn command(
    slot: &Mutex<Option<PlayerHandle>>,
    session_id: &Value,
    name: &Value,
    args: &Value,
) -> Result<(), String> {
    let command = parse(name, args)?;
    let Some(session_id) = session_id.as_str() else {
        return Err(NOT_RUNNING.to_string());
    };
    // One hold of the lock decides whose play it is and acts on it: stops it,
    // or copies the way to its socket.
    let channel = {
        let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
        match (command, slot.as_mut()) {
            (PlayerCommand::Stop, Some(player)) => {
                player.close_embedded_play(session_id);
                return Ok(());
            }
            (PlayerCommand::Stop, None) => return Ok(()),
            (_, Some(player)) => player.command_channel(session_id),
            (_, None) => None,
        }
    };
    let Some(channel) = channel else {
        return Err(NOT_RUNNING.to_string());
    };
    channel.send(command).map_err(str::to_string)
}
`,
  );
  // The one decision, in the one place, for stopping and for sending.
  assert.match(
    playerMod,
    /pub\(crate\) fn close_embedded_play\(&mut self, session_id: &str\) -> bool \{\s+match self \{\s+PlayerHandle::External\(_\) => \{\s+let _ = session_id;\s+false\s+\}\s+#\[cfg\(target_os = "linux"\)\]\s+PlayerHandle::Embedded\(session\) => \{\s+if !session\.is_play\(session_id\) \{\s+return false;\s+\}\s+session\.stop\(\);\s+true\s+\}\s+\}\s+\}/,
  );
  assert.ok(
    playerMod.includes(`    pub(crate) fn command_channel(
        &self,
        session_id: &str,
    ) -> Option<std::sync::Arc<control::Channel>> {
        match self {
            PlayerHandle::External(_) => {
                let _ = session_id;
                None
            }
            #[cfg(target_os = "linux")]
            PlayerHandle::Embedded(session) => session
                .is_play(session_id)
                .then(|| session.channel()),
        }
    }
`),
    'an external player has no way in; an embedded one only for its own session',
  );
  // One place stops a play from the view, and it is the function above.
  const calls = shippedRust().filter(([, code]) => code.includes('close_embedded_play('));
  assert.deepEqual(
    calls.map(([file, code]) => [file, count(code, 'close_embedded_play(')]),
    [
      ['src-tauri/src/player/control.rs', 1],
      ['src-tauri/src/player/mod.rs', 1],
    ],
    'its definition and one call',
  );
  // A play's identity is what it was started with, and so is its socket.
  assert.match(session, /pub\(crate\) fn is_play\(&self, session_id: &str\) -> bool \{\s+self\.session_id == session_id\s+\}/);
  assert.ok(session.includes('let session_id = std::mem::take(&mut start.session_id);\n        let channel = Arc::new(Channel::new(start.ipc_path.clone()));'));
  assert.ok(session.includes('        let session = Session {\n            session_id,\n            channel,\n            shared,\n'));
  assert.ok(host.includes('            ipc_path: ipc_path.to_string(),'));
  assert.match(host, /let start = Start \{\s+lib,\s+guest,\s+session_id,/);
  assert.match(host, /let started = Session::start\(start, surface\);/);
});

test('player_close is gone, and the two new commands are registered', () => {
  for (const [name, source] of [['commands.rs', commands], ['lib.rs', lib]]) {
    assert.equal(count(source, 'player_close'), 0, `${name} names no player_close`);
  }
  assert.equal(count(lib, '            commands::player_command,\n'), 1);
  assert.equal(count(lib, '            commands::player_fullscreen,\n'), 1);
  // App commands need no capability; the window plugin's fullscreen stays refused.
  assert.equal(count(read('src-tauri/capabilities/default.json'), 'fullscreen'), 0);
});

test('fullscreen is asked of the window only for the running embedded play, after the lock', () => {
  assert.equal(
    item(commands, 'pub async fn player_fullscreen(', 'player_fullscreen'),
    `pub async fn player_fullscreen(
    session_id: serde_json::Value,
    on: serde_json::Value,
    app: tauri::AppHandle,
) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        let on = on.as_bool() == Some(true);
        let state = app.state::<AppState>();
        let claim = claim_player_fullscreen(&state, &session_id, on);
        if claim.ask_window {
            if let Some(window) = app.webview_windows().into_values().next() {
                let _ = window.set_fullscreen(on);
            }
        }
        claim.held
    })
    .await
    .unwrap_or(false)
}
`,
  );
  assert.equal(
    item(control, 'pub(crate) fn is_running_embedded_play(', 'is_running_embedded_play'),
    `pub(crate) fn is_running_embedded_play(slot: &mut Option<PlayerHandle>, session_id: &Value) -> bool {
    let Some(session_id) = session_id.as_str() else {
        return false;
    };
    slot.as_mut()
        .is_some_and(|player| player.is_running_embedded_play(session_id))
}
`,
  );
  // Asked, and the page's claim on fullscreen recorded, under the slot's
  // lock; off always goes through; a refused "on" leaves the claim as it
  // was and answers what is held (review finding pc-2).
  assert.equal(
    item(commands, 'fn claim_player_fullscreen(', 'claim_player_fullscreen'),
    `fn claim_player_fullscreen(
    state: &AppState,
    session_id: &serde_json::Value,
    on: bool,
) -> FullscreenClaim {
    if !on {
        state.player_fullscreen_held.store(false, Ordering::SeqCst);
        return FullscreenClaim {
            ask_window: true,
            held: false,
        };
    }
    let mut slot = state.current_child.lock().unwrap_or_else(|e| e.into_inner());
    if !crate::player::control::is_running_embedded_play(&mut slot, session_id) {
        return FullscreenClaim {
            ask_window: false,
            held: state.player_fullscreen_held.load(Ordering::SeqCst),
        };
    }
    state.player_fullscreen_held.store(true, Ordering::SeqCst);
    FullscreenClaim {
        ask_window: true,
        held: true,
    }
}
`,
  );
});

test("the window's real fullscreen reaches the page from GDK, followed once from setup", () => {
  assert.equal(
    item(host, 'pub(crate) fn follow_fullscreen(', 'follow_fullscreen'),
    `pub(crate) fn follow_fullscreen(app: &tauri::AppHandle) {
    let Some(window) = app.webview_windows().into_values().next() else {
        return;
    };
    let Ok(gtk_window) = window.gtk_window() else {
        return;
    };
    let app = app.clone();
    gtk_window.connect_window_state_event(move |_, event| {
        let fullscreen = gtk::gdk::WindowState::FULLSCREEN;
        if event.changed_mask().contains(fullscreen) {
            let on = event.new_window_state().contains(fullscreen);
            // The page gives up its claim on fullscreen when it hears the
            // window has left it; so does the backend's copy of that claim
            // (review finding pc-2).
            if !on {
                app.state::<crate::AppState>()
                    .player_fullscreen_held
                    .store(false, std::sync::atomic::Ordering::SeqCst);
            }
            let _ = app.emit("window-fullscreen", serde_json::json!({ "on": on }));
        }
        gtk::glib::Propagation::Proceed
    });
}
`,
  );
  assert.equal(count(lib, 'player::embedded::follow_fullscreen(app.handle());'), 1);
  const setup = lib.slice(lib.indexOf('.setup(move |app| {'), lib.indexOf('.invoke_handler('));
  assert.ok(
    setup.includes('            #[cfg(target_os = "linux")]\n            player::embedded::follow_fullscreen(app.handle());\n'),
    'connected in setup, on the window thread',
  );
  for (const [file, code] of shippedRust()) {
    const expected = file === 'src-tauri/src/player/embedded/host.rs' ? 1 : 0;
    assert.equal(count(code, 'connect_window_state_event('), expected, file);
  }
});

test("an embedded play's state goes to the page as player-state, and only an embedded play's", () => {
  assert.ok(report.includes('pub(crate) const EVENT: &str = "player-state";'));
  // The notifier is made where the app is, and handed to `play` and nothing else.
  assert.ok(
    commands.includes(`        let on_state = state.app_handle.get().cloned().map(|app| {
            std::sync::Arc::new(move |player_state: &crate::player::report::PlayerState| {
                let _ = tauri::Emitter::emit(&app, crate::player::report::EVENT, player_state);
            }) as crate::player::report::StateNotify
        });
`),
  );
  assert.equal(count(commands, 'on_state'), 2, 'made, and given to play');
  assert.ok(commands.includes('                    playback_session,\n                    on_end,\n                    on_state,\n                )'));
  // `play` gives it to the reporter of a play the embedded player STARTED,
  // after the end-of-file watcher. The Rust tests in playback.rs prove both
  // sides with stand-ins: an embedded play reports, one that fell back does not.
  assert.ok(
    playback.includes(`    #[cfg(target_os = "linux")]
    if started_player == crate::player::PlayerKind::Embedded {
        if let Some(notify) = on_state {
            if let Err(e) = crate::player::report::spawn_reporter(
                ipc_path.clone(),
                stop_flag.clone(),
                state_session,
                spec.duration,
                notify,
            ) {
`),
  );
  const play = shipped(playback);
  assert.equal(count(play, 'spawn_reporter('), 1);
  assert.equal(count(play, 'on_state'), 3, 'the parameter, the guard and the one other system');
  assert.ok(play.indexOf('spawn_eof_watcher(\n') < play.indexOf('spawn_reporter('));
});

test("a play is handed Vela's own crop script, found among the bundled scripts", () => {
  // The launch tests in Rust prove what a launch does with the script; only
  // this holds that the play is given it (there is no Tauri harness).
  assert.equal(count(commands, 'let crop_script = resolve_resource("vela-crop.lua");'), 1);
  assert.equal([...commands.matchAll(/\bcrop_script\b/g)].length, 2, 'found, and put into the play');
  assert.match(
    commands,
    /let spec = playback::PlaySpec \{[^}]*\n {12}crop_script,\n/,
  );
});

test('every Node test file is run by the check script', () => {
  // The check lists its files by name; one left out would stop running
  // without a word. (tests/player-controls.test.mjs holds the same.)
  const check = JSON.parse(read('package.json')).scripts.check;
  for (const file of fs.readdirSync(path.join(repoRoot, 'tests')).filter((name) => name.endsWith('.test.mjs'))) {
    assert.ok(check.split(/\s+/).includes(`tests/${file}`), `tests/${file} is in the check script`);
  }
});
