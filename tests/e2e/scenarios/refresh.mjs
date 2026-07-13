// Library view refresh (.agents/plans/library-refresh-scan.md, slice 1):
// one app session, 14 cases in plan order — non-destructive first,
// view-destroying last, with explicit state restoration (re-add the removed
// view + a settling Refresh) between destructive phases. Every case asserts
// only after the refresh control re-enables (the action-settled signal).
//
// TWO mock sources are configured and "Mock JF A" is explicitly selected:
// case 13's empty-Home redirect requires `activeSource !== null`
// (+page.svelte:331), and selection also keeps `singleSource` true so the
// disappearance fallback is armed for the destructive cases.
import assert from "node:assert/strict";
import {
  pollUntil,
  mockSource,
  seedConfig,
  openLibraryGrid,
  goHome,
} from "../helpers.mjs";
import { startMockJellyfin } from "../mockjf.mjs";

let mockA;
let mockB;

const DELAY = 900; // in-flight window; WebDriver roundtrips are ~10ms each
// UserData like a real server's /Items/Latest: without it `played` is null and
// the card's context menu hides "Mark watched" — the edit the reverse phase of
// case 14 needs to trigger a newer same-root Home load.
const LATEST_SEED = [
  {
    Id: "a1",
    Name: "Alpha One",
    Type: "Movie",
    ProductionYear: 2020,
    RunTimeTicks: 100_000_000,
    UserData: { Played: false, PlaybackPositionTicks: 0 },
  },
];

async function clickRefresh(driver) {
  const b = await driver.find("css selector", "button.refreshbtn");
  await driver.click(b);
}
// Settled = the control re-enabled (disabled flips on synchronously at click).
async function settle(driver) {
  await driver.waitFor(
    `const b = document.querySelector('button.refreshbtn'); return !!b && !b.disabled`,
    "refresh to settle",
  );
}
async function banner(driver) {
  return driver.exec(
    `return document.querySelector('div.error')?.textContent ?? null`,
  );
}
async function sidebarNames(driver) {
  return driver.exec(
    `return [...document.querySelectorAll('button.sideitem')].map((b) => b.textContent.trim())`,
  );
}
async function posterLabels(driver) {
  return driver.exec(
    `return [...document.querySelectorAll('button.poster')].map((b) => b.getAttribute('aria-label'))`,
  );
}
// Node-side request log (case 14 needs arrival ordering, not UI state).
const latestGets = () =>
  mockA.state.requests.filter((r) => r.path.endsWith("/Items/Latest"));
async function onHome(driver) {
  return driver.exec(
    `return [...document.querySelectorAll('button.sideitem.active')].some((b) => b.textContent.trim() === 'Home')`,
  );
}
async function detailOpen(driver) {
  return driver.exec(`return !!document.querySelector('.detail')`);
}
async function clickSide(driver, label) {
  const el = await driver.find(
    "xpath",
    `//button[contains(@class,'sideitem') and normalize-space(.)='${label}']`,
  );
  await driver.click(el);
}
// Detail OPEN half of openDetailAndPlay (helpers.mjs:92) — without the play.
async function openDetail(driver, prefix) {
  const card = await driver.find(
    "css selector",
    `button.poster[aria-label^="${prefix}"]`,
  );
  await driver.click(card);
  await driver.waitFor(
    `return !!document.querySelector('.detail')`,
    "detail surface open",
  );
}
async function pressBack(driver) {
  const b = await driver.find("css selector", ".crumbs button.back");
  await driver.click(b);
}
// Mark a Home rail card watched via the real context menu. The edit runs
// refreshWatchState(), which on Home is loadHome(++homeGen): a newer
// SUCCESSFUL same-root Home load that bumps no navEpoch — the only way to
// supersede a refresh leg by generation rather than by navigation.
async function markWatchedFromHome(driver, prefix) {
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
        `//button[@role='menuitem' and normalize-space(.)='Mark watched']`,
      ),
    );
  await driver.click(item);
}
function removeViewA() {
  const at = mockA.state.views.findIndex((v) => v.id === "libA");
  assert.ok(at >= 0, "libA must exist before a destructive removal");
  const [saved] = mockA.state.views.splice(at, 1);
  return { saved, at };
}
// RESTORE A + settling Refresh (the between-phases contract of the plan).
// A goes back at its ORIGINAL index: sections render in server order, so
// appending would leave Library B first and change what `sections[0]` means
// for every later case (the empty-Home redirect opens sections[0]).
async function restoreA(driver, { saved, at }) {
  mockA.state.views.splice(at, 0, saved);
  await clickRefresh(driver);
  await settle(driver);
  await driver.waitFor(
    `return [...document.querySelectorAll('button.sideitem')].some((b) => b.textContent.trim() === 'Library A')`,
    "Library A restored in the sidebar",
  );
}
const latestRequests = () =>
  mockA.state.requests.filter((r) => r.path === "/Users/u1/Items/Latest")
    .length;
