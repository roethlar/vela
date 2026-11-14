// How a run of plays leaves the fullscreen it asked for, end to end in the
// embedded player (.agents/plans/embedded-player.md, Slice 5; review finding
// pc-2).
//
// Runs only in a run of the embedded player (`npm run e2e -- --embedded`).
// Fullscreen is asked for through the view's own control, so it is the page's
// (player-made) fullscreen; what the window really does is read from the
// `window-fullscreen` events. Every lifecycle event the page receives is
// recorded beside it, with the time it arrived, and printed per leg: that is
// the order the page sees.
//
// Covered, each leg on its own:
//   1. a playlist's first play ends cleanly and the backend starts the next:
//      fullscreen is kept across the gap;
//   2. that next play ends in an error (its server refuses the stream): the
//      window leaves fullscreen at once, long before the page's 50 s backstop;
//   3. mpv is told to quit over its socket, by something other than the page:
//      the window leaves fullscreen at once;
//   4. a clean end with no successor (the end of the run): leaves at once;
//   5. Close from the view: leaves at once;
//   6. a playlist's first play ends cleanly and the backend cannot start the
//      next (its server resolves no stream): the run is over, leaves at once.
import assert from 'node:assert/strict';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  makeClips,
  mockSource,
  openDetailAndPlay,
  openLibraryGrid,
  pollUntil,
  recordPlayerEvents,
  seedConfig,
  seedPlaylists,
  showPlayerBar,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

// The window must be out of fullscreen this soon after the play's view has
// gone. The page's backstop is 50 s; the wait for it is cut off at
// `GIVE_UP_MS`, far below that, so a run without a prompt leave fails fast.
const LEAVE_WITHIN_MS = 2500;
const GIVE_UP_MS = 10_000;
let mock;

function item(id, title) {
  return {
    ratingKey: `jf-mock:${id}`,
    title,
    year: null,
    summary: null,
    durationMs: 10_000,
    mediaType: 'movie',
    poster: null,
    seriesPoster: null,
    backdrop: null,
    viewOffsetMs: null,
    played: false,
    lastWatchedAtMs: null,
    addedAtMs: null,
    index: null,
    parentIndex: null,
    grandparentTitle: null,
    parentTitle: null,
    parentRatingKey: null,
    grandparentRatingKey: null,
    sourceId: 'jf-mock',
    providerIds: [],
    backing: null,
    canonicalId: null,
    watchKey: null,
    detailKey: null,
  };
}

// Every event the page's fullscreen decision hangs on, in arrival order. Ids
// and flags only: nothing else of a payload is kept.
async function recordEnds(driver) {
  await driver.exec(`
    window.__ends = [];
    const tauri = window.__TAURI_INTERNALS__;
    const names = ['playback-ended', 'player-opened', 'player-closed', 'continue-playing', 'window-fullscreen'];
    return Promise.all(names.map((name) => tauri.invoke('plugin:event|listen', {
      event: name,
      target: { kind: 'Any' },
      handler: tauri.transformCallback((e) => window.__ends.push({
        name,
        at: Date.now(),
        sessionId: typeof e.payload?.sessionId === 'string' ? e.payload.sessionId : null,
        runOver: typeof e.payload?.runOver === 'boolean' ? e.payload.runOver : null,
        on: typeof e.payload?.on === 'boolean' ? e.payload.on : null,
      })),
    }))).then(() => true);
  `);
}

const ends = (driver) => driver.exec(`return window.__ends`);
const mark = async (driver) => (await ends(driver)).length;

// The events of one leg, printed in the order the page received them.
function printLeg(leg, events, sessions) {
  const short = (id) => (id === null ? '-' : sessions.get(id) ?? 'other');
  const start = events[0]?.at ?? 0;
  console.log(`${leg}:`);
  for (const e of events) {
    const detail =
      e.name === 'window-fullscreen'
        ? `on=${e.on}`
        : `session=${short(e.sessionId)}${e.runOver === null ? '' : ` runOver=${e.runOver}`}`;
    console.log(`  +${String(e.at - start).padStart(5)} ms  ${e.name} ${detail}`);
  }
}

const click = async (driver, selector) => driver.click(await driver.find('css selector', selector));

