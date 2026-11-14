// A new play stops the one that is running (row 12a of the parity list in
// .agents/plans/embedded-player.md), in both players.
//
// Film A plays, at a known position; the user then starts film B. A must be
// stopped by Vela: its player stops answering, its Stopped check-in carries
// its position, its recent is stamped with its position (in place: neither
// closed nor moved to the front, as B is the newer play;
// recents::finish_session), and (external player) its mpv process is gone. B must play, its session open, untouched by A's end
// for a while after it; in the embedded player the view stays up across the
// change, for B (a late close of A must not dismiss B's view).
//
// How the user starts B differs, and that is the one branch, inside
// clickUnderPlay: the external player's user clicks Play on the page beside
// mpv's window; the embedded player's view covers the page, so no user of it
// can start another film during a play (they close, then play). There the
// click is the TEST-ONLY route clickUnderPlay (declared below, and listed by
// the parity test): it proves what the backend and the view do when a new
// play replaces a running embedded one, which is what a step-down relaunch
// (row 10) does without a user; it does not prove a user can do it.
//
// Both plays are started from the card's context menu, which is opened by an
// event sent to the card (as `curation` does), so that the same steps work
// while the embedded view covers the grid.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  PLAYER_VIEW_UP,
  clickUnderPlay,
  holdsFor,
  makeClips,
  mockSource,
  openLibraryGrid,
  pollUntil,
  processEnded,
  runPlayer,
  scenarioAppPids,
  seedConfig,
  socketAccepting,
  stopPlayAsUser,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { shown } from '../proof.mjs';

let mock;

const stoppedFor = (id) =>
  mock.state.checkins.filter((c) => c.endpoint === '/Stopped' && c.body.ItemId === id).map((c) => c.body);

// Opens the card's context menu and returns its Play entry.
async function menuPlay(driver, title) {
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^=${JSON.stringify(title)}]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, `the context menu of ${title}`);
  return driver.find('xpath', `//*[contains(@class,'ctxmenu')]//button[@role='menuitem' and normalize-space(.)='Play']`);
}

async function playing(socketPath, id, port) {
  const mpv = await MpvIpc.connect(socketPath);
  const loaded = await pollUntil(() => mpv.getProp('path').catch(() => null), `${id} to load`);
  assert.ok(loaded.startsWith(`http://127.0.0.1:${port}/Videos/${id}/stream`), `expected ${id}'s stream, got ${loaded}`);
  await pollUntil(() => mpv.getProp('time-pos').then((t) => t > 0.5).catch(() => false), `${id} to progress`);
  return mpv;
}

const recentOf = (configRoot, key) => {
  try {
    const cfg = JSON.parse(fs.readFileSync(path.join(configRoot, 'config', 'vela', 'config.json'), 'utf8'));
    return { first: cfg.recents?.[0] ?? null, entry: (cfg.recents ?? []).find((r) => r.item?.ratingKey === key) ?? null };
  } catch {
    return { first: null, entry: null }; // mid-write
  }
};

