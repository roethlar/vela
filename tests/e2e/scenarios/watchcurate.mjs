// Watched-state edits curate Continue Watching in one op (owner decision
// 2026-07-10): Mark watched/unwatched from the hero context menu must flip
// the server state (PlayedItems POST/DELETE), drop Vela's recents entry,
// tombstone the key, and clear the hero card — no second "remove" op. Runs
// against a mock server WITH a faithful Resume hub (serveResume), so the
// hero merges Vela's recents AND a live server hub copy — the two feeds the
// curation must clear together. Leg 3 proves the tombstone suppresses a
// server hub copy that outlives the action (the stale-cache / outside-play
// case curation.mjs never exercises: its Resume feed is hardcoded empty).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pollUntil, openLibraryGrid, goHome, playAndQuit, makeClips, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const HERO_CLIP = `[aria-label="Continue watching"] [aria-label^="Resume Mock Movie"]`;

let mock;

// Play via the grid card's context-menu Play and quit at ~6s; returns the
// first observed time-pos (the resume-position evidence).
function playClipAndQuit(driver) {
  return playAndQuit(driver, async () => {
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const play = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu (play)')
      .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Play']`));
    await driver.click(play);
  });
}

// Open the hero card's context menu and click the named entry.
async function heroMenuClick(driver, label) {
  await driver.exec(
    `const el = document.querySelector('${HERO_CLIP}');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  const btn = await driver
    .waitFor(`return !!document.querySelector('.ctxmenu')`, `hero context menu (${label})`)
    .then(() => driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='${label}']`));
  await driver.click(btn);
}

