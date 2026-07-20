// A clean EOF must refresh Home after its PlayedItems attempt settles, while
// Continue Playing Off still prevents any successor playback. The mock makes
// a follow-up Resume item eligible only on a successful PlayedItems response.
import assert from 'node:assert/strict';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  holdsFor,
  logicalPlaybackInfoIds,
  makeClips,
  mockSource,
  pollUntil,
  seedConfig,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const COMPLETED_ID = 'hub-completed';
const FOLLOWUP_ID = 'hub-followup';
const FALLBACK_ID = 'hub-fallback';
const RESUME_PATH = '/Users/u1/Items/Resume';

let mock;

function recentEpisode(endedAt) {
  return {
    item: {
      ratingKey: `jf-mock:${COMPLETED_ID}`,
      title: 'Completed Episode',
      durationMs: 10_000,
      mediaType: 'episode',
      viewOffsetMs: 1_000,
      played: false,
      sourceId: 'jf-mock',
      providerIds: [],
      index: 1,
      parentIndex: 1,
      grandparentTitle: 'Hub Series',
      parentTitle: 'Season 1',
      parentRatingKey: 'jf-mock:hub-season',
      grandparentRatingKey: 'jf-mock:hub-series',
    },
    started_at_ms: 0,
    ended_at_ms: endedAt,
  };
}

function fallbackRecent(endedAt) {
  return {
    item: {
      ratingKey: `jf-mock:${FALLBACK_ID}`,
      title: 'Stable Fallback',
      durationMs: 60_000,
      mediaType: 'movie',
      viewOffsetMs: 1_000,
      played: false,
      sourceId: 'jf-mock',
      providerIds: [],
    },
    started_at_ms: 0,
    ended_at_ms: endedAt,
  };
}

const resumeRequests = () =>
  mock.state.requests.filter((request) => request.path === RESUME_PATH).length;

const resumeResponses = () =>
  mock.state.served.filter((response) => response.path === RESUME_PATH).length;

const playbackInfoIds = () =>
  logicalPlaybackInfoIds(mock);

async function heroState(driver) {
  return driver.exec(
    `return {
       titles: [...document.querySelectorAll('[aria-label="Continue watching"] .flowcard')]
         .map((card) => card.getAttribute('title')),
       episode: document.querySelector('[aria-label="Continue watching"] + .flowmeta .y')
         ?.textContent.trim() ?? null,
     }`,
  );
}

async function startCompletedEpisode(driver, seen) {
  await driver.click(
    await driver.find(
      'css selector',
      '[aria-label="Continue watching"] .flowcard[title="Hub Series"]',
    ),
  );
  const socket = await waitForNewMpvSocket(seen);
  seen.add(socket);
  const mpv = await MpvIpc.connect(socket);
  await mpv.setProp('pause', true);
  const loaded = String(
    await pollUntil(
      () => mpv.getProp('path').catch(() => null),
      'the completed episode stream in mpv',
    ),
  );
  assert.ok(
    loaded.includes(`/Videos/${COMPLETED_ID}/stream`),
    `expected ${COMPLETED_ID}, got ${loaded}`,
  );
  return mpv;
}

async function finishNaturally(mpv) {
  await mpv.setProp('time-pos', 9.2);
  await mpv.setProp('pause', false);
  mpv.close();
}

async function proveExactRefreshesAndNoSuccessor(seen, resumeBaseline, what) {
  await holdsFor(
    () => {
      const asked = resumeRequests() - resumeBaseline;
      const answered = resumeResponses() - resumeBaseline;
      if (asked !== 2 || answered !== 2) {
        return `expected 2 Resume requests/responses, got ${asked}/${answered}`;
      }
      const plays = playbackInfoIds();
      if (plays.length !== 1 || plays[0] !== COMPLETED_ID) {
        return `unexpected playback negotiation: ${plays.join(',')}`;
      }
      const unexpectedSockets = [...mpvSocketSnapshot()].filter((socket) => !seen.has(socket));
      return unexpectedSockets.length > 0
        ? `unexpected successor mpv socket: ${unexpectedSockets.join(',')}`
        : false;
    },
    2_500,
    what,
  );
}

function resetForFailure(configRoot) {
  mock.state.requests.length = 0;
  mock.state.served.length = 0;
  mock.state.playedArrivals.length = 0;
  mock.state.playedServed.length = 0;
  mock.state.checkins.length = 0;
  mock.state.resumeAfterPlayed = null;
  mock.state.unauthNextPlayed = false;
  mock.state.playedDelayMs = 0;
  mock.state.delayNextResumeMs = 0;
  for (const data of Object.values(mock.state.userData)) {
    data.played = false;
    data.positionTicks = 0;
  }
  mock.state.userData[COMPLETED_ID].positionTicks = 10_000_000;
  mock.state.userData[FALLBACK_ID].positionTicks = 10_000_000;
  seedConfig(configRoot, [mockSource(mock)], {
    continue_playing: 'off',
    recents: [recentEpisode(200), fallbackRecent(100)],
  });
}

