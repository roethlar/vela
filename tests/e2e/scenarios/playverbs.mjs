// Explicit playback verbs replace the deleted ephemeral queue. Context menus
// and details distinguish Play from Resume / Play from Beginning; the Continue
// Watching card itself remains the sole visible Play/Resume control. The mpv
// start-position checks prove the distinct verbs drive distinct behavior.
import assert from "node:assert/strict";
import path from "node:path";
import {
  goHome,
  makeClips,
  mockSource,
  openLibraryGrid,
  playAndQuit,
  seedConfig,
} from "../helpers.mjs";
import { startMockJellyfin } from "../mockjf.mjs";

let mock;

async function openContextMenu(driver, title) {
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^="${title}"]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', {
       bubbles: true,
       cancelable: true,
       clientX: r.x + r.width / 2,
       clientY: r.y + r.height / 2,
     }));`,
  );
  await driver.waitFor(
    `return !!document.querySelector('.ctxmenu')`,
    `context menu for ${title}`,
  );
}

async function openHeroContextMenu(driver) {
  await driver.exec(
    `const el = document.querySelector('[aria-label="Continue watching"] .flowcard.center');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', {
       bubbles: true,
       cancelable: true,
       clientX: r.x + r.width / 2,
       clientY: r.y + r.height / 2,
     }));`,
  );
  await driver.waitFor(
    `return !!document.querySelector('.ctxmenu')`,
    "Continue Watching context menu",
  );
}

const menuLabels = (driver) =>
  driver.exec(
    `return [...document.querySelectorAll('.ctxmenu [role="menuitem"]')]
      .map((button) => button.textContent.trim())`,
  );

async function clickMenuItem(driver, label) {
  const button = await driver.find(
    "xpath",
    `//button[@role='menuitem' and normalize-space(.)='${label}']`,
  );
  await driver.click(button);
}

export default {
  name: "playverbs",

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ["fresh.mp4", "progress.mp4"]);
    mock = await startMockJellyfin({
      movies: [
        {
          id: "fresh",
          name: "Fresh Movie",
          year: 2020,
          runTimeTicks: 100_000_000,
          mediaFile: path.join(mediaDir, "fresh.mp4"),
        },
        {
          id: "progress",
          name: "Progress Movie",
          year: 2021,
          runTimeTicks: 100_000_000,
          mediaFile: path.join(mediaDir, "progress.mp4"),
        },
      ],
      serveResume: true,
    });
    mock.state.userData.progress.positionTicks = 50_000_000;
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    await openLibraryGrid(driver, { cardPrefix: "Fresh Movie" });

    assert.equal(
      await driver.exec(
        `return !!document.querySelector('button.queuechip, aside.drawer')`,
      ),
      false,
      "the deleted queue must have no chip or drawer",
    );

    await openContextMenu(driver, "Fresh Movie");
    const freshLabels = await menuLabels(driver);
    assert.ok(freshLabels.includes("Play"), "a fresh item offers Play");
    assert.ok(!freshLabels.includes("Resume"), "a fresh item does not offer Resume");
    assert.ok(
      !freshLabels.includes("Play from Beginning"),
      "a fresh item does not need a redundant beginning action",
    );
    assert.ok(
      !freshLabels.some((label) => /queue|play next/i.test(label)),
      "the context menu has no queue verbs",
    );
    await driver.exec(`document.querySelector('.menubackdrop').click()`);

    await openContextMenu(driver, "Progress Movie");
    const progressLabels = await menuLabels(driver);
    assert.ok(progressLabels.includes("Resume"), "an in-progress item offers Resume");
    assert.ok(
      progressLabels.includes("Play from Beginning"),
      "an in-progress item offers Play from Beginning",
    );
    assert.ok(!progressLabels.includes("Play"), "the ambiguous plain Play verb is absent");

    const resumedAt = await playAndQuit(driver, () => clickMenuItem(driver, "Resume"));
    assert.ok(
      Math.abs(resumedAt - 5) < 2,
      `Resume must start near the server's 5s position, got ${resumedAt}s`,
    );

    const progressCard = await driver.find(
      "css selector",
      'button.poster[aria-label^="Progress Movie"]',
    );
    await driver.click(progressCard);
    await driver.waitFor(
      `return !!document.querySelector('.detail .playactions')`,
      "the detail playback actions",
    );
    const detailLabels = await driver.exec(
      `return [...document.querySelectorAll('.detail .playactions button')]
        .map((button) => button.textContent.trim())`,
    );
    assert.deepEqual(
      detailLabels,
      ["Resume", "Play from Beginning"],
      "the detail exposes both explicit in-progress verbs",
    );

    const beganAt = await playAndQuit(driver, async () => {
      const beginning = await driver.find(
        "xpath",
        `//div[contains(@class,'detail')]//button[normalize-space(.)='Play from Beginning']`,
      );
      await driver.click(beginning);
    });
    assert.ok(
      beganAt < 2,
      `Play from Beginning must override the server resume point, got ${beganAt}s`,
    );

    await goHome(driver);
    await driver.waitFor(
      `const card = document.querySelector('[aria-label="Continue watching"] .flowcard.center');
       return card?.getAttribute('aria-label')?.startsWith('Resume ') ?? false`,
      "the in-progress Continue Watching card",
    );
    assert.equal(
      await driver.exec(
        `return document.querySelector('.flowactions') === null`,
      ),
      true,
      "the carousel must not duplicate its Play/Resume card with an action row",
    );
    await openHeroContextMenu(driver);
    const heroMenuLabels = await menuLabels(driver);
    assert.ok(heroMenuLabels.includes("Resume"), "the hero context menu retains Resume");
    assert.ok(
      heroMenuLabels.includes("Play from Beginning"),
      "the hero context menu retains Play from Beginning",
    );
    await driver.exec(`document.querySelector('.menubackdrop').click()`);
    await screenshot("01-explicit-playback-verbs");
  },
};
