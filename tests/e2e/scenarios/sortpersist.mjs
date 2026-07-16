// Per-library sort persistence (owner ask 2026-07-10, "sort should stick per
// library"): changing a library's sort must (a) drive the server query with
// the mapped SortBy, (b) persist to config (section_sorts), and (c) come back
// as the library's sort after an app RESTART — the fresh listing request must
// already carry the persisted SortBy, not the default SortName.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pollUntil, openLibraryGrid, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock;

export default {
  name: 'sortpersist',

  async seed({ configRoot }) {
    mock = await startMockJellyfin(); // default single movie, no stream needed
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const readCfg = () => JSON.parse(fs.readFileSync(configFile, 'utf8'));
    const listingSorts = () =>
      mock.state.requests
        .filter(
          (r) =>
            r.method === 'GET' &&
            r.path === `/Users/${mock.userId}/Items` &&
            r.query.ParentId &&
            r.query.SortBy,
        )
        .map((r) => r.query.SortBy);

    // Open the library (default sort) and switch to "Year (newest)".
    await openLibraryGrid(driver);
    await driver.exec(
      `const el = document.querySelector('select.sort');
       el.value = 'year:desc';
       el.dispatchEvent(new Event('change', { bubbles: true }));`,
    );
    // The re-sorted listing must reach the mock with the mapped SortBy…
    await pollUntil(
      () => listingSorts().some((s) => s.includes('ProductionYear')),
      'a ProductionYear-sorted listing request',
    );
    // …and the choice must persist to config for this section.
    await pollUntil(() => {
      try {
        const sorts = readCfg().section_sorts ?? {};
        return Object.values(sorts).includes('year:desc');
      } catch {
        return false;
      }
    }, 'the section_sorts entry in config.json');
    await screenshot('01-sorted');

    // Restart: the library must come back on the persisted sort. Gate on the
    // request evidence (the markwatched eh-15 discipline): after reopening
    // the section, the FIRST sorted listing must already be ProductionYear —
    // a regression to the default would send SortName instead.
    await restart();
    mock.state.requests.length = 0;
    await openLibraryGrid(driver);
    const firstSort = await pollUntil(
      () => listingSorts()[0] ?? null,
      'the post-restart listing request',
    );
    assert.ok(
      firstSort.includes('ProductionYear'),
      `post-restart listing must use the persisted sort, got SortBy=${firstSort}`,
    );
    const selectValue = await driver.exec(
      `return document.querySelector('select.sort')?.value ?? '(no select)'`,
    );
    assert.equal(selectValue, 'year:desc', 'the sort select must show the persisted choice');
    await screenshot('02-persisted-after-restart');
  },
};