const stopReports = () =>
  mock.state.served.filter((r) => r.method === 'POST' && r.path === '/Sessions/Playing/Stopped').length;

// The play is over for Vela after its last check-in and the page's reload.
async function settled(stopReportsBefore) {
  await pollUntil(() => stopReports() > stopReportsBefore, 'the stop report of the play that ended');
  let asked = mock.state.requests.length;
  let quietSince = Date.now();
  await pollUntil(
    () => {
      if (mock.state.requests.length !== asked) {
        asked = mock.state.requests.length;
        quietSince = Date.now();
        return false;
      }
      return Date.now() - quietSince >= 700;
    },
    'the page to finish reloading after the play',
    { intervalMs: 100 },
  );
}

// The next view to open after event `from`: its session.
async function nextOpened(driver, from, what) {
  return pollUntil(
    async () => (await ends(driver)).slice(from).find((e) => e.name === 'player-opened')?.sessionId ?? null,
    what,
    { timeoutMs: 20_000 },
  );
}

async function viewReady(driver) {
  await driver.waitFor(
    `const b = document.querySelector('.player button.playpause'); return !!b && !b.disabled`,
    'the player view to have its first state',
  );
}

// Player-made fullscreen, through the view's control. Returns the page's
// height before it.
async function fullscreenOn(driver) {
  const windowed = await driver.exec(`return window.innerHeight`);
  const from = await mark(driver);
  await showPlayerBar(driver);
  await click(driver, '.player button.fullscreen');
  await driver.waitFor(`return window.innerHeight > ${windowed}`, 'the Fullscreen control to fill the screen', {
    timeoutMs: 5000,
  });
  await pollUntil(
    async () => (await ends(driver)).slice(from).some((e) => e.name === 'window-fullscreen' && e.on === true),
    'the window to report fullscreen',
    { timeoutMs: 5000 },
  );
  return windowed;
}

// How long after `session`'s view went the window left fullscreen, or why it
// did not. A window still fullscreen at `GIVE_UP_MS` is put back by the test
// itself, so the next leg starts windowed.
async function leftAfterClose(driver, from, session, windowed) {
  let off = null;
  let closed = null;
  const deadline = Date.now() + 20_000 + GIVE_UP_MS;
  while (Date.now() < deadline) {
    const events = (await ends(driver)).slice(from);
    closed ??= events.find((e) => e.name === 'player-closed' && e.sessionId === session) ?? null;
    off = events.find((e) => e.name === 'window-fullscreen' && e.on === false) ?? null;
    // The window may leave before the view has gone (Close, the end of a
    // run), so both are waited for.
    if (closed && (off || Date.now() - closed.at > GIVE_UP_MS)) break;
    await new Promise((r) => setTimeout(r, 100));
  }
  if (!closed) return { error: "the play's view never closed" };
  if (!off) {
    await driver.exec(
      `return window.__TAURI_INTERNALS__.invoke('player_fullscreen', { sessionId: '', on: false }).then(() => true)`,
    );
    await driver.waitFor(`return window.innerHeight === ${windowed}`, 'the test to put the window back', {
      timeoutMs: 5000,
    });
    return {
      error: `still fullscreen ${GIVE_UP_MS} ms after the play's view went (only the 50 s backstop would leave it)`,
    };
  }
  await driver.waitFor(`return window.innerHeight === ${windowed}`, 'the page to be windowed again', {
    timeoutMs: 5000,
  });
  const delay = off.at - closed.at;
  if (delay > LEAVE_WITHIN_MS) {
    return { error: `left fullscreen ${delay} ms after the play's view went (bound ${LEAVE_WITHIN_MS} ms)` };
  }
  return { delay };
}

