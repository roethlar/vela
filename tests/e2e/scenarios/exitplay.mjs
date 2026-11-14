// Vela's exit stops the play (row 12b of the parity list in
// .agents/plans/embedded-player.md), in both players.
//
// A film plays; Vela exits. Vela's process must end promptly (the exit must
// not hang on the play, embedded or not), the play must stop answering, and
// in the external player mpv's own process must end with it: no player is
// left behind with nobody to record where it got to. That last is the exit
// sweep (lib.rs, RunEvent::Exit) the inventory names; in the embedded player
// the play is inside Vela's process, and the sweep waits a bounded moment for
// it before the process goes; what is shown there is only that Vela's
// process ends promptly with the play running, not how the play stopped.
//
// How Vela is made to exit is a TEST-ONLY route, the same in both players:
// the page asks for `exit_vela`, the command behind the "Exit Vela" button of
// the recovery screens (app.exit). A user closes the window instead, which
// Tauri turns into the same exit request (RunEvent::ExitRequested, then
// RunEvent::Exit) that the sweep handles; neither venue can press a window's
// close button (no window manager on the private X display; weston's
// decorations are drawn by GTK inside the window, out of WebDriver's reach),
// and WebDriver's "close window" destroys only the web view, which does not
// exit Tauri. So what is proven is what Vela does once its exit is under way;
// that a window close starts it is Tauri's, and not proven here.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  exitVelaFromPage,
  makeClips,
  mockSource,
  openDetailAndPlay,
  openLibraryGrid,
  pollUntil,
  processEnded,
  runPlayer,
  scenarioAppPids,
  seedConfig,
  socketAccepting,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { shown } from '../proof.mjs';

let mock;

// The processes, of anyone's, whose command line names the play's socket:
// the external player's mpv is started with it (--input-ipc-server).
function processesNaming(socketPath) {
  return fs
    .readdirSync('/proc')
    .filter((entry) => /^\d+$/.test(entry))
    .filter((pid) => {
      try {
        return fs.readFileSync(`/proc/${pid}/cmdline`, 'utf8').includes(socketPath) && !processEnded(pid);
      } catch {
        return false;
      }
    });
}

export default {
  name: 'exitplay',
  rows: ['12b'],
  testOnly: [
    {
      row: '12b',
      player: 'external',
      via: 'exitVelaFromPage',
      route: "the page invokes exit_vela (the recovery screens' Exit Vela command) during the play",
      proves: "that once Vela's exit is under way the running external mpv is stopped and its process ends, and that Vela's own process ends",
      unproven: "that a user's window close starts that exit (Tauri's own path; no venue here can press a window's close button)",
    },
    {
      row: '12b',
      player: 'embedded',
      via: 'exitVelaFromPage',
      route: "the page invokes exit_vela (the recovery screens' Exit Vela command) during the play",
      proves: "that once Vela's exit is under way Vela's process ends within 10 s with an embedded play running (the play cannot outlive it: it is inside that process). Not that the play stopped cleanly first: nothing here observes embedded::wait_for_unfinished_plays, and no Stopped check-in is sent on exit",
      unproven: "that a user's window close starts that exit (Tauri's own path; no venue here can press a window's close button)",
    },
  ],

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['stream.mp4']);
    mock = await startMockJellyfin({
      movies: [{ id: 'm1', name: 'Mock Movie', year: 2020, runTimeTicks: 100_000_000, mediaFile: path.join(mediaDir, 'stream.mp4') }],
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, configRoot }) {
    await openLibraryGrid(driver);
    const before = mpvSocketSnapshot();
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
    const socketPath = await waitForNewMpvSocket(before);
    const mpv = await MpvIpc.connect(socketPath);
    let playerPid;
    let app;
    try {
      const loaded = await pollUntil(() => mpv.getProp('path').catch(() => null), 'mpv to load the stream');
      assert.ok(loaded.startsWith(`http://127.0.0.1:${mock.port}/Videos/m1/stream`), `got ${loaded}`);
      await pollUntil(() => mpv.getProp('time-pos').then((t) => t > 0.5).catch(() => false), 'the play to progress');
      playerPid = await mpv.getProp('pid');
      const apps = scenarioAppPids(configRoot);
      assert.equal(apps.length, 1, `exactly one app of this scenario, got [${apps}]`);
      [app] = apps;
      if (runPlayer === 'embedded') {
        assert.equal(playerPid, app, 'the embedded play runs inside Vela');
      } else {
        assert.notEqual(playerPid, app, 'the external play is a program of its own');
        assert.deepEqual(processesNaming(socketPath), [String(playerPid)], 'the running mpv is the one with this socket');
      }
    } finally {
      mpv.close();
    }

    // Vela exits, the play still running. The command does not answer: the
    // process it would answer from is going.
    const exitAt = Date.now();
    await exitVelaFromPage(driver);

    await pollUntil(() => processEnded(app), "Vela's process to end", { timeoutMs: 10000, intervalMs: 100 });
    const exitMs = Date.now() - exitAt;
    console.log(`exitplay: Vela's process ended ${exitMs} ms after the exit was asked`);
    // In the external player this is mpv's own socket, and a live mpv would
    // still answer here. In the embedded player the socket is served from
    // inside Vela's process, which has just ended, so there this cannot fail
    // (guard finding atk-6): the embedded proof is the process end above.
    assert.equal(await socketAccepting(socketPath), false, 'the play stopped answering');
    if (runPlayer === 'external') {
      await pollUntil(() => processEnded(playerPid), "the external player's mpv to end with Vela", { timeoutMs: 5000 });
      assert.deepEqual(processesNaming(socketPath), [], 'no process with this play is left behind');
    }
    assert.deepEqual(scenarioAppPids(configRoot), [], 'no app of this scenario is left');
    const stops = mock.state.checkins.filter((c) => c.endpoint === '/Stopped').length;
    console.log(`exitplay: Stopped check-ins after the exit: ${stops} (not asserted: the exit does not wait for the tracker)`);
    // The end of its proof of its rows in this player (proof.mjs).
    shown('12b');
  },
};
