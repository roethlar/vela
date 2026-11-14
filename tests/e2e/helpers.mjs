// Shared scenario plumbing for the mock-server scenarios. The playback
// scenario keeps its own detailed inline flow — it IS the mpv-IPC probe
// test; these leaner helpers serve scenarios that use playback as a means
// (curation, resume, explicit play verbs, search).
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from './mpv.mjs';
import { testOnlyRoute } from './proof.mjs';

// Generate 10s test clips into <configRoot>/media; returns the dir. The
// clips back the mock servers' Range-capable /Videos/{id}/stream routes.
export function makeClips(configRoot, clipNames) {
  const mediaDir = path.join(configRoot, 'media');
  fs.mkdirSync(mediaDir, { recursive: true });
  for (const name of clipNames) {
    const ff = spawnSync('ffmpeg', [
      '-f', 'lavfi', '-i', 'testsrc=duration=10:size=320x180:rate=24',
      '-f', 'lavfi', '-i', 'sine=frequency=440:duration=10',
      '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest',
      path.join(mediaDir, name),
    ], { stdio: 'ignore' });
    if (ff.status !== 0) throw new Error('ffmpeg is required to generate the test clips');
  }
  return mediaDir;
}

// Config `sources` entry for a started mock server.
export function mockSource(
  mock,
  { id = 'jf-mock', name = 'Mock JF', kind = 'jellyfin' } = {},
) {
  return {
    id,
    kind,
    name,
    base_url: mock.baseUrl ?? `http://127.0.0.1:${mock.port}`,
    access_token: 'mock-token',
    user_id: mock.userId,
    device_id: 'e2e-device',
  };
}

// For the external player: no window, no sound. The embedded player keeps
// drawing whatever this says (it asserts its own output after the user's
// options) and takes the silence. One option per LINE — that is how Vela
// parses mpv_extra_args; a scenario that adds lines of its own starts from
// these.
export const DISPLAYLESS_MPV_ARGS = '--vo=null\n--ao=null';

// Seed the throwaway config: mock server sources + displayless mpv args.
export function seedConfig(configRoot, sources, extra = {}) {
  const configDir = path.join(configRoot, 'config', 'vela');
  fs.mkdirSync(configDir, { recursive: true });
  // A run of the suite can put every scenario in the embedded player
  // (`npm run e2e -- --embedded`, which sets this variable; see
  // tests/e2e/README.md). A scenario that names a player itself keeps its own.
  const player = process.env.VELA_E2E_PLAYER ? { player: process.env.VELA_E2E_PLAYER } : {};
  fs.writeFileSync(
    path.join(configDir, 'config.json'),
    JSON.stringify({
      sources,
      mpv_extra_args: DISPLAYLESS_MPV_ARGS,
      ...player,
      ...extra,
    }),
  );
}

// Seed Vela's independent versioned playlist store. The helper returns the
// file path so scenarios can assert byte-for-byte read-only playback.
export function seedPlaylists(configRoot, playlists) {
  const configDir = path.join(configRoot, 'config', 'vela');
  fs.mkdirSync(configDir, { recursive: true });
  const file = path.join(configDir, 'playlists.json');
  fs.writeFileSync(
    file,
    JSON.stringify({ schemaVersion: 1, playlists }, null, 2),
    { mode: 0o600 },
  );
  return file;
}

export async function pollUntil(fn, what, { timeoutMs = 15000, intervalMs = 250 } = {}) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await fn();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, intervalMs));
  }
}

// Sidebar → the mock library's grid.
export async function openLibraryGrid(driver, { section = 'Mock Library', cardPrefix = 'Mock Movie' } = {}) {
  await driver.waitFor(
    `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === '${section}')`,
    'seeded library in the sidebar',
  );
  const btn = await driver.find(
    'xpath',
    `//button[contains(@class,'sideitem') and normalize-space(.)='${section}']`,
  );
  await driver.click(btn);
  await driver.waitFor(
    `return !!document.querySelector('button.poster[aria-label^="${cardPrefix}"]')`,
    'movie card in the grid',
  );
}

