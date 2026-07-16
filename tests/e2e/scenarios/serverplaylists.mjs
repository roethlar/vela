// Read-only server playlists: per-source availability, exact server order,
// zero edit affordances/writes, and session-owned sequence advancement.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { makeClips, mockSource, pollUntil, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';

let healthy;
let unavailable;

export default {
  name: 'serverplaylists',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['server-one.mp4', 'server-two.mp4']);
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
      ],
      playlists: [
        { id: 'night', name: 'Server Night', itemIds: ['server-one', 'server-two'] },
      ],
    });
    unavailable = await startMockJellyfin({ failPlaylistList: true });
    seedConfig(configRoot, [
      mockSource(healthy, { id: 'jf-playlists', name: 'Playlist Server' }),
      mockSource(unavailable, { id: 'jf-offline', name: 'Unavailable Server' }),
    ]);
  },

  async cleanup() {
    await Promise.all([healthy?.close(), unavailable?.close()]);
  },

  async run({ driver, screenshot, configRoot }) {
    const localStore = path.join(configRoot, 'config', 'vela', 'playlists.json');
    assert.equal(fs.existsSync(localStore), false, 'server discovery must not create Vela storage');

    await driver.waitFor(
      `return document.querySelector('[data-source-id="jf-playlists"]')?.dataset.playlistState === 'available'
        && document.querySelector('[data-source-id="jf-offline"]')?.dataset.playlistState === 'unavailable'`,
      'healthy and unavailable server playlist groups',
      { timeoutMs: 25_000 },
    );
    assert.equal(
      await driver.exec(
        `return document.querySelector('[data-source-id="jf-offline"] .serverplayliststate')?.textContent.trim()`,
      ),
      'Unavailable',
    );
    const open = await driver.find(
      'css selector',
      'button[aria-label="Open Server Night from Playlist Server"]',
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
    await first.setProp('time-pos', 9.2);
    await first.setProp('pause', false);
    first.close();

    const second = await nextPlayer('server-two');
    await second.setProp('pause', true);
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
    second.close();
  },
};