export default {
  name: 'watchcurate',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['stream.mp4']);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Mock Movie',
        year: 2020,
        runTimeTicks: 100_000_000, // 10s, matching the real clip
        mediaFile: path.join(mediaDir, 'stream.mp4'),
      }],
      serveResume: true, // the hero must merge a LIVE server hub copy too
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const readCfg = () => JSON.parse(fs.readFileSync(configFile, 'utf8'));
    const stampedRecent = () => {
      try {
        return (readCfg().recents?.[0]?.item?.viewOffsetMs ?? 0) > 0;
      } catch {
        return false;
      }
    };
    // Gate UI assertions on the post-action hubs refetch actually reaching
    // the mock (the markwatched.mjs eh-15 pattern): the Resume request both
    // proves the refetch ran and bounds when the hero has settled.
    const resumeFetches = () =>
      mock.state.requests.filter(
        (r) => r.method === 'GET' && r.path === `/Users/${mock.userId}/Items/Resume`,
      ).length;

    // An unfinished play lands the movie in the hero — via Vela's recents
    // AND, with serveResume, the server's own Resume hub.
    await openLibraryGrid(driver);
    await playClipAndQuit(driver);
    await pollUntil(stampedRecent, 'the recents position stamp');
    await pollUntil(
      () => mock.state.userData.m1.positionTicks > 0,
      'the server-side resume point from the Stopped check-in',
    );
    await goHome(driver);
    await driver.waitFor(`return !!document.querySelector('${HERO_CLIP}')`, 'movie in the hero');

    // Leg 1 — Mark unwatched must be a one-op full reset: server DELETE,
    // recents entry gone, tombstone written, hero card gone.
    const beforeUnwatch = resumeFetches();
    await heroMenuClick(driver, 'Mark unwatched');
    await pollUntil(
      () => mock.state.requests.some((r) => r.method === 'DELETE' && r.path === `/Users/${mock.userId}/PlayedItems/m1`),
      'the PlayedItems DELETE',
    );
    assert.equal(mock.state.userData.m1.played, false, 'server watch state must stay unwatched');
    assert.equal(mock.state.userData.m1.positionTicks, 0, 'unwatch must reset the server resume point');
    await pollUntil(() => resumeFetches() > beforeUnwatch, 'a hubs refetch after mark-unwatched');
    await driver.waitFor(
      `return !document.querySelector('${HERO_CLIP}')`,
      'hero card gone after mark-unwatched',
    );
    assert.equal(readCfg().recents?.length ?? 0, 0, 'mark-unwatched must drop the recents entry');
    assert.ok(
      (readCfg().hidden_from_continue?.length ?? 0) > 0,
      'mark-unwatched must tombstone the key',
    );
    await screenshot('01-unwatched-cleared');

    // Replay leg — playing again is the opposite of "stop suggesting it":
    // the tombstone clears, the hero returns, and the session must start
    // from ~0 (a stale resume point from the server mock or Vela's own
    // stamp would surface here as a ~6s start).
    await openLibraryGrid(driver);
    const replayPos = await playClipAndQuit(driver);
    assert.ok(
      replayPos < 2,
      `replay after a full reset must start from 0, got ${replayPos}s`,
    );
    await pollUntil(
      () => (readCfg().hidden_from_continue?.length ?? 0) === 0,
      'the tombstone to clear on replay',
    );
    await goHome(driver);
    await driver.waitFor(`return !!document.querySelector('${HERO_CLIP}')`, 'movie back in the hero');

    // Leg 2 — Mark watched: server POST, recents gone, tombstone written,
    // hero card gone.
    const beforeWatch = resumeFetches();
    await heroMenuClick(driver, 'Mark watched');
    await pollUntil(
      () => mock.state.requests.some((r) => r.method === 'POST' && r.path === `/Users/${mock.userId}/PlayedItems/m1`),
      'the PlayedItems POST',
    );
    assert.equal(mock.state.userData.m1.played, true, 'server watch state must flip to played');
    assert.equal(mock.state.userData.m1.positionTicks, 0, 'mark-watched must reset the server resume point');
    await pollUntil(() => resumeFetches() > beforeWatch, 'a hubs refetch after mark-watched');
    await driver.waitFor(
      `return !document.querySelector('${HERO_CLIP}')`,
      'hero card gone after mark-watched',
    );
    assert.equal(readCfg().recents?.length ?? 0, 0, 'mark-watched must drop the recents entry');
    assert.ok(
      (readCfg().hidden_from_continue?.length ?? 0) > 0,
      'mark-watched must tombstone the key',
    );
    await screenshot('02-watched-cleared');

    // Leg 3 — the tombstone must suppress a LIVE server hub copy: simulate
    // a stale/cached or externally-revived Resume entry (played elsewhere)
    // while the leg-2 tombstone is still in place, force a Home refetch,
    // and the hero must stay empty. This is the accepted-edge semantic the
    // plan documents (suppressed until a Vela play) and the mechanism the
    // watched direction relies on against hub lag.
    mock.state.userData.m1 = { played: false, positionTicks: 60_000_000 }; // 6s in ticks
    const beforeRevive = resumeFetches();
    await openLibraryGrid(driver); // navigate away so goHome refetches (hubs settled empty after leg 2)
    await goHome(driver);
    await pollUntil(() => resumeFetches() > beforeRevive, 'a hubs refetch with the revived server copy');
    // The mock now LISTS the movie in Resume. A negative assertion needs a
    // settle window — the request landing precedes the render, so a single
    // immediate check would pass vacuously on broken code. Watch for the
    // hero card for a few seconds and require it to NEVER appear.
    let revived = false;
    await pollUntil(
      () => driver.exec(`return !!document.querySelector('${HERO_CLIP}')`),
      'hero card (must not appear)',
      { timeoutMs: 3000 },
    ).then(
      () => (revived = true),
      () => {}, // timeout = the card never appeared = the suppression held
    );
    assert.equal(
      revived,
      false,
      'tombstoned item must stay out of the hero even when the server hub serves it',
    );
    assert.ok(
      (readCfg().hidden_from_continue?.length ?? 0) > 0,
      'the tombstone must still be in place (no play happened)',
    );
    await screenshot('03-hub-suppressed');
  },
};
