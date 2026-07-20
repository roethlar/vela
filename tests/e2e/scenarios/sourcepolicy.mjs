// End-to-end duplicate-source policy boundary across two real mock Jellyfin
// protocol instances. The copies differ in quality and endpoint locality;
// the same fixture also proves merged hierarchy backings survive show/season
// navigation.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { makeClips, mockSource, pollUntil, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';

const MOVIE = 'Policy Movie';
const CANONICAL = 'imdb:tt-policy-movie';
let localMock;
let distantMock;

function configPath(configRoot) {
  return path.join(configRoot, 'config', 'vela', 'config.json');
}

function readConfig(configRoot) {
  return JSON.parse(fs.readFileSync(configPath(configRoot), 'utf8'));
}

function servedBy(streamUrl, mock) {
  const parsed = new URL(streamUrl);
  return Number(parsed.port) === mock.port && parsed.pathname === '/Videos/policy-movie/stream';
}

function movieSeed(file, mediaSource) {
  return {
    id: 'policy-movie',
    name: MOVIE,
    year: 2024,
    runTimeTicks: 100_000_000,
    mediaFile: file,
    providerIds: { Imdb: 'tt-policy-movie' },
    mediaSources: [mediaSource],
  };
}

function hierarchySeed(suffix) {
  const showId = `policy-show-${suffix}`;
  const seasonId = `policy-season-${suffix}`;
  return {
    show: {
      id: showId,
      name: 'Policy Show',
      year: 2025,
      type: 'Series',
      providerIds: { Tmdb: 'policy-show' },
    },
    children: {
      [showId]: [{
        id: seasonId,
        name: 'Season 1',
        type: 'Season',
        index: 1,
        seriesId: showId,
        providerIds: { Tmdb: 'policy-season-1' },
      }],
      [seasonId]: [{
        id: `policy-episode-${suffix}`,
        name: 'Policy Pilot',
        type: 'Episode',
        index: 1,
        parentIndex: 1,
        seriesId: showId,
        seasonId,
        seriesName: 'Policy Show',
        seasonName: 'Season 1',
        providerIds: { Tmdb: 'policy-episode-1' },
      }],
    },
  };
}

async function setPolicy(driver, configRoot, label, expected) {
  await driver.click(await driver.find('css selector', 'button[aria-label="Settings"]'));
  await driver.waitFor(
    `return !!document.querySelector('[role="dialog"][aria-label="Settings"]')`,
    'Settings dialog',
  );
  await driver.click(
    await driver.find(
      'xpath',
      `//*[@role='dialog' and @aria-label='Settings']//button[@role='tab' and normalize-space(.)='Player']`,
    ),
  );
  await driver.click(
    await driver.find(
      'xpath',
      `//*[@role='dialog' and @aria-label='Settings']//label[.//b[normalize-space(.)='${label}']]`,
    ),
  );
  await driver.click(
    await driver.find(
      'xpath',
      `//*[@role='dialog' and @aria-label='Settings']//button[normalize-space(.)='Save playback source preference']`,
    ),
  );
  await pollUntil(
    () => (readConfig(configRoot).playback_source_policy ?? 'best') === expected,
    `${label} to persist`,
  );
  await driver.click(
    await driver.find(
      'xpath',
      `//*[@role='dialog' and @aria-label='Settings']//button[@aria-label='Close']`,
    ),
  );
}

async function playDetail(driver, { choose = null, inspectChoices = false } = {}) {
  const before = mpvSocketSnapshot();
  await driver.click(await driver.find('css selector', '.detail button.playwide.primary'));
  if (choose) {
    await driver.waitFor(
      `return !!document.querySelector('.sourcechoicedialog[role="dialog"]')`,
      'Ask Every Time source dialog',
    );
    if (inspectChoices) {
      const choices = await driver.exec(
        `return [...document.querySelectorAll('.sourcechoices button.choice')]
          .map((button) => button.innerText.replaceAll('\\n', ' ').replaceAll(/\\s+/g, ' ').trim());`,
      );
      assert.ok(
        choices.some((text) => text.includes('Policy Local') && text.includes('This computer') && text.includes('1920×1080') && text.includes('SDR')),
        `local Ask choice must expose locality and quality: ${choices.join(' | ')}`,
      );
      assert.ok(
        choices.some((text) => text.includes('Policy Distant') && text.includes('Internet') && text.includes('3840×2160') && text.includes('HDR')),
        `distant Ask choice must expose locality and quality: ${choices.join(' | ')}`,
      );
      assert.deepEqual(
        [...mpvSocketSnapshot()].filter((socket) => !before.has(socket)),
        [],
        'Ask must not launch before the user chooses',
      );
    }
    await driver.click(
      await driver.find(
        'xpath',
        `//button[contains(concat(' ',normalize-space(@class),' '),' choice ') and .//span[normalize-space(.)='${choose}']]`,
      ),
    );
  }
  const mpv = await MpvIpc.connect(await waitForNewMpvSocket(before, { timeoutMs: 20_000 }));
  try {
    const loaded = await pollUntil(() => mpv.getProp('path').catch(() => null), `${MOVIE} stream`);
    mpv.quit();
    return loaded;
  } finally {
    mpv.close();
  }
}

