// Read-only server playlists: per-source availability, exact server order,
// zero edit affordances/writes, and session-owned sequence advancement.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  clickUnderPlay,
  holdsFor,
  homeButton,
  logicalPlaybackInfoIds,
  makeClips,
  mockSource,
  pollUntil,
  seedConfig,
  waitForPageUncovered,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { shown } from '../proof.mjs';

let healthy;
let unavailable;

export default {
  name: 'serverplaylists',
  rows: ['5'],
  testOnly: [
    {
      row: '5',
      player: 'embedded',
      via: 'clickUnderPlay',
      route: 'clickUnderPlay: going Home during the first server-playlist item',
      proves: 'that the dispatcher repaints Home for the next item underneath the player view',
      unproven: 'that a user can watch Home during the run: the player view covers the page',
    },
  ],

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, [
      'server-one.mp4',
      'server-two.mp4',
      'server-after.mp4',
    ]);
    healthy = await startMockJellyfin({
      movies: [
        {
          id: 'server-one',
          name: 'Server First',
          year: 2024,
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'server-one.mp4'),
        },
        {
          id: 'server-two',
          name: 'Server Second',
          year: 2025,
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'server-two.mp4'),
        },
        {
          id: 'server-after',
          name: 'Server After',
          year: 2026,
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'server-after.mp4'),
        },
      ],
      playlists: [
        { id: 'night', name: 'Server Night', itemIds: ['server-one', 'server-two'] },
      ],
    });
    unavailable = await startMockJellyfin({ failPlaylistList: true });
    seedConfig(
      configRoot,
      [
        mockSource(healthy, { id: 'jf-playlists', name: 'Playlist Server' }),
        mockSource(unavailable, { id: 'jf-offline', name: 'Unavailable Server' }),
      ],
      {
        continue_playing: 'on',
        recents: [
          {
            item: {
              ratingKey: 'jf-playlists:server-after',
              title: 'Server After',
              durationMs: 60_000,
              mediaType: 'movie',
              viewOffsetMs: 1_000,
              played: false,
              sourceId: 'jf-playlists',
              providerIds: [],
            },
            started_at_ms: 0,
            ended_at_ms: 1,
          },
        ],
      },
    );
  },

  async cleanup() {
    await Promise.all([healthy?.close(), unavailable?.close()]);
  },

  async run({ driver, screenshot, configRoot }) {
    const localStore = path.join(configRoot, 'config', 'vela', 'playlists.json');
    assert.equal(fs.existsSync(localStore), false, 'server discovery must not create Vela storage');

    // One get_server_playlists call returns every server's result together, so
    // the healthy server's entry appearing means the failing server's result
    // has arrived too.
    await driver.waitFor(
      `return document.querySelector(
        'button.serverplaylistitem[data-source-id="jf-playlists"]'
      )?.getAttribute('aria-label') === 'Open Server Night from Playlist Server (Jellyfin)'`,
      'the healthy server playlist entry, named with its server kind',
      { timeoutMs: 25_000 },
    );
    // Owner ruling 2026-11-12: a server with no available playlists shows
    // nothing — no sub-heading, no "Unavailable", no placeholder line.
    assert.deepEqual(
      await driver.exec(
        `const nav = document.querySelector('aside nav');
         const headings = [...nav.querySelectorAll('.sidegroup')].map((el) => el.textContent.trim());
         return {
           headings: headings.filter((text) => text === 'Server Playlists' || text === 'Sources'),
           entries: [...nav.querySelectorAll('.serverplaylistitem')].map((el) => [
             el.dataset.sourceId,
             el.textContent.trim(),
             el.querySelector('svg[role="img"]')?.getAttribute('aria-label') ?? null,
           ]),
           offlineInPlaylists: nav.querySelectorAll('.serverplaylistitem[data-source-id="jf-offline"]').length,
           placeholders: /Unavailable(?! Server)|No video playlists|Experimental/.test(nav.textContent),
           sources: [...nav.querySelectorAll('.sourceitem:not(.serverplaylistitem)')].map((el) => [
             el.getAttribute('aria-label'),
             el.querySelector('svg[role="img"]')?.getAttribute('aria-label') ?? null,
           ]),
         };`,
      ),
      {
        headings: ['Server Playlists', 'Sources'],
        entries: [['jf-playlists', 'Server Night', 'Jellyfin']],
        offlineInPlaylists: 0,
        placeholders: false,
        sources: [
          ['Playlist Server (Jellyfin)', 'Jellyfin'],
          ['Unavailable Server (Jellyfin)', 'Jellyfin'],
        ],
      },
    );
    const open = await driver.find(
      'css selector',
      'button[aria-label="Open Server Night from Playlist Server (Jellyfin)"]',
    );
    await driver.click(open);
    await driver.waitFor(
      `return document.querySelector('section.serverplaylist h1')?.textContent.trim() === 'Server Night'
        && document.querySelectorAll('ol[aria-label="Server playlist items"] > li').length === 2`,
      'the read-only server playlist detail',
    );
    assert.deepEqual(
      await driver.exec(
        `return [...document.querySelectorAll('section.serverplaylist .entrymeta strong')]
          .map((element) => element.textContent.trim())`,
      ),
      ['Server First', 'Server Second'],
      'server order must be preserved',
    );
    assert.deepEqual(
      await driver.exec(
        `return [...document.querySelectorAll('section.serverplaylist button, section.serverplaylist input')]
          .map((element) => element.textContent.trim())
          .filter((text) => ['Save', 'Remove', 'Up', 'Down', 'Delete playlist…', 'Delete permanently'].includes(text))`,
      ),
      [],
      'a server playlist must expose no edit affordance',
    );
    await screenshot('01-read-only-groups-and-detail');

    const seen = mpvSocketSnapshot();
    const play = await driver.find('css selector', 'section.serverplaylist button.primary');
    await driver.click(play);

    async function nextPlayer(expectedId) {
      const socket = await waitForNewMpvSocket(seen, { timeoutMs: 20_000 });
      seen.add(socket);
      const mpv = await MpvIpc.connect(socket);
      const loaded = await pollUntil(
        () => mpv.getProp('path').catch(() => null),
        `${expectedId} server-playlist stream`,
      );
      assert.ok(
        loaded.startsWith(`http://127.0.0.1:${healthy.port}/Videos/${expectedId}/stream`),
        `expected ${expectedId}, got ${loaded}`,
      );
      return mpv;
    }

    const first = await nextPlayer('server-one');
    await first.setProp('pause', true);
    // Establish Home before the intermediate EOF. A later navigation would
    // perform its own load and could not prove the dispatcher repainted the
    // backend-owned successor. In the embedded player the view covers the
    // page during the run, so no user can go Home now; Home is reached there
    // through the test-only route clickUnderPlay, and what is proven is the
    // page's repaint underneath the view, not that a user sees it.
    await clickUnderPlay(driver, await homeButton(driver), 'going Home during the first server-playlist item');
    await driver.waitFor(
      `return document.querySelector('[aria-label="Continue watching"] .flowcard.center')?.getAttribute('title') === 'Server First'`,
      'Server First rendered before server-playlist advancement',
    );
    // Hold the successor until the early Home read has captured no Server
    // Second recent. The dispatcher refresh is then the only repaint source.
    healthy.state.playbackInfoDelayMs = 3_000;
    await first.setProp('time-pos', 9.2);
    await first.setProp('pause', false);
    first.close();

    const second = await nextPlayer('server-two');
    await second.setProp('pause', true);
    await driver.waitFor(
      `const cards = [...document.querySelectorAll('[aria-label="Continue watching"] .flowcard')];
       return document.querySelector('[aria-label="Continue watching"] .flowcard.center')?.getAttribute('title') === 'Server Second'
         && cards.some((card) => card.getAttribute('title') === 'Server Second')
         && !cards.some((card) => card.getAttribute('title') === 'Server First');`,
      'the dispatcher repaint with Server Second rendered and Server First suppressed',
    );
    assert.equal(
      fs.existsSync(localStore),
      false,
      'playing a server playlist must not create or mutate Vela playlist storage',
    );
    assert.deepEqual(
      healthy.state.requests.filter(
        (request) => request.path.startsWith('/Playlists/') && request.method !== 'GET',
      ),
      [],
      'Vela must never write to a server playlist',
    );
    assert.deepEqual(healthy.state.contractViolations, []);
    assert.deepEqual(unavailable.state.contractViolations, []);

    // Lose the owning playlist server before the next cursor read. Even with
    // another title waiting in Continue Playing, owner loss must stop this run
    // without treating the cached playlist as exhausted or rerouting it.
    const playlistItemRequests = () => healthy.state.requests.filter(
      (request) => request.method === 'GET' && request.path === '/Playlists/night/Items',
    ).length;
    const beforeOwnerLoss = playlistItemRequests();
    healthy.state.failPlaylistItems = true;
    await second.setProp('time-pos', 9.2);
    await second.setProp('pause', false);
    second.close();

    await pollUntil(
      () => playlistItemRequests() > beforeOwnerLoss,
      'the failed owner-only playlist cursor read',
    );
    await holdsFor(
      () => [...mpvSocketSnapshot()].some((socket) => !seen.has(socket))
        ? 'owner loss launched another player'
        : null,
      3_000,
      'offline server playlist to remain stopped',
    );
    assert.deepEqual(
      logicalPlaybackInfoIds(healthy),
      ['server-one', 'server-two'],
      'owner loss must not masquerade as playlist exhaustion and enter Continue Playing',
    );
    assert.deepEqual(
      logicalPlaybackInfoIds(unavailable),
      [],
      'an alternate configured server must never receive a rerouted playlist play',
    );
    // A run stopped by owner loss leaves the user's page usable (the embedded
    // player's view must not outlive its last play).
    await waitForPageUncovered(driver, 'after owner loss stopped the server playlist');
    // The end of its proof of its rows in this player (proof.mjs).
    shown('5');
  },
};