export default {
  name: 'embeddedfullscreenend',
  onlyPlayer: 'embedded',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['one.mp4', 'two.mp4', 'three.mp4']);
    mock = await startMockJellyfin({
      movies: [
        { id: 'm1', name: 'Mock Movie One', year: 2020, runTimeTicks: 100_000_000, mediaFile: path.join(mediaDir, 'one.mp4') },
        { id: 'm2', name: 'Mock Movie Two', year: 2021, runTimeTicks: 100_000_000, mediaFile: path.join(mediaDir, 'two.mp4') },
        { id: 'm3', name: 'Mock Movie Three', year: 2022, runTimeTicks: 100_000_000, mediaFile: path.join(mediaDir, 'three.mp4') },
      ],
    });
    seedConfig(configRoot, [mockSource(mock)]);
    seedPlaylists(configRoot, [
      {
        id: 'p-pair',
        name: 'Pair',
        items: [
          { id: 'e1', item: item('m1', 'Mock Movie One'), sourceName: 'Mock JF' },
          { id: 'e2', item: item('m2', 'Mock Movie Two'), sourceName: 'Mock JF' },
        ],
        createdMs: 1,
        updatedMs: 1,
      },
    ]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Playlists')`,
      'the Playlists sidebar entry',
    );
    await recordPlayerEvents(driver);
    await recordEnds(driver);
    const sessions = new Map();
    const failures = [];

    // Legs 1 and 2: the playlist. Its first play ends cleanly; the backend
    // starts the second, whose stream the server refuses.
    let stopsBefore = stopReports();
    let from = await mark(driver);
    const playlists = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Playlists']`,
    );
    await driver.click(playlists);
    await driver.waitFor(
      `return !!document.querySelector('section.playlists .playlistgrid button[aria-label^="Open Pair,"]')`,
      'the seeded playlist',
    );
    await click(driver, 'section.playlists .playlistgrid button[aria-label^="Open Pair,"]');
    await driver.waitFor(
      `return document.querySelectorAll('section.playlists ol.entries > li').length === 2`,
      'the seeded playlist entries',
    );
    let before = mpvSocketSnapshot();
    const play = await driver.find(
      'xpath',
      `//ol[@aria-label='Playlist items']/li[.//strong[normalize-space(.)='Mock Movie One']]//div[contains(@class,'entryactions')]/button[normalize-space(.)='Play']`,
    );
    await driver.click(play);
    const first = await nextOpened(driver, from, 'the first play of the playlist to open its view');
    sessions.set(first, 'one');
    let mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
    await viewReady(driver);
    let windowed = await fullscreenOn(driver);
    await screenshot('01-playlist-fullscreen');
    mock.state.failStreams.add('m2');
    const gapFrom = await mark(driver);
    try {
      await mpv.setProp('time-pos', 8.5);
    } finally {
      mpv.close();
    }
    const second = await nextOpened(driver, gapFrom, 'the backend to start the playlist\'s second play');
    sessions.set(second, 'two (stream refused)');
    const gap = (await ends(driver)).slice(gapFrom);
    const secondOpenedAt = gap.findIndex((e) => e.name === 'player-opened' && e.sessionId === second);
    if (gap.slice(0, secondOpenedAt).some((e) => e.name === 'window-fullscreen' && e.on === false)) {
      failures.push('leg 1: fullscreen was left in the gap between a play and the successor the backend started');
    }
    const errorEnd = await leftAfterClose(driver, gapFrom + secondOpenedAt, second, windowed);
    if (errorEnd.error) failures.push(`leg 2 (an end in an error): ${errorEnd.error}`);
    else console.log(`leg 2: left fullscreen ${errorEnd.delay} ms after the view went`);
    await settled(stopsBefore);
    printLeg('legs 1 and 2 (playlist: clean end with a backend successor, then an end in an error)', (await ends(driver)).slice(from), sessions);
    mock.state.failStreams.delete('m2');

    // Leg 3: mpv told to quit over its socket by something other than the page.
    stopsBefore = stopReports();
    from = await mark(driver);
    before = mpvSocketSnapshot();
    await openLibraryGrid(driver);
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie Three"]');
    const third = await nextOpened(driver, from, 'the third play to open its view');
    sessions.set(third, 'three');
    mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
    await viewReady(driver);
    windowed = await fullscreenOn(driver);
    mpv.quit();
    mpv.close();
    const quitEnd = await leftAfterClose(driver, from, third, windowed);
    if (quitEnd.error) failures.push(`leg 3 (quit over the socket): ${quitEnd.error}`);
    else console.log(`leg 3: left fullscreen ${quitEnd.delay} ms after the view went`);
    await settled(stopsBefore);
    printLeg('leg 3 (quit over the socket)', (await ends(driver)).slice(from), sessions);

    // Leg 4: a clean end with no successor: the end of the run.
    stopsBefore = stopReports();
    from = await mark(driver);
    before = mpvSocketSnapshot();
    await openLibraryGrid(driver);
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie Three"]');
    const fourth = await nextOpened(driver, from, 'the fourth play to open its view');
    sessions.set(fourth, 'three again');
    mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
    await viewReady(driver);
    windowed = await fullscreenOn(driver);
    try {
      await mpv.setProp('time-pos', 8.5);
    } finally {
      mpv.close();
    }
    const cleanEnd = await leftAfterClose(driver, from, fourth, windowed);
    if (cleanEnd.error) failures.push(`leg 4 (clean end, no successor): ${cleanEnd.error}`);
    else console.log(`leg 4: left fullscreen ${cleanEnd.delay} ms after the view went`);
    await settled(stopsBefore);
    printLeg('leg 4 (clean end, no successor)', (await ends(driver)).slice(from), sessions);

    // Leg 5: Close from the view.
    stopsBefore = stopReports();
    from = await mark(driver);
    before = mpvSocketSnapshot();
    await openLibraryGrid(driver);
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie One"]');
    const fifth = await nextOpened(driver, from, 'the fifth play to open its view');
    sessions.set(fifth, 'one again');
    await viewReady(driver);
    windowed = await fullscreenOn(driver);
    await showPlayerBar(driver);
    await click(driver, '.player button.close');
    const closeEnd = await leftAfterClose(driver, from, fifth, windowed);
    if (closeEnd.error) failures.push(`leg 5 (Close): ${closeEnd.error}`);
    else console.log(`leg 5: left fullscreen ${closeEnd.delay} ms after the view went`);
    await settled(stopsBefore);
    printLeg('leg 5 (Close)', (await ends(driver)).slice(from), sessions);

    // Leg 6: the playlist again; its first play ends cleanly, and the backend
    // cannot start the next (the server no longer resolves a stream for it).
    // The run is over: the window leaves at once.
    stopsBefore = stopReports();
    from = await mark(driver);
    await driver.click(
      await driver.find('xpath', `//button[contains(@class,'sideitem') and normalize-space(.)='Playlists']`),
    );
    await driver.waitFor(
      `return !!document.querySelector('section.playlists .playlistgrid button[aria-label^="Open Pair,"]')`,
      'the seeded playlist',
    );
    await click(driver, 'section.playlists .playlistgrid button[aria-label^="Open Pair,"]');
    await driver.waitFor(
      `return document.querySelectorAll('section.playlists ol.entries > li').length === 2`,
      'the seeded playlist entries',
    );
    before = mpvSocketSnapshot();
    await driver.click(
      await driver.find(
        'xpath',
        `//ol[@aria-label='Playlist items']/li[.//strong[normalize-space(.)='Mock Movie One']]//div[contains(@class,'entryactions')]/button[normalize-space(.)='Play']`,
      ),
    );
    const sixth = await nextOpened(driver, from, 'the playlist\'s first play to open its view again');
    sessions.set(sixth, 'one, playlist again');
    mpv = await MpvIpc.connect(await waitForNewMpvSocket(before));
    await viewReady(driver);
    windowed = await fullscreenOn(driver);
    mock.state.failPlaybackInfo = true;
    try {
      await mpv.setProp('time-pos', 8.5);
    } finally {
      mpv.close();
    }
    const noSuccessor = await leftAfterClose(driver, from, sixth, windowed);
    if (noSuccessor.error) failures.push(`leg 6 (playlist, next play cannot start): ${noSuccessor.error}`);
    else console.log(`leg 6: left fullscreen ${noSuccessor.delay} ms after the view went`);
    await settled(stopsBefore);
    mock.state.failPlaybackInfo = false;
    const leg6 = (await ends(driver)).slice(from);
    if (leg6.some((e) => e.name === 'player-opened' && e.sessionId !== sixth)) {
      failures.push('leg 6: a next play started, though the server resolved no stream for it');
    }
    printLeg('leg 6 (playlist, next play cannot start)', leg6, sessions);

    assert.deepEqual(failures, []);
  },
};
