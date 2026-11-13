// The page's side of a play that runs inside Vela's own window
// (.agents/plans/embedded-player.md, "Frontend: the player view", and plan
// review finding ep-2).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { nextPlayerSession } from '../src/lib/playerView.ts';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file) => fs.readFileSync(path.join(repoRoot, file), 'utf8').replace(/\r\n/g, '\n');
const page = read('src/routes/+page.svelte');
const view = read('src/lib/PlayerView.svelte');
const css = read('src/app.css');
const commands = read('src-tauri/src/commands.rs');
const playerMod = read('src-tauri/src/player/mod.rs');
const session = read('src-tauri/src/player/embedded/session.rs');
const host = read('src-tauri/src/player/embedded/host.rs');

const opened = (sessionId) => ({ kind: 'opened', sessionId });
const closed = (sessionId) => ({ kind: 'closed', sessionId });
const after = (events) => events.reduce(nextPlayerSession, null);

test("a play's view opens with it and closes with it", () => {
  assert.equal(after([]), null);
  assert.equal(after([opened('a')]), 'a');
  assert.equal(after([opened('a'), closed('a')]), null);
});

test("a replaced play's late close does not dismiss the view of the play that replaced it", () => {
  // Auto-advance, a replacement play, an Automatic step-down: the new play
  // opens first, and only then does the old one finish shutting down.
  assert.equal(after([opened('a'), opened('b'), closed('a')]), 'b', "b's view stays open");
  assert.equal(after([opened('a'), opened('b'), closed('a'), closed('b')]), null);
  // The old close may also arrive twice, or for a play whose view never showed.
  assert.equal(after([opened('a'), opened('b'), closed('a'), closed('a')]), 'b');
  assert.equal(after([opened('b'), closed('never-shown')]), 'b');
  assert.equal(after([closed('a')]), null);
});

test('the page shows the player view for exactly the session the events name', () => {
  // Both events go through the rule above, and nothing else decides which
  // session's view is showing.
  assert.match(
    page,
    /listen<\{ sessionId: string \}>\("player-opened", \(\{ payload \}\) => \{\s+playerSession = nextPlayerSession\(playerSession, \{\s+kind: "opened",\s+sessionId: payload\.sessionId,\s+\}\);/,
  );
  assert.match(
    page,
    /listen<\{ sessionId: string \}>\("player-closed", \(\{ payload \}\) => \{\s+playerSession = nextPlayerSession\(playerSession, \{\s+kind: "closed",\s+sessionId: payload\.sessionId,\s+\}\);/,
  );
  assert.match(page, /let playerSession = \$state<string \| null>\(null\);/);
  assert.equal(
    [...page.matchAll(/(?<!let )playerSession = /g)].length,
    2,
    'after its declaration, the showing session is assigned only by those two listeners',
  );
  // `playback-ended` names no session and fires twice after a clean end; it
  // keeps its one job, the watch-state refresh.
  assert.match(page, /listen\("playback-ended", refreshWatchState\)/);
  // The view is keyed by session, so a successor gets a fresh one.
  assert.match(
    page,
    /\{#if playerSession\}\s+\{#key playerSession\}\s+<PlayerView sessionId=\{playerSession\} \/>\s+\{\/key\}\s+\{\/if\}/,
  );
});

test('the player view tells the backend where it is and closes only its own play', () => {
  assert.match(
    view,
    /function reportRect\(\) \{\s+const r = root\.getBoundingClientRect\(\);\s+invoke\("player_set_rect", \{ x: r\.left, y: r\.top, width: r\.width, height: r\.height \}\)/,
  );
  // On mount, and whenever its size changes.
  assert.match(view, /onMount\(\(\) => \{[\s\S]{0,200}reportRect\(\);\s+const observer = new ResizeObserver\(reportRect\);\s+observer\.observe\(root\);/);
  assert.match(view, /invoke\("player_close", \{ sessionId \}\)/);
  assert.equal(view.split('invoke(').length - 1, 2, 'the view calls those two commands and no others');
});

test("Close asks the player itself whose play it is, under the player's own lock", () => {
  // Whether a Close stops a play is decided by `close_embedded_play`, which
  // the Rust tests in player/mod.rs prove: only the embedded play of that
  // session, never an external player. This proves the command does nothing
  // but that, so there is no check made in one place and acted on in another
  // (review finding ec-5: the active session was read, its lock released,
  // and then whatever player was current was stopped).
  const start = commands.indexOf('\npub async fn player_close(');
  assert.notEqual(start, -1);
  const body = commands.slice(start, commands.indexOf('\n}\n', start) + 3);
  assert.equal(
    body,
    `
pub async fn player_close(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut slot = state
        .current_child
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(player) = slot.as_mut() {
        player.close_embedded_play(&session_id);
    }
    Ok(())
}
`,
  );
  // The one decision, in the one place.
  assert.match(
    playerMod,
    /pub\(crate\) fn close_embedded_play\(&mut self, session_id: &str\) -> bool \{\s+match self \{\s+PlayerHandle::External\(_\) => \{\s+let _ = session_id;\s+false\s+\}\s+#\[cfg\(target_os = "linux"\)\]\s+PlayerHandle::Embedded\(session\) => \{\s+if !session\.is_play\(session_id\) \{\s+return false;\s+\}\s+session\.stop\(\);\s+true\s+\}\s+\}\s+\}/,
  );
  assert.equal(commands.split('close_embedded_play(').length - 1, 1);
  // A play's identity is what it was started with.
  assert.match(session, /pub\(crate\) fn is_play\(&self, session_id: &str\) -> bool \{\s+self\.session_id == session_id\s+\}/);
  assert.match(session, /let session_id = std::mem::take\(&mut start\.session_id\);/);
  assert.match(host, /let started = Session::start\(Start \{\s+lib,\s+guest,\s+session_id,/);
});

test('the page gets out of the way of the picture only while the view is mounted', () => {
  // The picture is on a surface under the window: it shows where the page
  // paints nothing.
  assert.match(
    view,
    /onMount\(\(\) => \{\s+document\.documentElement\.classList\.add\("player-open"\);[\s\S]{0,400}return \(\) => \{\s+observer\.disconnect\(\);\s+document\.documentElement\.classList\.remove\("player-open"\);/,
  );
  assert.match(
    css,
    /html\.player-open,\nhtml\.player-open body \{\n  background: transparent !important;\n\}\nhtml\.player-open \.app \{\n  visibility: hidden;\n\}/,
  );
  assert.equal(css.split('player-open').length - 1, 3, 'and that is all the class does');
  assert.match(view, /\.player \{[^}]*background: transparent;/);
});
