// Every failure reports on the surface that OWNS it — and on no other
// (.agents/plans/per-surface-status.md; owner decision 2026-07-14).
//
// This scenario exists because I claimed slices 2 and 4 could not be guarded here, and the
// claim was FALSE — twice over, both times because I reasoned about the code instead of
// reading it:
//
//   1. "the harness cannot fail a Play". It can. `play_by_key` RESOLVES THE STREAM before
//      it spawns mpv (commands.rs:2247), and the mock owns that endpoint.
//   2. "so seed a bogus mpv_path". That does nothing: `resolve_mpv` VALIDATES the
//      configured path and silently falls back to mpv on PATH (playback.rs:207). The first
//      version of this scenario did exactly that and timed out waiting for a failure that
//      was never going to come.
//
// The door is `failPlaybackInfo` on the mock: the stream resolve fails, so the Play fails,
// and a queue jump fails the same way (queue_play_at -> play_by_key). Which is all those
// two surfaces ever needed.
import assert from "node:assert/strict";
import { pollUntil, mockSource, seedConfig, openLibraryGrid, goHome } from "../helpers.mjs";
import { startMockJellyfin } from "../mockjf.mjs";

let mock;

const MOVIES = [
  { id: "m1", name: "Alpha One", year: 2001 },
  { id: "m2", name: "Beta Two", year: 2002 },
];

// The VIEW's banner. Nothing that belongs to another surface may ever appear here — that
// shared surface is what eight rounds of review defects came out of.
const banner = (driver) =>
  driver.exec(`return document.querySelector('div.error')?.textContent ?? null`);
const detailError = (driver) =>
  driver.exec(`return document.querySelector('div.detailerror')?.textContent ?? null`);
const drawerError = (driver) =>
  driver.exec(`return document.querySelector('div.drawererror')?.textContent ?? null`);
const chipFailed = (driver) =>
  driver.exec(`return !!document.querySelector('button.queuechip.failed')`);

