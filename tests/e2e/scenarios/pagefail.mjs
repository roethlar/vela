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
// not a witness: it says when the request arrived, never when the client was handed the
// answer and ran the code under test (codex + grok, r21).
//
// So: wait for the parked response to actually be SERVED — the catch resumes on it, and
// deciding whether to publish is the very next thing it does — and then hold the window
// OPEN, failing the moment a banner appears rather than sampling once at the end.
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
    // Wait for the REPAINT to settle, not just for "a banner". The edit's repaint
    // clears the banner as it starts and the edit publishes only once it is done
    // (codex r18) — so a bare non-null poll would return the PREVIOUS banner, and
    // the capture below would land in the window where it has been cleared.
    await pollUntil(
      async () =>
        (await cardCount(driver)) === 60 && (await banner(driver)) ? true : null,
      "the failed edit's banner, once its repaint has settled",
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

    // The banner published during the run must be UNTAGGED — a failed EDIT, not a
    // failed listing. A tagged listing banner was already preserved by the narrow
    // predicate this widening replaced (`errorGen > gridActionBaseGen &&
    // errorGen === loadGen`), so arming one would leave the widening unguarded:
    // restore the narrow condition and the case would still pass (grok r18).
    mock.state.unauthNextPlayed = true;
    await watchToggle(driver, "Movie 059", "Mark watched"); // the EDIT 401s
    await pollUntil(
      async () => ((await banner(driver)) ? true : null),
      "the failed edit's banner (untagged — nobody's generation owns it)",
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

    // ── 4. A failed edit whose own repaint ALSO fails must report BOTH ──
    // Cases 2 and 3 both let the edit's recovery repaint SUCCEED, so neither one
    // exercises the ordering at all: delete the `await` in setWatched and both stay
    // green (codex r19). The repaint CLEARS the banner as it starts and publishes
    // its OWN failure when it lands, so the edit and its recovery are two writers
    // racing for one surface — and whoever loses it, the user is missing something
    // they need: what became of the change they asked for, or why the grid it left
    // behind is empty. Both are true. Both have to be on screen.
    //
    // The two failures must RENDER DIFFERENTLY or no assertion can tell which one
    // survived — two 401s collapse into the same RECONNECT_REQUIRED sentence (codex
    // r12), which is how case 27 in refresh.mjs came to guard nothing. So: a 401
    // edit against a 500 listing.
    mock.state.unauthNextPlayed = true; // the edit 401s...
    mock.state.failNextItems = true; // ...and the repaint it kicks off 500s
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () =>
        (await cardCount(driver)) === 0 && (await banner(driver)) ? true : null,
      "the repaint fails too, leaving an empty grid and a banner",
    );
    const bothWriters = await banner(driver);
    assert.ok(
      bothWriters.includes("reconnect"),
      `the edit is what the user ASKED for, and its failure must survive its own recovery — drop the await and the failing repaint publishes last, erasing it — got ${JSON.stringify(bothWriters)}`,
    );
    assert.ok(
      bothWriters.includes("500"),
      `...and the repaint's failure is the only thing explaining the empty grid it left behind — publish the edit alone and it is erased — got ${JSON.stringify(bothWriters)}`,
    );

    // ── 5. A failed edit must not paint on a root the user has LEFT ──
    // The publish waits for the repaint; the user does not. A slow recovery leaves
    // time to go Home, whose own load clears the surface — and the edit's failure
    // then lands on a view it says nothing about, covering that view's status
    // (codex + grok, r19).
    //
    // LEAVING is the surface changing AND a new load taking over the content, which
    // is what goHome does. A modal opening moves `navEpoch` alone and must NOT drop
    // the message: the grid, and the item the edit was about, are still right there.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    mock.state.unauthNextPlayed = true; // the edit 401s...
    mock.state.itemsDelayMs = 6000; // ...and its recovery repaint parks
    const served5 = servedCount("/Items");
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(armedShotsBound, "the 401 edit and its parked repaint to both bind");
    await goHome(driver);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem.active')].some((b) => b.textContent.trim() === 'Home')`,
      "Home",
    );
    await noEditBannerAfterParked(driver, {
      endsWith: "/Items",
      before: served5,
      what: "the user left the grid this edit was about: its failure describes a view that is gone, and does not belong pasted over the one they are standing on",
    });

    // ── 6. A successful refresh must not RETRACT a failed edit's message ──
    // The banner can hold two failures with different OWNERS: a listing failure, owned
    // by the load that produced it and retractable once a refresh replaces those cards
    // (codex r11), and an edit's failure, owned by no load and retractable by nobody.
    // r19 combined them into one string under the LISTING's tag — so the refresh's
    // retract took the edit's message with it, and a user whose grid was repaired
    // never learned their change had failed (codex + grok, r20). The same loss r19
    // fixed, arriving through the retract door instead of the publish door.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    // A refresh that will SUCCEED, but slowly: it stays in flight while the edit runs,
    // then claims the grid and repairs it — which is what arms the retract branch.
    mock.state.viewsDelayMs = 6000;
    const refresh4 = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh4);

    // Inside that window: the edit 401s and its recovery repaint 500s. The repaint
    // claims a generation NEWER than the action's, so it is not silenced — it publishes
    // a TAGGED listing failure, and the edit adds its untagged one.
    mock.state.unauthNextPlayed = true;
    mock.state.failNextItems = true;
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () => {
        const b = await banner(driver);
        return b && b.includes("500") && b.includes("reconnect") ? true : null;
      },
      "both failures on screen before the refresh settles",
    );

    // The refresh now claims the grid and succeeds. Its cards replace the ones the
    // failed repaint never loaded, so the LISTING diagnostic is superseded — and the
    // retract must take that, and only that.
    await settle(driver);
    mock.state.viewsDelayMs = 0;
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "the refresh repairs the grid",
    );
    const afterRetract = await banner(driver);
    assert.ok(
      afterRetract && afterRetract.includes("reconnect"),
      `a refresh may retract the listing failure it superseded — never the edit failure it did not: the user's change still failed and nothing else will tell them — got ${JSON.stringify(afterRetract)}`,
    );
    assert.ok(
      !afterRetract.includes("500"),
      `...and the listing diagnostic IS the refresh's to take back: its cards are on screen now — got ${JSON.stringify(afterRetract)}`,
    );

    // ── 7. A leave that moves NO load generation must still drop the edit ──
    // `navEpoch` and `loadGen` together are not a proof of root identity, in either
    // direction (codex + grok, r20). Here is the first direction: tearing down a search
    // root bumps `navEpoch` and NOTHING else — no load runs — so a gate that demands
    // both counters move keeps the edit's failure and paints it on the torn-down view.
    // (The real one that bit us is the Plex link screen, which replaces the whole view
    // while bumping no load generation; it needs plex.tv, so the search teardown stands
    // in for it — the same hole, reachable from a mock.)
    const box = await driver.find(
      "css selector",
      'input[aria-label="Search your libraries"]',
    );
    await driver.type(box, `Movie 05${ENTER}`);
    await pollUntil(
      async () => ((await cardCount(driver)) > 0 ? true : null),
      "the search root, with results",
    );
    mock.state.unauthNextPlayed = true; // the edit 401s...
    mock.state.itemsDelayMs = 6000; // ...and the search re-run it kicks off parks
    const served7 = servedCount("/Items");
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(armedShotsBound, "the 401 edit and its parked re-run to both bind");
    // Tear the search root down: a one-character query. No load; only navEpoch moves.
    await driver.exec(
      `const i = document.querySelector('input[aria-label="Search your libraries"]');
       i.value = ''; i.dispatchEvent(new Event('input', { bubbles: true }));`,
    );
    await driver.type(box, `M${ENTER}`);
    await driver.waitFor(
      `return document.body.innerText.includes('Search needs at least 2 characters.')`,
      "the search root torn down",
    );
    await noEditBannerAfterParked(driver, {
      endsWith: "/Items",
      before: served7,
      what: "the search root the edit was made in is gone — its failure does not belong on what replaced it, and no load generation moved to say so",
    });

    // ── 8. Re-entering the SAME root must not drop the edit ──
    // The other direction. Re-selecting the library you are already in bumps BOTH
    // counters and goes nowhere: the grid, and the item, are exactly where they were.
    // A gate that reads "both moved" as "the user left" silently swallows the failure
    // (codex r20) — the same silent loss r18 and r19 were both spent fixing.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "back on the library grid",
    );
    mock.state.unauthNextPlayed = true; // the edit 401s...
    mock.state.itemsDelayMs = 6000; // ...and its recovery repaint parks
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(armedShotsBound, "the 401 edit and its parked repaint to both bind");
    // Re-select the library we are ALREADY in: navEpoch++ and a fresh resetAndLoad
    // (loadGen++), same root.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    // A POSITIVE assertion can simply wait for the thing it wants, so poll rather than
    // sleep: a late publish should not read as a failure.
    await pollUntil(
      async () => {
        const b = await banner(driver);
        return b && b.includes("reconnect") ? true : null;
      },
      "the user never left — they re-entered the library they were standing in, and their edit still failed, so it must be reported",
    );

    // ── 9. The root is the one the edit was MADE in, not the one it lands in ──
    // The edit's own server call is the LONGEST wait in setWatched, and the user can
    // leave during it. Reading the root in the catch — after the call has already
    // failed — reads whatever root they walked to, compares it against itself, and
    // always matches: the failure then lands on a view it says nothing about, covering
    // that view's status (codex r20). The signature has to be taken BEFORE the call.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    mock.state.unauthNextPlayed = true; // the edit 401s...
    mock.state.playedDelayMs = 6000; // ...but not for six seconds
    const served9 = servedCount("/PlayedItems/m59");
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () =>
        !mock.state.unauthNextPlayed && mock.state.playedDelayMs === 0 ? true : null,
      "the parked, doomed edit to reach the server",
    );
    // Leave while it is still in flight — the failure has not happened yet.
    await goHome(driver);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem.active')].some((b) => b.textContent.trim() === 'Home')`,
      "Home",
    );
    await noEditBannerAfterParked(driver, {
      endsWith: "/PlayedItems/m59",
      before: served9,
      what: "the edit was made in a library the user has since left: its failure belongs to that grid, not to the Home they are standing on",
    });

    // ── 10. Two failures, ONE sentence — the survivor must be the weaker claim ──
    // A 401 on a listing and a 401 on an edit both collapse into the same constant
    // RECONNECT_REQUIRED sentence. Deduplicating the banner on TEXT alone silently
    // decides ownership: the tagged listing part is already there, the edit's untagged
    // part is dropped as a duplicate, and the refresh then retracts the only line left —
    // the grid is repaired and nothing says the edit failed (codex r21).
    //
    // Case 6 cannot see this: it deliberately uses a 500 against a 401 so its assertions
    // can tell the two apart. The identical-text case is the one that breaks.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    mock.state.viewsDelayMs = 6000; // a refresh that will SUCCEED, slowly
    const refresh5 = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh5);

    mock.state.unauthNextPlayed = true; // the edit 401s...
    mock.state.unauthNextItems = true; // ...and its repaint 401s with the SAME sentence
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () => {
        const b = await banner(driver);
        return b && b.includes("reconnect") ? true : null;
      },
      "the one sentence both failures produce",
    );

    await settle(driver);
    mock.state.viewsDelayMs = 0;
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "the refresh repairs the grid",
    );
    const survivor = await banner(driver);
    assert.ok(
      survivor && survivor.includes("reconnect"),
      `the refresh repaired the grid, which retires the LISTING's reason for this sentence — but the edit's reason still holds, and dropping the line leaves the user with no sign their change failed — got ${JSON.stringify(survivor)}`,
    );

    // ── 11. A page failure must not take the edit's message with it ──
    // The ownership algebra lived in addError, but setError REPLACED the whole list —
    // and every listing writer uses setError. So an ordinary page failure wiped the
    // edit's untagged part, or (when both render the same 401 sentence) re-tagged it as
    // listing-owned; the refresh's retract then took it. The r21 silent loss, reverse
    // ordering, through the setError door (codex + grok, r22).
    //
    // Case 10 cannot see it: there the edit publishes LAST, so addError runs last and the
    // algebra saves it. Here the listing failure comes second.
    //
    // Everything must happen INSIDE an in-flight refresh. The refresh CLICK clears the
    // surface, so an edit published before it is gone whatever the ownership rules say —
    // a first draft of this case "went red" for exactly that reason and would have
    // guarded nothing.
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    mock.state.viewsDelayMs = 6000; // a refresh that will SUCCEED, slowly
    const refresh6 = await driver.find("css selector", "button.refreshbtn");
    await driver.click(refresh6);

    // The edit fails; its recovery repaint SUCCEEDS and claims a generation NEWER than
    // the action's. One untagged part on screen, over a healthy grid.
    mock.state.unauthNextPlayed = true;
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () => {
        const b = await banner(driver);
        return b && b.includes("reconnect") && (await cardCount(driver)) === 60
          ? true
          : null;
      },
      "the edit's failure, over a healthy repainted grid",
    );

    // Now an ordinary page failure — the SAME 401 sentence, published by a LISTING.
    // Newer than the action's generation, so it is published rather than silenced.
    mock.state.unauthNextItems = true;
    await scrollGridToEnd(driver);
    await pollUntil(
      async () => (mock.state.unauthNextItems === false ? true : null),
      "the doomed page request to reach the server",
    );

    // The refresh claims the grid and succeeds. It may retract the PAGE's diagnostic —
    // those cards are back — but the edit's failure was never its to take.
    await settle(driver);
    mock.state.viewsDelayMs = 0;
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "the refresh repairs the grid",
    );
    const afterPageFail = await banner(driver);
    assert.ok(
      afterPageFail && afterPageFail.includes("reconnect"),
      `a page failure that renders the same sentence as the edit's must not hand the edit's message to the refresh's retract — got ${JSON.stringify(afterPageFail)}`,
    );

    // ── 12. The recovery repaint belongs to the edit's grid, not to wherever the ──
    //         user ended up
    // The catch repainted the CURRENT root before checking whether it was still the
    // edit's root. So an edit parked in a library, with the user since gone Home, reset
    // HOME — clearing Home's own still-applicable failure — and only then noticed the
    // root had moved and bailed. The user loses a diagnostic that was nothing to do with
    // the edit, on a view they are actually looking at (codex r22).
    await openLibraryGrid(driver, {
      section: "Big Library",
      cardPrefix: "Movie 000",
    });
    await pollUntil(
      async () => ((await cardCount(driver)) === 60 ? true : null),
      "a healthy grid to edit from",
    );
    mock.state.unauthNextPlayed = true; // the edit will 401...
    mock.state.playedDelayMs = 6000; // ...but not for six seconds
    const served12 = servedCount("/PlayedItems/m59");
    await watchToggle(driver, "Movie 059", "Mark watched");
    await pollUntil(
      async () =>
        !mock.state.unauthNextPlayed && mock.state.playedDelayMs === 0 ? true : null,
      "the parked, doomed edit to reach the server",
    );

    // Leave for Home, and give Home a failure of its OWN — nothing to do with the edit,
    // and not superseded by anything.
    mock.state.failNextLatest = true;
    await goHome(driver);
    await pollUntil(
      async () => {
        const b = await banner(driver);
        return b && b.includes("500") ? true : null;
      },
      "Home's own failure, on the view the user is now looking at",
    );

    // The parked edit now fails. It must not touch this view at all.
    await pollUntil(
      async () => (servedCount("/PlayedItems/m59") > served12 ? true : null),
      "the parked 401 to be delivered",
    );
    await holdsFor(
      async () => {
        const b = await banner(driver);
        return b && b.includes("500") ? null : `Home's failure is gone (banner: ${JSON.stringify(b)})`;
      },
      4000,
      "an edit made in a library the user has left must not reset Home, nor clear the failure Home is reporting",
    );

    assert.equal(
      mock.state.contractViolations.length,
      0,
      `mock contract clean — got ${JSON.stringify(mock.state.contractViolations)}`,
    );
  },
};
