// Bug 3 (owner UX ruling 2026-07-05): clicking a source must never dead-end on
// "Nothing on your home screen yet". Two directions, one scenario:
//   FIX — a local source (no Home hubs, no recents) whose per-source Home is
//     empty but which has library sections lands on its content, not the
//     dead-end.
//   REGRESSION GUARD (codex plan-review r1, finding 3) — a server source that
//     DOES return Home hubs keeps its per-source Home; it is NOT force-browsed.
// Both sources are seeded together so the Sources group renders (needs >1
// source), giving per-source sidebar buttons to click.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { makeClips } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const CLIP = 'E2E Clip One (2021).mp4'; // parses to title "E2E Clip One"

let mock;

export default {
  name: 'sourcedeadend',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, [CLIP]);
    // A non-empty /Items/Latest makes the JF source contribute a "Recently
    // Added" Home hub — the condition the regression guard depends on.
    mock = await startMockJellyfin({
      latest: [{ Id: 'm1', Name: 'Mock Movie', Type: 'Movie', ProductionYear: 2020, RunTimeTicks: 100_000_000 }],
    });
    const configDir = path.join(configRoot, 'config', 'vela');
    fs.mkdirSync(configDir, { recursive: true });
    fs.writeFileSync(
      path.join(configDir, 'config.json'),
      JSON.stringify({
        local_folders: [{ id: 'e2e-local', name: 'E2E Media', path: mediaDir, kind: 'movie' }],
        sources: [
          {
            id: 'jf-mock',
            kind: 'jellyfin',
            name: 'Mock JF',
            base_url: `http://127.0.0.1:${mock.port}`,
            access_token: 'mock-token',
            user_id: mock.userId,
            device_id: 'e2e-device',
          },
        ],
        mpv_extra_args: '--vo=null\n--ao=null',
      }),
    );
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    // Two sources ⇒ the Sources group renders per-source buttons to click.
    // Plain local folders aggregate under one source named "Local" (the E2E
    // Media folder is a *section* under it, local.rs); the JF source is "Mock JF".
    await driver.waitFor(
      `return document.readyState === 'complete'
       && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock JF')
       && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Local')`,
      'both seeded sources in the sidebar',
    );

    // --- Regression guard: the JF source has Home hubs, so clicking it keeps
    // its per-source Home (rail visible) and does NOT force-browse.
    const jf = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Mock JF']`,
    );
    await driver.click(jf);
    await driver.waitFor(
      `return [...document.querySelectorAll('.home section.rail h2')].some(h => h.textContent.includes('Recently Added'))`,
      'the JF per-source Home rail (Recently Added)',
    );
    const jfHome = await driver.exec(`return {
      deadEnd: document.body.innerText.includes('Nothing on your home screen yet'),
      browsed: !!document.querySelector('.crumbs'),
      homeActive: [...document.querySelectorAll('button.sideitem')]
        .some(b => b.classList.contains('active') && b.textContent.trim() === 'Home'),
    }`);
    assert.ok(!jfHome.deadEnd, 'a server source with hubs must not show the dead-end');
    assert.ok(!jfHome.browsed, 'a server source with hubs must NOT be force-browsed (no crumbs)');
    assert.ok(jfHome.homeActive, 'a server source with hubs stays on Home');
    await screenshot('01-jf-home-kept');

    // --- Fix: the local source's per-source Home is empty (no hubs, no
    // recents) but it has a section (E2E Media), so clicking it lands on its
    // content instead of the dead-end.
    const local = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Local']`,
    );
    await driver.click(local);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="E2E Clip"]')`,
      'the local source auto-opened onto its content grid',
    );
    const localView = await driver.exec(`return {
      deadEnd: document.body.innerText.includes('Nothing on your home screen yet'),
      browsed: !!document.querySelector('.crumbs'),
    }`);
    assert.ok(!localView.deadEnd, 'clicking the local source must not dead-end');
    assert.ok(localView.browsed, 'the local source with an empty Home lands on its content (browse view)');
    await screenshot('02-local-autobrowse');

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
      `return !!document.querySelector('button.poster[aria-label^="E2E Clip"]')
       || document.body.innerText.includes('Nothing on your home screen yet')`,
      'the scoped Home to settle (content or dead-end)',
    );
    const afterHome = await driver.exec(`return {
      deadEnd: document.body.innerText.includes('Nothing on your home screen yet'),
      onContent: !!document.querySelector('button.poster[aria-label^="E2E Clip"]'),
    }`);
    assert.ok(!afterHome.deadEnd, 'the Home button on a scoped local source must not dead-end (finding 1)');
    assert.ok(afterHome.onContent, 'the Home button on a scoped local source lands on its content (finding 1)');
    await screenshot('03-home-no-deadend');
  },
};
