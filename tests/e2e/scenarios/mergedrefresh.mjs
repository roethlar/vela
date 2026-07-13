// Library refresh in the MERGED multi-source scope
// (.agents/plans/library-refresh-scan.md, slice 1 — the two behaviors no
// single-source scenario can reach):
//   1. the `type-grid` content leg reloads the merged listing in place;
//   2. a PARTIAL sections aggregate must NOT force Home — `get_sections`
//      skips a failing source by design, so a type vanishing from the
//      refreshed tabs is not evidence of deletion (plan: Non-goals).
//
// Mock A is a SHOWS-ONLY provider so mock B is the SOLE Movies provider:
// if both served Movies, the merged Movies type would survive one source's
// failure and a faulty force-Home-when-the-active-type-disappears
// implementation would pass this scenario (plan-review r7, finding 5).
import assert from "node:assert/strict";
import { pollUntil, mockSource, seedConfig } from "../helpers.mjs";
import { startMockJellyfin } from "../mockjf.mjs";

let mockA;
let mockB;

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
async function clickSide(driver, label) {
  const el = await driver.find(
    "xpath",
    `//button[contains(@class,'sideitem') and normalize-space(.)='${label}']`,
  );
  await driver.click(el);
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
async function onHome(driver) {
  return driver.exec(
    `return [...document.querySelectorAll('button.sideitem.active')].some((b) => b.textContent.trim() === 'Home')`,
  );
}
async function banner(driver) {
  return driver.exec(
    `return document.querySelector('div.error')?.textContent ?? null`,
  );
}
const hasCard = (driver, prefix) =>
  driver.waitFor(
    `return !!document.querySelector('button.poster[aria-label^="${prefix}"]')`,
    `card "${prefix}" on screen`,
  );

export default {
  name: "mergedrefresh",

  async seed({ configRoot }) {
    // A: TV shows only — contributes the "show" type and NO movies.
    mockA = await startMockJellyfin({
      views: [
        {
          id: "showsA",
          name: "Shows A",
          collectionType: "tvshows",
          movies: [{ id: "s1", name: "Sierra Show", year: 2019 }],
        },
      ],
    });
    // B: the sole Movies provider.
    mockB = await startMockJellyfin({
      views: [
        {
          id: "libB",
          name: "Library B",
          collectionType: "movies",
          movies: [{ id: "b1", name: "Beta One", year: 2021 }],
        },
      ],
    });
    // No source is selected: two sources with no selection is the merged
    // "All" scope, where the sidebar consolidates into type tabs.
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
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some((b) => b.textContent.trim() === 'Movies')`,
      "merged type tabs (two sources, no selection)",
    );
    const tabs0 = await sidebarNames(driver);
    assert.ok(
      tabs0.includes("Movies") && tabs0.includes("TV Shows"),
      "both providers' types must tab up in the merged scope",
    );

    // ── 1. Merged type-grid reload ──────────────────────────────────────
    // The new card must appear WITHOUT re-entering the tab: only the
    // `type-grid` content leg can put it there.
    await clickSide(driver, "Movies");
    await hasCard(driver, "Beta One");
    mockB.state.addMovie("libB", { id: "b2", name: "Beta Two", year: 2022 });
    await clickRefresh(driver);
    await settle(driver);
    await hasCard(driver, "Beta Two");
    await screenshot("01-merged-type-grid-reloaded");

    // ── 2. Partial aggregate must not force Home ─────────────────────────
    // Mock B (the SOLE Movies provider) fails its next /Users/{id}/Views, so
    // the refreshed aggregate carries A's shows only: the Movies type
    // DISAPPEARS from the tabs while the user is browsing it.
    mockB.state.failNextViews = true;
    await clickRefresh(driver);
    await settle(driver);
    const tabs = await sidebarNames(driver);
    assert.ok(
      !tabs.includes("Movies"),
      "the sole Movies provider failed, so Movies must drop from the refreshed tabs (else this case cannot detect its target regression)",
    );
    assert.ok(
      tabs.includes("TV Shows"),
      "the surviving source's type must remain",
    );
    assert.equal(
      await onHome(driver),
      false,
      "a partial aggregate must NOT force Home — a transient one-server failure is indistinguishable from deletion, so the merged scope runs no disappearance fallback",
    );
    assert.equal(
      await banner(driver),
      null,
      "a partial aggregate is normal, not a failure: no error banner",
    );
    // The content leg still ran (B answered the listing; only Views failed),
    // so the merged grid keeps its cards instead of emptying underneath us.
    assert.ok(
      (await posterLabels(driver)).some((l) => l?.startsWith("Beta")),
      "the merged grid keeps its content",
    );
    await screenshot("02-partial-aggregate-no-forced-home");

    // Mock A's own library is still fully browsable while B is degraded.
    await clickSide(driver, "TV Shows");
    await hasCard(driver, "Sierra Show");

    // ── Recovery: a follow-up Refresh reloads the Movies content leg ─────
    await clickRefresh(driver);
    await settle(driver);
    await pollUntil(
      async () =>
        (await sidebarNames(driver)).includes("Movies") ? true : null,
      "Movies returns to the tabs once B recovers",
    );
    await clickSide(driver, "Movies");
    await hasCard(driver, "Beta Two");
    // Added while the grid is already open: only a content-leg reload of the
    // merged listing can surface it — a sidebar-only refresh cannot.
    mockB.state.addMovie("libB", { id: "b3", name: "Beta Three", year: 2023 });
    await clickRefresh(driver);
    await settle(driver);
    await hasCard(driver, "Beta Three");
    await screenshot("03-recovered-content-leg");

    // Session-wide invariant: no listing contract violations anywhere (a
    // merged listing that asked A for Movies, or B for Series, would land
    // here rather than failing an assertion above).
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
