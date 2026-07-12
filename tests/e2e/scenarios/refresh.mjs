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
const LATEST_SEED = [
  {
    Id: "a1",
    Name: "Alpha One",
    Type: "Movie",
    ProductionYear: 2020,
    RunTimeTicks: 100_000_000,
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
function removeViewA() {
  const saved = mockA.state.views.find((v) => v.id === "libA");
  assert.ok(saved, "libA must exist before a destructive removal");
  mockA.state.views = mockA.state.views.filter((v) => v.id !== "libA");
  return saved;
}
// RESTORE A + settling Refresh (the between-phases contract of the plan).
async function restoreA(driver, saved) {
  mockA.state.views.push(saved);
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
    await settle(driver);
    mockA.state.viewsDelayMs = 0;
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
    await clickRefresh(driver);
    await settle(driver);
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

    // Session-wide invariant: no listing contract violations anywhere.
    assert.equal(
      mockA.state.contractViolations.length,
      0,
      "mock A contract clean",
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
  };
}
