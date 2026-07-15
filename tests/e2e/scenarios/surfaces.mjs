// Every failure reports on the surface that OWNS it — and on no other
// (.agents/plans/per-surface-status.md; owner decision 2026-07-14).
//
// This scenario exists because I claimed the detail failure could not be
// guarded here. The claim was false because I reasoned about the code instead
// of reading it:
//
// "The harness cannot fail a Play" was wrong. A NUL in mpv's configured extra
// args passes stream resolution but makes Command::spawn fail deterministically,
// which exercises the exact post-resolve / pre-success boundary. A merely bogus
// `mpv_path` would not: resolve_mpv validates it and falls back to PATH. The edit
// leg separately proves a watch-state failure stays on the edit's own line.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { pollUntil, mockSource, seedConfig, openLibraryGrid } from "../helpers.mjs";
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
const editError = (driver) =>
  driver.exec(`return document.querySelector('div.scanerror')?.textContent ?? null`);

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
    seedConfig(configRoot, [mockSource(mock, { id: "jf", name: "Mock JF" })], {
      recents: [],
      hidden_from_continue: ["jf:m1", "sentinel"],
      mpv_extra_args: "\0",
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, configRoot }) {
    const configFile = path.join(configRoot, "config", "vela", "config.json");
    await openLibraryGrid(driver, {
      section: "Mock Library",
      cardPrefix: "Alpha One",
    });
    // Every playback attempt from here on resolves successfully, then fails
    // before mpv can spawn because argv contains the configured NUL.

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
    assert.ok(
      mock.state.served.some(
        (r) => r.path === "/Items/m1/PlaybackInfo" && r.status === 200,
      ),
      "stream resolution succeeded before the deterministic mpv spawn failure",
    );
    const failedPlayConfig = JSON.parse(fs.readFileSync(configFile, "utf8"));
    assert.deepEqual(
      failedPlayConfig.recents ?? [],
      [],
      "a failed launch must not create a recent",
    );
    assert.deepEqual(
      failedPlayConfig.hidden_from_continue ?? [],
      ["jf:m1", "sentinel"],
      "a failed play must not clear either its tombstone or unrelated state",
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

    // ── 2. A failed watch-state edit reports on the edit line ──────────────
    mock.state.unauthNextPlayed = true;
    await ctxMenu(driver, "Beta Two", "Mark watched");
    await pollUntil(
      async () => ((await editError(driver)) ? true : null),
      "the failed edit on its own line",
    );
    assert.ok(
      (await editError(driver)).includes("Couldn't mark"),
      "the edit line must name the failed action",
    );
    assert.equal(
      await banner(driver),
      null,
      "a failed edit is not a fact about the grid",
    );

    // ── 3. A transport failure must not put the request URL on screen ──────
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
      `return [...document.querySelectorAll('div.error, div.scanerror, div.detailerror, div.mpverror')]
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

  },
};
