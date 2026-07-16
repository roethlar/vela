// Default TV-only continuation: next episode, season rollover, end-of-show,
// and a delayed lookup that must yield to a newer manual play.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  holdsFor,
  makeClips,
  mockSource,
  pollUntil,
  seedConfig,
  seedPlaylists,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock;

function recentItem({ id, title, mediaType = 'movie', endedAt, episode = null }) {
  return {
    item: {
      ratingKey: `jf-mock:${id}`,
      title,
      durationMs: mediaType === 'episode' ? 10_000 : 60_000,
      mediaType,
      viewOffsetMs: 1_000,
      played: false,
      sourceId: 'jf-mock',
      providerIds: [],
      ...(episode
        ? {
            index: episode.index,
            parentIndex: episode.parentIndex,
            grandparentTitle: 'Race Show',
            parentTitle: `Season ${episode.parentIndex}`,
            parentRatingKey: `jf-mock:${episode.seasonId}`,
            grandparentRatingKey: 'jf-mock:show-1',
          }
        : {}),
    },
    started_at_ms: 0,
    ended_at_ms: endedAt,
  };
}

function seededRecents() {
  return [
    recentItem({
      id: 'e1',
      title: 'Episode One',
      mediaType: 'episode',
      endedAt: 200,
      episode: { index: 1, parentIndex: 1, seasonId: 's1' },
    }),
    recentItem({ id: 'manual', title: 'Manual Movie', endedAt: 100 }),
  ];
}

async function openPlayerSettings(driver) {
  await driver.click(await driver.find('css selector', 'button[aria-label="Settings"]'));
  await driver.waitFor(`return !!document.querySelector('[role="dialog"]')`, 'Settings dialog');
  await driver.click(
    await driver.find('xpath', `//button[@role='tab' and normalize-space(.)='Player']`),
  );
  await driver.waitFor(
    `return !!document.querySelector('select#continue-playing')`,
    'Continue Playing selector',
  );
}

async function nextMpv(seen, expectedId) {
  const socket = await waitForNewMpvSocket(seen, { timeoutMs: 20_000 });
  seen.add(socket);
  const mpv = await MpvIpc.connect(socket);
  await mpv.setProp('pause', true);
  const loaded = String(await pollUntil(
    () => mpv.getProp('path').catch(() => null),
    `${expectedId} stream in mpv`,
  ));
  assert.ok(loaded.includes(`/Videos/${expectedId}/stream`), `expected ${expectedId}, got ${loaded}`);
  return mpv;
}

async function finishNaturally(mpv) {
  await mpv.setProp('time-pos', 9.2);
  await mpv.setProp('pause', false);
  mpv.close();
}

const playbackInfoIds = () =>
  mock.state.requests
    .filter((request) => /\/Items\/[^/]+\/PlaybackInfo$/.test(request.path))
    .map((request) => request.path.match(/^\/Items\/([^/]+)\/PlaybackInfo$/)?.[1]);

const resumeResponses = () =>
  mock.state.served.filter(
    (response) => response.path === `/Users/${mock.userId}/Items/Resume`,
  ).length;

function resetMockPlaybackState() {
  mock.state.requests.length = 0;
  mock.state.served.length = 0;
  mock.state.playedArrivals.length = 0;
  mock.state.playedServed.length = 0;
  mock.state.checkins.length = 0;
  for (const data of Object.values(mock.state.userData)) {
    data.played = false;
    data.positionTicks = 0;
  }
  // The faithful Resume hub must carry the same two items as the seeded local
  // recents. E1 therefore remains a live stale-hub threat until PlayedItems is
  // actually served.
  mock.state.userData.e1.positionTicks = 10_000_000;
  mock.state.userData.manual.positionTicks = 10_000_000;
}

