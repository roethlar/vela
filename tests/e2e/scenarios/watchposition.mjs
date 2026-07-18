// A successful watch edit revalidates the server-authoritative library without
// tearing down the mounted grid, losing pagination depth, or resetting the
// user's scroll position. The same preserved snapshot must survive a failed
// revalidation, and a delayed buffer from an old library must never publish
// over a library selected while it was in flight.
import assert from "node:assert/strict";
import {
  holdsFor,
  mockSource,
  openLibraryGrid,
  pollUntil,
  seedConfig,
} from "../helpers.mjs";
import { startMockJellyfin } from "../mockjf.mjs";

const PAGE = 60;
const BIG_MOVIES = Array.from({ length: 130 }, (_, i) => ({
  id: `position-${i}`,
  name: `Position ${String(i).padStart(3, "0")}`,
  year: 2000 + (i % 20),
}));
const OTHER_MOVIES = Array.from({ length: 7 }, (_, i) => ({
  id: `other-${i}`,
  name: `Other ${String(i).padStart(3, "0")}`,
  year: 2020 + i,
}));
const BIG_TITLES = BIG_MOVIES.map((movie) => movie.name);
const OTHER_TITLES = OTHER_MOVIES.map((movie) => movie.name);

let mock;

const itemsPath = () => `/Users/${mock.userId}/Items`;
const listingRequests = (parentId) =>
  mock.state.requests.filter(
    (request) =>
      request.method === "GET" &&
      request.path === itemsPath() &&
      request.query.ParentId === parentId &&
      request.query.searchTerm === undefined,
  );
const listingOffsetsSince = (parentId, before) =>
  listingRequests(parentId)
    .slice(before)
    .map((request) => Number(request.query.StartIndex));
const itemsServed = (status = null) =>
  mock.state.served.filter(
    (response) =>
      response.method === "GET" &&
      response.path === itemsPath() &&
      (status === null || response.status === status),
  ).length;

async function gridState(driver, target = null) {
  return driver.exec(`
    const grid = document.querySelector('main.grid');
    const target = ${JSON.stringify(target)};
    const card = target
      ? [...document.querySelectorAll('main.grid button.poster')]
          .find((button) => button.title === target)
      : null;
    return {
      titles: [...document.querySelectorAll('main.grid button.poster')]
        .map((button) => button.title),
      scrollTop: grid?.scrollTop ?? null,
      scrollHeight: grid?.scrollHeight ?? null,
      clientHeight: grid?.clientHeight ?? null,
      watched: target ? !!card?.querySelector('.watchedbadge') : null,
      viewError: document.querySelector('div.error')?.textContent ?? null,
      editError: [...document.querySelectorAll('div.scanerror')]
        .map((element) => element.textContent).join(' | ') || null,
    };
  `);
}

function assertExactTitles(actual, expected, where) {
  assert.deepEqual(actual, expected, `${where}: exact mounted card set changed`);
}

async function openExactLibrary(driver, { section, prefix, expected }) {
  await openLibraryGrid(driver, { section, cardPrefix: prefix });
  await driver.waitFor(
    `const expected = ${JSON.stringify(expected)};
     const actual = [...document.querySelectorAll('main.grid button.poster')]
       .map((button) => button.title);
     return JSON.stringify(actual) === JSON.stringify(expected);`,
    `${section} exact card set`,
  );
}

async function scrollGridToEnd(driver) {
  await driver.waitFor(`return !!document.querySelector('main.grid')`, "the browse grid");
  await driver.exec(`
    const grid = document.querySelector('main.grid');
    grid.scrollTop = grid.scrollHeight;
    grid.dispatchEvent(new Event('scroll'));
  `);
}

async function loadTwoPages(driver) {
  await openExactLibrary(driver, {
    section: "Position Library",
    prefix: BIG_TITLES[0],
    expected: BIG_TITLES.slice(0, PAGE),
  });
  const before = listingRequests("libPosition").length;
  await scrollGridToEnd(driver);
  await driver.waitFor(
    `return document.querySelectorAll('main.grid button.poster').length === ${PAGE * 2}`,
    "the second full library page",
  );
  assert.deepEqual(
    listingOffsetsSince("libPosition", before),
    [PAGE],
    "loading the second page must request offset 60 exactly once",
  );
  const state = await gridState(driver);
  assertExactTitles(state.titles, BIG_TITLES.slice(0, PAGE * 2), "after page two");
}

