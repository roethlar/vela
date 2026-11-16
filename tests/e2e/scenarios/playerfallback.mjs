// The player setting where the embedded player cannot run
// (.agents/plans/embedded-player.md, Slice 2; decisions D1 and D3).
//
// A config that asks for the embedded player must still play: in the external
// player, with one line on its on-screen display saying why, shown once per run
// rather than at the start of every play. A config that does not ask for it
// gets no such line.
//
// "Cannot run" here is the suite's ordinary venue, an X11 display: the
// embedded player needs Wayland. In a run of the embedded player itself
// there is nothing to fall back from, so this scenario is not part of it.
//
// mpv runs with `--vo=null` here, so nothing is drawn. The notice is observed
// where it arrives: Vela sends it as a `show-text` command over mpv's IPC, and
// mpv logs the commands it runs (this one at trace level).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { makeClips, mockSource, openDetailAndPlay, openLibraryGrid, pollUntil, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const NOTICE = 'Embedded player: needs a Wayland session';
let mock;
let logFile;
let configRootPath;

// Change only the player setting, in the config the app itself has been
// writing. Call it while the app is down.
function setPlayer(player) {
  const file = path.join(configRootPath, 'config', 'vela', 'config.json');
  const config = JSON.parse(fs.readFileSync(file, 'utf8'));
  if (player) config.player = player;
  else delete config.player;
  fs.writeFileSync(file, JSON.stringify(config));
}

const stopReports = () =>
  mock.state.served.filter((r) => r.method === 'POST' && r.path === '/Sessions/Playing/Stopped').length;

// A play is over for Vela only after its last check-in with the server, and the
// page then reloads what it is showing. This scenario starts several plays in
// one run of the app; starting the next one before that reload has finished
// clicks a card that is about to be replaced.
async function waitForThePlayToSettle(stopReportsBefore) {
  await pollUntil(
    () => stopReports() > stopReportsBefore,
    'the stop report of the play that just ended',
  );
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

// Start a play, let it run long enough for a notice to arrive, report whether
// one did, stop it, and leave the app ready for the next play.
async function playAndReadNotice(driver) {
  const stopReportsBefore = stopReports();
  const before = mpvSocketSnapshot();
  await openLibraryGrid(driver);
  await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
  const socketPath = await waitForNewMpvSocket(before);
  const mpv = await MpvIpc.connect(socketPath);
  let commands;
  try {
    await pollUntil(() => mpv.getProp('path').catch(() => null), 'mpv to load the stream');
    await pollUntil(
      () => mpv.getProp('time-pos').then((t) => t > 0.3).catch(() => false),
      'playback to start',
    );
    // Vela sends a notice 1.5 s after it connects. Wait well past that, so
    // "no notice" below means none was sent rather than none yet.
    await new Promise((r) => setTimeout(r, 3500));
    const log = fs.readFileSync(logFile, 'utf8');
    commands = log.split('\n').filter((line) => line.includes('Run command: show-text'));
    mpv.quit();
  } finally {
    mpv.close();
  }
  await waitForThePlayToSettle(stopReportsBefore);
  return commands;
}

async function waitForHome(driver) {
  await driver.waitFor(
    `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock Library')`,
    'seeded library in the sidebar',
  );
}

export default {
  name: 'playerfallback',
  onlyPlayer: 'external',
  onlyPlayerWhy:
    'its premise is a venue where the embedded player cannot run (X11), so the play goes to the external player; in a run of the embedded player there is nothing to fall back from',
  // Not row 11. Its notice is the D3 fallback notice, for a play the user DID
  // ask for; row 11's notice is for a play the user did not ask for, a
  // step-down (playback.rs, osd_notice), which no venue here can force. The
  // two ride the same delivery (commands.rs, `osd_notice.or(fallback_notice)`),
  // so this scenario is evidence of that delivery, named in row 11's
  // exemptions, not a proof of the row (guard finding atk-2).
  rows: [],
  noRows:
    'decision D3, the fallback notice where the embedded player cannot run; it rides the delivery row 11 uses (spawn_osd_notice), but is not the unrequested-play notice of row 11',

  async seed({ configRoot }) {
    configRootPath = configRoot;
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
      // One option per line. The log is how this scenario sees what mpv was
      // told; mpv logs `show-text` at trace level only.
      mpv_extra_args: `--vo=null\n--ao=null\n--msg-level=cplayer=trace\n--log-file=${logFile}`,
      player: 'embedded',
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, restart }) {
    await waitForHome(driver);

    // 1. Embedded is asked for and cannot run: the play still starts, in mpv,
    //    and says why exactly once.
    const first = await playAndReadNotice(driver);
    assert.equal(first.length, 1, `one notice on the first fallback play, got: ${first.join(' | ')}`);
    assert.ok(first[0].includes(NOTICE), `the notice must say why: ${first[0]}`);

    // 2. The next play of the same run falls back the same way and says
    //    nothing: the user has been told.
    const second = await playAndReadNotice(driver);
    assert.deepEqual(second, [], 'the same fallback is not announced on every play');

    // 3. A new run of Vela tells the user again, once.
    await restart();
    await waitForHome(driver);
    const afterRestart = await playAndReadNotice(driver);
    assert.equal(afterRestart.length, 1, 'a new run explains the fallback again');
    assert.ok(afterRestart[0].includes(NOTICE));

    // 4. Without the setting there is nothing to fall back from, and no notice.
    await restart(() => setPlayer(null));
    await waitForHome(driver);
    assert.deepEqual(await playAndReadNotice(driver), [], 'an external play shows no fallback notice');

    // 5. The same when external is chosen explicitly.
    await restart(() => setPlayer('external'));
    await waitForHome(driver);
    assert.deepEqual(await playAndReadNotice(driver), [], 'an explicit external choice shows no notice');
  },
};
