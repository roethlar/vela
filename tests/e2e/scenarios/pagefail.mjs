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
// The production promise is exactly 8s, but making this scenario spend 16 real seconds
// would turn ownership into a slow timing test. Intercept only the next two exact 8s
// requests, accelerate them to separate deadlines, and refuse the first cancellation so
// its callback really runs stale. Everything else continues to use the native clock.
async function installEditTimerProbe(driver) {
  await driver.exec(`
    (() => {
      if (window.__velaEditTimerProbe) throw new Error('edit timer probe already installed');
      const nativeSetTimeout = window.setTimeout;
      const nativeClearTimeout = window.clearTimeout;
      const probe = {
        nativeSetTimeout,
        nativeClearTimeout,
        acceleratedMs: [3000, 5000],
        requestedMs: [],
        fired: [],
        handles: [],
        firstCancellationIgnored: false,
      };
      window.__velaEditTimerProbe = probe;
      window.setTimeout = function (callback, delay, ...args) {
        if (delay !== 8000 || probe.requestedMs.length >= 2) {
          return nativeSetTimeout.call(window, callback, delay, ...args);
        }
        const timer = probe.requestedMs.length + 1;
        probe.requestedMs.push(delay);
        const handle = nativeSetTimeout.call(window, () => {
          probe.fired.push(timer);
          callback(...args);
        }, probe.acceleratedMs[timer - 1]);
        probe.handles.push(handle);
        return handle;
      };
      window.clearTimeout = function (handle) {
        if (
          !probe.firstCancellationIgnored &&
          probe.handles.length > 0 &&
          handle === probe.handles[0]
        ) {
          probe.firstCancellationIgnored = true;
          return;
        }
        return nativeClearTimeout.call(window, handle);
      };
    })();
  `);
}
async function editTimerProbeState(driver) {
  return driver.exec(`
    const probe = window.__velaEditTimerProbe;
    return probe ? {
      requestedMs: [...probe.requestedMs],
      fired: [...probe.fired],
      firstCancellationIgnored: probe.firstCancellationIgnored,
    } : null;
  `);
}
async function restoreEditTimerProbe(driver) {
  await driver.exec(`
    (() => {
      const probe = window.__velaEditTimerProbe;
      if (!probe) return;
      window.setTimeout = probe.nativeSetTimeout;
      window.clearTimeout = probe.nativeClearTimeout;
      for (const handle of probe.handles) {
        probe.nativeClearTimeout.call(window, handle);
      }
      delete window.__velaEditTimerProbe;
    })();
  `);
}
async function settle(driver) {
  await driver.waitFor(
    `const b = document.querySelector('button.refreshbtn'); return !!b && !b.disabled`,
    "refresh to settle",
  );
}
async function contextMenuItem(driver, prefix, label) {
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
  return item;
}
// Toggle a card's watch state from its context menu (refresh.mjs:162).
async function watchToggle(driver, prefix, label) {
  const item = await contextMenuItem(driver, prefix, label);
  await driver.click(item);
}
async function gridState(driver) {
  return driver.exec(
    `return {
       labels: [...document.querySelectorAll('main.grid button.poster')]
         .map((b) => b.getAttribute('aria-label')),
       edit: [...document.querySelectorAll('div.scanerror')]
         .map((e) => e.textContent).join(' | ') || null,
     }`,
  );
}
function assertExactLabels(actual, expected, where) {
  assert.equal(actual.length, expected.length, `${where}: poster cardinality changed`);
  const mismatch = actual.findIndex((label, i) => label !== expected[i]);
  assert.equal(
    mismatch,
    -1,
    `${where}: poster ${mismatch} changed from ${JSON.stringify(expected[mismatch])} to ${JSON.stringify(actual[mismatch])}`,
  );
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
const itemsPath = () => `/Users/${mock.userId}/Items`;
const listingArrivals = () =>
  mock.state.requests.filter(
    (r) =>
      r.method === "GET" &&
      r.path === itemsPath() &&
      r.query.ParentId === "libBig" &&
      r.query.searchTerm === undefined,
  ).length;
const itemsServed = (status = null) =>
  mock.state.served.filter(
    (s) =>
      s.method === "GET" &&
      s.path === itemsPath() &&
      (status === null || s.status === status),
  ).length;
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

    // ── 4. A failed edit never reloads or loses the browse grid ─────────────
    // The backend rolled its temporary curation back and the frontend never changed the
    // card, so there is no new browse truth to fetch. A recovery listing here is itself
    // the defect: it blanks every loaded card while a dead server times out, can lose the
    // held pages to a newer generation, and manufactures a view failure unrelated to the
    // edit. Guard exact identity continuously — count alone accepts a substituted card.
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
    const expectedLabels = (await gridState(driver)).labels;
    assert.equal(expectedLabels.length, 60, "case 4 starts with one exact full page");
    assert.ok(
      expectedLabels.some((label) => label?.startsWith("Movie 059")),
      "the attempted card is part of the exact identity snapshot",
    );
    assert.equal(await banner(driver), null, "case 4 starts without a view failure");
    assert.equal(await editLine(driver), null, "case 4 starts without an edit failure");
    assert.equal(mock.state.unauthNextPlayed, false, "edit one-shot starts neutral");
    assert.equal(mock.state.failNextItems, false, "listing failure starts neutral");
    assert.equal(mock.state.unauthNextItems, false, "listing auth starts neutral");
    assert.equal(mock.state.itemsDelayMs, 0, "listing delay starts neutral");

    const arrivalsBefore4 = listingArrivals();
    const servedBefore4 = itemsServed();
    let failedLine;
    try {
      mock.state.unauthNextPlayed = true; // the edit 401s...
      mock.state.failNextItems = true; // ...but no recovery listing may consume this
      mock.state.itemsDelayMs = 6000; // or this: old code stays visibly blank while parked
      await watchToggle(driver, "Movie 059", "Mark watched");
      failedLine = await pollUntil(
        async () => {
          const current = await gridState(driver);
          assertExactLabels(current.labels, expectedLabels, "while the edit fails");
          return current.edit?.includes("Couldn't mark “Movie 059” watched")
            ? current.edit
            : null;
        },
        "the named failed-edit line while the exact grid remains",
      );

      const settled = await gridState(driver);
      assertExactLabels(settled.labels, expectedLabels, "after the edit fails");
      assert.equal(
        listingArrivals(),
        arrivalsBefore4,
        "a failed edit has no new browse truth and must not request the listing",
      );
      assert.equal(
        itemsServed(),
        servedBefore4,
        "no recovery listing response may be served when no request exists",
      );
      assert.equal(mock.state.unauthNextPlayed, false, "the doomed edit reached the server");
      assert.equal(
        mock.state.failNextItems,
        true,
        "the next-listing failure remains armed because recovery made no listing request",
      );
      assert.equal(mock.state.unauthNextItems, false, "no unrelated listing auth flag changed");
      assert.equal(
        mock.state.itemsDelayMs,
        6000,
        "the next-listing delay remains armed because recovery made no listing request",
      );
      assert.equal(
        await banner(driver),
        null,
        "the failed edit must not manufacture a view failure",
      );
      assert.ok(
        failedLine.includes("failed on all 1 configured source(s): Mock JF"),
        "the edit failure names the source without exposing provider error details",
      );
    } finally {
      // A passing no-request guard deliberately leaves these armed. Disarm before the
      // healthy Refresh and on every assertion path so this case cannot poison later ones.
      mock.state.unauthNextPlayed = false;
      mock.state.failNextItems = false;
      mock.state.unauthNextItems = false;
      mock.state.itemsDelayMs = 0;
      assert.equal(mock.state.unauthNextPlayed, false);
      assert.equal(mock.state.failNextItems, false);
      assert.equal(mock.state.unauthNextItems, false);
      assert.equal(mock.state.itemsDelayMs, 0);
    }

    const refreshArrivals4 = listingArrivals();
    const refreshServed4 = itemsServed(200);
    const healthyRefresh = await driver.find("css selector", "button.refreshbtn");
    await driver.click(healthyRefresh);
    await pollUntil(
      async () => (listingArrivals() > refreshArrivals4 ? true : null),
      "the explicit healthy Refresh listing request",
    );
    await pollUntil(
      async () => (itemsServed(200) > refreshServed4 ? true : null),
      "the explicit healthy Refresh listing response",
    );
    await settle(driver);
    const afterRefresh4 = await gridState(driver);
    assertExactLabels(afterRefresh4.labels, expectedLabels, "after explicit healthy Refresh");
    assert.equal(await banner(driver), null, "healthy Refresh leaves the view healthy");
    assert.equal(
      afterRefresh4.edit,
      failedLine,
      "Refresh repairs the view; it does not erase the edit that failed",
    );
    assert.equal(
      mock.state.userData.m59.played,
      false,
      "the failed operation and healthy Refresh leave the target unwatched",
    );
    await contextMenuItem(driver, "Movie 059", "Mark watched");
    const menuBackdrop = await driver.find("css selector", ".menubackdrop");
    await driver.click(menuBackdrop);

    // ── 4b. Failed edits auto-dismiss, and only their own timer may do it ────
    // Timer A is deliberately allowed to fire even after edit B cancels it. B must clear A
    // synchronously at its click, publish its own delayed failure, survive A's stale
    // callback, and disappear only when B's callback runs. The probe also makes 8000 itself
    // observable: a changed duration is not accelerated and fails before a long wait can
    // accidentally accept it.
    try {
      await installEditTimerProbe(driver);

      mock.state.unauthNextPlayed = true;
      await watchToggle(driver, "Movie 056", "Mark watched");
      const failedA = await pollUntil(
        async () => {
          const current = await gridState(driver);
          assertExactLabels(current.labels, expectedLabels, "while timer failure A publishes");
          return current.edit?.includes("Couldn't mark “Movie 056” watched")
            ? current.edit
            : null;
        },
        "timer failure A",
      );
      assert.equal(await banner(driver), null, "timer failure A does not become a view failure");
      let probe = await editTimerProbeState(driver);
      assert.deepEqual(probe?.requestedMs, [8000], "failure A requests the exact 8s promise");
      assert.deepEqual(probe?.fired, [], "failure A's accelerated deadline has not passed yet");

      mock.state.unauthNextPlayed = true;
      mock.state.playedDelayMs = 500; // keep B in flight while its click synchronously clears A
      const servedB = servedCount("/PlayedItems/m55");
      await watchToggle(driver, "Movie 055", "Mark watched");
      assert.equal(
        await editLine(driver),
        null,
        "starting edit B clears failure A immediately, before B has an outcome",
      );
      assert.equal(
        servedCount("/PlayedItems/m55"),
        servedB,
        "the delayed edit B response is still pending at the immediate-clear assertion",
      );
      probe = await editTimerProbeState(driver);
      assert.equal(
        probe?.firstCancellationIgnored,
        true,
        "the probe keeps A's cancelled callback queued so stale ownership is exercised",
      );
      await pollUntil(
        async () =>
          !mock.state.unauthNextPlayed && mock.state.playedDelayMs === 0 ? true : null,
        "the delayed timer failure B to reach the server",
      );
      await pollUntil(
        async () => (servedCount("/PlayedItems/m55") > servedB ? true : null),
        "timer failure B's delayed 401",
      );
      const failedB = await pollUntil(
        async () => {
          const current = await gridState(driver);
          assertExactLabels(current.labels, expectedLabels, "while timer failure B publishes");
          return current.edit?.includes("Couldn't mark “Movie 055” watched")
            ? current.edit
            : null;
        },
        "timer failure B",
      );
      assert.notEqual(failedB, failedA, "failure B replaces failure A with its own exact outcome");
      assert.equal(await banner(driver), null, "timer failure B does not become a view failure");
      probe = await editTimerProbeState(driver);
      assert.deepEqual(
        probe?.requestedMs,
        [8000, 8000],
        "both published failures request the exact 8s promise",
      );
      assert.deepEqual(
        probe?.fired,
        [],
        "failure B publishes before the forced stale callback's deadline",
      );

      await pollUntil(
        async () => ((await editTimerProbeState(driver))?.fired.includes(1) ? true : null),
        "failure A's deliberately uncancelled stale callback",
        { timeoutMs: 4000, intervalMs: 50 },
      );
      assert.equal(
        await editLine(driver),
        failedB,
        "failure A's stale callback may not erase the newer failure B",
      );
      let timerGrid = await gridState(driver);
      assertExactLabels(timerGrid.labels, expectedLabels, "after failure A's stale callback");
      assert.equal(await banner(driver), null, "a stale edit timer does not touch the view banner");
      probe = await editTimerProbeState(driver);
      assert.deepEqual(probe?.fired, [1], "only failure A's callback has fired so far");

      await pollUntil(
        async () => {
          const currentProbe = await editTimerProbeState(driver);
          return currentProbe?.fired.length === 2 && (await editLine(driver)) === null
            ? true
            : null;
        },
        "failure B to auto-dismiss on its own accelerated 8s callback",
        { timeoutMs: 6500, intervalMs: 50 },
      );
      probe = await editTimerProbeState(driver);
      assert.deepEqual(probe?.fired, [1, 2], "each exact 8s timer callback ran once, in order");
      timerGrid = await gridState(driver);
      assertExactLabels(timerGrid.labels, expectedLabels, "after failure B auto-dismisses");
      assert.equal(timerGrid.edit, null, "failure B's own timer clears only the edit line");
      assert.equal(await banner(driver), null, "auto-dismissal does not touch the view banner");
    } finally {
      mock.state.unauthNextPlayed = false;
      mock.state.playedDelayMs = 0;
      await restoreEditTimerProbe(driver);
    }

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
        return e && e.includes("failed on all 1 configured source(s): Mock JF") ? true : null;
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
    const priorFailure6 = await editLine(driver);
    assert.ok(
      priorFailure6?.includes("Couldn't mark “Movie 059” watched"),
      "case 6 starts with the exact prior failure still visible",
    );
    const served6 = servedCount("/PlayedItems/m58");
    await watchToggle(driver, "Movie 058", "Mark watched"); // succeeds
    assert.equal(
      await editLine(driver),
      null,
      "a new edit clears the previous failure immediately, before its 8s timer can satisfy a poll",
    );
    await pollUntil(
      async () => (servedCount("/PlayedItems/m58") > served6 ? true : null),
      "the successful newer edit to be delivered",
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