export default {
  name: 'replaceplay',
  rows: ['12a'],
  testOnly: [
    {
      row: '12a',
      player: 'embedded',
      via: 'clickUnderPlay',
      route: 'clickUnderPlay: Play on the second film, from its context menu, while the first plays under the player view',
      proves: 'that a new play stops a running embedded play (its Session ends, its Stopped check-in and recent carry its position) and that the view stays up for the new play',
      unproven: 'that a user can start a play during a play: the player view covers the page (they close, then play); the backend does this without a user in a step-down relaunch',
    },
  ],

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['a.mp4', 'b.mp4']);
    mock = await startMockJellyfin({
      movies: [
        { id: 'm1', name: 'Mock Movie', year: 2020, runTimeTicks: 100_000_000, mediaFile: path.join(mediaDir, 'a.mp4') },
        { id: 'm2', name: 'Other Movie', year: 2021, runTimeTicks: 100_000_000, mediaFile: path.join(mediaDir, 'b.mp4') },
      ],
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, configRoot }) {
    await openLibraryGrid(driver);
    await driver.waitFor(`return !!document.querySelector('button.poster[aria-label^="Other Movie"]')`, 'the second film');

    // A plays, from where the user put it.
    const seen = mpvSocketSnapshot();
    await driver.click(await menuPlay(driver, 'Mock Movie'));
    const socketA = await waitForNewMpvSocket(seen);
    seen.add(socketA);
    const a = await playing(socketA, 'm1', mock.port);
    const pidA = await a.getProp('pid');
    const [app] = scenarioAppPids(configRoot);
    assert.ok(app, "the scenario's app is running");
    if (runPlayer === 'embedded') assert.equal(pidA, app, 'the embedded play runs inside Vela');
    else assert.notEqual(pidA, app, 'the external play is a program of its own');
    await a.setProp('time-pos', 4);
    await new Promise((r) => setTimeout(r, 1500)); // let Vela observe the position
    // Held there, A cannot end by itself (at the end of its file) while the
    // scenario waits for it to be stopped: only B's start can stop it.
    await a.setProp('pause', true);
    assert.deepEqual(stoppedFor('m1'), [], 'A is still running');

    // The user starts B.
    await clickUnderPlay(driver, await menuPlay(driver, 'Other Movie'), 'playing Other Movie while Mock Movie plays');

    // A is stopped by Vela.
    await pollUntil(async () => !(await socketAccepting(socketA)), 'the running play to be stopped by the new one', {
      timeoutMs: 5000,
    });
    a.close();
    if (runPlayer === 'external') {
      await pollUntil(() => processEnded(pidA), "the first play's mpv process to end", { timeoutMs: 5000 });
    }
    const [stoppedA] = await pollUntil(() => {
      const stops = stoppedFor('m1');
      return stops.length > 0 ? stops : null;
    }, "the first play's Stopped check-in");
    const stoppedMs = stoppedA.PositionTicks / 10_000;
    assert.ok(stoppedMs >= 3000 && stoppedMs <= 8000, `A's Stopped carries its position, got ${stoppedMs} ms`);

    // B plays, in the same player.
    const socketB = await waitForNewMpvSocket(seen);
    const b = await playing(socketB, 'm2', mock.port);
    try {
      const pidB = await b.getProp('pid');
      if (runPlayer === 'embedded') assert.equal(pidB, app, 'the new play is embedded too');
      else assert.notEqual(pidB, app, 'the new play is external too');
      // A's recent gets its position, in place: B is the newer play, so A's
      // recency is not stamped (recents::finish_session).
      const stamped = await pollUntil(() => {
        const { entry } = recentOf(configRoot, 'jf-mock:m1');
        return (entry?.item?.viewOffsetMs ?? 0) > 0 ? entry : null;
      }, "the first play's recent to get its position");
      const offset = stamped.item.viewOffsetMs;
      assert.ok(offset >= 3000 && offset <= 8000, `A's recent is stamped with its position, got ${offset} ms`);
      // A's end touches nothing of B's: B's session stays open, and in the
      // embedded player the view stays up for B (the view exists only there).
      await holdsFor(
        async () => {
          const { first } = recentOf(configRoot, 'jf-mock:m2');
          if (first && (first.item?.ratingKey !== 'jf-mock:m2' || first.ended_at_ms !== 0)) {
            return `the newest recent is ${JSON.stringify(first?.item?.ratingKey)}, ended ${first?.ended_at_ms}`;
          }
          if (runPlayer === 'embedded' && !(await driver.exec(PLAYER_VIEW_UP))) return 'the player view left';
          if (!(await socketAccepting(socketB))) return 'B stopped';
          return false;
        },
        1500,
        "the new play to go on after the old one's end",
      );
      assert.equal(recentOf(configRoot, 'jf-mock:m2').first?.item?.ratingKey, 'jf-mock:m2', "B's recent is the newest");
      await stopPlayAsUser(driver, b, socketB, 'the second play');
    } finally {
      b.close();
    }
    await pollUntil(() => stoppedFor('m2').length > 0, "the second play's Stopped check-in");
    assert.equal(stoppedFor('m1').length, 1, 'A was stopped once');
    // The end of its proof of its rows in this player (proof.mjs).
    shown('12a');
  },
};
