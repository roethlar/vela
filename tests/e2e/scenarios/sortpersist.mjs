// Per-library sort persistence and independent direction: changing the field
// preserves direction, the arrow toggles only direction, both dimensions reach
// Jellyfin and config, and the complete token is restored on the first listing
// request after an app restart.
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
        .map((r) => ({ by: r.query.SortBy, order: r.query.SortOrder }));
    const sortUi = (expectedField, expectedLabel) =>
      driver.waitFor(
        `const field = document.querySelector('select[aria-label="Sort by"]');
         const direction = document.querySelector('button[aria-label^="Sort direction:"]');
         if (!field || !direction) return null;
         const state = {
           field: field?.value ?? null,
           arrow: direction?.textContent.trim() ?? null,
           label: direction?.getAttribute('aria-label') ?? null,
           title: direction?.getAttribute('title') ?? null,
         };
         return state.field === ${JSON.stringify(expectedField)}
           && state.label === ${JSON.stringify(expectedLabel)}
           ? state
           : null;`,
        `${expectedField} with ${expectedLabel}`,
      );

    // The default is title ascending, represented independently by the field
    // selector and accessible arrow button.
    await openLibraryGrid(driver);
    const ascendingLabel = 'Sort direction: ascending; activate for descending';
    const descendingLabel = 'Sort direction: descending; activate for ascending';
    assert.deepEqual(await sortUi('titleSort', ascendingLabel), {
      field: 'titleSort',
      arrow: '↑',
      label: ascendingLabel,
      title: ascendingLabel,
    });

    // Change only the field. The current ascending direction must survive and
    // Jellyfin must receive both mapped query dimensions.
    const beforeField = listingSorts().length;
    await driver.exec(
      `const el = document.querySelector('select[aria-label="Sort by"]');
       el.value = 'year';
       el.dispatchEvent(new Event('change', { bubbles: true }));`,
    );
    const ascendingFieldRequest = await pollUntil(
      () =>
        listingSorts()
          .slice(beforeField)
          .find((request) => request.by.includes('ProductionYear')),
      'an ascending ProductionYear listing request after the field change',
    );
    assert.deepEqual(ascendingFieldRequest, {
      by: 'ProductionYear,PremiereDate',
      order: 'Ascending',
    });
    assert.deepEqual(await sortUi('year', ascendingLabel), {
      field: 'year',
      arrow: '↑',
      label: ascendingLabel,
      title: ascendingLabel,
    });

    // Toggle to descending. The field stays fixed while the visible arrow,
    // accessible state, and Jellyfin SortOrder all change together.
    const beforeDescending = listingSorts().length;
    await driver.click(
      await driver.find('css selector', `button[aria-label="${ascendingLabel}"]`),
    );
    const descendingRequest = await pollUntil(
      () =>
        listingSorts()
          .slice(beforeDescending)
          .find(
            (request) =>
              request.by.includes('ProductionYear') && request.order === 'Descending',
          ),
      'a descending ProductionYear listing request after the arrow toggle',
    );
    assert.deepEqual(descendingRequest, {
      by: 'ProductionYear,PremiereDate',
      order: 'Descending',
    });
    assert.deepEqual(await sortUi('year', descendingLabel), {
      field: 'year',
      arrow: '↓',
      label: descendingLabel,
      title: descendingLabel,
    });

    // Toggle back to ascending and persist the exact complete token. Count
    // requests from this click so the earlier ascending field-change request
    // cannot satisfy the assertion.
    const beforeAscending = listingSorts().length;
    await driver.click(
      await driver.find('css selector', `button[aria-label="${descendingLabel}"]`),
    );
    await pollUntil(
      () =>
        listingSorts()
          .slice(beforeAscending)
          .some(
            (request) =>
              request.by.includes('ProductionYear') && request.order === 'Ascending',
          ),
      'a new ascending ProductionYear listing request after the second arrow toggle',
    );
    await pollUntil(() => {
      try {
        const sorts = readCfg().section_sorts ?? {};
        return Object.values(sorts).includes('year:asc');
      } catch {
        return false;
      }
    }, 'the ascending section_sorts token in config.json');
    assert.deepEqual(await sortUi('year', ascendingLabel), {
      field: 'year',
      arrow: '↑',
      label: ascendingLabel,
      title: ascendingLabel,
    });
    await screenshot('01-field-and-direction');

    // Restart: after reopening the section, the FIRST listing request and both
    // controls must already carry the persisted ascending year token.
    await restart();
    mock.state.requests.length = 0;
    await openLibraryGrid(driver);
    const firstSort = await pollUntil(
      () => listingSorts()[0] ?? null,
      'the post-restart listing request',
    );
    assert.deepEqual(
      firstSort,
      { by: 'ProductionYear,PremiereDate', order: 'Ascending' },
      'the first post-restart listing must use the persisted field and direction',
    );
    assert.deepEqual(await sortUi('year', ascendingLabel), {
      field: 'year',
      arrow: '↑',
      label: ascendingLabel,
      title: ascendingLabel,
    });
    await screenshot('02-persisted-after-restart');
  },
};