// Every request matching `endsWith` has had its response SENT — nothing armed is still
// parked in the mock. Waiting for a request to ARRIVE proves only that the client asked;
// it says nothing about when the client was handed the answer and ran the code under
// test. An assertion that something bad did NOT happen passes just as well by asking too
// early, so it needs this (codex + grok, r21).
export const allDelivered = (mock, endsWith) => {
  const asked = mock.state.requests.filter((r) => r.path.endsWith(endsWith)).length;
  const answered = mock.state.served.filter((s) => s.path.endsWith(endsWith)).length;
  return asked > 0 && answered >= asked;
};

// A successful Jellyfin play asks PlaybackInfo twice for the same item: once
// to enumerate candidates and once to revalidate the exact selected source.
// Keep scenario assertions expressed in logical plays, including repeated
// plays of the same item (A,A,A,A -> A,A).
export function logicalPlaybackInfoIds(mock) {
  const raw = mock.state.requests
    .filter((request) => /\/Items\/[^/]+\/PlaybackInfo$/.test(request.path))
    .map((request) => request.path.match(/^\/Items\/([^/]+)\/PlaybackInfo$/)?.[1]);
  const logical = [];
  for (let index = 0; index < raw.length;) {
    logical.push(raw[index]);
    index += raw[index + 1] === raw[index] ? 2 : 1;
  }
  return logical;
}

// Hold a condition open for a window, failing the MOMENT it breaks — rather than
// sleeping and sampling once at the end, which cannot tell "it never happened" from
// "it has not happened yet".
export async function holdsFor(check, ms, what) {
  const deadline = Date.now() + ms;
  for (;;) {
    const broke = await check();
    if (broke) throw new Error(`${what} — broke during the hold: ${broke}`);
    if (Date.now() >= deadline) return;
    await new Promise((r) => setTimeout(r, 200));
  }
}

// The player view's controls hide while a play runs untouched; a pointer
// move over the view shows them for a few seconds. Returns once the bar is
// visible, so a click that follows at once reaches it.
export async function showPlayerBar(driver) {
  await driver.movePointer(400, 300);
  await driver.waitFor(
    `const bar = document.querySelector('.player .bar');
     return !!bar && !bar.classList.contains('hidden') && getComputedStyle(bar).visibility === 'visible';`,
    'the player controls to show',
    { timeoutMs: 3000, intervalMs: 50 },
  );
}

// The embedded player's mpv draws at the size of the page, in pixels: waits
// for that, and returns the page's height. The page's own size is the window
// the compositor really made (fullscreen or not), not what Tauri says it
// asked for. Embedded player only (it reads mpv's `osd-dimensions`).
export async function drawnAtPageSize(driver, mpv, what) {
  let last = null;
  let page = null;
  try {
    await pollUntil(
      async () => {
        page = await driver.exec(`return [window.innerWidth, window.innerHeight, window.devicePixelRatio]`);
        const [width, height, ratio] = page;
        last = await mpv.getProp('osd-dimensions').catch(() => null);
        return last && last.w === Math.round(width * ratio) && last.h === Math.round(height * ratio);
      },
      `${what}: mpv to draw at the page's size`,
      { timeoutMs: 5000 },
    );
  } catch (err) {
    err.message += ` (page ${JSON.stringify(page)}, mpv ${last ? `${last.w}x${last.h}` : 'none'})`;
    throw err;
  }
  console.log(`${what}: page ${page[0]}x${page[1]}, mpv ${last.w}x${last.h}`);
  return page[1];
}