async function setStableScroll(driver) {
  const positioned = await driver.exec(`
    const grid = document.querySelector('main.grid');
    if (!grid) throw new Error('browse grid is missing');
    const max = grid.scrollHeight - grid.clientHeight;
    const requested = Math.min(777, max - 800);
    if (requested <= 0) {
      return { scrollTop: grid.scrollTop, max, distanceFromEnd: max - grid.scrollTop };
    }
    grid.scrollTop = requested;
    return {
      scrollTop: grid.scrollTop,
      max,
      distanceFromEnd: max - grid.scrollTop,
    };
  `);
  assert.ok(positioned.scrollTop > 0, "the position guard requires nonzero grid scroll");
  assert.ok(
    positioned.distanceFromEnd >= 700,
    `the position guard must stay clear of infinite-scroll threshold; got ${JSON.stringify(positioned)}`,
  );
  return positioned.scrollTop;
}

// Keep a browser-side observer alive across every WebDriver round trip. A
// one-off sample after the delayed response cannot distinguish a continuously
// mounted grid from an empty/remounted grid that happened to recover in time.
async function installGridProbe(driver, expectedTitles, expectedScroll) {
  await stopGridProbe(driver);
  await driver.exec(`
    (() => {
      const expectedTitles = ${JSON.stringify(expectedTitles)};
      const expectedScroll = ${JSON.stringify(expectedScroll)};
      const grid = document.querySelector('main.grid');
      if (!grid) throw new Error('cannot install grid probe without a grid');
      const probe = {
        grid,
        expectedTitles,
        expectedScroll,
        violations: [],
        observer: null,
        interval: null,
      };
      const sample = () => {
        const current = document.querySelector('main.grid');
        const titles = [...document.querySelectorAll('main.grid button.poster')]
          .map((button) => button.title);
        let failure = null;
        if (!probe.grid.isConnected || current !== probe.grid) {
          failure = 'the browse grid was removed or replaced';
        } else if (JSON.stringify(titles) !== JSON.stringify(probe.expectedTitles)) {
          failure = 'the exact mounted card set changed';
        } else if (current.scrollTop !== probe.expectedScroll) {
          failure = 'scrollTop changed from ' + probe.expectedScroll + ' to ' + current.scrollTop;
        }
        if (failure && !probe.violations.includes(failure)) probe.violations.push(failure);
      };
      probe.observer = new MutationObserver(sample);
      probe.observer.observe(document.body, { childList: true, subtree: true });
      probe.interval = window.setInterval(sample, 25);
      grid.addEventListener('scroll', sample, { passive: true });
      probe.sample = sample;
      probe.removeScroll = () => grid.removeEventListener('scroll', sample);
      window.__velaWatchPositionProbe = probe;
      sample();
    })();
  `);
}

async function probeState(driver) {
  return driver.exec(`
    const probe = window.__velaWatchPositionProbe;
    if (!probe) return null;
    probe.sample();
    return { violations: [...probe.violations] };
  `);
}

async function stopGridProbe(driver) {
  await driver.exec(`
    (() => {
      const probe = window.__velaWatchPositionProbe;
      if (!probe) return;
      probe.observer?.disconnect();
      if (probe.interval != null) window.clearInterval(probe.interval);
      probe.removeScroll?.();
      delete window.__velaWatchPositionProbe;
    })();
  `);
}

async function assertProbeClean(driver, where) {
  const probe = await probeState(driver);
  assert.ok(probe, `${where}: grid probe disappeared`);
  assert.deepEqual(probe.violations, [], `${where}: grid continuity failed`);
}

async function watchToggle(driver, title, label = "Mark watched") {
  await driver.exec(`
    const card = [...document.querySelectorAll('main.grid button.poster')]
      .find((button) => button.title === ${JSON.stringify(title)});
    if (!card) throw new Error('missing card: ' + ${JSON.stringify(title)});
    const rect = card.getBoundingClientRect();
    card.dispatchEvent(new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: rect.x + rect.width / 2,
      clientY: rect.y + rect.height / 2,
    }));
  `);
  const action = await driver
    .waitFor(`return !!document.querySelector('.ctxmenu')`, `context menu for ${title}`)
    .then(() =>
      driver.find(
        "xpath",
        `//button[@role='menuitem' and normalize-space(.)='${label}']`,
      ),
    );
  await driver.click(action);
}

async function waitForPlayed(itemId) {
  await pollUntil(
    () =>
      mock.state.playedServed.some(
        (response) => response.itemId === itemId && response.status === 200,
      )
        ? true
        : null,
    `the successful PlayedItems response for ${itemId}`,
  );
}

