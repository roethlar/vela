// Continue Watching must CONTINUE: after a mid-clip quit, playing the item
// again (from the hero) must start mpv at the stamped position, not 0:00.
// The hero's resume position is source-agnostic (2026-07-04 hero decision):
// Vela's own recents stamp drives it, independent of server thresholds —
// exercised here over a mock server stream.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  pollUntil, openLibraryGrid, goHome, playAndQuit, openDetailAndPlay,
  makeClips, mockSource, seedConfig,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const HERO_CLIP = `[aria-label="Continue watching"] [aria-label^="Play Mock Movie"]`;

let mock;

export default {
  name: 'resume',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['stream.mp4']);
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1',
        name: 'Mock Movie',
        year: 2020,
        runTimeTicks: 100_000_000, // 10s, matching the real clip
        mediaFile: path.join(mediaDir, 'stream.mp4'),
      }],
      // Above the whole clip: the server NEVER stores a resume point (like
      // a real sub-threshold play), so Vela's recents stamp is the only
      // store and the resume assertion guards the source-agnostic fallback
      // — a server-offset pass-through would mask its regression (br-1).
      minResumeTicks: 200_000_000,
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const stampedMs = () => {
      try {
        return JSON.parse(fs.readFileSync(configFile, 'utf8')).recents?.[0]?.item?.viewOffsetMs ?? 0;
      } catch {
        return 0;
      }
    };

    // First watch (card → info page → Play): quit mid-clip, position stamped.
    await openLibraryGrid(driver);
    const firstWatchStart = await playAndQuit(driver, () =>
      openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]'),
    );
    assert.ok(firstWatchStart < 2, `a fresh item must start near 0, got ${firstWatchStart}s`);
    await pollUntil(() => stampedMs() > 0, 'the recents position stamp');
    const stamp = stampedMs();

    // Continue from the hero: the center card click-plays (the nav flip kept
    // click-to-play on the carousel), and mpv must open at the stamped position.
    await goHome(driver);
    await driver.waitFor(`return !!document.querySelector('${HERO_CLIP}')`, 'clip in the hero');
    await screenshot('01-hero-before-resume');
    const resumeStart = await playAndQuit(driver, async () => {
      const heroCard = await driver.find('css selector', HERO_CLIP);
      await driver.click(heroCard);
    });
    assert.ok(
      Math.abs(resumeStart - stamp / 1000) < 1.5,
      `resume must start at the stamped ${stamp}ms, got ${resumeStart}s`,
    );
  },
};