export default {
  name: 'completionhub',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['hub-completed.mp4']);
    const episode = (id, name, index, mediaFile = undefined) => ({
      id,
      name,
      type: 'Episode',
      seriesId: 'hub-series',
      seasonId: 'hub-season',
      seriesName: 'Hub Series',
      seasonName: 'Season 1',
      parentIndex: 1,
      index,
      runTimeTicks: 100_000_000,
      mediaFile,
    });
    mock = await startMockJellyfin({
      movies: [{ id: FALLBACK_ID, name: 'Stable Fallback' }],
      children: {
        'hub-series': [
          { id: 'hub-season', name: 'Season 1', type: 'Season', seriesId: 'hub-series', index: 1 },
        ],
        'hub-season': [
          episode(
            COMPLETED_ID,
            'Completed Episode',
            1,
            path.join(mediaDir, 'hub-completed.mp4'),
          ),
          episode(FOLLOWUP_ID, 'Follow-up Episode', 2),
        ],
      },
      serveResume: true,
    });
    mock.state.userData[COMPLETED_ID].positionTicks = 10_000_000;
    seedConfig(configRoot, [mockSource(mock)], {
      continue_playing: 'off',
      recents: [recentEpisode(200)],
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot, restart }) {
    await driver.waitFor(
      `return document.querySelector('[aria-label="Continue watching"] .flowcard.center')
        ?.getAttribute('title') === 'Hub Series'`,
      'the completed episode in Continue Watching',
    );
    assert.equal((await heroState(driver)).episode, 'S1 · E1 – Completed Episode');

    const successSockets = mpvSocketSnapshot();
    const successMpv = await startCompletedEpisode(driver, successSockets);
    mock.state.exposeResumeAfterPlayed(COMPLETED_ID, FOLLOWUP_ID);
    mock.state.delayNextResumeMs = 1_000;
    mock.state.playedDelayMs = 5_000;
    const successResumeBaseline = resumeResponses();
    const successPlayedBaseline = mock.state.playedServed.length;
    await finishNaturally(successMpv);

    await pollUntil(
      () => mock.state.playedArrivals.length > successPlayedBaseline,
      'the delayed successful PlayedItems request',
    );
    await pollUntil(
      () => resumeResponses() === successResumeBaseline + 1,
      'the tracker Home response before PlayedItems succeeds',
    );
    assert.equal(
      mock.state.playedServed.length,
      successPlayedBaseline,
      'the first post-EOF Home response must precede PlayedItems success',
    );
    assert.equal(mock.state.userData[COMPLETED_ID].played, false);
    assert.equal(mock.state.userData[FOLLOWUP_ID].positionTicks, 0);
    await pollUntil(
      async () => {
        const state = await heroState(driver);
        return state.titles.length === 0 ? state : null;
      },
      'local suppression with the server transition still pending',
    );

    await pollUntil(
      () => mock.state.playedServed.length === successPlayedBaseline + 1,
      'the successful PlayedItems response',
      { timeoutMs: 10_000 },
    );
    assert.equal(mock.state.userData[COMPLETED_ID].played, true);
    assert.equal(mock.state.userData[FOLLOWUP_ID].positionTicks, 10_000_000);
    await pollUntil(
      () => resumeResponses() === successResumeBaseline + 2,
      'the post-success Home response',
    );
    await pollUntil(
      async () => {
        const state = await heroState(driver);
        return state.titles.length === 1 &&
          state.titles[0] === 'Hub Series' &&
          state.episode === 'S1 · E2 – Follow-up Episode'
          ? state
          : null;
      },
      'the newly eligible follow-up without manual Refresh',
    );
    await proveExactRefreshesAndNoSuccessor(
      successSockets,
      successResumeBaseline,
      'success must stop after the tracker and post-attempt refreshes',
    );
    await screenshot('01-success-followup');

    await restart(() => resetForFailure(configRoot));
    await driver.waitFor(
      `return document.querySelectorAll('[aria-label="Continue watching"] .flowcard').length === 2`,
      'the failure leg Continue Watching cards',
    );

    const failureSockets = mpvSocketSnapshot();
    const failureMpv = await startCompletedEpisode(driver, failureSockets);
    mock.state.exposeResumeAfterPlayed(COMPLETED_ID, FOLLOWUP_ID);
    mock.state.unauthNextPlayed = true;
    mock.state.delayNextResumeMs = 1_000;
    mock.state.playedDelayMs = 5_000;
    const failureResumeBaseline = resumeResponses();
    const failurePlayedBaseline = mock.state.playedServed.length;
    await finishNaturally(failureMpv);

    await pollUntil(
      () => mock.state.playedArrivals.length > failurePlayedBaseline,
      'the delayed failing PlayedItems request',
    );
    await pollUntil(
      () => resumeResponses() === failureResumeBaseline + 1,
      'the tracker Home response before PlayedItems fails',
    );
    assert.equal(
      mock.state.playedServed.length,
      failurePlayedBaseline,
      'the first failure-leg Home response must precede the 401',
    );
    await pollUntil(
      async () => {
        const state = await heroState(driver);
        return state.titles.length === 1 && state.titles[0] === 'Stable Fallback'
          ? state
          : null;
      },
      'local suppression retaining the stable fallback before the 401',
    );

    await pollUntil(
      () => mock.state.playedServed.length === failurePlayedBaseline + 1,
      'the PlayedItems 401 response',
      { timeoutMs: 10_000 },
    );
    assert.equal(mock.state.playedServed.at(-1)?.status, 401);
    assert.equal(mock.state.userData[COMPLETED_ID].played, false);
    assert.equal(
      mock.state.userData[FOLLOWUP_ID].positionTicks,
      0,
      'a failed PlayedItems edit must not expose the follow-up',
    );
    await pollUntil(
      () => resumeResponses() === failureResumeBaseline + 2,
      'the unconditional post-failure Home response',
    );
    await pollUntil(
      async () => {
        const state = await heroState(driver);
        return state.titles.length === 1 && state.titles[0] === 'Stable Fallback'
          ? state
          : null;
      },
      'the stable fallback after the failed attempt refresh',
    );
    await proveExactRefreshesAndNoSuccessor(
      failureSockets,
      failureResumeBaseline,
      'failure must stop after the tracker and unconditional post-attempt refreshes',
    );
    await screenshot('02-failure-fallback');
  },
};
