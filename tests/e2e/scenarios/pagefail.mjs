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
  goHome,
  allDelivered,
  holdsFor,
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
// The EDIT's own surface (owner ruling 2026-07-14). Renders as div.scanerror, like the
// scan's — NEVER div.error, which is the view's. A test that reads the wrong one is the
// whole point of the split.
async function editLine(driver) {
  return driver.exec(
    `return [...document.querySelectorAll('div.scanerror')].map((e) => e.textContent).join(' | ') || null`,
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
const ENTER = ""; // WebDriver Enter (search.mjs uses the literal char)
// Both of the edit's one-shots bind at request ARRIVAL. A case that navigates before
// they are consumed guards nothing: the edit would not have failed, or its recovery
// repaint would not have parked, and the race never runs (codex r20). Every timed wait
// below is measured from HERE — the parked response is 6s after the request arrives,
// not 6s after the click.
async function armedShotsBound() {
  return !mock.state.unauthNextPlayed && mock.state.itemsDelayMs === 0 ? true : null;
}
const servedCount = (endsWith) =>
  mock.state.served.filter((s) => s.path.endsWith(endsWith)).length;
// An ABSENCE assertion ("the edit's failure must NOT be published here") passes just as
// well by asking too early as by being right. A fixed sleep measured from the PARK is
// not a witness: it says when the request ARRIVED, never when the answer went out (codex
// + grok, r21).
//
// So: wait for the parked response to actually be SENT, then hold the window OPEN and
// fail the moment a banner appears, rather than sampling once at the end.
//
// Be precise about what this proves. `served` is a SERVER-dispatch witness — it cannot
// see the client parse the body, resume the catch, and render (codex r23). Nothing in
// the mock can. The hold is what covers that gap: it keeps asking for five seconds after
// dispatch, so a publish that is going to happen has to beat a window it has no reason
// to, rather than merely beating a single sample.
async function noEditBannerAfterParked(driver, { endsWith, before, what }) {
  await pollUntil(
    async () => (servedCount(endsWith) > before ? true : null),
    `the parked ${endsWith} response to be SERVED (${what})`,
  );
  const deadline = Date.now() + 5000;
  while (Date.now() < deadline) {
    const b = await banner(driver);
    if (b && b.includes("reconnect"))
      assert.fail(`${what} — got ${JSON.stringify(b)}`);
    await new Promise((r) => setTimeout(r, 250));
  }
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
    // The retract takes parts owned by a LOAD it replaced, and nothing else. An untagged
    // part — here the search validation message, which no load owns — is not the refresh's
    // to take back. (Before the surface split this case used a failed EDIT as its untagged
    // writer; the edit no longer writes to this banner at all, which is the point.)
    mock.state.viewsDelayMs = 6000; // the action stays in flight throughout
    const refresh2 = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh2);

    const box = await driver.find(
      "css selector",
      'input[aria-label="Search your libraries"]',
    );
    await driver.type(box, `M${ENTER}`); // too short: an UNTAGGED banner part
    await driver.waitFor(
      `return document.body.innerText.includes('Search needs at least 2 characters.')`,
      "the untagged validation message",
    );
    const beforeSettle = await banner(driver);

    await settle(driver);
    mock.state.viewsDelayMs = 0;
    assert.equal(
      await banner(driver),
      beforeSettle,
      "the refresh superseded no load that published this: a part no load owns is not its to retract",
    );

    // ── 3. A failing refresh must not erase the banner explaining the grid ──
    // The action's own sections leg fails, so it has something to say — and it must say it
    // AFTER what is already there, not over it.
    await driver.exec(
      `const i = document.querySelector('input[aria-label="Search your libraries"]');
       i.value = ''; i.dispatchEvent(new Event('input', { bubbles: true }));`,
    );
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    mock.state.viewsDelayMs = 6000;
    const refresh3 = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh3);
    mock.state.failNextViews = true; // ...and its own leg will fail

    await driver.type(box, `M${ENTER}`); // an untagged part, published during the run
    await driver.waitFor(
      `return document.body.innerText.includes('Search needs at least 2 characters.')`,
      "the untagged part, published during the action's run",
    );

    await settle(driver);
    mock.state.viewsDelayMs = 0;
    const both = await banner(driver);
    assert.ok(
      both && both.includes("Search needs at least 2 characters."),
      `a part published during the run, by a writer the action never superseded, must survive the action's own diagnostic — got ${JSON.stringify(both)}`,
    );
    assert.ok(
      both.includes("Views"),
      `...and the refresh must still report its own failure alongside it — got ${JSON.stringify(both)}`,
    );

    // ── 4. The edit's failure and the view's never touch each other ─────────
    // THE POINT OF THE SPLIT. Both fail; both are reported; neither erases the other. This
    // one assertion replaces six cases (the old 5-11), every one of which existed only to
    // police two writers fighting over a single banner.
    await driver.exec(
      `const i = document.querySelector('input[aria-label="Search your libraries"]');
       i.value = ''; i.dispatchEvent(new Event('input', { bubbles: true }));`,
    );
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    mock.state.unauthNextPlayed = true; // the edit 401s...
    mock.state.failNextItems = true; // ...and its recovery repaint 500s
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () => {
        const e = await editLine(driver);
        const b = await banner(driver);
        return e && b ? true : null;
      },
      "both failures, each on its own surface",
    );
    // ...AND THE LIBRARY IS STILL THERE. The repaint is a RE-ENTRY of the root the user is
    // already standing on, not a navigation away from it: blanking the grid before the
    // fetch means a failed re-fetch leaves them with nothing. They asked to mark ONE item
    // watched, and it cost them the whole view — with the server down they could not even
    // retry, because there was nothing left to right-click (owner playtest, 0.1.47).
    assert.equal(
      await cardCount(driver),
      60,
      "a failed watch-state repaint must not empty the library: the user asked to change one item, not to lose the view",
    );
    assert.ok(
      (await editLine(driver)).includes("reconnect"),
      "the edit the user ASKED for failed: that is reported on the edit's line",
    );
    assert.ok(
      (await banner(driver)).includes("500"),
      "...and the repaint's failure, which is the only thing explaining the empty grid it left behind, is reported on the VIEW's",
    );
    assert.ok(
      !(await banner(driver)).includes("reconnect"),
      "the edit's failure must not appear on the view's banner at all — that shared surface is what eight review rounds of defects came from",
    );

    // ── 5. An action's outcome is reported wherever the user is ─────────────
    // The old cases 5/7/9 asserted the OPPOSITE — that a failed edit must be SUPPRESSED if
    // the user navigated away — because on the shared banner it would have covered the new
    // view's own status. On its own line it covers nothing, and suppressing it was only
    // ever a way to lose a failure the user needed. The scan already behaves this way.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    mock.state.unauthNextPlayed = true;
    mock.state.playedDelayMs = 6000; // the edit is still in flight when the user leaves
    const served5 = servedCount("/PlayedItems/m59");
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () =>
        !mock.state.unauthNextPlayed && mock.state.playedDelayMs === 0 ? true : null,
      "the parked, doomed edit to reach the server",
    );
    await goHome(driver);
    await pollUntil(
      async () => (servedCount("/PlayedItems/m59") > served5 ? true : null),
      "the parked 401 to be delivered",
    );
    await pollUntil(
      async () => {
        const e = await editLine(driver);
        return e && e.includes("reconnect") ? true : null;
      },
      "the user asked for this change and it did not happen — they are told so, wherever they now are",
    );

    // ── 6. A newer edit supersedes an older one's outcome ───────────────────
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "the grid",
    );
    await watchToggle(driver, "Movie 058", "Mark watched"); // succeeds
    await pollUntil(
      async () => ((await editLine(driver)) === null ? true : null),
      "a new edit clears the previous one's failure — a stale outcome is not an outcome",
    );

    // ── (no case 7) ────────────────────────────────────────────────────────
    // Losing the LAST source must abandon an edit in flight — its outcome is about an item
    // in a library that no longer exists, and Welcome offers nothing that could clear it
    // (the r14 rule, which cost three separate defects). `onSourcesChanged` bumps
    // `editAttempt` and clears the line for exactly this.
    //
    // NOT GUARDED HERE: this scenario cannot remove a source (that is Settings UI, and
    // pagefail seeds a single mock). A first draft of this case called a `__vela_removeSource`
    // hook that does not exist — a test that asserted nothing while looking like a guard.
    // Deleted rather than shipped. Recorded as an open gap in the plan.

    // ── 8. The heal must RETRACT the failure it repairs ─────────────────────
    // (was case 12) The heal rebuilds Home successfully, so the previous load's diagnostic
    // describes rails that are now on screen. Fresh rails under a stale "couldn't load" is
    // the r11 lie, and an untagged Home failure has nothing that ever retracts it — so
    // leaving it is permanent.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    mock.state.unauthNextPlayed = true;
    mock.state.playedDelayMs = 6000;
    const served8 = servedCount("/PlayedItems/m59");
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () =>
        !mock.state.unauthNextPlayed && mock.state.playedDelayMs === 0 ? true : null,
      "the parked, doomed edit to reach the server",
    );
    mock.state.failNextLatest = true; // Home's OWN load will fail
    await goHome(driver);
    await pollUntil(
      async () => {
        const b = await banner(driver);
        return b && b.includes("500") ? true : null;
      },
      "Home's own failure",
    );
    const latestBefore8 = servedCount("/Items/Latest");
    await pollUntil(
      async () => (servedCount("/PlayedItems/m59") > served8 ? true : null),
      "the parked 401 to be delivered",
    );
    await pollUntil(
      async () => (servedCount("/Items/Latest") > latestBefore8 ? true : null),
      "the heal's Home load to be delivered",
    );
    // HOLD it: polling for the absence would pass on the transient blank while the heal's
    // reload is still in flight — which is exactly how this case was vacuous once.
    await holdsFor(
      async () => {
        const b = await banner(driver);
        return b && b.includes("500")
          ? `Home's stale failure is still up (banner: ${JSON.stringify(b)})`
          : null;
      },
      3000,
      "the heal rebuilt Home, so the failure describing the rails it just replaced must go with it — nothing else ever retracts an untagged Home failure",
    );

    // ── 9. Skipping the repaint must not skip the HEAL ──────────────────────
    // (was case 14) The backend curates BEFORE the server call and rolls back on failure. A
    // Home load INSIDE that window captures the transient state, so an edit failure that
    // re-fetches nothing leaves Continue Watching showing an item the server still has.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    // A successful edit empties the hubs, so the next goHome REALLY loads Home rather than
    // serving it from cache — without this the curated window is never captured and the
    // case guards nothing.
    const servedRepaint = servedCount("/Items");
    await watchToggle(driver, "Movie 057", "Mark watched"); // 058 is already watched (case 6)
    await pollUntil(
      async () => (servedCount("/Items") > servedRepaint ? true : null),
      "the successful edit's repaint to be served (the hubs are now empty)",
    );
    await pollUntil(
      async () =>
        (await driver.exec(
          `return !!document.querySelector('button.poster[aria-label^="Movie 059"]')`,
        ))
          ? true
          : null,
      "the repainted grid",
    );

    mock.state.unauthNextPlayed = true;
    mock.state.playedDelayMs = 6000;
    const served9 = servedCount("/PlayedItems/m59");
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () =>
        !mock.state.unauthNextPlayed && mock.state.playedDelayMs === 0 ? true : null,
      "the parked, doomed edit to reach the server (the backend has now curated)",
    );
    await goHome(driver);
    await pollUntil(
      async () => (allDelivered(mock, "/Items/Latest") ? true : null),
      "Home's own load, inside the curated window, to be delivered",
    );
    const latestBefore9 = servedCount("/Items/Latest");
    await pollUntil(
      async () => (servedCount("/PlayedItems/m59") > served9 ? true : null),
      "the parked 401 to be delivered (the backend now rolls back)",
    );
    await pollUntil(
      async () => (servedCount("/Items/Latest") > latestBefore9 ? true : null),
      "the rolled-back edit must re-fetch the watch state, or Continue Watching keeps showing an item the server still has — and nothing but the heal can issue this request",
    );

    assert.equal(
      mock.state.contractViolations.length,
      0,
      `mock contract clean — got ${JSON.stringify(mock.state.contractViolations)}`,
    );
  },
};
