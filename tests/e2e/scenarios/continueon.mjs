// Continue Playing On must walk the literal rendered Continue Watching list,
// never repeat a key, and begin only after a playlist's final item.
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

function mediaItem(id, title, durationMs = 60_000) {
  return {
    ratingKey: `jf-mock:${id}`,
    title,
    durationMs,
    mediaType: 'movie',
    viewOffsetMs: 1_000,
    played: false,
    sourceId: 'jf-mock',
    providerIds: [],
  };
}

function recent(id, title, endedAt, durationMs = 60_000) {
  return {
    item: mediaItem(id, title, durationMs),
    started_at_ms: 0,
    ended_at_ms: endedAt,
  };
}

function playlistEntry(id, item) {
  return { id, item, sourceName: 'Mock JF' };
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

const stoppedResponses = () =>
  mock.state.served.filter(
    (response) =>
      response.method === 'POST' && response.path === '/Sessions/Playing/Stopped',
  ).length;

const isResumeRequest = (request) =>
  request.path === `/Users/${mock.userId}/Items/Resume`;

function requestIndexAfter(start, predicate) {
  const relative = mock.state.requests.slice(start).findIndex(predicate);
  return relative < 0 ? -1 : start + relative;
}

export default {
  name: 'continueon',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['on-a.mp4', 'on-b.mp4', 'on-c.mp4']);
    // Raw Resume order is deliberately C, A, B. Vela recents stamp A then B,
    // so the rendered carousel order is A, B, C. Continuation must follow the
    // latter even while the post-EOF Resume response is parked.
    mock = await startMockJellyfin({
      movies: [
        {
          id: 'on-c',
          name: 'On Charlie',
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'on-c.mp4'),
        },
        {
          id: 'on-a',
          name: 'On Alpha',
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'on-a.mp4'),
        },
        {
          id: 'on-b',
          name: 'On Beta',
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'on-b.mp4'),
        },
      ],
      serveResume: true,
    });
    for (const data of Object.values(mock.state.userData)) data.positionTicks = 10_000_000;
    seedConfig(configRoot, [mockSource(mock)], {
      continue_playing: 'on',
      recents: [recent('on-a', 'On Alpha', 300), recent('on-b', 'On Beta', 200)],
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const readConfig = () => JSON.parse(fs.readFileSync(configFile, 'utf8'));
    await driver.waitFor(
      `return document.querySelectorAll('[aria-label="Continue watching"] .flowcard').length === 3`,
      'the merged three-item Continue Watching flow',
    );
    assert.deepEqual(
      await driver.exec(
        `return [...document.querySelectorAll('[aria-label="Continue watching"] .flowcard')]
          .map((card) => card.getAttribute('title'))`,
      ),
      ['On Alpha', 'On Beta', 'On Charlie'],
      'the rendered order must differ from the raw server Resume order',
    );

    const seen = mpvSocketSnapshot();
    await driver.click(await driver.find('css selector', '.flowactions button.primary'));
    const alpha = await nextMpv(seen, 'on-a');
    const resumeRequests = () =>
      mock.state.requests.filter(
        (request) => request.path === `/Users/${mock.userId}/Items/Resume`,
      ).length;
    const resumeResponses = () =>
      mock.state.served.filter(
        (response) => response.path === `/Users/${mock.userId}/Items/Resume`,
      ).length;
    const resumeAskedBefore = resumeRequests();
    const resumeServedBefore = resumeResponses();
    const alphaRequestBoundary = mock.state.requests.length;
    mock.state.delayNextResumeMs = 5_000;
    await finishNaturally(alpha);
    await pollUntil(
      () => resumeRequests() > resumeAskedBefore,
      'the parked post-EOF Resume request',
    );
    const earlyAlphaResumeIndex = requestIndexAfter(alphaRequestBoundary, isResumeRequest);
    const beta = await nextMpv(seen, 'on-b');
    const betaPlaybackIndex = requestIndexAfter(
      earlyAlphaResumeIndex + 1,
      (request) => request.path === '/Items/on-b/PlaybackInfo',
    );
    // Early tracker refresh + dispatcher refresh + post-start refresh. The
    // latest Resume after PlaybackInfo is therefore the post-start request,
    // while `state.requests` supplies the required arrival provenance.
    await pollUntil(
      () => resumeRequests() >= resumeAskedBefore + 3,
      'all post-Alpha Resume requests',
    );
    const postStartAlphaResumeIndex = mock.state.requests.reduce(
      (latest, request, index) =>
        index > betaPlaybackIndex && isResumeRequest(request) ? index : latest,
      -1,
    );
    assert.ok(
      earlyAlphaResumeIndex < betaPlaybackIndex && betaPlaybackIndex < postStartAlphaResumeIndex,
      `expected early Resume < Beta PlaybackInfo < post-start Resume, got ${earlyAlphaResumeIndex}, ${betaPlaybackIndex}, ${postStartAlphaResumeIndex}`,
    );
    await pollUntil(
      () => resumeResponses() >= resumeServedBefore + 3,
      'all post-Alpha Resume responses',
      { timeoutMs: 10_000 },
    );

    // Charlie is deliberately retained only in the already-rendered hub. The
    // early post-Beta Home load sees neither a Charlie server resume point nor
    // a Charlie local recent, and its response is held until after the newer
    // post-start load has centered Charlie.
    mock.state.userData['on-c'].positionTicks = 0;
    const betaResumeAskedBefore = resumeRequests();
    const betaResumeServedBefore = resumeResponses();
    const betaRequestBoundary = mock.state.requests.length;
    mock.state.delayNextResumeMs = 8_000;
    mock.state.playbackInfoDelayMs = 3_000;
    await finishNaturally(beta);
    await pollUntil(
      () => requestIndexAfter(betaRequestBoundary, isResumeRequest) >= 0,
      'the parked early post-Beta Resume request',
    );
    const earlyBetaResumeIndex = requestIndexAfter(betaRequestBoundary, isResumeRequest);
    await pollUntil(
      () =>
        requestIndexAfter(
          earlyBetaResumeIndex + 1,
          (request) => request.path === '/Items/on-c/PlaybackInfo',
        ) >= 0,
      'the delayed successful Charlie PlaybackInfo request',
    );
    const charliePlaybackIndex = requestIndexAfter(
      earlyBetaResumeIndex + 1,
      (request) => request.path === '/Items/on-c/PlaybackInfo',
    );
    assert.ok(
      earlyBetaResumeIndex < charliePlaybackIndex,
      'the old Resume load must begin before Charlie stream negotiation',
    );
    assert.equal(
      mock.state.userData['on-c'].positionTicks,
      0,
      'the old Resume tuple must have no Charlie hub item',
    );
    await holdsFor(
      () => {
        const hasCharlie = (readConfig().recents ?? []).some(
          (entry) => entry.item?.ratingKey === 'jf-mock:on-c',
        );
        return hasCharlie ? 'Charlie was recorded before delayed PlaybackInfo succeeded' : false;
      },
      1_000,
      'the old Home load must settle its local reads before Charlie is recorded',
    );
    const charlie = await nextMpv(seen, 'on-c');
    await pollUntil(
      () =>
        (readConfig().recents ?? []).some(
          (entry) => entry.item?.ratingKey === 'jf-mock:on-c' && entry.ended_at_ms === 0,
        ),
      'the open Charlie local recent',
    );
    await pollUntil(
      () => resumeRequests() >= betaResumeAskedBefore + 3,
      'all post-Beta Resume requests',
    );
    const postCharlieResumeIndex = mock.state.requests.reduce(
      (latest, request, index) =>
        index > charliePlaybackIndex && isResumeRequest(request) ? index : latest,
      -1,
    );
    assert.ok(
      charliePlaybackIndex < postCharlieResumeIndex,
      'Charlie PlaybackInfo must precede its post-start Home request',
    );
    await driver.waitFor(
      `return document.querySelector('[aria-label="Continue watching"] .flowcard.center')?.getAttribute('title') === 'On Charlie'`,
      'Charlie centered by the post-start local recent',
    );
    await pollUntil(
      () => resumeResponses() >= betaResumeServedBefore + 3,
      'all post-Beta Resume responses including the delayed old load',
      { timeoutMs: 12_000 },
    );
    await holdsFor(
      async () =>
        (await driver.exec(
          `return document.querySelector('[aria-label="Continue watching"] .flowcard.center')?.getAttribute('title') === 'On Charlie'`,
        ))
          ? false
          : 'the delayed older tuple displaced Charlie',
      1_000,
      'the stale post-Beta generation must not overwrite Charlie',
    );
    await finishNaturally(charlie);
    await pollUntil(() => stoppedResponses() >= 3, 'three final Stopped responses');
    await holdsFor(
      () =>
        playbackInfoIds().length > 3
          ? `a key repeated: ${playbackInfoIds().join(',')}`
          : false,
      2_500,
      'the exhausted On run must not repeat a key',
    );
    assert.deepEqual(playbackInfoIds(), ['on-a', 'on-b', 'on-c']);
    assert.deepEqual(mock.state.contractViolations, []);
    await screenshot('01-rendered-order-no-repeat');

    // Fresh On run with a real Vela playlist. The intermediate A EOF must
    // advance to B inside the backend; only B's terminal EOF may hand control
    // to the retained Continue Watching feed and start C.
    await restart(() => {
      seedConfig(configRoot, [mockSource(mock)], {
        continue_playing: 'on',
        recents: [recent('on-c', 'On Charlie', 100)],
      });
      seedPlaylists(configRoot, [
        {
          id: 'continue-boundary',
          name: 'Continue Boundary',
          items: [
            playlistEntry('boundary-a', mediaItem('on-a', 'On Alpha', 10_000)),
            playlistEntry('boundary-b', mediaItem('on-b', 'On Beta', 10_000)),
          ],
          createdMs: 1,
          updatedMs: 1,
        },
      ]);
      mock.state.requests.length = 0;
      mock.state.served.length = 0;
      mock.state.checkins.length = 0;
      for (const data of Object.values(mock.state.userData)) {
        data.played = false;
        data.positionTicks = 0;
      }
    });
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Playlists')`,
      'Playlists sidebar entry',
    );
    await driver.click(
      await driver.find(
        'xpath',
        `//button[contains(@class,'sideitem') and normalize-space(.)='Playlists']`,
      ),
    );
    await driver.waitFor(
      `return !!document.querySelector('section.playlists .playlistgrid button[aria-label^="Open Continue Boundary,"]')`,
      'the Continue Boundary playlist',
    );
    await driver.click(
      await driver.find(
        'css selector',
        'section.playlists .playlistgrid button[aria-label^="Open Continue Boundary,"]',
      ),
    );
    await driver.waitFor(
      `return document.querySelectorAll('section.playlists ol.entries > li').length === 2`,
      'the two playlist entries',
    );

    const boundarySockets = mpvSocketSnapshot();
    await driver.click(
      await driver.find(
        'xpath',
        `//ol[@aria-label='Playlist items']/li[1]//div[contains(@class,'entryactions')]/button[1]`,
      ),
    );
    const playlistAlpha = await nextMpv(boundarySockets, 'on-a');
    await finishNaturally(playlistAlpha);
    const playlistBeta = await nextMpv(boundarySockets, 'on-b');
    assert.deepEqual(
      playbackInfoIds(),
      ['on-a', 'on-b'],
      'playlist item B must start before any Continue Watching candidate',
    );
    await finishNaturally(playlistBeta);
    const afterPlaylist = await nextMpv(boundarySockets, 'on-c');
    await finishNaturally(afterPlaylist);
    await pollUntil(() => stoppedResponses() >= 3, 'playlist-boundary Stopped responses');
    await holdsFor(
      () =>
        playbackInfoIds().length > 3
          ? `unexpected post-boundary repeat: ${playbackInfoIds().join(',')}`
          : false,
      2_500,
      'the playlist-to-Continue-Watching run must stop when exhausted',
    );
    assert.deepEqual(playbackInfoIds(), ['on-a', 'on-b', 'on-c']);
    assert.deepEqual(mock.state.contractViolations, []);
    await screenshot('02-playlist-boundary');
  },
};
