// Continue Watching curation: "Remove from Continue Watching" (hero context
// menu) tombstones the item and empties the hero; the tombstone survives an
// app restart; replaying the item clears it and the hero returns. Runs
// against a mock server with no hub content (no Resume/Latest), so the hero
// is fed by Vela's recents alone — exactly the feed the tombstone must
// suppress. Plays go through the context menu's direct Play; the nav-flip
// detail route is the playback scenario's job.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pollUntil, openLibraryGrid, goHome, playAndQuit, makeClips, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const HERO_CLIP = `[aria-label="Continue watching"] [aria-label^="Resume Mock Movie"]`;
const EMPTY_HOME = 'Nothing on your home screen yet';

let mock;

// Lean play-and-quit via the grid card's context-menu Play: get an
// unfinished session into recents; mpv is just a means here.
async function playClipAndQuit(driver) {
  await playAndQuit(driver, async () => {
    await driver.exec(
      `const el = document.querySelector('button.poster[aria-label^="Mock Movie"]');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const play = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, 'context menu (play)')
      .then(() => driver.find('xpath', `(//button[@role='menuitem' and (normalize-space(.)='Resume' or normalize-space(.)='Play')])[1]`));
    await driver.click(play);
  });
}

export default {
  name: 'curation',

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
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const readCfg = () => JSON.parse(fs.readFileSync(configFile, 'utf8'));
    const stampedRecent = () => {
      try {
        return (readCfg().recents?.[0]?.item?.viewOffsetMs ?? 0) > 0;
      } catch {
        return false;
      }
    };

    // An unfinished play lands the movie in the hero.
    await openLibraryGrid(driver);
    await playClipAndQuit(driver);
    await pollUntil(stampedRecent, 'the recents position stamp');
    // Kept for the restart leg: removal will drop this entry, and proving
    // tombstone APPLICATION needs a feed item carrying the hidden key.
    const stampedEntry = readCfg().recents[0];
    await goHome(driver);
    await driver.waitFor(`return !!document.querySelector('${HERO_CLIP}')`, 'movie in the hero');

    // Remove from Continue Watching via the hero's real context menu.
    await driver.exec(
      `const el = document.querySelector('${HERO_CLIP}');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const removeBtn = await driver.waitFor(
      `return !!document.querySelector('.ctxmenu')`,
      'hero context menu',
    ).then(() =>
      driver.find(
        'xpath',
        `//button[@role='menuitem' and normalize-space(.)='Remove from Continue Watching']`,
      ),
    );
    await driver.click(removeBtn);

    // Hero empties (the mock serves no hubs, so a recents-less home falls
    // back to the empty state) and the tombstone is persisted.
    await driver.waitFor(
      `return !document.querySelector('${HERO_CLIP}') && document.body.innerText.includes('${EMPTY_HOME}')`,
      'hero to empty after removal',
    );
    await pollUntil(
      () => (readCfg().hidden_from_continue?.length ?? 0) > 0,
      'the tombstone in config.json',
    );
    await screenshot('01-removed');

    // Restart leg. Reinserting the stamped entry next to the surviving
    // tombstone (app down, so no config-lock race) makes the post-restart
    // assertions depend on tombstone APPLICATION — the hero must suppress a
    // feed item that is actually present (both-feeds suppression,
    // +page.svelte heroItems) — not merely on the entry having been removed.
    const { execSync } = await import('node:child_process');
    // Only THIS scenario's app counts: scope pgrep hits to processes whose
    // environ carries our unique XDG_CONFIG_HOME — a user-launched Vela (or
    // a leaked twin from another scenario) must neither satisfy nor break
    // the restart check.
    const scopedPids = () =>
      execSync('pgrep -x vela || true')
        .toString()
        .trim()
        .split('\n')
        .filter(Boolean)
        .filter((pid) => {
          try {
            return fs
              .readFileSync(`/proc/${pid}/environ`, 'utf8')
              .includes(`XDG_CONFIG_HOME=${path.join(configRoot, 'config')}\0`);
          } catch {
            return false; // process died mid-scan
          }
        });
    const before = scopedPids();
    assert.equal(before.length, 1, `exactly one scenario app before restart, got [${before}]`);
    await restart(() => {
      const cfg = readCfg();
      assert.equal(cfg.recents?.length ?? 0, 0, 'removal should have dropped the recents entry');
      cfg.recents = [stampedEntry];
      fs.writeFileSync(configFile, JSON.stringify(cfg));
    });
    // The old app must be GONE (not merely joined by a new one) and exactly
    // one new instance must own the config.
    const after = await pollUntil(() => {
      const pids = scopedPids();
      return pids.length === 1 && pids[0] !== before[0] ? pids : null;
    }, `the old app (${before[0]}) to exit and exactly one new instance to run`);
    assert.notEqual(after[0], before[0]);
    await driver.waitFor(
      `return document.readyState === 'complete' && document.body.innerText.includes('${EMPTY_HOME}')`,
      'empty home after restart',
    );
    const heroAfterRestart = await driver.exec(`return !!document.querySelector('${HERO_CLIP}')`);
    assert.equal(heroAfterRestart, false, 'removed item must stay out of the hero across restart');
    assert.ok((readCfg().hidden_from_continue?.length ?? 0) > 0, 'tombstone must survive restart');

    // Replaying the item clears the tombstone and the hero returns.
    await openLibraryGrid(driver);
    await playClipAndQuit(driver);
    await pollUntil(
      () => (readCfg().hidden_from_continue?.length ?? 0) === 0,
      'the tombstone to clear on replay',
    );
    await goHome(driver);
    await driver.waitFor(`return !!document.querySelector('${HERO_CLIP}')`, 'movie back in the hero');
    await screenshot('02-restored');
  },
};