async function playVersion(driver, sourceName) {
  const before = mpvSocketSnapshot();
  await driver.exec(
    `const el = document.querySelector('.detail button.posterframe');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', {
       bubbles: true, cancelable: true,
       clientX: r.x + r.width / 2, clientY: r.y + r.height / 2,
     }));`,
  );
  await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, 'detail context menu');
  await driver.click(
    await driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Play Version']`),
  );
  await driver.waitFor(
    `return !!document.querySelector('[role="group"][aria-label="Play Version"]')`,
    'Play Version submenu',
  );
  await driver.click(
    await driver.find(
      'xpath',
      `//*[@role='group' and @aria-label='Play Version']//button[@role='menuitem' and normalize-space(.)='${sourceName}']`,
    ),
  );
  const mpv = await MpvIpc.connect(await waitForNewMpvSocket(before, { timeoutMs: 20_000 }));
  try {
    const loaded = await pollUntil(() => mpv.getProp('path').catch(() => null), `${sourceName} explicit stream`);
    mpv.quit();
    return loaded;
  } finally {
    mpv.close();
  }
}

export default {
  name: 'sourcepolicy',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['policy-local.mp4', 'policy-distant.mp4']);
    const localHierarchy = hierarchySeed('local');
    const distantHierarchy = hierarchySeed('distant');
    localMock = await startMockJellyfin({
      views: [
        {
          id: 'movies-local',
          name: 'Policy Movies Local',
          collectionType: 'movies',
          movies: [movieSeed(path.join(mediaDir, 'policy-local.mp4'), {
            Id: 'policy-1080-sdr',
            SupportsDirectPlay: true,
            SupportsDirectStream: true,
            Width: 1920,
            Height: 1080,
            Bitrate: 8_000_000,
            MediaStreams: [{ Type: 'Video', Width: 1920, Height: 1080, VideoRange: 'SDR' }],
          })],
        },
        {
          id: 'tv-local',
          name: 'Policy TV Local',
          collectionType: 'tvshows',
          movies: [localHierarchy.show],
        },
      ],
      children: localHierarchy.children,
    });
    distantMock = await startMockJellyfin({
      listenHost: '::',
      connectHost: '::ffff:127.0.0.1',
      views: [
        {
          id: 'movies-distant',
          name: 'Policy Movies Distant',
          collectionType: 'movies',
          movies: [movieSeed(path.join(mediaDir, 'policy-distant.mp4'), {
            Id: 'policy-4k-hdr',
            SupportsDirectPlay: true,
            SupportsDirectStream: true,
            Width: 3840,
            Height: 2160,
            Bitrate: 20_000_000,
            MediaStreams: [{ Type: 'Video', Width: 3840, Height: 2160, VideoRangeType: 'HDR10' }],
          })],
        },
        {
          id: 'tv-distant',
          name: 'Policy TV Distant',
          collectionType: 'tvshows',
          movies: [distantHierarchy.show],
        },
      ],
      children: distantHierarchy.children,
    });
    seedConfig(
      configRoot,
      [
        mockSource(localMock, { id: 'policy-local', name: 'Policy Local' }),
        mockSource(distantMock, { id: 'policy-distant', name: 'Policy Distant' }),
      ],
      {
        playback_display_resolution: '1080p',
        playback_display_hdr: 'disabled',
      },
    );
  },

  async cleanup() {
    await Promise.all([localMock?.close(), distantMock?.close()]);
  },

  async run({ driver, screenshot, configRoot }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Movies')`,
      'merged Movies type',
    );
    await driver.click(
      await driver.find('xpath', `//button[contains(@class,'sideitem') and normalize-space(.)='Movies']`),
    );
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="${MOVIE}"]')`,
      'merged policy movie',
    );
    assert.equal(
      await driver.exec(
        `return document.querySelectorAll('button.poster[aria-label^="${MOVIE}"]').length`,
      ),
      1,
      'the policy title must collapse to one card',
    );
    await driver.click(
      await driver.find('css selector', `button.poster[aria-label^="${MOVIE}"]`),
    );
    await driver.waitFor(`return !!document.querySelector('.detail button.playwide.primary')`, 'policy detail play');

    const best = await playDetail(driver);
    assert.ok(
      servedBy(best, distantMock),
      `Prefer Best must choose 4K HDR, got ${best}`,
    );

    await setPolicy(driver, configRoot, 'Prefer Compatible', 'compatible');
    const compatible = await playDetail(driver);
    assert.ok(
      servedBy(compatible, localMock),
      `Prefer Compatible must choose 1080p SDR for the 1080p SDR override, got ${compatible}`,
    );

    await setPolicy(driver, configRoot, 'Prefer Fastest Source', 'fastest');
    const fastest = await playDetail(driver);
    assert.ok(
      servedBy(fastest, localMock),
      `Prefer Fastest must choose the loopback source before the higher-quality mapped endpoint, got ${fastest}`,
    );

    await setPolicy(driver, configRoot, 'Prefer Best', 'best');
    const explicit = await playVersion(driver, 'Policy Local');
    assert.ok(servedBy(explicit, localMock));
    await pollUntil(
      () => readConfig(configRoot).merged_overrides?.[CANONICAL] === 'policy-local',
      'the manual source override to persist',
    );
    const overridden = await playDetail(driver);
    assert.ok(
      servedBy(overridden, localMock),
      `the persistent manual override must beat Prefer Best, got ${overridden}`,
    );

    await setPolicy(driver, configRoot, 'Ask Every Time', 'ask');
    const askedDistant = await playDetail(driver, { choose: 'Policy Distant', inspectChoices: true });
    assert.ok(servedBy(askedDistant, distantMock));
    assert.equal(
      readConfig(configRoot).merged_overrides?.[CANONICAL],
      'policy-local',
      'Ask must ignore, but never overwrite, the older automatic-mode override',
    );
    const askedLocal = await playDetail(driver, { choose: 'Policy Local' });
    assert.ok(
      servedBy(askedLocal, localMock),
      'a second standalone Ask play must prompt again and accept a different source',
    );
    await screenshot('01-policies-and-ask');

    // Return from movie detail, then drill the merged show. The season page's
    // one episode row must expose Play Version with both physical sources,
    // proving cross-source hierarchy backings survived both parent fetches.
    await driver.click(await driver.find('css selector', '.crumbs button.back'));
    await driver.click(
      await driver.find('xpath', `//button[contains(@class,'sideitem') and normalize-space(.)='TV Shows']`),
    );
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Policy Show"]')`,
      'merged Policy Show',
    );
    await driver.click(await driver.find('css selector', 'button.poster[aria-label^="Policy Show"]'));
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Season 1"]')`,
      'merged Policy Show season',
    );
    await driver.click(await driver.find('css selector', 'button.poster[aria-label^="Season 1"]'));
    await driver.waitFor(
      `return document.querySelectorAll('[aria-label="Episodes"] button.eprow').length === 1 &&
        document.querySelector('[aria-label="Episodes"] .eptitle')?.textContent.includes('Policy Pilot')`,
      'one merged Policy Pilot episode',
    );
    await driver.exec(
      `const el = document.querySelector('[aria-label="Episodes"] button.eprow');
       const r = el.getBoundingClientRect();
       el.dispatchEvent(new MouseEvent('contextmenu', {
         bubbles: true, cancelable: true,
         clientX: r.x + r.width / 2, clientY: r.y + r.height / 2,
       }));`,
    );
    await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, 'merged episode context menu');
    await driver.click(
      await driver.find('xpath', `//button[@role='menuitem' and normalize-space(.)='Play Version']`),
    );
    const hierarchySources = await driver.waitFor(
      `const group = document.querySelector('[role="group"][aria-label="Play Version"]');
       if (!group) return null;
       return [...group.querySelectorAll('button[role="menuitem"]')].map((button) => button.textContent.trim());`,
      'both merged episode source choices',
    );
    assert.deepEqual(hierarchySources.sort(), ['Policy Distant', 'Policy Local']);
    assert.deepEqual(localMock.state.contractViolations, []);
    assert.deepEqual(distantMock.state.contractViolations, []);
    await screenshot('02-merged-hierarchy');
  },
};
