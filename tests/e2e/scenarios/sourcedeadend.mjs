// Bug 3 (owner UX ruling 2026-07-05): clicking a source must never dead-end on
// the designed empty-Home state. Two directions, one scenario, both legs
// now server sources (the original local leg died with the local sources):
//   FIX — a server whose per-source Home settles empty (no Resume/Latest
//     hubs, no recents) but which has library sections lands on its content,
//     not the dead-end.
//   REGRESSION GUARD (codex plan-review r1, finding 3) — a server source that
//     DOES return Home hubs keeps its per-source Home; it is NOT force-browsed.
// Both sources are seeded together so the Sources group renders (needs >1
// source), giving per-source sidebar buttons to click.
import assert from 'node:assert/strict';
import { mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mockHubs;
let mockEmpty;

export default {
  name: 'sourcedeadend',

  async seed({ configRoot }) {
    // A non-empty /Items/Latest makes the first server contribute a
    // "Recently Added" Home hub — the condition the regression guard
    // depends on. The second serves a library but no hub content, so its
    // per-source Home settles empty.
    mockHubs = await startMockJellyfin({
      latest: [{ Id: 'm1', Name: 'Mock Movie', Type: 'Movie', ProductionYear: 2020, RunTimeTicks: 100_000_000 }],
    });
    mockEmpty = await startMockJellyfin({
      movies: [{ id: 'e1', name: 'Empty Home Movie', year: 2021 }],
    });
    seedConfig(configRoot, [
      mockSource(mockHubs, { id: 'jf-hubs', name: 'Mock Hubs' }),
      mockSource(mockEmpty, { id: 'jf-empty', name: 'Mock Empty' }),
    ]);
  },

  async cleanup() {
    await mockHubs?.close();
    await mockEmpty?.close();
  },

  async run({ driver, screenshot }) {
    // Two sources ⇒ the Sources group renders per-source buttons to click.
    await driver.waitFor(
      `return document.readyState === 'complete'
       && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock Hubs')
       && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock Empty')`,
      'both seeded sources in the sidebar',
    );

    // --- Regression guard: the hub-serving source keeps its per-source Home
    // (rail visible) and does NOT force-browse.
    const jf = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Mock Hubs']`,
    );
    await driver.click(jf);
    await driver.waitFor(
      `return [...document.querySelectorAll('.home section.rail h2')].some(h => h.textContent.includes('Recently Added'))`,
      'the hub-serving per-source Home rail (Recently Added)',
    );
    const jfHome = await driver.exec(`return {
      deadEnd: document.body.innerText.includes('No titles on Home yet'),
      browsed: !!document.querySelector('.crumbs'),
      homeActive: [...document.querySelectorAll('button.sideitem')]
        .some(b => b.classList.contains('active') && b.textContent.trim() === 'Home'),
    }`);
    assert.ok(!jfHome.deadEnd, 'a server source with hubs must not show the dead-end');
    assert.ok(!jfHome.browsed, 'a server source with hubs must NOT be force-browsed (no crumbs)');
    assert.ok(jfHome.homeActive, 'a server source with hubs stays on Home');
    await screenshot('01-hubs-home-kept');

    // --- Fix: the hub-less source's per-source Home settles empty (no hubs,
    // no recents) but it has a section (Mock Library), so clicking it lands
    // on its content instead of the dead-end.
    const empty = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Mock Empty']`,
    );
    await driver.click(empty);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Empty Home Movie"]')`,
      'the hub-less source auto-opened onto its content grid',
    );
    const emptyView = await driver.exec(`return {
      deadEnd: document.body.innerText.includes('No titles on Home yet'),
      browsed: !!document.querySelector('.crumbs'),
    }`);
    assert.ok(!emptyView.deadEnd, 'clicking the hub-less source must not dead-end');
    assert.ok(emptyView.browsed, 'the hub-less source with an empty Home lands on its content (browse view)');
    await screenshot('02-empty-autobrowse');

    // --- Finding 1 (codex r1): reaching the empty scoped Home via the Home
    // button (the same goHome() path Back uses from a top-level section) must
    // ALSO land on content, not the dead-end. The routing is reactive, not a
    // tail of selectSource — pre-fixup this dead-ended and re-clicking the
    // source early-returned (a trap the user couldn't click out of).
    const homeBtn = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Home']`,
    );
    await driver.click(homeBtn);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Empty Home Movie"]')
       || document.body.innerText.includes('No titles on Home yet')`,
      'the scoped Home to settle (content or dead-end)',
    );
    const afterHome = await driver.exec(`return {
      deadEnd: document.body.innerText.includes('No titles on Home yet'),
      onContent: !!document.querySelector('button.poster[aria-label^="Empty Home Movie"]'),
    }`);
    assert.ok(!afterHome.deadEnd, 'the Home button on a scoped hub-less source must not dead-end (finding 1)');
    assert.ok(afterHome.onContent, 'the Home button on a scoped hub-less source lands on its content (finding 1)');
    await screenshot('03-home-no-deadend');
  },
};
