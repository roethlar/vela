// The Player window setting (Slice 3 of `.agents/plans/mpv-window-geometry.md`)
// across restarts of the real app: the first play of a run opens as the
// setting says, the end of a play (or quitting Vela while one plays) remembers
// the window only in remember mode, and leaving remember deletes what was remembered. mpv runs with --vo=null
// here, so only its options and properties are asserted, never a drawn window.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  holdsFor,
  makeClips,
  mockSource,
  openDetailAndPlay,
  openLibraryGrid,
  pollUntil,
  seedConfig,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { acceptedConnections } from '../judge.mjs';

let mock;

const POSTER = 'button.poster[aria-label^="Mock Movie"]';

async function openPlayerSettings(driver) {
  await driver.waitFor(
    `return !!document.querySelector('button[aria-label="Settings"]')`,
    'the Settings button',
  );
  await driver.click(await driver.find('css selector', 'button[aria-label="Settings"]'));
  await driver.waitFor(`return !!document.querySelector('[role="dialog"]')`, 'Settings dialog');
  await driver.click(
    await driver.find('xpath', `//button[@role='tab' and normalize-space(.)='Player']`),
  );
  await driver.waitFor(
    `return !!document.querySelector('select#player-window')`,
    'the Player window selector',
  );
}

export default {
  name: 'windowsetting',
  // The setting is about the external player's own window.
  onlyPlayer: 'external',
  onlyPlayerWhy:
    "the setting says how mpv's own window opens; the embedded player has no window of its own (the plan's inventory: window-state inheritance is external-only by nature)",
  rows: [],
  noRows: "the external player's window: its state and size from one play to the next, and the setting for a run's first play; not a row of the inventory",

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['stream.mp4']);
    mock = await startMockJellyfin({
      movies: [
        {
          id: 'm1',
          name: 'Mock Movie',
          year: 2020,
          runTimeTicks: 100_000_000,
          mediaFile: path.join(mediaDir, 'stream.mp4'),
        },
      ],
    });
    // What the last play of a previous run left: fullscreen, with a windowed
    // size to return to, on a screen that is not connected here. No mode is
    // stored, which means remember.
    seedConfig(configRoot, [mockSource(mock)], {
      mpv_window: {
        fullscreen: true,
        maximized: false,
        screen: 'no-such-screen',
        width: 800,
        height: 450,
      },
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const readConfig = () => JSON.parse(fs.readFileSync(configFile, 'utf8'));
    const readConfigSafely = () => {
      try {
        return readConfig();
      } catch {
        return null;
      }
    };

    // Start the movie from its detail page and wait until Vela has recorded
    // the start, so the end of this play can be told apart from the last.
    async function play() {
      const previous = readConfigSafely()?.recents?.[0]?.session_id ?? null;
      await openLibraryGrid(driver);
      const before = mpvSocketSnapshot();
      await openDetailAndPlay(driver, POSTER);
      const socket = await waitForNewMpvSocket(before, { timeoutMs: 20000 });
      const mpv = await MpvIpc.connect(socket);
      await pollUntil(() => mpv.getProp('path').catch(() => null), 'the stream in mpv');
      await mpv.setProp('pause', true);
      const recent = await pollUntil(() => {
        const head = readConfigSafely()?.recents?.[0];
        return head && head.session_id !== previous && head.ended_at_ms === 0 ? head : null;
      }, 'the recorded start of this play');
      // Wait until Vela is watching this mpv before the play is changed or
      // ended (review finding pw-8). Vela learns how the window is left only
      // over its own connections to mpv's socket, and it opens them by
      // trying every 100 ms; this scenario can reach mpv, change its window
      // and quit it within a few milliseconds of the socket appearing, so
      // now and then Vela was not yet connected and mpv ended without ever
      // telling it anything. A person cannot be that quick: mpv's window is
      // not even there yet. Nothing a user sees says Vela is connected, so
      // this waits on the sockets themselves: mpv has accepted this
      // scenario's connection and Vela's two (the end-of-file watcher and
      // the reader of the window state, the only ones Vela opens for a play
      // like this one, with no notice to show and no Automatic quality).
      await pollUntil(
        () => acceptedConnections(fs.readFileSync('/proc/net/unix', 'utf8'), socket) >= 3,
        "Vela's own connections to this mpv",
        { intervalMs: 20 },
      );
      return { mpv, session: recent.session_id };
    }

    async function setFullscreen(mpv, value) {
      await mpv.setProp('fullscreen', value);
      await pollUntil(
        async () => (await mpv.getProp('fullscreen').catch(() => null)) === value,
        `fullscreen ${value} readback`,
      );
    }

    // The end of a play is written in one go with its recents stamp: once the
    // stamp is there, so is anything the play remembers.
    async function quitAndWaitForEnd({ mpv, session }) {
      mpv.quit();
      mpv.close();
      return pollUntil(() => {
        const config = readConfigSafely();
        const head = config?.recents?.find((entry) => entry.session_id === session);
        return head && head.ended_at_ms > 0 ? config : null;
      }, 'the end of the play in config.json');
    }

    // Run 1, remember: the first play opens as the stored window says, on the
    // stored screen even though it is gone, and the play still starts.
    const first = await play();
    assert.equal(
      await first.mpv.getProp('fullscreen'),
      true,
      'nothing but the stored window can make the first play of a run fullscreen',
    );
    assert.equal(await first.mpv.getProp('window-maximized'), false);
    assert.equal(
      await first.mpv.getProp('screen-name'),
      'no-such-screen',
      'a stored screen that is gone is still only a request to mpv',
    );
    assert.equal(await first.mpv.getProp('geometry'), '800x450', 'the stored size');
    // It stays fullscreen to the end, so it never shows a windowed size of
    // its own (and with no window here it reports 0x0).
    await quitAndWaitForEnd(first);

    // The next play of the run opens as that one left the window, and still
    // has the size to return to when the user leaves fullscreen.
    const next = await play();
    assert.equal(await next.mpv.getProp('fullscreen'), true);
    assert.equal(
      await next.mpv.getProp('geometry'),
      '800x450',
      'a play that stayed fullscreen must hand on the size it opened at',
    );
    await setFullscreen(next.mpv, false);
    const afterFirst = await quitAndWaitForEnd(next);
    assert.deepEqual(
      afterFirst.mpv_window,
      {
        fullscreen: false,
        maximized: false,
        screen: 'no-such-screen',
        width: 800,
        height: 450,
      },
      'the end of the play remembers how it left the window, and keeps what it did not observe',
    );
    assert.equal(afterFirst.mpv_window_mode, undefined, 'remember is stored as no value');

    // Quitting Vela while a video still plays keeps that play's window too:
    // the play never reaches its own end before the process goes.
    const showing = await play();
    assert.equal(await showing.mpv.getProp('fullscreen'), false);
    await setFullscreen(showing.mpv, true);
    // Quit through the app's own exit, as its Exit Vela button does; ending
    // the WebDriver session alone does not let the app run its exit path.
    await driver
      .exec(`window.__TAURI_INTERNALS__.invoke('exit_vela').catch(() => {}); return true;`)
      .catch(() => {});
    await pollUntil(
      () => readConfigSafely()?.mpv_window?.fullscreen === true,
      'the window of the play still showing when Vela quit, in config.json',
      { timeoutMs: 5000 },
    );
    showing.mpv.close();
    await restart();
    // Run 2, remember: the first play opens as that play was left.
    const reopened = await play();
    assert.equal(
      await reopened.mpv.getProp('fullscreen'),
      true,
      'the run after a quit mid-play must open as that play was left',
    );
    await quitAndWaitForEnd(reopened);

    // Leaving remember deletes the remembered window in the same write.
    await openPlayerSettings(driver);
    assert.equal(
      await driver.exec(`return document.querySelector('select#player-window')?.value`),
      'remember',
      'a missing setting shows as remember',
    );
    await driver.exec(
      `const select = document.querySelector('select#player-window');
       select.value = 'fullscreen';
       select.dispatchEvent(new Event('change', { bubbles: true }));`,
    );
    await driver.click(
      await driver.find('xpath', `//button[normalize-space(.)='Save player window']`),
    );
    await pollUntil(() => {
      const config = readConfigSafely();
      return config?.mpv_window_mode === 'fullscreen' && !('mpv_window' in config);
    }, 'fullscreen mode saved and the remembered window deleted');
    await driver.exec(
      `document.querySelector('select#player-window').scrollIntoView({ block: 'center' })`,
    );
    await screenshot('01-player-window-fullscreen');
    await driver.click(await driver.find('css selector', 'button[aria-label="Close"]'));

    // Run 3, fullscreen: the first play opens fullscreen with nothing stored,
    // and leaving fullscreen is not remembered.
    await restart();
    const second = await play();
    assert.equal(
      await second.mpv.getProp('fullscreen'),
      true,
      'fullscreen mode must open the first play of a run fullscreen',
    );
    await setFullscreen(second.mpv, false);
    const afterSecond = await quitAndWaitForEnd(second);
    assert.equal(afterSecond.mpv_window, undefined, 'fullscreen mode must not remember a window');

    // Run 4, default: nothing from Vela, even with a window stored by hand.
    await restart(() => {
      const config = readConfig();
      config.mpv_window_mode = 'default';
      config.mpv_window = { fullscreen: true };
      fs.writeFileSync(configFile, JSON.stringify(config));
    });
    const third = await play();
    assert.equal(
      await third.mpv.getProp('fullscreen'),
      false,
      'default mode must leave the first play to mpv',
    );
    await openPlayerSettings(driver);
    assert.equal(
      await driver.exec(`return document.querySelector('select#player-window')?.value`),
      'default',
    );
    await driver.click(await driver.find('css selector', 'button[aria-label="Close"]'));
    const afterThird = await quitAndWaitForEnd(third);
    assert.deepEqual(afterThird.mpv_window, { fullscreen: true }, 'default mode writes nothing');
    await holdsFor(
      () => (JSON.stringify(readConfigSafely()?.mpv_window) === '{"fullscreen":true}' ? false : 'changed'),
      1_000,
      'nothing writes the window after the play has ended',
    );
  },
};
