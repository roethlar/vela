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
import {
  pollUntil,
  mockSource,
  seedConfig,
  openLibraryGrid,
} from "../helpers.mjs";
import { startMockJellyfin } from "../mockjf.mjs";

let mock;

// 130, not 65: page 2 must come back FULL (60) so the library still has a page 3
// for case 2 to fail. With 65, page 2 returns 5, `hasMore` goes false, and a scroll
// can no longer ask for anything.
const MOVIES = Array.from({ length: 130 }, (_, i) => ({
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
// Toggle a card's watch state from its context menu (refresh.mjs:162). Used here
// as a NON-listing writer to the shared error banner.
async function watchToggle(driver, prefix, label) {
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^="${prefix}"]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  const item = await driver
    .waitFor(`return !!document.querySelector('.ctxmenu')`, "context menu")
    .then(() =>
      driver.find(
        "xpath",
        `//button[@role='menuitem' and normalize-space(.)='${label}']`,
      ),
    );
  await driver.click(item);
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

  async run({ driver }) {
    // Page 1: the app asks for 60 and gets 60, so it knows there is more.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "the first full page of 60",
    );

    // Refresh FIRST, and make its sections leg slow: for the whole of that window
    // the action is live and owns the banner, so any listing failure is SILENCED.
    // (Order matters. If the detail opened first, the epoch would already have
    // moved and the failure would take the ordinary publish path — a different,
    // already-correct case.)
    mock.state.viewsDelayMs = 2500;
    const refresh = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh);

    // Now page 2 is requested, and dies — silently, because the action expects to
    // replace it.
    mock.state.failNextItems = true;
    await scrollGridToEnd(driver);
    await pollUntil(
      async () => (mock.state.failNextItems === false ? true : null),
      "the doomed page-2 request must actually reach the server",
    );
    assert.equal(
      await banner(driver),
      null,
      "precondition: the action silenced it — nothing on screen says the page failed",
    );

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

    // ── 2. The refresh may only retract a banner it SUPERSEDED ─────────
    // `setError` clears the generation tag on every NON-listing write, so a
    // gen-scoped retraction cannot take down a banner that merely shares its text
    // (r12-2). Nothing guarded that any more: refresh case 25 was written when a
    // scan wrote to the shared banner, and r15 moved scans onto their own surface,
    // so the case went vacuous — revert the funnel and it still passes (grok r17).
    //
    // The collision needs a LISTING failure (tagged) replaced by a NON-listing
    // failure with the SAME rendered text, on a grid root, with cards still on
    // screen. Only a library that PAGES can do that: a failed first page leaves an
    // empty grid and nothing to act on. A 401 gives both writers the identical
    // sentence via friendlyError.
    mock.state.unauthNextItems = true;
    await scrollGridToEnd(driver); // page 3 dies 401 -> TAGGED banner
    await pollUntil(
      async () => ((await banner(driver)) ? true : null),
      "the listing's 401 must banner",
    );

    // A watch-state edit now fails with the same 401 — a NON-listing writer taking
    // over the banner. It must clear the listing's tag with it.
    mock.state.unauthNextPlayed = true;
    // A card that is actually ON SCREEN: the grid is scrolled to its end, so the
    // first card's context menu would open above the viewport and be unclickable.
    await watchToggle(driver, "Movie 119", "Mark watched");
    await pollUntil(
      async () => ((await banner(driver)) ? true : null),
      "the watch-state 401 must banner",
    );

    // The refresh now claims the grid at a HIGHER generation and succeeds. It
    // superseded the listing — but the banner on screen is the watch-state
    // failure's, which it never superseded and must not touch. If the tag survived
    // the non-listing write, the refresh retracts it by generation and the user is
    // left with no sign their edit failed.
    const beforeRefresh = await banner(driver);
    const refresh2 = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh2);
    await settle(driver);
    assert.equal(
      await banner(driver),
      beforeRefresh,
      "the refresh superseded the LISTING, not the watch-state edit: it must not retract a banner it never superseded, however alike the two messages read",
    );

    assert.equal(
      mock.state.contractViolations.length,
      0,
      `mock contract clean — got ${JSON.stringify(mock.state.contractViolations)}`,
    );
  },
};
