// Pagination must survive a refresh-silenced page failure (r8-4 — declined once
// by the author, OVERTURNED on independent re-adjudication).
//
// The refresh silences a listing load already in flight, because it means to
// REPLACE that grid. If the user opens a detail while the action waits on its
// sections leg, `navEpoch` moves: the content leg early-returns without ever
// claiming the grid, and settlement drops the held failure as belonging to a view
// the user has left. So nothing replaced the page, nothing reported it — and if
// the failure ALSO killed `hasMore`, the user presses Back to a library that is
// truncated, cannot load more, and says nothing about why. Silent, permanent, and
// invisible.
//
// This needs a library big enough to PAGE (the app loads 60 at a time), which no
// other scenario has — hence its own mock rather than disturbing refresh.mjs's
// 28 cases.
import assert from "node:assert/strict";
import { pollUntil, mockSource, seedConfig } from "../helpers.mjs";
import { startMockJellyfin } from "../mockjf.mjs";

let mock;

const MOVIES = Array.from({ length: 65 }, (_, i) => ({
  id: `m${i}`,
  name: `Movie ${String(i).padStart(3, "0")}`,
  year: 2000 + (i % 20),
}));

async function cardCount(driver) {
  return driver.exec(
    `return document.querySelectorAll('button.poster').length`,
  );
}
async function banner(driver) {
  return driver.exec(
    `return document.querySelector('div.error')?.textContent ?? null`,
  );
}
async function settle(driver) {
  await driver.waitFor(
    `const b = document.querySelector('button.refreshbtn'); return !!b && !b.disabled`,
    "refresh to settle",
  );
}
// The grid loads the next page when scrolled near its end (onScroll -> loadMore).
async function scrollGridToEnd(driver) {
  await driver.exec(
    `const g = document.querySelector('main.grid');
     g.scrollTop = g.scrollHeight;
     g.dispatchEvent(new Event('scroll'));`,
  );
}

export default {
  name: "pagefail",

  async seed({ configRoot }) {
    mock = await startMockJellyfin({
      views: [
        {
          id: "libBig",
          name: "Big Library",
          collectionType: "movies",
          movies: MOVIES,
        },
      ],
      latest: [],
    });
    seedConfig(configRoot, [mockSource(mock, { id: "jf-big", name: "Mock JF" })]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run(driver) {
    // Page 1: the app asks for 60 and gets 60, so it knows there is more.
    const side = await driver.find(
      "xpath",
      `//button[contains(@class,'sideitem') and normalize-space(.)='Big Library']`,
    );
    await driver.click(side);
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "the first full page of 60",
    );

    // Page 2 is in flight and doomed, and the refresh will silence it.
    mock.state.itemsDelayMs = 700; // still loading when we click Refresh...
    mock.state.failNextItems = true; // ...and it dies
    mock.state.viewsDelayMs = 2500; // the action's sections leg lands much later
    await scrollGridToEnd(driver);
    const refresh = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh);

    // The user opens a detail while the action is still waiting on sections. That
    // is navigation: the content leg will never claim the grid, and settlement
    // will (correctly) drop the failure it was holding for a view the user left.
    const card = await driver.find(
      "css selector",
      `button.poster[aria-label^="Movie 000"]`,
    );
    await driver.click(card);
    await driver.waitFor(
      `return !!document.querySelector('.detail')`,
      "detail surface open",
    );
    await settle(driver);
    mock.state.itemsDelayMs = 0;
    mock.state.viewsDelayMs = 0;

    // Back to the library. Nothing replaced page 2 and nothing reported it — so
    // the library MUST still be able to load it. Killing `hasMore` on the
    // silenced failure is what made that impossible, silently and permanently.
    const back = await driver.find("css selector", ".detail button.back, .crumbs button.back");
    await driver.click(back);
    await driver.waitFor(
      `return !document.querySelector('.detail')`,
      "back on the grid",
    );
    assert.equal(
      await cardCount(driver),
      60,
      "precondition: back on the truncated grid, page 2 never arrived",
    );

    await scrollGridToEnd(driver);
    await pollUntil(
      async () => ((await cardCount(driver)) > 60 ? true : null),
      "the library must still be able to load the page the refresh silenced but never replaced — otherwise it is truncated forever, with no banner to say why",
    );

    assert.equal(
      mock.state.contractViolations.length,
      0,
      `mock contract clean — got ${JSON.stringify(mock.state.contractViolations)}`,
    );
  },
};
