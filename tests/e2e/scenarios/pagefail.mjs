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
    `return document.querySelectorAll('main.grid button.poster').length`,
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
  // The grid is only in the DOM once it has cards — a reload in flight replaces it
  // with a skeleton, so wait rather than assume.
  await driver.waitFor(
    `return !!document.querySelector('main.grid')`,
    "the browse grid",
  );
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
    // gen-scoped retraction cannot take down a banner that merely shares its
    // generation lineage (r12-2). Nothing guarded that any more: case 25 was
    // written when a scan wrote to the shared banner, and r15 moved scans onto
    // their own surface, so it went vacuous — revert the funnel and it still
    // passed (grok r17).
    //
    // Both writes must happen DURING the action: the refresh clears the banner at
    // the click, so nothing published before it can survive to be wrongly
    // retracted. And the tagged listing failure must leave CARDS on screen, or
    // there is nothing left to drive a non-listing write with — which rules out a
    // failed reload (it empties the grid) and leaves a failed PAGE, on a
    // generation newer than the action's (so it is published, not silenced).
    mock.state.viewsDelayMs = 6000; // the action stays in flight throughout
    const refresh2 = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh2);

    // A successful edit re-enters the root, claiming a generation NEWER than the
    // action's. Its reload succeeds, so the grid keeps cards.
    await watchToggle(driver, "Movie 119", "Mark watched");
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "the edit's reload lands (a newer generation now owns the grid)",
    );

    // Now its NEXT page fails. Newer than the action's generation, so the action
    // does not silence it: a TAGGED banner, with the cards still there.
    mock.state.unauthNextItems = true;
    await scrollGridToEnd(driver);
    await pollUntil(
      async () => ((await banner(driver)) ? true : null),
      "the newer load's 401 must banner (tagged)",
    );

    // A failing edit now takes the banner over — a NON-listing write, which must
    // carry the tag away with it.
    mock.state.unauthNextPlayed = true;
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () => ((await banner(driver)) ? true : null),
      "the failed edit's banner",
    );
    const beforeSettle = await banner(driver);

    // The action finally claims the grid and succeeds. It superseded the LISTING —
    // not the edit. If the tag survived the non-listing write, it retracts by
    // generation and the user never learns their edit failed.
    await settle(driver);
    mock.state.viewsDelayMs = 0;
    assert.equal(
      await banner(driver),
      beforeSettle,
      "the refresh superseded the listing, not the edit: a banner it never published and never superseded is not its to retract",
    );

    // ── 3. A failing refresh must not erase the banner that explains the grid ──
    // Case 2's refresh SUCCEEDS, so settlement publishes nothing and never runs the
    // preservation branch at all — remove that branch and case 2 stays green
    // (codex r17). Here the action's own sections leg FAILS, so it has something to
    // say, and a banner published DURING the run by a load it never superseded must
    // survive that — it is the one explaining why the grid is empty.
    mock.state.viewsDelayMs = 6000; // the action is in flight throughout
    const refresh3 = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh3);
    mock.state.failNextViews = true; // ...and its own leg will fail (consumed at respond)

    // A newer generation takes the grid and its listing dies: a TAGGED banner the
    // action did not silence and will not supersede. The grid empties — which is
    // the whole point: that failure is the only thing explaining the emptiness.
    mock.state.unauthNextItems = true;
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () => ((await banner(driver)) ? true : null),
      "the newer load's 401 must banner",
    );

    await settle(driver);
    mock.state.viewsDelayMs = 0;
    const both = await banner(driver);
    assert.ok(
      both && both.includes("reconnect"),
      `the failure that explains this empty grid must survive the refresh's own diagnostic — got ${JSON.stringify(both)}`,
    );
    assert.ok(
      both.includes("Views"),
      `...and the refresh must still report its own failure alongside it — got ${JSON.stringify(both)}`,
    );

    assert.equal(
      mock.state.contractViolations.length,
      0,
      `mock contract clean — got ${JSON.stringify(mock.state.contractViolations)}`,
    );
  },
};
