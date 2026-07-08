// Continue Watching must CONTINUE: after a mid-clip quit, playing the item
// again (from the hero) must start mpv at the stamped position, not 0:00.
// Vela's own recents stamp is the only progress store local files have
// (2026-07-04 hero decision: source-agnostic, independent of server
// thresholds).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pollUntil, openLibraryGrid, goHome, playAndQuit } from '../helpers.mjs';
import playback from './playback.mjs';

const HERO_CLIP = `[aria-label="Continue watching"] [aria-label^="Play E2E Clip"]`;

export default {
  name: 'resume',
  seed: playback.seed,

  async run({ driver, screenshot, configRoot }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const stampedMs = () => {
      try {
        return JSON.parse(fs.readFileSync(configFile, 'utf8')).recents?.[0]?.item?.viewOffsetMs ?? 0;
      } catch {
        return 0;
      }
    };

    // First watch: quit mid-clip, position stamped.
    await openLibraryGrid(driver);
    const card = await driver.find('css selector', 'button.poster[aria-label^="E2E Clip"]');
    const firstWatchStart = await playAndQuit(driver, card);
    assert.ok(firstWatchStart < 2, `a fresh item must start near 0, got ${firstWatchStart}s`);
    await pollUntil(() => stampedMs() > 0, 'the recents position stamp');
    const stamp = stampedMs();

    // Continue from the hero: mpv must open at the stamped position.
    await goHome(driver);
    const heroCard = await driver
      .waitFor(`return !!document.querySelector('${HERO_CLIP}')`, 'clip in the hero')
      .then(() => driver.find('css selector', HERO_CLIP));
    await screenshot('01-hero-before-resume');
    const resumeStart = await playAndQuit(driver, heroCard);
    assert.ok(
      Math.abs(resumeStart - stamp / 1000) < 1.5,
      `resume must start at the stamped ${stamp}ms, got ${resumeStart}s`,
    );
  },
};
