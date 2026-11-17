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
import { acceptedConnections } from './judge.mjs';
import { isWindows, processAlive, windowsAppPids, windowsToolAsync } from './windows.mjs';

// Generate 10s test clips into <configRoot>/media; returns the dir. With
// withTracks, make 60s clips with two named audio tracks and one subtitle
// (use a Matroska filename). These back the mock servers' stream routes.
export function makeClips(configRoot, clipNames, { withTracks = false } = {}) {
  const mediaDir = path.join(configRoot, 'media');
  fs.mkdirSync(mediaDir, { recursive: true });
  const seconds = withTracks ? 60 : 10;
  const subtitles = path.join(mediaDir, 'tracks.srt');
  if (withTracks) {
    fs.writeFileSync(subtitles, '1\n00:00:00,000 --> 00:01:00,000\nTrack selection test\n');
  }
  for (const name of clipNames) {
    const ff = spawnSync('ffmpeg', [
      '-f', 'lavfi', '-i', `testsrc=duration=${seconds}:size=320x180:rate=24`,
      '-f', 'lavfi', '-i', `sine=frequency=440:duration=${seconds}`,
      ...(withTracks ? [
        '-f', 'lavfi', '-i', `sine=frequency=880:duration=${seconds}`,
        '-i', subtitles,
        '-map', '0:v:0', '-map', '1:a:0', '-map', '2:a:0', '-map', '3:s:0',
        '-metadata:s:a:0', 'title=Main audio', '-metadata:s:a:0', 'language=eng',
        '-metadata:s:a:1', 'title=Commentary', '-metadata:s:a:1', 'language=deu',
        '-metadata:s:s:0', 'title=Test subtitles', '-metadata:s:s:0', 'language=eng',
        '-disposition:a:0', 'default', '-disposition:a:1', '0', '-disposition:s:0', '0',
        '-c:s', 'srt',
      ] : []),
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

// A Settings control shows the product default until its setting's load
// answers (Settings.svelte, createDraft: a choice made before then wins), so
// a value read the moment a tab opens can be that default on a slow machine
// (the first Windows runs). Wait for the control's confirmed load before
// comparing values, even if the expected value is the product default.
export async function settingValue(driver, selector, expected, what) {
  let shown;
  try {
    await pollUntil(async () => {
      shown = await driver.exec(`
        const control = document.querySelector(${JSON.stringify(selector)});
        return control && { value: control.value, busy: control.getAttribute('aria-busy') === 'true' };
      `);
      return shown && !shown.busy && shown.value === expected;
    }, what, { timeoutMs: 10000 });
  } catch {
    throw new Error(`${what}: ${selector} shows ${JSON.stringify(shown?.value)}, not ${JSON.stringify(expected)}${shown?.busy ? ' (still loading)' : ''}`);
  }
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

// Wait for the TOTAL live accepted clients, including the scenario's own.
// Finding the socket, reading mpv, or seeing a recorded Start does not prove
// Vela's readers connected: playback.rs retries their connects every 100 ms.
// Callers derive the count from playback::play for their particular launch.
// Do not include a short-lived notice/control/probe connection in a persistent
// count: it can disappear, or stand in for a reader that has not arrived yet.
// Both players expose this socket: build_launch_for sets input-ipc-server,
// and embedded::session checks that libmpv took it. There is no embedded
// bypass; its extra player::report::spawn_reporter must be counted at the call.
// This proves connection, not that an event has already been consumed.
// Inject the /proc reader and polling limits for socket-free unit tests.
export async function velaConnected(socketPath, connections, {
  readProcNetUnix = () => fs.readFileSync('/proc/net/unix', 'utf8'),
  timeoutMs = 15000,
  intervalMs = 20,
} = {}) {
  assert.ok(Number.isInteger(connections) && connections > 0, 'connections must be a positive integer');
  if (isWindows) {
    await windowsToolAsync('wait-clients', [socketPath, connections]);
    return;
  }
  await pollUntil(
    () => acceptedConnections(readProcNetUnix(), socketPath) >= connections,
    `mpv at ${socketPath} to accept ${connections} connections (Vela and the scenario)`,
    { timeoutMs, intervalMs },
  );
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
  if (isWindows) return !processAlive(pid);
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
  if (isWindows) return windowsAppPids(configRoot);
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

// Where stopStill puts a play, paused, before it is stopped (seconds), and
// how far either way from there a report of where it stopped may be (ms):
// plexStopProblems's (judge.mjs) 500 ms. No position the play was at before
// may lie inside that band, so a report of any earlier one fails. Downward
// the band is narrower in effect: multiplex waits, before the stop, for a
// "playing" report at the held position, and plexStopProblems lets no report
// say the play got further than the stopped one, so the stopped report may
// be no lower than Vela's own "playing" report of the hold. When that
// report is exact, the band is 0 ms below the hold and 500 ms above it.
export const STILL_AT_S = 9;
export const STILL_TOLERANCE_MS = 500;

// Holds a running play still and then stops it (`stop()`), so that where it
// stopped does not depend on when anything reads it, however long the stop
// takes to act (review finding pa-7, verification round 2: a play that kept
// running while a stop took 900 ms to act was reported correctly and judged
// against a position read before it, and failed). In order:
// - pauses the play and waits, on mpv's own state, until mpv says it is
//   paused, not seeking, at one position, on `steadyReads` reads in a row;
// - moves it, still paused, to `atS`, a position it has not been at: further
//   on than any it reached (`furthestMs`, the furthest the caller knows it
//   got before, and where the pause held it), by more than `toleranceMs`;
// - waits the same way until mpv says it is paused, not seeking, and at
//   `atS` (within 50 ms), on `steadyReads` reads in a row;
// - hands that position (seconds) to `positionAtStop` and awaits it: the
//   caller waits there until Vela has shown that it saw the play there;
// - checks the play is still paused there, and stops it.
// A paused play does not move, so however late the stop acts, mpv holds the
// play where `positionAtStop` was told until the stop acts. That is mpv's
// position, not what Vela is then told: `multiplex` judges that. With this
// hold it found the embedded player reporting about 180 ms short of it (a
// stop freed the renderer before mpv was told to end the file, and mpv then
// reported the audio's position as the play's last); fixed 2026-11-16.
// Returns that position.
export async function stopStill(
  mpv,
  stop,
  { positionAtStop, furthestMs, atS = STILL_AT_S, toleranceMs = STILL_TOLERANCE_MS, steadyReads = 3, everyMs = 50, timeoutMs = 5000 },
) {
  const read = async () => ({
    pause: await mpv.getProp('pause'),
    seeking: await mpv.getProp('seeking'),
    t: await mpv.getProp('time-pos'),
  });
  // Resolves with the position once `steadyReads` reads in a row find the
  // play paused, not seeking, at one position that `fits`.
  const steady = async (fits, what) => {
    let run = [];
    let last = null;
    try {
      return await pollUntil(
        async () => {
          last = await read();
          const still = last.pause === true && last.seeking === false && typeof last.t === 'number' && fits(last.t);
          run = still && (run.length === 0 || run[0] === last.t) ? [...run, last.t] : still ? [last.t] : [];
          return run.length >= steadyReads ? last.t : null;
        },
        what,
        { timeoutMs, intervalMs: everyMs },
      );
    } catch (err) {
      err.message += ` (mpv last said ${JSON.stringify(last)})`;
      throw err;
    }
  };
  await mpv.setProp('pause', true);
  const pausedAt = await steady(() => true, 'mpv to hold the play paused');
  const atMs = Math.round(atS * 1000);
  const furthest = Math.max(furthestMs, Math.round(pausedAt * 1000));
  assert.ok(
    furthest < atMs - toleranceMs,
    `the play is held at ${atMs} ms, which it has not been near: it got no further than ${furthest} ms before, ` +
      `more than ${toleranceMs} ms short of it`,
  );
  await mpv.setProp('time-pos', atS);
  const heldAt = await steady((t) => Math.abs(Math.round(t * 1000) - atMs) <= 50, `mpv to hold the play paused at ${atS} s`);
  // A seek may land on a nearby frame. The exclusion must hold against
  // the position actually handed to the caller, not just the requested one.
  assert.ok(
    furthest < Math.round(heldAt * 1000) - toleranceMs,
    `the held position (${Math.round(heldAt * 1000)} ms) is more than ${toleranceMs} ms beyond the earlier ${furthest} ms`,
  );
  await positionAtStop(heldAt);
  const now = await read();
  assert.deepEqual(
    now,
    { pause: true, seeking: false, t: heldAt },
    `the play is still held at ${heldAt} s, paused, as it is stopped`,
  );
  await stop();
  return heldAt;
}

// Run `start()` (whatever triggers playback), then drive the resulting mpv
// session: seek to 6s, let Vela observe it, stop it as its user would
// (stopPlayAsUser; `key` picks the embedded view's key over its Close
// control), and prove the stop acted. Returns the first time-pos sample
// observed after load (the resume-position evidence).
// `during(mpv)`, if given, runs once the play has progressed, before the seek.
// `positionAtStop(seconds)`, if given, asks for the position at the stop: the
// play is then held still, paused at STILL_AT_S, before it is stopped
// (stopStill), and `positionAtStop` is handed that position and awaited
// before the stop. The play then stops there however long the stop takes (in
// the embedded player the view's controls can take seconds to come up). Such
// a play is stopped while paused, never while it moves.
export async function playAndQuit(driver, start, { key = null, during = null, positionAtStop = null } = {}) {
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
    // playback::play: this client + position/window reader + EOF watcher;
    // these Original plays have no notice or health sampler. An embedded
    // play also has player::report::spawn_reporter for its view's state.
    await velaConnected(socketPath, runPlayer === 'embedded' ? 4 : 3);
    if (during) await during(mpv);
    // The furthest the play is known to have got before the seek.
    const beforeSeekMs = Math.round((await mpv.getProp('time-pos')) * 1000);
    await mpv.setProp('time-pos', 6);
    await new Promise((r) => setTimeout(r, 1500)); // let Vela observe ≥6s
    const stop = () => stopPlayAsUser(driver, mpv, socketPath, 'the play', { key });
    if (positionAtStop) await stopStill(mpv, stop, { positionAtStop, furthestMs: beforeSeekMs });
    else await stop();
  } finally {
    mpv.close();
  }
  return firstPos;
}