// Starts recording, in the page, every `player-state` and
// `window-fullscreen` event, the same events the player view listens to:
// `window.__playerEvents.states` and `.fullscreen`.
export async function recordPlayerEvents(driver) {
  await driver.exec(`
    window.__playerEvents = { states: [], fullscreen: [] };
    const tauri = window.__TAURI_INTERNALS__;
    const state = tauri.transformCallback((e) => window.__playerEvents.states.push(e.payload));
    const fullscreen = tauri.transformCallback((e) => window.__playerEvents.fullscreen.push(e.payload));
    return Promise.all([
      tauri.invoke('plugin:event|listen', { event: 'player-state', target: { kind: 'Any' }, handler: state }),
      tauri.invoke('plugin:event|listen', { event: 'window-fullscreen', target: { kind: 'Any' }, handler: fullscreen }),
    ]).then(() => true);
  `);
}

export async function goHome(driver) {
  await driver.click(await homeButton(driver));
}

export function homeButton(driver) {
  return driver.find('xpath', `//button[contains(@class,'sideitem') and normalize-space(.)='Home']`);
}

// The player this run plays in: the external one unless the run was started
// with `--embedded` (tests/e2e/README.md, "The embedded player"). A scenario
// branches on it only where what a user does or sees legitimately differs
// between the two players, and says why where it does.
export const runPlayer = process.env.VELA_E2E_PLAYER || 'external';

// The embedded player's view is up and covers the page.
export const PLAYER_VIEW_UP = `return document.documentElement.classList.contains('player-open') && !!document.querySelector('.player[aria-label="Video"]')`;

// Whether anything still accepts connections on a play's IPC socket. mpv
// leaves its socket file behind when it quits, so only a connection attempt
// tells a running play from one that has ended.
export function socketAccepting(socketPath) {
  return new Promise((resolve) => {
    const probe = net.createConnection(socketPath);
    probe.once('connect', () => {
      probe.destroy();
      resolve(true);
    });
    probe.once('error', () => resolve(false));
  });
}

// Whether process `pid` has ended: it is gone, or it is a zombie waiting to be
// reaped (it runs nothing any more).
export function processEnded(pid) {
  try {
    const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
    // The state follows the command name, which is in parentheses.
    return stat.slice(stat.lastIndexOf(')') + 2, stat.lastIndexOf(')') + 3) === 'Z';
  } catch {
    return true;
  }
}

// The pids of THIS scenario's app: Vela processes whose environment carries
// the scenario's own XDG_CONFIG_HOME (a Vela the user runs, or one of another
// scenario, never counts).
export function scenarioAppPids(configRoot) {
  const mine = `XDG_CONFIG_HOME=${path.join(configRoot, 'config')}\0`;
  return fs
    .readdirSync('/proc')
    .filter((entry) => /^\d+$/.test(entry))
    .filter((pid) => {
      try {
        return (
          fs.readFileSync(`/proc/${pid}/comm`, 'utf8').trim() === 'vela' &&
          fs.readFileSync(`/proc/${pid}/environ`, 'utf8').includes(mine) &&
          !processEnded(pid)
        );
      } catch {
        return false; // gone mid-scan, or not ours to read
      }
    })
    .map(Number);
}

// Waits until the page is the user's again: no player view covers it. In the
// external player that is always so (mpv draws in a window of its own); the
// embedded player's view leaves with its play's `player-closed`, a moment
// after the play has stopped.
export async function waitForPageUncovered(driver, what) {
  await driver.waitFor(
    `return !document.documentElement.classList.contains('player-open') && !document.querySelector('.player')`,
    `the page to be uncovered ${what}`,
  );
}

