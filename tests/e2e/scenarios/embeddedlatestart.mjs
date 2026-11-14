// An embedded play that does not start in time
// (.agents/plans/embedded-player.md; review findings ec-2 and ec-6).
//
// Runs only in a run of the embedded player (`npm run e2e -- --embedded`).
//
// Two promises are checked on one staged play. The play is made to sit for
// eight seconds before it gives mpv its file, and Vela's wait for it to start
// is cut to half a second (a development build honours both variables set
// below; a released Vela waits twenty seconds and never delays). A play that
// has not started is asked to stop and given five more seconds; this one sits
// through those too.
//
// - **The play is not failed** (decision D3, ec-6). The same play starts in
//   the external player, with the one line that says so, while the embedded
//   player is still sitting there.
// - **The play that came too late stays out of it** (ec-2). When its eight
//   seconds are up, the embedded player finds that nobody is waiting: it never
//   gives mpv the file, never opens the player view, and goes away.
//
// After that the embedded player is not tried again in this run, and the user
// is not told a second time.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot } from '../mpv.mjs';
import { makeClips, mockSource, openDetailAndPlay, openLibraryGrid, pollUntil, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const NOTICE = 'Embedded player: could not start';
let mock;
let logFile;

const stopReports = () =>
  mock.state.served.filter((r) => r.method === 'POST' && r.path === '/Sessions/Playing/Stopped').length;

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

// The program a player's mpv runs in: `vela` for the embedded player, `mpv`
// for the external one. `null` once the socket is gone.
async function playerAt(socketPath) {
  let mpv;
  try {
    mpv = await MpvIpc.connect(socketPath, { timeoutMs: 300 });
    const pid = await mpv.getProp('pid');
    return { mpv, program: fs.readFileSync(`/proc/${pid}/comm`, 'utf8').trim() };
  } catch {
    mpv?.close();
    return null;
  }
}

// Every mpv socket that has appeared since `before`, with the program behind it.
async function newPlayers(before) {
  const players = [];
  for (const socketPath of mpvSocketSnapshot()) {
    if (before.has(socketPath)) continue;
    const player = await playerAt(socketPath);
    if (player) players.push({ socketPath, ...player });
  }
  return players;
}

const showTextCommands = () =>
  fs.existsSync(logFile)
    ? fs.readFileSync(logFile, 'utf8').split('\n').filter((line) => line.includes('Run command: show-text'))
    : [];

export default {
  name: 'embeddedlatestart',
  onlyPlayer: 'embedded',
  onlyPlayerWhy:
    'an embedded play that does not start in time carries on in the external player (decision D3); in the external player there is nothing to fall back from',
  rows: [],
  noRows: 'decision D3, a late embedded start; not a row of the inventory',

  async seed({ configRoot }) {
    logFile = path.join(configRoot, 'mpv-commands.log');
    const mediaDir = makeClips(configRoot, ['stream.mp4']);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Mock Movie',
        year: 2020,
        runTimeTicks: 100_000_000,
        mediaFile: path.join(mediaDir, 'stream.mp4'),
      }],
    });
    seedConfig(configRoot, [mockSource(mock)], {
      // The log is how the notice is seen: Vela sends it as a `show-text`
      // command, which mpv logs at trace level.
      mpv_extra_args: `--vo=null\n--ao=null\n--msg-level=cplayer=trace\n--log-file=${logFile}`,
    });
  },

  environment() {
    return { VELA_EMBEDDED_LOAD_DELAY_MS: '8000', VELA_EMBEDDED_START_LIMIT_MS: '500' };
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock Library')`,
      'seeded library in the sidebar',
    );
    // Remember if the player view ever shows, however briefly.
    await driver.exec(`
      window.__playerViewShown = false;
      new MutationObserver(() => {
        if (document.documentElement.classList.contains('player-open')) window.__playerViewShown = true;
      }).observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
      return true;
    `);
    const playerViewShown = () => driver.exec('return window.__playerViewShown');

    // 1. The play starts, and it is mpv, the program, that plays it.
    let stopsBefore = stopReports();
    const before = mpvSocketSnapshot();
    const startedAt = Date.now();
    await openLibraryGrid(driver);
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
    const external = await pollUntil(
      async () => {
        const players = await newPlayers(before);
        const found = players.find((player) => player.program === 'mpv');
        for (const player of players) if (player !== found) player.mpv.close();
        return found ?? null;
      },
      'the play to start in the external player',
    );
    let late;
    try {
      // 2. The embedded player that did not make it is still there: the play
      //    went on without waiting for it to come back. While it is there it
      //    has no file, and it never gets one.
      const embeddedSockets = [...mpvSocketSnapshot()].filter(
        (socketPath) => !before.has(socketPath) && socketPath !== external.socketPath,
      );
      assert.equal(
        embeddedSockets.length,
        1,
        'the embedded player that has not started is still there when the external one takes over',
      );
      late = embeddedSockets[0];
      let sawItAlive = false;
      {
        await pollUntil(
          async () => {
            const player = await playerAt(late);
            if (!player) return true;
            try {
              sawItAlive = true;
              assert.equal(player.program, 'vela', 'the other player is the embedded one');
              assert.equal(
                await player.mpv.getProp('path').catch(() => null),
                null,
                'a play that was given up on is never handed its file',
              );
            } finally {
              player.mpv.close();
            }
            return false;
          },
          'the embedded player that came too late to go away',
          { timeoutMs: 12000, intervalMs: 150 },
        );
      }
      assert.ok(sawItAlive, 'the embedded player was seen waiting, without a file, before it went');
      // It has had its turn now: its eight seconds are over and it is gone.
      assert.ok(Date.now() - startedAt >= 8000, 'and it went only when its wait was over');
      assert.equal(await playerViewShown(), false, 'the player view never opened for a play that was given up on');
      assert.equal(
        await driver.exec(`return !!document.querySelector('.player')`),
        false,
      );
      // Meanwhile the play itself has been running in the external player.
      await pollUntil(() => external.mpv.getProp('path').catch(() => null), 'the external player to load the stream');
      await pollUntil(
        () => external.mpv.getProp('time-pos').then((t) => t > 0.3).catch(() => false),
        'the external player to play',
      );
      assert.equal(await external.mpv.getProp('pause'), false, 'and it carries on');

      // 3. The user is told why, once.
      const notices = await pollUntil(
        () => {
          const commands = showTextCommands();
          return commands.length > 0 ? commands : null;
        },
        'the notice on the external player',
      );
      assert.equal(notices.length, 1, `one notice, got: ${notices.join(' | ')}`);
      assert.ok(notices[0].includes(NOTICE), `the notice says why: ${notices[0]}`);
      external.mpv.quit();
    } finally {
      external.mpv.close();
    }
    await settled(stopsBefore);

    // 4. The next play goes straight to the external player and says nothing.
    stopsBefore = stopReports();
    const beforeSecond = mpvSocketSnapshot();
    await openLibraryGrid(driver);
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
    const second = await pollUntil(
      async () => {
        const players = await newPlayers(beforeSecond);
        return players[0] ?? null;
      },
      'the second play to start',
    );
    try {
      assert.equal(second.program, 'mpv', 'the embedded player is not tried again in this run');
      await pollUntil(
        () => second.mpv.getProp('time-pos').then((t) => t > 0.3).catch(() => false),
        'the second play to progress',
      );
      // Vela sends a notice 1.5 s after it connects; wait well past that.
      // (Each mpv starts its log afresh, so these are this play's commands.)
      await new Promise((r) => setTimeout(r, 3500));
      assert.deepEqual(showTextCommands(), [], 'the user is not told a second time');
      assert.equal(await playerViewShown(), false);
      second.mpv.quit();
    } finally {
      second.mpv.close();
    }
    await settled(stopsBefore);
  },
};
