// Server scan trigger (.agents/plans/library-refresh-scan.md, slice 2):
// one app session, 5 cases in plan order against a single JF mock. The mock
// serves TWO scannable libraries (lib1/lib2 with matching VirtualFolders
// entries) and ONE grouped view (grouped1: sidebar entry exists, but its
// VirtualFolders entry is removed BEFORE app launch — the real grouped-folder
// shape, view id ∉ VirtualFolders ItemIds).
//
// RED without slice 2: no "Scan Library" menu entry, no request.
import assert from "node:assert/strict";
import {
  pollUntil,
  mockSource,
  seedConfig,
  allDelivered,
  holdsFor,
} from "../helpers.mjs";
import { startMockJellyfin } from "../mockjf.mjs";

let mock;

const DELAY = 900; // in-flight window; WebDriver roundtrips are ~10ms each

const GROUPED_MSG = "groups multiple server libraries";
const FORBIDDEN_MSG = "administrator permission required";

async function notice(driver) {
  return driver.exec(
    `return document.querySelector('div.notice')?.textContent ?? null`,
  );
}
// A SCAN's failure has its own surface (`div.scanerror`), separate from the
// view's error banner (`div.error`) — a scan may not touch the view's, and the
// two can be on screen at once (codex r15). Assert on the right one.
async function banner(driver) {
  return driver.exec(
    `return document.querySelector('div.scanerror')?.textContent ?? null`,
  );
}
async function viewBanner(driver) {
  return driver.exec(
    `return document.querySelector('div.error')?.textContent ?? null`,
  );
}
// The scan entry lives in the sidebar section context menu; WebDriver classic
// has no right-click, so dispatch a real `contextmenu` MouseEvent on the
// sidebar button (the app's oncontextmenu handler opens the menu).
async function openScanMenu(driver, label) {
  await driver.exec(
    `const b = [...document.querySelectorAll('button.sideitem')].find((x) => x.textContent.trim() === ${JSON.stringify(label)});
     if (!b) throw new Error('no sidebar entry ${label}');
     const r = b.getBoundingClientRect();
     b.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + 8, clientY: r.y + 8 }));`,
  );
  await driver.waitFor(
    `return !!document.querySelector('.ctxmenu')`,
    "section context menu open",
  );
}
async function scanVia(driver, label) {
  await openScanMenu(driver, label);
  const item = await driver.find(
    "xpath",
    `//div[contains(@class,'ctxmenu')]//button[@role='menuitem' and normalize-space(.)='Scan Library']`,
  );
  await driver.click(item);
}
const refreshPosts = (id) =>
  mock.state.requests.filter(
    (r) => r.method === "POST" && r.path === `/Items/${id}/Refresh`,
  );
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export default {
  name: "scanlib",

  async seed({ configRoot }) {
    mock = await startMockJellyfin({
      views: [
        {
          id: "lib1",
          name: "Library One",
          collectionType: "movies",
          movies: [{ id: "m1", name: "Mock Movie One", year: 2020 }],
        },
        {
          id: "grouped1",
          name: "Grouped Stuff",
          collectionType: "movies",
          movies: [],
        },
        {
          id: "lib2",
          name: "Library Two",
          collectionType: "movies",
          movies: [],
        },
      ],
    });
    // grouped1 keeps its sidebar entry (state.views) but loses its
    // VirtualFolders entry — the grouped-folder shape the resolver refuses.
    mock.state.virtualFolders = mock.state.virtualFolders.filter(
      (f) => f.ItemId !== "grouped1",
    );
    seedConfig(configRoot, [mockSource(mock, { id: "jf-scan" })]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some((b) => b.textContent.trim() === 'Library One')`,
      "libraries in the sidebar",
    );

    // ── 1. Happy path ───────────────────────────────────────────────────
    // Scan lib1 → resolution GET precedes POST /Items/lib1/Refresh carrying
    // the FULL scan_query param set → transient notice rendered.
    await scanVia(driver, "Library One");
    await pollUntil(
      async () => (await notice(driver)) === "Scan started — Library One",
      "scan notice for Library One",
    );
    assert.equal(await banner(driver), null, "happy path must not banner");
    assert.equal(
      await viewBanner(driver),
      null,
      "and must not disturb the view's own banner",
    );
    const posts1 = refreshPosts("lib1");
    assert.equal(posts1.length, 1, "exactly one refresh POST for lib1");
    // Assert what Vela SENDS, param for param. Checking only Recursive and
    // RegenerateTrickplay let a scan silently become a destructive metadata
    // rewrite (ReplaceAllMetadata=true) on the user's real server without a
    // single test going red (lrs-7).
    assert.deepEqual(
      {
        Recursive: posts1[0].query.Recursive,
        MetadataRefreshMode: posts1[0].query.MetadataRefreshMode,
        ImageRefreshMode: posts1[0].query.ImageRefreshMode,
        ReplaceAllMetadata: posts1[0].query.ReplaceAllMetadata,
        ReplaceAllImages: posts1[0].query.ReplaceAllImages,
        RegenerateTrickplay: posts1[0].query.RegenerateTrickplay,
      },
      {
        Recursive: "true",
        MetadataRefreshMode: "Default",
        ImageRefreshMode: "Default",
        ReplaceAllMetadata: "false",
        ReplaceAllImages: "false",
        RegenerateTrickplay: "false",
      },
      "the scan must stay a plain non-destructive scan",
    );
    const vfGet = mock.state.requests.findIndex(
      (r) => r.method === "GET" && r.path === "/Library/VirtualFolders",
    );
    assert.ok(vfGet >= 0, "VirtualFolders resolution GET must occur");
    assert.ok(
      vfGet < mock.state.requests.indexOf(posts1[0]),
      "resolution GET must precede the refresh POST",
    );
    await screenshot("scanlib-01-notice");

    // ── 2. Grouped view refused ─────────────────────────────────────────
    // grouped1 has no VirtualFolders entry → guidance banner, and NO
    // POST /Items/grouped1/Refresh (the false-success case this guards is a
    // 204 with nothing scanned).
    await scanVia(driver, "Grouped Stuff");
    await pollUntil(
      async () => ((await banner(driver)) ?? "").includes(GROUPED_MSG),
      "grouped-libraries guidance banner",
    );
    assert.equal(
      refreshPosts("grouped1").length,
      0,
      "grouped view must never reach the refresh POST",
    );
    await screenshot("scanlib-02-grouped");

    // ── 3. Non-admin at resolution ──────────────────────────────────────
    // The step every real non-admin hits: the VirtualFolders GET itself is
    // elevation-gated. Friendly banner, and NO refresh POST occurs.
    mock.state.failNextVirtualFolders = true;
    const lib1PostsBefore = refreshPosts("lib1").length;
    await scanVia(driver, "Library One");
    await pollUntil(
      async () => ((await banner(driver)) ?? "").includes(FORBIDDEN_MSG),
      "friendly administrator-permission banner",
    );
    assert.equal(
      refreshPosts("lib1").length,
      lib1PostsBefore,
      "failed resolution must not be followed by a refresh POST",
    );

    // ── 4. Retry lifecycle ──────────────────────────────────────────────
    // 403 on the POST → admin-refusal banner; scan again → success notice
    // shown and the stale failure banner GONE (per-attempt exclusivity).
    mock.state.failNextItemRefresh = true;
    await scanVia(driver, "Library One");
    await pollUntil(
      async () => ((await banner(driver)) ?? "").includes(FORBIDDEN_MSG),
      "admin-refusal banner from the refresh POST",
    );
    await scanVia(driver, "Library One");
    await pollUntil(
      async () => (await notice(driver)) === "Scan started — Library One",
      "retry publishes the success notice",
    );
    assert.equal(
      await banner(driver),
      null,
      "retry success must clear the stale failure banner",
    );

    // Exclusivity runs BOTH ways. Only failure-then-success was covered, so
    // deleting `scanNotice = null` at attempt start left every assertion green
    // while a failing scan displayed its banner NEXT TO the previous attempt's
    // "Scan started" — reporting a scan that failed as if it had begun
    // (codex r3). A success notice is on screen right now; fail the next one.
    mock.state.failNextItemRefresh = true;
    await scanVia(driver, "Library One");
    await pollUntil(
      async () => ((await banner(driver)) ?? "").includes(FORBIDDEN_MSG),
      "the failing attempt banners",
    );
    assert.equal(
      await notice(driver),
      null,
      "a failing attempt must clear the previous attempt's success notice",
    );

    // ── 5. Out-of-order completions ─────────────────────────────────────
    // Phase 1: lib1 slow, lib2 fast — lib2's success notice appears; when
    // lib1's delayed response lands, the published status must NOT change
    // (latest-attempt ownership across ALL sections, not per key).
    mock.state.itemRefreshDelayMs = DELAY;
    const lib1Before = refreshPosts("lib1").length;
    await scanVia(driver, "Library One"); // POST parked for DELAY ms
    // The one-shot delay binds to whichever POST ARRIVES first, and scanVia
    // returns at menu click — before the app's GET→POST chain. Wait until
    // lib1's POST is logged so lib2's cannot capture the delay and false-pass
    // the out-of-order case (codex code review r1, finding 3).
    await pollUntil(
      async () => (refreshPosts("lib1").length > lib1Before ? true : null),
      "lib1's POST arrived and captured the one-shot delay",
    );
    await scanVia(driver, "Library Two"); // responds immediately
    await pollUntil(
      async () => (await notice(driver)) === "Scan started — Library Two",
      "fast lib2 notice while lib1 is in flight",
    );
    const lib1Landed = refreshPosts("lib1").length;
    // Wait for the parked 204 to be DELIVERED, not for a stopwatch to expire: the
    // request count records ARRIVAL, so a sleep sized from the delay can assert while
    // the stale result is still pending and pass a missing scan-attempt gate (codex
    // r21).
    await pollUntil(
      async () => (allDelivered(mock, "/Refresh") ? true : null),
      "lib1's parked 204 to be delivered",
    );
    assert.equal(
      refreshPosts("lib1").length,
      lib1Landed,
      "lib1's POST was already recorded at arrival (delay is respond-side)",
    );
    assert.equal(
      await notice(driver),
      "Scan started — Library Two",
      "stale lib1 completion must not overwrite lib2's notice",
    );
    assert.equal(
      await banner(driver),
      null,
      "stale lib1 completion must not publish a banner",
    );

    // Phase 2: both successful — lib1's armed auto-clear timer may not wipe
    // lib2's newer notice once lib1's original ~4s deadline passes. lib2's
    // POST is delayed so its own legitimate auto-clear (its arming + 4s)
    // falls well AFTER lib1's deadline, keeping the assertion window wide.
    await scanVia(driver, "Library One");
    await pollUntil(
      async () => (await notice(driver)) === "Scan started — Library One",
      "lib1 success notice (timer armed)",
    );
    const t0 = Date.now(); // lib1's auto-clear deadline ≈ t0 + 4000
    mock.state.itemRefreshDelayMs = 1500;
    await scanVia(driver, "Library Two");
    await pollUntil(
      async () => (await notice(driver)) === "Scan started — Library Two",
      "lib2 notice supersedes lib1's",
    );
    // lib2's timer is armed NOW — the expiry deadline below is measured from
    // here, not from t0. Measuring from lib1's t0 (after first waiting past
    // lib1's deadline) left ~2.8s of slack, so a timer stretched to 7-8s still
    // cleared inside the window and passed (codex r5).
    const armed = Date.now();
    const past = t0 + 4300 - Date.now();
    if (past > 0) await sleep(past);
    assert.equal(
      await notice(driver),
      "Scan started — Library Two",
      "lib1's expired timer must not clear lib2's notice",
    );

    // ...and the OWNING notice must actually expire, on the ~4s the app
    // PROMISES. Everything above only proves an older timer cannot clear a
    // newer notice, so a no-op auto-clear passed while "Scan started" stuck on
    // screen forever (codex r3). The deadline is measured from when lib2's
    // notice APPEARED (that is when its timer is armed) — measuring from lib1's
    // t0 left so much slack that a timer stretched to 8-10s still passed
    // (codex r4). Budget: 4s promise + 2s for WebDriver/CI jitter.
    const budget = 6000; // the app's 4s promise + WebDriver/CI jitter
    const left = Math.max(budget - (Date.now() - armed), 500);
    await pollUntil(
      async () => ((await notice(driver)) === null ? true : null),
      "the owning attempt's notice auto-clears on its promised ~4s deadline",
      { timeoutMs: left },
    );
    const took = Date.now() - armed;
    assert.ok(
      took <= budget,
      `the notice must clear on its ~4s promise, not linger (took ${took}ms from arming)`,
    );

    // Phase 3: stale FAILURE. Phases 1-2 only ever superseded a stale
    // SUCCESS, so the failure half of latest-attempt ownership
    // (`if (scanAttempt !== attempt) return` in scanSection's catch) had no
    // guard: delete it and everything above stays green while a slow scan's
    // 403 lands on top of a newer scan's success notice (lrs-5). This phase
    // is only expressible because the mock now binds failNextItemRefresh at
    // request ARRIVAL — bound at respond time, fast lib2 would steal the 403
    // armed for parked lib1.
    mock.state.itemRefreshDelayMs = DELAY;
    mock.state.failNextItemRefresh = true; // both bind to lib1's POST on arrival
    const lib1Before3 = refreshPosts("lib1").length;
    await scanVia(driver, "Library One"); // parked, WILL FAIL 403
    await pollUntil(
      async () => (refreshPosts("lib1").length > lib1Before3 ? true : null),
      "lib1's POST arrived and captured BOTH one-shots (delay + failure)",
    );
    await scanVia(driver, "Library Two"); // fast success, supersedes lib1
    await pollUntil(
      async () => (await notice(driver)) === "Scan started — Library Two",
      "lib2's success notice while lib1's failure is still parked",
    );
    await pollUntil(
      async () => (allDelivered(mock, "/Refresh") ? true : null),
      "lib1's parked 403 to be delivered",
    );
    // ...and HOLD it: a banner that appears a tick after a single sample is a banner
    // this case was written to catch.
    await holdsFor(
      async () => await banner(driver),
      1500,
      "a superseded scan's FAILURE must not banner over a newer success",
    );
    assert.equal(
      await notice(driver),
      "Scan started — Library Two",
      "the newer scan's notice survives the stale failure",
    );
  },
};