// Listing requests for one view — the only way to see a content leg that ran.
const listingsFor = (viewId) =>
  mockA.state.requests.filter(
    (r) => r.path === "/Users/u1/Items" && r.query.ParentId === viewId,
  ).length;

export default {
  name: "refresh",

  async seed({ configRoot }) {
    mockA = await startMockJellyfin({
      views: [
        {
          id: "libA",
          name: "Library A",
          collectionType: "movies",
          movies: [
            { id: "a1", name: "Alpha One", year: 2020 },
            { id: "a2", name: "Alpha Two", year: 2021 },
          ],
        },
      ],
      latest: LATEST_SEED,
    });
    // Second source: exists so a source picker renders and selection is real
    // (case 13's redirect eligibility); its own content is incidental.
    mockB = await startMockJellyfin({
      movies: [{ id: "r1", name: "Remote One", year: 2019 }],
    });
    seedConfig(configRoot, [
      mockSource(mockA, { id: "jf-a", name: "Mock JF A" }),
      mockSource(mockB, { id: "jf-b", name: "Mock JF B" }),
    ]);
  },

  async cleanup() {
    await mockA?.close();
    await mockB?.close();
  },

  async run({ driver, screenshot }) {
    // Select source A: sections become A's libraries; singleSource holds.
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some((b) => b.textContent.trim() === 'Mock JF A')`,
      "source picker with Mock JF A",
    );
    await clickSide(driver, "Mock JF A");
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')].some((b) => b.textContent.trim() === 'Library A')`,
      "source A's library in the sidebar",
    );

    // ── 1. Sidebar + Home rails ─────────────────────────────────────────
    // Mutate Latest AND push a second view → Refresh → new sidebar entry
    // appears without restart AND the Home rail reflects the mutated Latest.
    await goHome(driver);
    mockA.state.latest.push({
      Id: "a2",
      Name: "Alpha Two",
      Type: "Movie",
      ProductionYear: 2021,
      RunTimeTicks: 100_000_000,
      UserData: { Played: false, PlaybackPositionTicks: 0 },
    });
    // Push the view empty, then addMovie so userData stays coherent.
    mockA.state.views.push({
      id: "libB",
      name: "Library B",
      collectionType: "movies",
      movies: [],
    });
    mockA.state.addMovie("libB", { id: "b1", name: "Beta One", year: 2022 });
    await clickRefresh(driver);
    await settle(driver);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')].some((b) => b.textContent.trim() === 'Library B')`,
      "new library B in the sidebar without restart",
    );
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Alpha Two"]')`,
      "Home rail reflecting the mutated Latest (home content leg re-fetched)",
    );
    await screenshot("01-refresh-home");

    // ── 2. Mixed-success aggregation (contract (b)), on Home ────────────
    let mark = mockA.state.requests.length;
    mockA.state.failNextViews = true;
    await clickRefresh(driver);
    await settle(driver);
    const failBanner = await banner(driver);
    assert.ok(failBanner, "the sections failure must surface a banner");
    const homeLegOk = mockA.state.requests
      .slice(mark)
      .some((r) => r.path === "/Users/u1/Items/Latest");
    assert.ok(
      homeLegOk,
      "the HOME leg's request must have run (and succeeded) alongside the failure",
    );
    await screenshot("02-mixed-failure-banner");
    await clickRefresh(driver);
    await settle(driver);
    assert.equal(
      await banner(driver),
      null,
      "a following successful refresh must clear the banner",
    );

    // ── 3. Visible grid replaced from offset zero ───────────────────────
    await openLibraryGrid(driver, {
      section: "Library A",
      cardPrefix: "Alpha",
    });
    mockA.state.addMovie("libA", { id: "a3", name: "Alpha Three", year: 2023 });
    mockA.state.removeMovie("libA", "a2");
    mark = mockA.state.requests.length;
    await clickRefresh(driver);
    await settle(driver);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Alpha Three"]')`,
      "the added card after refresh, without re-entering the library",
    );
    const listing = mockA.state.requests
      .slice(mark)
      .filter(
        (r) => r.path === "/Users/u1/Items" && r.query.ParentId === "libA",
      );
    assert.ok(
      listing.length > 0,
      "refresh must re-request the visible listing",
    );
    assert.equal(
      listing[listing.length - 1].query.StartIndex ?? "0",
      "0",
      "the post-refresh listing must reload from offset zero",
    );
    const alphas = (await posterLabels(driver)).filter((l) =>
      l?.startsWith("Alpha"),
    );
    assert.equal(
      alphas.length,
      2,
      `exact replacement: expected 2 cards, got ${alphas}`,
    );
    assert.ok(
      alphas.some((l) => l.startsWith("Alpha Three")),
      "added card visible",
    );
    assert.ok(
      !alphas.some((l) => l.startsWith("Alpha Two")),
      "removed card gone",
    );
    await screenshot("03-grid-replaced");

    // ── 4. Navigation wins (error) ──────────────────────────────────────
    mockA.state.failNextViews = true;
    mockA.state.viewsDelayMs = DELAY;
    await clickRefresh(driver);
    await goHome(driver); // while the failing sections response is in flight
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
    assert.equal(
      await banner(driver),
      null,
      "a delayed failure must not banner after navigation",
    );

    // ── 5. Navigation wins (content leg) ────────────────────────────────
    await openLibraryGrid(driver, {
      section: "Library A",
      cardPrefix: "Alpha",
    });
    mockA.state.viewsDelayMs = DELAY;
    await clickRefresh(driver);
    await clickSide(driver, "Library B"); // open B while A's refresh is in flight
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Beta One"]')`,
      "library B's own cards",
    );
    // The CARDS cannot prove the gate: loadMore reads LIVE `active`
    // (+page.svelte:727), so an ungated stale leg would simply clear B's grid
    // and re-list B — same cards, every assertion green (lrs-8/codex r2:
    // lrs-2). The request log is what distinguishes them: the superseded leg
    // must issue NO listing at all.
    const bListingsBefore = listingsFor("libB");
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
    assert.equal(
      listingsFor("libB"),
      bListingsBefore,
      "the superseded content leg must not re-list B (it must be dropped, not redirected at the new root)",
    );
    const bLabels = (await posterLabels(driver)).filter((l) =>
      l?.startsWith("Alpha"),
    );
    assert.equal(
      bLabels.length,
      0,
      "B's grid must not be overwritten with A's listing",
    );
    assert.equal(
      await banner(driver),
      null,
      "no error banner for the superseded content leg",
    );
    assert.equal(
      mockA.state.contractViolations.length,
      0,
      "no listing contract violations",
    );

    // ── 6. Detail opened mid-refresh ────────────────────────────────────
    await openLibraryGrid(driver, {
      section: "Library A",
      cardPrefix: "Alpha",
    });
    mockA.state.failNextViews = true;
    mockA.state.viewsDelayMs = DELAY;
    await clickRefresh(driver);
    await openDetail(driver, "Alpha One"); // detail OPEN must bump navEpoch
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
    assert.ok(
      await detailOpen(driver),
      "the detail must remain open through settlement",
    );
    assert.equal(
      await banner(driver),
      null,
      "no banner after detail-open navigation",
    );

    // ── 7. Detail closed mid-refresh ────────────────────────────────────
    // (detail is still open over A from case 6)
    mockA.state.failNextViews = true;
    mockA.state.viewsDelayMs = DELAY;
    await clickRefresh(driver);
    await pressBack(driver); // detail CLOSE must bump navEpoch too
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
    assert.equal(await detailOpen(driver), false, "detail closed");
    assert.equal(
      await banner(driver),
      null,
      "no banner over the revealed grid after close",
    );

    // ── 8. Root-identity mismatch (fallback), destructive ───────────────
    await openLibraryGrid(driver, {
      section: "Library A",
      cardPrefix: "Alpha",
    });
    mockA.state.viewsDelayMs = DELAY;
    let saved = removeViewA();
    await clickRefresh(driver);
    await clickSide(driver, "Library B"); // navigate away while in flight
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Beta One"]')`,
      "library B's grid before settlement",
    );
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
    assert.equal(
      await onHome(driver),
      false,
      "the delayed response must NOT force Home",
    );
    assert.ok(
      (await posterLabels(driver)).some((l) => l?.startsWith("Beta One")),
      "B's grid stays: settlement root (B) does not match the missing key (A)",
    );
    await restoreA(driver, saved);

    // ── 9. Positive deleted-library fallback, destructive ───────────────
    await openLibraryGrid(driver, {
      section: "Library A",
      cardPrefix: "Alpha",
    });
    saved = removeViewA();
    const hubCount = latestRequests();
    await clickRefresh(driver);
    await settle(driver);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem.active')].some((b) => b.textContent.trim() === 'Home')`,
      "app lands on Home after its root library vanished",
    );
    assert.ok(
      latestRequests() > hubCount,
      "a NEW hub fetch must follow the fallback (hubs-empty conditional alone cannot: rails were seeded non-empty)",
    );
    await screenshot("04-deleted-library-fallback");
    await restoreA(driver, saved);

    // ── 10. Detail over a removed library, destructive ──────────────────
    await openLibraryGrid(driver, {
      section: "Library A",
      cardPrefix: "Alpha",
    });
    await openDetail(driver, "Alpha One");
    saved = removeViewA();
    await clickRefresh(driver);
    await settle(driver);
    assert.ok(
      await detailOpen(driver),
      "the detail surface STAYS OPEN (root kind detail)",
    );
    await pressBack(driver);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem.active')].some((b) => b.textContent.trim() === 'Home')`,
      "Back reveals HOME, not the dead grid (hidden parent reconciled)",
    );
    await restoreA(driver, saved);

    // ── 11. Deletion racing a detail OPEN, destructive ──────────────────
    await openLibraryGrid(driver, {
      section: "Library A",
      cardPrefix: "Alpha",
    });
    mockA.state.viewsDelayMs = DELAY;
    saved = removeViewA();
    await clickRefresh(driver);
    await openDetail(driver, "Alpha One"); // detail-open bumps navEpoch mid-flight
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
    const side11 = await sidebarNames(driver);
    assert.ok(
      !side11.includes("Library A"),
      "the sidebar must drop A at settlement",
    );
    assert.ok(await detailOpen(driver), "detail still open at settlement");
    await pressBack(driver);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem.active')].some((b) => b.textContent.trim() === 'Home')`,
      "Back → Home, not A's orphan (root-identity gate, not a pure epoch gate)",
    );
    await restoreA(driver, saved);

    // ── 12. Deletion racing a detail CLOSE, destructive ─────────────────
    await openLibraryGrid(driver, {
      section: "Library A",
      cardPrefix: "Alpha",
    });
    await openDetail(driver, "Alpha One");
    mockA.state.viewsDelayMs = DELAY;
    saved = removeViewA();
    await clickRefresh(driver);
    await pressBack(driver); // close while the removal response is in flight
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem.active')].some((b) => b.textContent.trim() === 'Home')`,
      "the revealed root reconciles to Home (still rooted on the missing key)",
    );
    await restoreA(driver, saved);

    // ── 13. Empty-Home redirect deferral, destructive ───────────────────
    // Guard-prove eligibility: empty rails + selection + no detail → the
    // select(sections[0]) redirect actually fires.
    await goHome(driver);
    mockA.state.latest = [];
    await clickRefresh(driver);
    await settle(driver);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Alpha"]')`,
      "the empty-Home redirect lands in library A (eligibility guard)",
    );
    // Deferral: detail open must hold the redirect back.
    await openDetail(driver, "Alpha One");
    saved = removeViewA();
    const latestBeforeFallback = latestRequests();
    await clickRefresh(driver);
    await settle(driver);
    // settle() watches only the refresh control, and the fallback's Home
    // re-fetch is FIRE-AND-FORGET (forceHomeForRemovedRoot), so the redirect's
    // `!loading` gate may still be shut here: asserting now would pass even
    // with the detail-deferral guard deleted (lrs-8). Wait for that fetch to
    // arrive AND be applied — the window in which a missing guard closes the
    // detail.
    await pollUntil(
      async () => (latestRequests() > latestBeforeFallback ? true : null),
      "the fallback's Home re-fetch arrived (the redirect's gate can now open)",
    );
    await new Promise((r) => setTimeout(r, 600)); // let it apply: loading clears, the effect re-runs
    assert.ok(
      await detailOpen(driver),
      "the redirect must be DEFERRED while the detail is open",
    );
    await pressBack(driver);
    await pollUntil(async () => {
      const side = await sidebarNames(driver);
      if (side.includes("Library A")) return null;
      const home = await onHome(driver);
      const beta = (await posterLabels(driver)).some((l) =>
        l?.startsWith("Beta One"),
      );
      return home || beta ? true : null;
    }, "after Back the redirect may fire (B's grid or Home both valid)");
    await screenshot("05-redirect-deferred");
    mockA.state.latest = [...LATEST_SEED, mockAExtraLatest()];
    await restoreA(driver, saved);

    // ── 14. Stale older Home load, non-destructive ──────────────────────
    // A plain (non-refresh) Home load is triggered by re-selecting source A
    // from All; arm it slow + failing, then Refresh while it is in flight.
    await goHome(driver);
    await clickSide(driver, "All");
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')].some((b) => b.textContent.trim() === 'Movies')`,
      "merged All view settled (type tabs)",
    );
    mockA.state.delayNextLatestMs = DELAY;
    mockA.state.failNextLatest = true;
    const latestBefore = latestGets().length;
    await clickSide(driver, "Mock JF A"); // plain Home load consumes the flags
    // The flags bind at request ARRIVAL; wait for the older load's Latest to
    // be logged before refreshing, or the refresh's own Latest request could
    // capture the delay+failure instead (codex code review r1, finding 2).
    await pollUntil(
      async () => (latestGets().length > latestBefore ? true : null),
      "older Home load's Latest request arrived (flags bound)",
    );
    await clickRefresh(driver); // refresh claims a NEW homeGen
    await settle(driver);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Alpha"]')`,
      "the refresh's Home leg lands: rails present",
    );
    // The armed stale load fails AFTER the refresh settles — give it time to
    // land, then assert it published nothing.
    await new Promise((r) => setTimeout(r, DELAY + 600));
    assert.equal(
      await banner(driver),
      null,
      "the superseded load's late failure publishes NO banner",
    );
    assert.ok(
      (await posterLabels(driver)).some((l) => l?.startsWith("Alpha")),
      "the refreshed rails were not overwritten by the stale load",
    );
    await screenshot("06-stale-load-superseded");

    // ── 14b. The stranded `loading` flag has an observable guard ────────
    // The older Home load above set `loading = true`, and the refresh STOLE
    // its `homeGen` — so that load's own generation-gated finally can no
    // longer clear the flag, and ONLY the refresh leg's finally can (the r1
    // fix). Nothing above observes it: rails render and the control
    // re-enables either way (lrs-4). A stranded flag is observable in exactly
    // one place — the `!loading`-gated empty-Home redirect stops working
    // forever.
    //
    // ORDER IS LOAD-BEARING: this must stay the FIRST thing after the
    // stale-load phase. goHome() clears `loading`, and so does ANY later
    // SUCCESSFUL Home load (loadHome's own finally) — so anything that
    // reloads Home first heals the stranded flag and hides the bug. Placed
    // after one, this very assertion passed with the r1 fix deleted (proven
    // vacuous on the VM).
    mockA.state.latest = [];
    await clickRefresh(driver);
    await settle(driver);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Alpha"]')`,
      "the empty-Home redirect still fires (loading was released, not stranded)",
    );
    mockA.state.latest = [...LATEST_SEED, mockAExtraLatest()]; // rails back

    // ── 14 (reverse phase). A SUPERSEDED refresh leg publishes nothing ───
    // The plan requires both orderings; only the one above was implemented,
    // so leg-failure generation ownership (`current: () => hg === homeGen`)
    // had no guard at all (lrs-3). Here the REFRESH's Home leg is the slow
    // failing one, and a newer successful same-root Home load supersedes it:
    // marking watched from a Home rail card runs refreshWatchState(), which
    // on Home does loadHome(++homeGen) — a new homeGen WITHOUT a navEpoch
    // bump, so only the leg's generation check can suppress the stale failure.
    await goHome(driver); // 14b redirected us into A's grid
    await pollUntil(async () => {
      const rails = (await posterLabels(driver)).some((l) =>
        l?.startsWith("Alpha"),
      );
      return (await onHome(driver)) && rails ? true : null;
    }, "back on Home with rails (the reverse phase needs a rail card to edit)");
    mockA.state.delayNextLatestMs = DELAY;
    mockA.state.failNextLatest = true;
    const latestBeforeRefresh = latestGets().length;
    await clickRefresh(driver); // ITS Home leg binds the delay + failure
    await pollUntil(
      async () => (latestGets().length > latestBeforeRefresh ? true : null),
      "the refresh's Home leg Latest request arrived (flags bound to IT)",
    );
    await markWatchedFromHome(driver, "Alpha One"); // newer successful Home load
    await settle(driver);
    await new Promise((r) => setTimeout(r, DELAY + 600)); // the refresh's failure lands late
    assert.equal(
      await banner(driver),
      null,
      "a refresh leg superseded by a newer same-root Home load must publish NO failure",
    );
    assert.ok(
      (await posterLabels(driver)).some((l) => l?.startsWith("Alpha")),
      "the newer Home load's rails survive",
    );

    // ── 15. The app's own redirect must not swallow the failure ─────────
    // Sections FAIL while the Home leg comes back EMPTY: Home settles with no
    // rails, so the empty-Home effect redirects into sections[0] — using the
    // STALE section list, since the refresh of it just failed. That redirect
    // is the APP navigating, not the user, so contract (b) still holds: the
    // sections failure must surface. With select()'s navEpoch bump applied to
    // the auto path (the pre-lrs-1 behavior), the action reads "the user
    // navigated" and publishes nothing — the user browses a stale library
    // with no error (codex code review r2, lrs-1).
    await goHome(driver);
    // 14b left Home rail-less and us in A's grid; goHome re-fetches with the
    // restored seed. Wait for the rails to LAND before emptying them again —
    // otherwise the redirect could fire before this case's Refresh, making it
    // a grid refresh that never runs the Home leg this case is about.
    await pollUntil(async () => {
      const rails = (await posterLabels(driver)).some((l) =>
        l?.startsWith("Alpha"),
      );
      return (await onHome(driver)) && rails ? true : null;
    }, "Home rails restored before emptying them again");
    mockA.state.latest = [];
    mockA.state.failNextViews = true;
    await clickRefresh(driver);
    await settle(driver);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Alpha"]')`,
      "the empty-Home redirect lands in library A (the app navigated)",
    );
    assert.ok(
      await banner(driver),
      "the sections failure must still banner after the app's own redirect",
    );
    await screenshot("07-autoredirect-keeps-banner");

    // ── 16. A failing older listing must not banner over the refresh ─────
    // An ordinary listing load (entering a library) publishes its own failures
    // DIRECTLY. The refresh's content leg used to claim `loadGen` only after
    // the sections response, so a listing that died during a slow sections
    // fetch landed a banner the action could no longer clear — a false failure
    // over the fresh cards the action then loaded (codex r3). The action now
    // claims the generation at the CLICK, invalidating that load.
    //
    // Restore the rails BEFORE going Home: case 15 left Home rail-less, so a
    // bare goHome() would (correctly) auto-redirect into library A, and that
    // redirect's own listing would race — and swallow — the one-shot listing
    // flags this case arms below.
    mockA.state.latest = [...LATEST_SEED, mockAExtraLatest()];
    await goHome(driver);
    await pollUntil(async () => {
      const rails = (await posterLabels(driver)).some((l) =>
        l?.startsWith("Alpha"),
      );
      return (await onHome(driver)) && rails ? true : null;
    }, "Home with rails (so no auto-redirect races the listing flags)");
    mockA.state.itemsDelayMs = 600; // the doomed listing is parked...
    mockA.state.failNextItems = true; // ...and will 500
    mockA.state.viewsDelayMs = 1400; // sections lands well AFTER that failure
    await clickSide(driver, "Library A"); // starts the doomed listing
    await clickRefresh(driver); // must claim loadGen HERE, not later
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Alpha One"]')`,
      "the refresh's own listing lands",
    );
    assert.equal(
      await banner(driver),
      null,
      "the superseded failing listing must not banner over the refreshed cards",
    );

    // ── 17. The redirect must not fire MID-refresh ──────────────────────
    // Settled Home with NO libraries and NO rails: the redirect is ineligible
    // (no sections). Add the first library and make the Home leg slow. The
    // sections leg lands first, and without the `!refreshing` gate the effect
    // sees sections>0 + empty hubs + !loading (the Home leg deliberately never
    // raises `loading`) and throws the user into the new library — discarding
    // the rails that were about to arrive (codex r3).
    //
    // Reach Home WITH rails first (case 16 left us in a grid): stripping the
    // server while Home is empty but its sections are still cached would let
    // the legitimate empty-Home redirect fire and list a library the mock no
    // longer serves — a mock contract violation, not a bug under test.
    await goHome(driver);
    await pollUntil(async () => {
      const rails = (await posterLabels(driver)).some((l) =>
        l?.startsWith("Alpha"),
      );
      return (await onHome(driver)) && rails ? true : null;
    }, "Home with rails before stripping the server bare");
    const savedViews = mockA.state.views;
    mockA.state.views = [];
    mockA.state.latest = [];
    await clickRefresh(driver);
    await settle(driver);
    await pollUntil(async () => {
      const side = await sidebarNames(driver);
      return !side.includes("Library A") && (await onHome(driver))
        ? true
        : null;
    }, "settled on an empty Home with no libraries");
    // Now the server gains its first library AND rails, but Home answers slowly.
    mockA.state.views = savedViews;
    mockA.state.latest = [...LATEST_SEED];
    mockA.state.delayNextLatestMs = DELAY;
    await clickRefresh(driver);
    await settle(driver);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')].some((b) => b.textContent.trim() === 'Library A')`,
      "the new library appears in the sidebar",
    );
    assert.ok(
      await onHome(driver),
      "the refresh must NOT redirect mid-action: the Home leg's rails were still in flight",
    );
    assert.ok(
      (await posterLabels(driver)).some((l) => l?.startsWith("Alpha")),
      "the Home leg's rails landed (and were not discarded by a mid-action redirect)",
    );
    await screenshot("08-no-midrefresh-redirect");

    // ── 18. A failed refresh must not empty a healthy library ───────────
    // A listing is in flight when Refresh is clicked, and the action's sections
    // leg then FAILS. The r3-2 design claimed `loadGen` at the click, which
    // invalidated that listing — and since the leg returned early, its result
    // was discarded with nothing to replace it: the library rendered EMPTY
    // ("Nothing in this view yet"), unable to paginate, until the user
    // navigated away (codex r5; r4's skeleton assertion passed straight
    // through it, because a released flag is not the same as a usable grid).
    // The in-flight load is now left alone: it populates the grid, and the
    // sections failure is reported on top.
    mockA.state.itemsDelayMs = 700; // a listing is in flight at click time...
    await clickSide(driver, "Library A");
    mockA.state.failNextViews = true; // ...and the refresh's sections leg dies
    await clickRefresh(driver);
    await settle(driver);
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Alpha One"]')`,
      "the library's own cards arrive: a failed refresh must not empty a healthy library",
    );
    assert.equal(
      await driver.exec(`return !!document.querySelector('.skelgrid')`),
      false,
      "and the grid is not left on its loading skeleton",
    );
    assert.ok(await banner(driver), "the sections failure is still reported");

    // ── 19. A scroll mid-refresh must not corrupt the reloaded grid ─────
    // `onScroll` starts a paged load with the CURRENT generation. It must not
    // be able to append that page across the action's reset — which is what
    // the r3-2 click-time claim allowed, by handing the scroll the action's own
    // generation (codex r4). The scroll's page is parked so it lands AFTER the
    // action has reset and reloaded: its generation is stale by then and it
    // must be dropped, leaving exactly the reloaded first page.
    for (let i = 0; i < 70; i++) {
      mockA.state.addMovie("libB", {
        id: `bulk${i}`,
        name: `Bulk ${String(i).padStart(2, "0")}`,
        year: 2000 + (i % 20),
      });
    }
    await clickSide(driver, "Library B");
    await driver.waitFor(
      `return document.querySelectorAll('button.poster').length >= 60`,
      "library B's first page (PAGE=60) is on screen and scrollable",
    );
    mockA.state.viewsDelayMs = 1200; // hold the action open...
    await clickRefresh(driver);
    mockA.state.itemsDelayMs = 2400; // ...and park the scroll's page past the reset
    await driver.exec(
      `const g = document.querySelector('main.grid');
       g.scrollTop = g.scrollHeight;
       g.dispatchEvent(new Event('scroll'));`,
    );
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
    await driver.waitFor(
      `return document.querySelectorAll('button.poster').length === 60`,
      "the action's reloaded first page is on screen",
    );
    await new Promise((r) => setTimeout(r, 2600)); // the parked page lands here
    const after = await posterLabels(driver);
    assert.equal(
      after.length,
      60,
      "a stale paged load must not append across the action's reset",
    );
    assert.equal(
      new Set(after).size,
      60,
      "no duplicate cards (a stale page appended over the reloaded one)",
    );
    assert.equal(
      await banner(driver),
      null,
      "no stray failure from a scroll-triggered load",
    );

    // Session-wide invariant: no listing contract violations anywhere.
    assert.equal(
      mockA.state.contractViolations.length,
      0,
      `mock A contract clean — got ${JSON.stringify(mockA.state.contractViolations)}`,
    );
    assert.equal(
      mockB.state.contractViolations.length,
      0,
      "mock B contract clean",
    );
  },
};

// Case 1 mutated the Latest seed; case 13's restore puts both entries back so
// later assertions ("Alpha" rail present) keep matching the mutated state.
function mockAExtraLatest() {
  return {
    Id: "a2",
    Name: "Alpha Two",
    Type: "Movie",
    ProductionYear: 2021,
    RunTimeTicks: 100_000_000,
    UserData: { Played: false, PlaybackPositionTicks: 0 },
  };
}