// Stops the running play the way its user stops it, and returns once the
// player has stopped answering and the page is the user's again.
//
// The one branch is the stop itself, because that is where the two users'
// gestures differ: the external player is quit in its own window (mpv's
// `quit`, here sent over its IPC socket, the command mpv's `q` runs); the
// embedded player has no window of its own and takes no key itself, so its
// user stops it with the player view's Close control or one of the view's
// keys (`key`: 'q', or Driver.KEY.Escape, which closes when the window is not
// fullscreen). Everything after the stop is the same wait in both players.
export async function stopPlayAsUser(driver, mpv, socketPath, what, { key = null } = {}) {
  if (runPlayer === 'embedded') {
    await driver.waitFor(PLAYER_VIEW_UP, `the player view to be up before stopping ${what}`);
    if (key) {
      // A key goes to whatever has focus. The view takes it when it opens
      // (PlayerView.svelte), and that is where the user's key lands; were
      // focus elsewhere (a text field), `q` would be typed, not obeyed, so
      // say so instead of pressing it (venue finding atk-6).
      const focused = await driver.exec(
        `const a = document.activeElement; return a && a.closest('.player[aria-label="Video"]') ? null : (a ? a.outerHTML.slice(0, 160) : 'nothing');`,
      );
      assert.equal(focused, null, `the player view must have focus before its key stops ${what}; focus is on ${focused}`);
      await driver.pressKeys(key);
    } else {
      // The controls hide while a play runs untouched.
      await showPlayerBar(driver);
      await driver.click(await driver.find('css selector', '.player button.close'));
    }
  } else {
    mpv.quit();
  }
  mpv.close();
  // Prove the stop acted (eh-7): the socket must stop accepting well before
  // a 10 s clip could reach its end.
  await pollUntil(async () => !(await socketAccepting(socketPath)), `${what} to stop`, {
    timeoutMs: 4000,
  });
  await waitForPageUncovered(driver, `after ${what} stopped`);
  // The Close route moved the pointer onto the view (showPlayerBar). With the
  // view gone it rests over whatever the page shows there, and with the seat
  // weston gives (--fake-seat) that element is really hovered (a card lifts by
  // 4 px) for every later step. A user's pointer is wherever they left it;
  // here it is put where it hovers nothing, so no later step depends on what
  // the page happens to show at (400, 300) (venue finding atk-6).
  if (runPlayer === 'embedded') await restPointerOnNothing(driver);
}

// Moves the pointer to a point of the page over nothing that reacts to it:
// no control, link, card or field, on the page's background.
export async function restPointerOnNothing(driver) {
  const point = await driver.exec(`
    const reacts = (el) => !el || !!el.closest('button, a, input, select, textarea, label, [role=button], [role=link], [role=menuitem], [role=slider], .poster, .flowcard');
    const w = window.innerWidth, h = window.innerHeight;
    for (const [x, y] of [[w - 6, h - 6], [w - 6, 6], [6, h - 6], [Math.floor(w / 2), h - 6], [6, 6]]) {
      if (!reacts(document.elementFromPoint(x, y)) && !reacts(document.elementFromPoint(x + 2, y + 2))) return [x, y];
    }
    return null;`);
  if (!point) throw new Error('no point of the page where the pointer hovers nothing');
  await driver.movePointer(point[0], point[1]);
  const hovered = await driver.exec(`
    const deepest = [...document.querySelectorAll(':hover')].pop();
    const r = deepest?.closest('button, a, input, select, textarea, label, [role=button], [role=link], [role=menuitem], [role=slider], .poster, .flowcard');
    return r ? r.outerHTML.slice(0, 160) : null;`);
  assert.equal(hovered, null, `the pointer, put to rest at ${point}, still hovers ${hovered}`);
}

// Clicks a control of the page WHILE a play runs, which only the external
// player's user can do: mpv's window is beside the page. In the embedded
// player the view covers the page, so no user can reach the control; a
// scenario that needs the page put in a given state during a play (to prove
// what the BACKEND then does to it) does it there through the DOM, after
// checking that the view really is up. That is a TEST-ONLY route: it stands
// for nothing an embedded user does, and what it proves in that player is the
// backend's and the page's behaviour underneath the view, never that a user
// could take the step.
// The runner fails a scenario that takes it without declaring it (`testOnly`,
// via 'clickUnderPlay', for the embedded player).
export async function clickUnderPlay(driver, elementId, what) {
  if (runPlayer === 'embedded') {
    testOnlyRoute('clickUnderPlay');
    await driver.waitFor(PLAYER_VIEW_UP, `the player view to cover the page before ${what}`);
    await driver.exec('arguments[0].click(); return true', [
      { 'element-6066-11e4-a52e-4f735466cecf': elementId },
    ]);
  } else {
    await driver.click(elementId);
  }
}