async function ctxMenu(driver, prefix, label) {
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

export default {
  name: "surfaces",

  async seed({ configRoot }) {
    mock = await startMockJellyfin({ movies: MOVIES });
    seedConfig(configRoot, [mockSource(mock, { id: "jf", name: "Mock JF" })]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver }) {
    await openLibraryGrid(driver, {
      section: "Mock Library",
      cardPrefix: "Alpha One",
    });
    // Every playback attempt from here on fails at the stream resolve — deterministically,
    // and before mpv is ever spawned.
    mock.state.failPlaybackInfo = true;

    // ── 1. A Play failure from an OPEN DETAIL reports on the detail ─────────
    // Not on the view's banner underneath it. The detail layers OVER the grid and outlives
    // things that replace what is below it, so the view's clear is not its owner (r24).
    const card = await driver.find(
      "css selector",
      'button.poster[aria-label^="Alpha One"]',
    );
    await driver.click(card);
    await driver.waitFor(
      `return !!document.querySelector('.detail button.playwide')`,
      "the detail surface, with its Play button",
    );
    const play = await driver.find("css selector", ".detail button.playwide");
    await driver.click(play);

    await pollUntil(
      async () => ((await detailError(driver)) ? true : null),
      "the Play failure, reported ON the detail the user is looking at",
    );
    assert.ok(
      (await detailError(driver)).includes("Couldn't play"),
      "the line must say WHAT failed, not just dump the backend's string: giving a writer its own line is only half the job if the line cannot be told apart from the grid's (owner playtest, 0.1.46 — with Plex both were the SAME sentence, twice)",
    );
    assert.equal(
      await banner(driver),
      null,
      "...and the view's banner must not have been touched: a Play failure is not a fact about the grid",
    );

    // Closing the detail dismisses what it was saying — its surface is gone.
    const back = await driver.find(
      "css selector",
      ".detail button.back, .crumbs button.back",
    );
    await driver.click(back);
    await pollUntil(
      async () => ((await detailError(driver)) === null ? true : null),
      "the detail's failure goes with the detail",
    );

    // ── 2. A queue failure reports INSIDE the drawer ────────────────────────
    // Same spawn failure, reached through queue_play_at — a different surface, so a
    // different report.
    await ctxMenu(driver, "Beta Two", "Add to queue");
    const chip = await driver.find("css selector", "button.queuechip");
    await driver.click(chip);
    await driver.waitFor(
      `return !!document.querySelector('aside.drawer')`,
      "the queue drawer",
    );

    // Jump to the queued item: play_by_key again, and the stream still will not resolve.
    await driver.exec(
      `const rows = [...document.querySelectorAll('aside.drawer button')];
       const row = rows.find((b) => b.textContent.includes('Beta Two'));
       row.click();`,
    );
    await pollUntil(
      async () => ((await drawerError(driver)) ? true : null),
      "the queue's failure, reported inside the drawer it belongs to",
    );
    assert.ok(
      (await drawerError(driver)).includes("Couldn't play"),
      "...and it says what it was trying to do",
    );
    assert.equal(
      await banner(driver),
      null,
      "...and never on the view's banner — that is the door r24 found open (setError(null) wiped it on the next navigation)",
    );

    // ── 2b. A transport failure must not put the request URL on screen ─────
    // reqwest's message carries the WHOLE url. It tells the user nothing they can act on,
    // and it carries what must never be displayed: a Jellyfin user GUID and item key, and
    // — the reason this is a rule — Plex builds urls with `?X-Plex-Token=…` in the query
    // (plex_library.rs:878). Repo guidance forbids a token-bearing url in an error or any
    // UI text, and the only way to keep that promise is to never let a raw url through the
    // funnel at all.
    //
    // The mock is still up here, so a transport error is not reachable in-scenario — but
    // the funnel is: assert that NOTHING any surface displays ever contains one.
    const surfacesText = await driver.exec(
      `return [...document.querySelectorAll('div.error, div.scanerror, div.drawererror, div.detailerror, div.mpverror')]
         .map((e) => e.textContent).join(' | ')`,
    );
    assert.ok(
      !/https?:\/\/\S*\?/.test(surfacesText),
      `no surface may display a url QUERY — that is where the secret is (Plex: ?X-Plex-Token=…). The path may stay; it is diagnostic and carries nothing. Got ${JSON.stringify(surfacesText)}`,
    );
    assert.ok(
      !/token/i.test(surfacesText),
      `...and no surface may display anything that looks like a token — got ${JSON.stringify(surfacesText)}`,
    );

    // ── 3. Closing the drawer dismisses what it was reporting ───────────────
    // The user has seen it. Without this a queue failure has nothing that can ever clear
    // it, and would sit over every view forever.
    // Its OWN close button — the chip is behind the drawer's backdrop, which is also why
    // "navigate with the drawer open" is not a state that exists (see case 4).
    const close = await driver.find("css selector", "aside.drawer button.drawerclose");
    await driver.click(close);
    await driver.waitFor(
      `return !document.querySelector('aside.drawer')`,
      "the drawer closed",
    );
    assert.equal(
      await drawerError(driver),
      null,
      "the drawer's failure goes with the drawer",
    );

    // ── 4. A queue failure the drawer cannot show goes on the CHIP ─────────
    // The ONLY reachable queue failure is a jump from the open drawer — "Play next" and
    // "Add to queue" just insert, and cannot fail. So the state the chip's mark exists for
    // is: start a play, close the drawer, and have it fail with the drawer shut.
    //
    // That state was UNREACHABLE until this commit, because closing the drawer abandoned
    // the in-flight action. It dropped a real failure on the floor — the user asked for a
    // play and got neither the play nor a word about it — and it made the chip's mark dead
    // code. Building this case is what found it.
    await ctxMenu(driver, "Beta Two", "Add to queue");
    const chip3 = await driver.find("css selector", "button.queuechip");
    await driver.click(chip3);
    await driver.waitFor(
      `return !!document.querySelector('aside.drawer')`,
      "the queue drawer",
    );

    mock.state.playbackInfoDelayMs = 4000; // the play is still in flight when we leave
    await driver.exec(
      `const rows = [...document.querySelectorAll('aside.drawer button')];
       const row = rows.find((b) => b.textContent.includes('Beta Two'));
       row.click();`,
    );
    await pollUntil(
      async () => (mock.state.playbackInfoDelayMs === 0 ? true : null),
      "the doomed play to actually reach the server (or the case guards nothing)",
    );

    const close2 = await driver.find("css selector", "aside.drawer button.drawerclose");
    await driver.click(close2);
    await driver.waitFor(
      `return !document.querySelector('aside.drawer')`,
      "the drawer closed while the play is still in flight",
    );

    await pollUntil(
      async () => ((await chipFailed(driver)) ? true : null),
      "the play failed after the drawer was shut: the chip is the only surface left, and a failure with nowhere to appear is a failure lost",
    );
    assert.equal(
      await banner(driver),
      null,
      "...and still nothing on the view's banner",
    );

    // ── 5. ...and navigating does not erase it ──────────────────────────────
    // THE r24 DEFECT, guarded. Every load start calls setError(null); before the surfaces
    // were split, that wiped the queue's failure — still true, still the user's to act on.
    await goHome(driver);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem.active')].some((b) => b.textContent.trim() === 'Home')`,
      "Home",
    );
    assert.ok(
      await chipFailed(driver),
      "a load starting fresh says nothing about the queue: clearing the view is not a licence to delete the queue's failure (codex + grok, r24)",
    );
    assert.equal(
      await banner(driver),
      null,
      "still nothing on the view's banner",
    );
  },
};