export default {
  name: 'continuetv',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, [
      'special.mp4',
      'e1.mp4',
      'e2.mp4',
      'e3.mp4',
      'manual.mp4',
    ]);
    const episode = (id, name, seasonId, parentIndex, index, file) => ({
      id,
      name,
      type: 'Episode',
      seriesId: 'show-1',
      seasonId,
      seriesName: 'Race Show',
      seasonName: parentIndex === 0 ? 'Specials' : `Season ${parentIndex}`,
      parentIndex,
      index,
      runTimeTicks: 100_000_000,
      mediaFile: path.join(mediaDir, file),
    });
    mock = await startMockJellyfin({
      movies: [
        {
          id: 'manual',
          name: 'Manual Movie',
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'manual.mp4'),
        },
      ],
      children: {
        'show-1': [
          { id: 's0', name: 'Specials', type: 'Season', seriesId: 'show-1', index: 0 },
          { id: 's1', name: 'Season 1', type: 'Season', seriesId: 'show-1', index: 1 },
          { id: 's2', name: 'Season 2', type: 'Season', seriesId: 'show-1', index: 2 },
        ],
        s0: [episode('sp1', 'Special One', 's0', 0, 1, 'special.mp4')],
        s1: [
          episode('e1', 'Episode One', 's1', 1, 1, 'e1.mp4'),
          episode('e2', 'Episode Two', 's1', 1, 2, 'e2.mp4'),
        ],
        s2: [episode('e3', 'Episode Three', 's2', 2, 1, 'e3.mp4')],
      },
      serveResume: true,
    });
    mock.state.userData.e1.positionTicks = 10_000_000;
    mock.state.userData.manual.positionTicks = 10_000_000;
    // Missing continue_playing is intentional: only-tv is the default.
    seedConfig(configRoot, [mockSource(mock)], { recents: seededRecents() });
    seedPlaylists(configRoot, [
      {
        id: 'manual-race',
        name: 'Manual Race',
        items: [
          {
            id: 'manual-entry',
            item: { ...seededRecents()[1].item, viewOffsetMs: null },
            sourceName: 'Mock JF',
          },
        ],
        createdMs: 1,
        updatedMs: 1,
      },
    ]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const readConfig = () => JSON.parse(fs.readFileSync(configFile, 'utf8'));
    const centeredEpisode = () =>
      driver.exec(
        `return document.querySelector('[aria-label="Continue watching"] + .flowmeta .y')?.textContent.trim() ?? null`,
      );
    const exactEpisodeHero = (episodeText) =>
      driver.exec(
        `const cards = [...document.querySelectorAll('[aria-label="Continue watching"] .flowcard')];
         return document.querySelector('.flowmeta .y')?.textContent.trim() === ${JSON.stringify(episodeText)}
           && cards.filter((card) => card.getAttribute('title') === 'Race Show').length === 1;`,
      );

    await driver.waitFor(
      `return document.querySelectorAll('[aria-label="Continue watching"] .flowcard').length === 2`,
      'the seeded Continue Watching flow',
    );
    await openPlayerSettings(driver);
    assert.equal(
      await driver.exec(`return document.querySelector('select#continue-playing')?.value`),
      'only-tv',
      'missing config must render the TV-only default',
    );
    await driver.click(await driver.find('css selector', 'button[aria-label="Close"]'));

    // `playback-ended` also refreshes after a user closes mpv. That refresh
    // must never be mistaken for the clean-EOF continuation signal.
    const quitSockets = mpvSocketSnapshot();
    await driver.click(
      await driver.find('css selector', '[aria-label="Continue watching"] .flowcard.center'),
    );
    const quitEpisode = await nextMpv(quitSockets, 'e1');
    const quitPlayedArrivals = mock.state.playedArrivals.length;
    const quitPlayedServed = mock.state.playedServed.length;
    const quitTombstones = [...(readConfig().hidden_from_continue ?? [])];
    const quitResumeResponses = resumeResponses();
    const quitStoppedResponses = mock.state.served.filter(
      (response) =>
        response.method === 'POST' && response.path === '/Sessions/Playing/Stopped',
    ).length;
    quitEpisode.quit();
    quitEpisode.close();
    await pollUntil(
      () =>
        mock.state.served.filter(
          (response) =>
            response.method === 'POST' && response.path === '/Sessions/Playing/Stopped',
        ).length > quitStoppedResponses,
      'the user-quit Stopped response',
    );
    await pollUntil(
      () => resumeResponses() > quitResumeResponses,
      'the user-quit Home repaint',
    );
    await holdsFor(
      async () => {
        if (playbackInfoIds().length > 1) {
          return `user quit incorrectly continued: ${playbackInfoIds().join(',')}`;
        }
        if (mock.state.playedArrivals.length !== quitPlayedArrivals) {
          return 'user quit sent a PlayedItems request';
        }
        if (mock.state.playedServed.length !== quitPlayedServed) {
          return 'user quit received a PlayedItems response';
        }
        const tombstones = readConfig().hidden_from_continue ?? [];
        if (JSON.stringify(tombstones) !== JSON.stringify(quitTombstones)) {
          return `user quit changed completion tombstones: ${JSON.stringify(tombstones)}`;
        }
        if ((await centeredEpisode()) !== 'S1 · E1 – Episode One') {
          return `the quit episode stopped being the centered eligible item`;
        }
        const action = await driver.exec(
          `return document.querySelector('[aria-label="Continue watching"] .flowcard.center')
            ?.getAttribute('aria-label') ?? null`,
        );
        return action?.startsWith('Resume ')
          ? false
          : `the quit item action became ${JSON.stringify(action)}`;
      },
      2_500,
      'user quit must not curate, mark played, or continue',
    );
    assert.deepEqual(playbackInfoIds(), ['e1']);

    await restart(() => {
      seedConfig(configRoot, [mockSource(mock)], { recents: seededRecents() });
      resetMockPlaybackState();
    });
    await driver.waitFor(
      `return document.querySelectorAll('[aria-label="Continue watching"] .flowcard').length === 2`,
      'the clean-EOF Continue Watching flow',
    );

    const seen = mpvSocketSnapshot();
    await driver.click(
      await driver.find('css selector', '[aria-label="Continue watching"] .flowcard.center'),
    );
    const first = await nextMpv(seen, 'e1');
    const cleanPlayedArrivals = mock.state.playedArrivals.length;
    const cleanPlayedServed = mock.state.playedServed.length;
    mock.state.playedDelayMs = 8_000;
    // Hold successful E2 negotiation long enough for both pre-successor Home
    // reads to settle without an E2 recent. Only the required post-start
    // refresh can then make E2 render.
    mock.state.playbackInfoDelayMs = 3_000;
    await finishNaturally(first);
    await pollUntil(
      () => mock.state.playedArrivals.length > cleanPlayedArrivals,
      'the automatic E1 PlayedItems request to arrive',
    );
    const second = await nextMpv(seen, 'e2');
    assert.equal(
      mock.state.playedServed.length,
      cleanPlayedServed,
      'E2 must launch before the delayed E1 PlayedItems response is served',
    );
    await pollUntil(
      async () => {
        if (mock.state.playedServed.length !== cleanPlayedServed) {
          throw new Error('the E1 PlayedItems response settled before the pre-response repaint proof');
        }
        return exactEpisodeHero('S1 · E2 – Episode Two');
      },
      'E2 rendered with E1 suppressed while PlayedItems is parked',
      { timeoutMs: 7_000 },
    );
    assert.equal(mock.state.userData.e1.played, false, 'the delayed server mutation is still pending');
    await pollUntil(
      () => mock.state.playedServed.length > cleanPlayedServed,
      'the delayed E1 PlayedItems response',
      { timeoutMs: 12_000 },
    );
    assert.equal(mock.state.userData.e1.played, true, 'clean E1 EOF must mark the server item played');
    await holdsFor(
      async () =>
        (await exactEpisodeHero('S1 · E2 – Episode Two'))
          ? false
          : 'E1 resurfaced or E2 left the hero after PlayedItems settled',
      1_000,
      'the post-response hero must retain E2 and suppress E1',
    );
    await finishNaturally(second);
    const third = await nextMpv(seen, 'e3');
    await finishNaturally(third);
    await pollUntil(
      () =>
        mock.state.served.filter(
          (response) =>
            response.method === 'POST' && response.path === '/Sessions/Playing/Stopped',
        ).length >= 3,
      'three final Stopped responses',
    );
    await holdsFor(
      () =>
        playbackInfoIds().length > 3
          ? `unexpected fourth play: ${playbackInfoIds().join(',')}`
          : false,
      2_500,
      'the end of the show must stop',
    );
    assert.deepEqual(playbackInfoIds(), ['e1', 'e2', 'e3']);
    assert.ok(!playbackInfoIds().includes('sp1'), 'regular playback must skip Specials');
    assert.deepEqual(mock.state.contractViolations, []);
    await screenshot('01-season-rollover');

    // Fresh run: park the hierarchy lookup after E1 ends, then start a movie
    // through Playlists. That component invokes the backend directly, so the
    // page-level continuation attempt counter cannot cancel the old lookup for
    // us: the backend expected-session guard itself must keep E2 from replacing
    // the user's newer player.
    await restart(() => {
      seedConfig(configRoot, [mockSource(mock)], { recents: seededRecents() });
      resetMockPlaybackState();
    });
    await driver.waitFor(
      `return document.querySelectorAll('[aria-label="Continue watching"] .flowcard').length === 2`,
      'the reset Continue Watching flow',
    );
    const raceSockets = mpvSocketSnapshot();
    await driver.click(
      await driver.find('css selector', '[aria-label="Continue watching"] .flowcard.center'),
    );
    const racedEpisode = await nextMpv(raceSockets, 'e1');
    const hierarchyResponses = () =>
      mock.state.served.filter(
        (response) => response.path === `/Users/${mock.userId}/Items`,
      ).length;
    const hierarchyServedBefore = hierarchyResponses();
    mock.state.delayNextChildrenMs = 5_000;
    await finishNaturally(racedEpisode);
    await pollUntil(
      () =>
        mock.state.requests.some(
          (request) =>
            request.path === `/Users/${mock.userId}/Items` &&
            request.query.ParentId === 'show-1',
        ),
      'the parked next-episode hierarchy request',
    );
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Playlists')`,
      'the Playlists sidebar entry',
    );
    await driver.click(
      await driver.find(
        'xpath',
        `//button[contains(@class,'sideitem') and normalize-space(.)='Playlists']`,
      ),
    );
    await driver.waitFor(
      `return !!document.querySelector('section.playlists .playlistgrid button[aria-label^="Open Manual Race,"]')`,
      'the Manual Race playlist',
    );
    await driver.click(
      await driver.find(
        'css selector',
        'section.playlists .playlistgrid button[aria-label^="Open Manual Race,"]',
      ),
    );
    await driver.waitFor(
      `return document.querySelectorAll('section.playlists ol.entries > li').length === 1`,
      'the manual playlist entry',
    );
    await driver.click(
      await driver.find(
        'xpath',
        `//ol[@aria-label='Playlist items']/li[1]//div[contains(@class,'entryactions')]/button[1]`,
      ),
    );
    const manual = await nextMpv(raceSockets, 'manual');
    await pollUntil(
      () => hierarchyResponses() > hierarchyServedBefore,
      'the delayed hierarchy response after manual playback started',
      { timeoutMs: 10_000 },
    );
    await holdsFor(
      async () => {
        if (playbackInfoIds().includes('e2')) return 'the stale lookup launched E2';
        const loaded = await manual.getProp('path').catch(() => null);
        return String(loaded).includes('/Videos/manual/stream')
          ? false
          : `the manual player changed to ${loaded}`;
      },
      5_000,
      'manual playback must win the delayed continuation race',
    );
    manual.quit();
    manual.close();
    assert.deepEqual(playbackInfoIds(), ['e1', 'manual']);
    assert.deepEqual(mock.state.contractViolations, []);
    await screenshot('02-manual-race-won');
  },
};