// Starts Vela's exit from the page, through the command behind the recovery
// screens' "Exit Vela" button (`exit_vela`, app.exit). A TEST-ONLY route in
// either player: no venue here can press a window's close button, and
// WebDriver's close window only destroys the web view. The page may be gone
// before the answer comes, so nothing is awaited from Vela. Declared as
// `testOnly` via 'exitVelaFromPage'.
export async function exitVelaFromPage(driver) {
  testOnlyRoute('exitVelaFromPage');
  await driver.exec(`window.__TAURI_INTERNALS__.invoke('exit_vela'); return true;`).catch(() => {});
}

// Sends input into mpv's own input layer over its IPC socket (mpv's `mouse`
// and `keypress` commands), as if in mpv's window. A TEST-ONLY route: no
// venue here can click or type into the external player's window. Declared
// as `testOnly` via 'mpvInput'.
export function mpvInput(mpv, ...command) {
  if (!['mouse', 'keypress'].includes(command[0])) throw new Error(`mpvInput: not an input command: ${command[0]}`);
  testOnlyRoute('mpvInput');
  return mpv.cmd(...command);
}

// The nav flip (74ff385) routes library card clicks to the info page;
// playback goes through the detail page's Play/Resume button.
export async function openDetailAndPlay(driver, cardSelector) {
  const card = await driver.find('css selector', cardSelector);
  await driver.click(card);
  await driver.waitFor(
    `return !!document.querySelector('.detail button.playwide')`,
    'detail page Play button',
  );
  const play = await driver.find('css selector', '.detail button.playwide');
  await driver.click(play);
}

// Run `start()` (whatever triggers playback), then drive the resulting mpv
// session: seek to 6s, let Vela observe it, stop it as its user would
// (stopPlayAsUser; `key` picks the embedded view's key over its Close
// control), and prove the stop acted. Returns the first time-pos sample
// observed after load (the resume-position evidence).
// `during(mpv)`, if given, runs once the play has progressed, before the seek.
// `beforeStop(mpv)`, if given, runs after the seek has been observed, just
// before the stop: the last moment the play's position can be read.
export async function playAndQuit(driver, start, { key = null, during = null, beforeStop = null } = {}) {
  const before = mpvSocketSnapshot();
  await start();
  const socketPath = await waitForNewMpvSocket(before).catch(async (err) => {
    // The play error banner (play() sets `error`) is the usual culprit —
    // surface the page state instead of a bare socket timeout.
    const text = await driver.exec(`return document.body.innerText.slice(0, 400)`).catch(() => '?');
    err.message += ` — page: ${JSON.stringify(text)}`;
    throw err;
  });
  const mpv = await MpvIpc.connect(socketPath);
  let firstPos;
  try {
    firstPos = await pollUntil(
      () => mpv.getProp('time-pos').catch(() => null).then((t) => (t == null ? null : t)),
      'first time-pos sample',
    );
    await pollUntil(
      () => mpv.getProp('time-pos').then((t) => t > 0.5).catch(() => false),
      'playback to progress',
    );
    if (during) await during(mpv);
    await mpv.setProp('time-pos', 6);
    await new Promise((r) => setTimeout(r, 1500)); // let Vela observe ≥6s
    if (beforeStop) await beforeStop(mpv);
    await stopPlayAsUser(driver, mpv, socketPath, 'the play', { key });
  } finally {
    mpv.close();
  }
  return firstPos;
}