async function waitForPageThree(driver, before) {
  // The second buffered response is a server-dispatch witness, not proof its
  // frontend finally block has released loadingMore. Re-dispatch the user's
  // end-scroll until the request arrives; once it does, exact offset evidence
  // below still fails any duplicate or restarted continuation.
  await pollUntil(
    async () => {
      await scrollGridToEnd(driver);
      return listingRequests("libPosition").length > before ? true : null;
    },
    "the preserved listing to accept page-three pagination",
  );
  await driver.waitFor(
    `return document.querySelectorAll('main.grid button.poster').length === ${BIG_MOVIES.length}`,
    "the final ten-title page",
  );
  assert.deepEqual(
    listingOffsetsSince("libPosition", before),
    [PAGE * 2],
    "the preserved listing must continue at offset 120",
  );
  const final = await gridState(driver);
  assertExactTitles(final.titles, BIG_TITLES, "after page-three continuation");
}

export default {
  name: "watchposition",

  async seed({ configRoot }) {
    mock = await startMockJellyfin({
      views: [
        {
          id: "libPosition",
          name: "Position Library",
          collectionType: "movies",
          movies: BIG_MOVIES,
        },
        {
          id: "libOther",
          name: "Other Library",
          collectionType: "movies",
          movies: OTHER_MOVIES,
        },
      ],
      latest: [],
    });
    seedConfig(configRoot, [
      mockSource(mock, { id: "jf-position", name: "Position Mock" }),
    ]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver }) {
    try {
      // ── 1. Delayed successful revalidation keeps the 120-card grid live ──
      await loadTwoPages(driver);
      const successTarget = BIG_MOVIES[89];
      const successScroll = await setStableScroll(driver);
      const successTitles = BIG_TITLES.slice(0, PAGE * 2);
      const successBefore = listingRequests("libPosition").length;
      const successServedBefore = itemsServed();
      await installGridProbe(driver, successTitles, successScroll);

      mock.state.itemsDelayMs = 4000;
      await watchToggle(driver, successTarget.name);
      await waitForPlayed(successTarget.id);
      await pollUntil(
        () =>
          listingRequests("libPosition").length === successBefore + 1 &&
          mock.state.itemsDelayMs === 0
            ? true
            : null,
        "the delayed offset-zero revalidation request",
      );
      assert.deepEqual(
        listingOffsetsSince("libPosition", successBefore),
        [0],
        "revalidation must start at offset zero and wait before requesting offset 60",
      );
      await driver.waitFor(
        `return !![...document.querySelectorAll('main.grid button.poster')]
          .find((button) => button.title === ${JSON.stringify(successTarget.name)})
          ?.querySelector('.watchedbadge')`,
        "the confirmed local watched badge while revalidation is parked",
      );
      assert.equal(
        itemsServed(),
        successServedBefore,
        "the continuity window must run while offset zero is still parked",
      );
      await holdsFor(
        async () => {
          const probe = await probeState(driver);
          return probe?.violations[0] ?? false;
        },
        1000,
        "the exact grid, depth, and scroll during delayed successful revalidation",
      );
      await pollUntil(
        () =>
          listingRequests("libPosition").length === successBefore + 2 &&
          itemsServed() >= successServedBefore + 2
            ? true
            : null,
        "both successful buffered revalidation pages",
      );
      assert.deepEqual(
        listingOffsetsSince("libPosition", successBefore),
        [0, PAGE],
        "the 120-card snapshot must rebuild from offsets 0 then 60",
      );
      await holdsFor(
        async () => {
          const probe = await probeState(driver);
          return probe?.violations[0] ?? false;
        },
        300,
        "the published successful buffer",
      );
      await assertProbeClean(driver, "successful revalidation");
      const success = await gridState(driver, successTarget.name);
      assertExactTitles(success.titles, successTitles, "after successful revalidation");
      assert.equal(success.scrollTop, successScroll, "successful revalidation restores scrollTop");
      assert.equal(success.watched, true, "server-authoritative publication keeps the watched badge");
      assert.equal(success.viewError, null, "successful revalidation leaves no view error");
      assert.equal(success.editError, null, "successful edit leaves no edit error");
      await stopGridProbe(driver);

      const successPageThreeBefore = listingRequests("libPosition").length;
      await waitForPageThree(driver, successPageThreeBefore);

      // ── 2. A failed revalidation keeps confirmed local state and paging ──
      await openExactLibrary(driver, {
        section: "Other Library",
        prefix: OTHER_TITLES[0],
        expected: OTHER_TITLES,
      });
      await loadTwoPages(driver);
      const failureTarget = BIG_MOVIES[99];
      const failureScroll = await setStableScroll(driver);
      const failureTitles = BIG_TITLES.slice(0, PAGE * 2);
      const failureStart = await gridState(driver);
      assert.equal(failureStart.viewError, null, "failed-revalidation leg starts healthy");
      assert.equal(failureStart.editError, null, "failed-revalidation leg starts without an edit error");
      const failureBefore = listingRequests("libPosition").length;
      const failedItemsServedBefore = itemsServed(500);
      await installGridProbe(driver, failureTitles, failureScroll);

      mock.state.failNextItems = true;
      await watchToggle(driver, failureTarget.name);
      await waitForPlayed(failureTarget.id);
      await pollUntil(
        () =>
          listingRequests("libPosition").length === failureBefore + 1 &&
          itemsServed(500) === failedItemsServedBefore + 1
            ? true
            : null,
        "the failed offset-zero revalidation response",
      );
      assert.deepEqual(
        listingOffsetsSince("libPosition", failureBefore),
        [0],
        "a failed first revalidation page must not request offset 60",
      );
      await driver.waitFor(
        `return !!document.querySelector('div.error')`,
        "the revalidation failure on the view banner",
      );
      await assertProbeClean(driver, "failed revalidation");
      const failed = await gridState(driver, failureTarget.name);
      assertExactTitles(failed.titles, failureTitles, "after failed revalidation");
      assert.equal(failed.scrollTop, failureScroll, "failed revalidation preserves scrollTop");
      assert.equal(failed.watched, true, "failed revalidation retains the confirmed local badge");
      assert.ok(failed.viewError, "failed revalidation reports on the view banner");
      assert.equal(failed.editError, null, "a successful edit must not report a false edit failure");
      assert.equal(mock.state.userData[failureTarget.id].played, true, "the edit itself succeeded");
      await stopGridProbe(driver);

      const failurePageThreeBefore = listingRequests("libPosition").length;
      await waitForPageThree(driver, failurePageThreeBefore);

      // ── 3. Navigation supersedes a delayed buffer from the old library ──
      await openExactLibrary(driver, {
        section: "Other Library",
        prefix: OTHER_TITLES[0],
        expected: OTHER_TITLES,
      });
      await loadTwoPages(driver);
      const staleTarget = BIG_MOVIES[109];
      const staleScroll = await setStableScroll(driver);
      const staleStart = await gridState(driver);
      assert.equal(staleStart.viewError, null, "stale-root leg starts healthy");
      assert.equal(staleStart.editError, null, "stale-root leg starts without an edit error");
      const staleBefore = listingRequests("libPosition").length;
      const staleServedBefore = itemsServed();
      await installGridProbe(driver, BIG_TITLES.slice(0, PAGE * 2), staleScroll);

      mock.state.itemsDelayMs = 6000;
      await watchToggle(driver, staleTarget.name);
      await waitForPlayed(staleTarget.id);
      await pollUntil(
        () =>
          listingRequests("libPosition").length === staleBefore + 1 &&
          mock.state.itemsDelayMs === 0
            ? true
            : null,
        "the old library's delayed offset-zero buffer",
      );
      assert.equal(
        itemsServed(),
        staleServedBefore,
        "the old-root response must still be parked before navigation",
      );
      await holdsFor(
        async () => {
          const probe = await probeState(driver);
          return probe?.violations[0] ?? false;
        },
        500,
        "the old library before navigation",
      );
      await assertProbeClean(driver, "old root before navigation");
      await stopGridProbe(driver);

      await openExactLibrary(driver, {
        section: "Other Library",
        prefix: OTHER_TITLES[0],
        expected: OTHER_TITLES,
      });
      const destinationServed = itemsServed();
      assert.equal(
        destinationServed,
        staleServedBefore + 1,
        "the destination must render while the old-root response is still parked",
      );
      const destination = await gridState(driver);
      assert.equal(destination.viewError, null, "navigation clears the prior library failure");
      await installGridProbe(driver, OTHER_TITLES, destination.scrollTop);

      await pollUntil(
        () => (itemsServed() > destinationServed ? true : null),
        "the delayed old-root response to be served",
        { timeoutMs: 10000 },
      );
      await holdsFor(
        async () => {
          const probe = await probeState(driver);
          return probe?.violations[0] ?? false;
        },
        1500,
        "the destination after the old-root buffer settles",
      );
      await assertProbeClean(driver, "destination after stale work settled");
      assert.deepEqual(
        listingOffsetsSince("libPosition", staleBefore),
        [0],
        "stale old-root work must stop after its first await",
      );
      const final = await gridState(driver);
      assertExactTitles(final.titles, OTHER_TITLES, "after stale old-root settlement");
      assert.equal(final.viewError, null, "stale old-root work cannot publish a destination error");
      assert.equal(final.editError, null, "the successful old-root edit leaves no edit failure");
      await stopGridProbe(driver);

      assert.deepEqual(
        mock.state.contractViolations,
        [],
        "watch-position requests must preserve the Jellyfin query contract",
      );
    } finally {
      await stopGridProbe(driver).catch(() => {});
    }
  },
};
