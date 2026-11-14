// Playlist playback context: arbitrary start, same-key tracker replacement,
// live edit re-read, mixed-source advancement, unavailable skip, silent
// resume, window-state inheritance by every play, dispatcher-owned recents,
// and byte-identical read-only playback.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  clickUnderPlay,
  holdsFor,
  homeButton,
  logicalPlaybackInfoIds,
  makeClips,
  mockSource,
  pollUntil,
  runPlayer,
  seedConfig,
  seedPlaylists,
  stopPlayAsUser,
  waitForPageUncovered,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';
import { shown } from '../proof.mjs';

let mockA;
let mockB;

function item(sourceId, id, title, viewOffsetMs = null) {
  return {
    ratingKey: `${sourceId}:${id}`,
    title,
    year: null,
    summary: null,
    durationMs: 60_000,
    mediaType: 'movie',
    poster: null,
    seriesPoster: null,
    backdrop: null,
    viewOffsetMs,
    played: false,
    lastWatchedAtMs: null,
    addedAtMs: null,
    index: null,
    parentIndex: null,
    grandparentTitle: null,
    parentTitle: null,
    parentRatingKey: null,
    grandparentRatingKey: null,
    sourceId,
    providerIds: [],
    backing: null,
    canonicalId: null,
    watchKey: null,
    detailKey: null,
  };
}

function playlistEntry(id, sourceName, savedItem) {
  return { id, item: savedItem, sourceName };
}

async function clickRowAction(driver, title, action) {
  const button = await driver.find(
    'xpath',
    `//ol[@aria-label='Playlist items']/li[.//strong[normalize-space(.)='${title}']]//div[contains(@class,'entryactions')]/button[normalize-space(.)='${action}']`,
  );
  await driver.click(button);
}

const stoppedResponses = (mock) =>
  mock.state.served.filter(
    (response) => response.method === 'POST' && response.path === '/Sessions/Playing/Stopped',
  ).length;

const stoppedRequests = (mock) =>
  mock.state.requests.filter(
    (request) => request.method === 'POST' && request.path === '/Sessions/Playing/Stopped',
  ).length;

const playbackInfoIds = (mock) =>
  logicalPlaybackInfoIds(mock);

async function waitForMpvProperty(mpv, property, expected, what) {
  await pollUntil(
    async () => {
      try {
        const value = await mpv.getProp(property);
        return value === expected ? { value } : null;
      } catch {
        return null;
      }
    },
    what,
  );
}

// Window-state inheritance is the external player's alone: it is about mpv's
// own window (`--fullscreen`, `--window-maximized` handed to the next play),
// and the embedded player has none (the plan's "Behaviour inventory": not in
// the list, external-only by nature). The embedded launch deliberately hands
// its mpv no window state (`build_launch_for`, held by the Rust test
// the_full_embedded_launch_is_exactly_the_recorded_one), and there these are
// options of a library that draws in Vela's window, which change nothing on
// screen. The embedded counterpart, Vela's own window staying fullscreen from
// one play of a run to the next, is proven by the `continuetv` scenario.
const windowStates = runPlayer === 'external';

async function waitForWindowState(mpv, expected, what) {
  if (!windowStates) return;
  await pollUntil(
    async () => {
      try {
        const [fullscreen, maximized] = await Promise.all([
          mpv.getProp('fullscreen'),
          mpv.getProp('window-maximized'),
        ]);
        return fullscreen === expected.fullscreen && maximized === expected.maximized
          ? { fullscreen, maximized }
          : null;
      } catch {
        return null;
      }
    },
    what,
  );
}

async function setWindowState(mpv, state, what) {
  if (!windowStates) return;
  await mpv.setProp('fullscreen', state.fullscreen);
  await waitForMpvProperty(mpv, 'fullscreen', state.fullscreen, `${what} fullscreen readback`);
  await mpv.setProp('window-maximized', state.maximized);
  await waitForMpvProperty(
    mpv,
    'window-maximized',
    state.maximized,
    `${what} maximized readback`,
  );
  await waitForWindowState(mpv, state, what);
}

export default {
  name: 'playlistplay',
  rows: ['5'],
  testOnly: [
    {
      row: '5',
      player: 'embedded',
      via: 'clickUnderPlay',
      route: 'clickUnderPlay: the three "Move Alpha Edited Next up" clicks and going Home while Beta Start plays',
      proves: 'that the dispatcher re-reads a playlist edited during a play, and repaints Home, underneath the player view',
      unproven: 'that a user can edit a playlist or watch Home during a play: the player view covers the page',
    },
  ],

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['b0.mp4', 'a1.mp4', 'a2.mp4', 'b1.mp4']);
    mockA = await startMockJellyfin({
      movies: [
        { id: 'a0', name: 'Anchor', year: 2020, runTimeTicks: 600_000_000 },
        {
          id: 'a1',
          name: 'Alpha Original Next',
          year: 2021,
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'a1.mp4'),
        },
        {
          id: 'a2',
          name: 'Alpha Edited Next',
          year: 2022,
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'a2.mp4'),
        },
      ],
    });
    mockB = await startMockJellyfin({
      movies: [
        {
          id: 'b0',
          name: 'Beta Start',
          year: 2023,
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'b0.mp4'),
        },
        {
          id: 'b1',
          name: 'Beta Last',
          year: 2024,
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'b1.mp4'),
        },
      ],
    });
    mockA.state.userData.a2.positionTicks = 10_000_000; // 1s resume
    seedConfig(
      configRoot,
      [
        mockSource(mockA, { id: 'jf-a', name: 'Mock JF A' }),
        mockSource(mockB, { id: 'jf-b', name: 'Mock JF B' }),
      ],
      {
        mpv_extra_args:
          '--vo=null\n--ao=null\n--window-maximized=yes\n--fullscreen=yes',
      },
    );
    seedPlaylists(configRoot, [
      {
        id: 'p-sequence',
        name: 'Sequence',
        items: [
          playlistEntry('e0', 'Mock JF A', item('jf-a', 'a0', 'Anchor')),
          playlistEntry('e1', 'Mock JF B', item('jf-b', 'b0', 'Beta Start')),
          playlistEntry('e2', 'Mock JF A', item('jf-a', 'a1', 'Alpha Original Next')),
          playlistEntry('e3', 'Removed source', item('gone', 'g0', 'Unavailable Ghost')),
          playlistEntry('e4', 'Mock JF B', item('jf-b', 'b1', 'Beta Last')),
          playlistEntry('e5', 'Mock JF A', item('jf-a', 'a2', 'Alpha Edited Next', 1_000)),
        ],
        createdMs: 1,
        updatedMs: 1,
      },
    ]);
  },

  async cleanup() {
    await Promise.all([mockA?.close(), mockB?.close()]);
  },

  async run({ driver, screenshot, configRoot }) {
    const playlistFile = path.join(configRoot, 'config', 'vela', 'playlists.json');
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const readStore = () => JSON.parse(fs.readFileSync(playlistFile, 'utf8'));
    const readConfig = () => JSON.parse(fs.readFileSync(configFile, 'utf8'));

    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Playlists')`,
      'the Playlists sidebar entry',
    );
    const playlists = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Playlists']`,
    );
    await driver.click(playlists);
    await driver.waitFor(
      `return !!document.querySelector('section.playlists .playlistgrid button[aria-label^="Open Sequence,"]')`,
      'the seeded Sequence playlist',
    );
    const sequence = await driver.find(
      'css selector',
      'section.playlists .playlistgrid button[aria-label^="Open Sequence,"]',
    );
    await driver.click(sequence);
    await driver.waitFor(
      `return document.querySelectorAll('section.playlists ol.entries > li').length === 6`,
      'the seeded sequence',
    );
    assert.equal(
      await driver.exec(
        `return !![...document.querySelectorAll('section.playlists ol.entries > li')]
          .find((row) => row.querySelector('.entrymeta strong')?.textContent.trim() === 'Unavailable Ghost')
          ?.classList.contains('unavailable')`,
      ),
      true,
      'the removed-source entry is visible and unavailable before playback',
    );
    const bytesBeforePlayback = fs.readFileSync(playlistFile);

    const seenSockets = mpvSocketSnapshot();
    async function nextMpv(expectedPort, expectedId) {
      const socket = await waitForNewMpvSocket(seenSockets, { timeoutMs: 20000 });
      seenSockets.add(socket);
      const mpv = await MpvIpc.connect(socket);
      const loaded = await pollUntil(
        () => mpv.getProp('path').catch(() => null),
        `${expectedId} stream in mpv`,
      );
      assert.ok(
        loaded.startsWith(`http://127.0.0.1:${expectedPort}/Videos/${expectedId}/stream`),
        `expected ${expectedId} from :${expectedPort}, got ${loaded}`,
      );
      return { mpv, socket };
    }

    // Arbitrary start index: Anchor at index 0 has no media file and must
    // never be resolved when Beta Start at index 1 is clicked.
    await clickRowAction(driver, 'Beta Start', 'Play');
    const firstBeta = await nextMpv(mockB.port, 'b0');
    await firstBeta.mpv.setProp('pause', true);
    await setWindowState(
      firstBeta.mpv,
      { fullscreen: false, maximized: false },
      'the first manually-started player to leave fullscreen and maximized state',
    );
    const firstRecent = await pollUntil(() => {
      try {
        const recent = readConfig().recents?.[0];
        return recent?.item?.ratingKey === 'jf-b:b0' ? recent : null;
      } catch {
        return null;
      }
    }, 'the first Beta session recent');
    assert.equal(firstRecent.ended_at_ms, 0);
    assert.deepEqual(fs.readFileSync(playlistFile), bytesBeforePlayback);
    assert.deepEqual(playbackInfoIds(mockA), [], 'starting at index 1 must not touch Anchor');

    // Replace the same key while its old Stopped response is parked. The old
    // tracker must not close or stamp the replacement session.
    const stoppedBefore = stoppedResponses(mockB);
    const stoppedAskedBefore = stoppedRequests(mockB);
    mockB.state.delayNextStoppedMs = 5_000;
    if (runPlayer === 'embedded') {
      // The one branch on the player here, on how the user replaces the play.
      // The external player's user presses Play on the row while mpv plays in
      // its own window beside the page, and the new play stops the old one.
      // The embedded player's view covers the playlist, so its user cannot
      // reach the row during the play: they close the play (the view's Close)
      // and press Play again. The old Stopped is parked either way, and the
      // assertions below are the same for both. What the embedded route does
      // NOT reach is a new play stopping a running one (row 12 of the parity
      // list); that stays with the external player here.
      await stopPlayAsUser(driver, firstBeta.mpv, firstBeta.socket, 'the first Beta play');
    }
    await clickRowAction(driver, 'Beta Start', 'Play');
    const secondBeta = await nextMpv(mockB.port, 'b0');
    await secondBeta.mpv.setProp('pause', true);
    // The configured options say yes/yes, so no/no here can only come from
    // the first player's observed window: a manual play inherits it too.
    await waitForWindowState(
      secondBeta.mpv,
      { fullscreen: false, maximized: false },
      'the manual replacement to inherit the first player\'s window state over configured yes',
    );
    firstBeta.mpv.close();
    const replacement = await pollUntil(() => {
      try {
        const recent = readConfig().recents?.[0];
        return recent?.item?.ratingKey === 'jf-b:b0' && recent.session_id !== firstRecent.session_id
          ? recent
          : null;
      } catch {
        return null;
      }
    }, 'the replacement same-key session');
    assert.equal(replacement.ended_at_ms, 0);
    // The hazard is live: the old play's Stopped was asked and is still
    // parked while the replacement runs, in either player's route.
    await pollUntil(
      () => stoppedRequests(mockB) > stoppedAskedBefore,
      'the old play to ask its Stopped',
      { timeoutMs: 4_000 },
    );
    assert.equal(
      stoppedResponses(mockB),
      stoppedBefore,
      'the old Stopped must still be parked while the replacement session runs',
    );
    await pollUntil(
      () => stoppedResponses(mockB) > stoppedBefore,
      'the delayed old Stopped response to be delivered',
      { timeoutMs: 10_000 },
    );
    await holdsFor(
      () => {
        try {
          const recent = readConfig().recents?.[0];
          if (recent?.session_id !== replacement.session_id) return 'replacement session changed';
          if (recent.ended_at_ms !== 0) return 'old tracker closed the replacement session';
          return false;
        } catch {
          return false;
        }
      },
      1_000,
      'the stale same-key tracker must remain a no-op',
    );

    // Edit while B0 is still playing. Moving A2 from the tail to immediately
    // after B0 makes it the next item only if dispatch re-reads the store.
    // In the embedded player no user can edit the playlist during a play (the
    // view covers it); the edit is made there through the test-only route
    // clickUnderPlay, so that what the backend does with a store edited
    // mid-play is still proven in that player. Unproven there: that a user
    // could make the edit.
    for (const expectedIndex of [4, 3, 2]) {
      await driver.waitFor(
        `return !document.querySelector('button[aria-label="Move Alpha Edited Next up"]')?.disabled`,
        'the next move-up action to become available',
      );
      const move = await driver.find(
        'css selector',
        'button[aria-label="Move Alpha Edited Next up"]',
      );
      await clickUnderPlay(driver, move, 'editing the playlist while Beta Start plays');
      await pollUntil(
        () =>
          readStore().playlists[0].items.findIndex(
            (entry) => entry.item.title === 'Alpha Edited Next',
          ) === expectedIndex,
        `Alpha Edited Next to move to index ${expectedIndex}`,
      );
    }
    const editedOrder = [
      'Anchor',
      'Beta Start',
      'Alpha Edited Next',
      'Alpha Original Next',
      'Unavailable Ghost',
      'Beta Last',
    ];
    assert.deepEqual(
      readStore().playlists[0].items.map((entry) => entry.item.title),
      editedOrder,
    );
    const bytesAfterEdit = fs.readFileSync(playlistFile);

    // Keep Home visible across the backend-owned boundary. Navigating there
    // after A2 starts would itself refetch and make a missing dispatcher repaint
    // pass vacuously. In the embedded player Home is under the view during
    // the run, out of the user's sight, and is reached by the test-only route
    // (clickUnderPlay); what is proven there is the page's repaint underneath
    // the view, not that a user watches Home during a play.
    await clickUnderPlay(driver, await homeButton(driver), 'going Home while Beta Start plays');
    await driver.waitFor(
      `return document.querySelector('[aria-label="Continue watching"] .flowcard.center')?.getAttribute('title') === 'Beta Start'`,
      'the active Beta Start recent on Home before playlist advancement',
    );
    // Let the pre-successor Home read settle while A2 cannot yet be recorded;
    // without the dispatcher repaint, A2 may exist in mpv/config but not here.
    mockA.state.playbackInfoDelayMs = 3_000;

    await setWindowState(
      secondBeta.mpv,
      { fullscreen: false, maximized: false },
      'the playlist predecessor window state before clean EOF',
    );

    async function releaseToEof(session) {
      await session.mpv.setProp('time-pos', 9.2);
      await session.mpv.setProp('pause', false);
      session.mpv.close();
    }

    await releaseToEof(secondBeta);
    const editedNext = await nextMpv(mockA.port, 'a2');
    await editedNext.mpv.setProp('pause', true);
    await waitForWindowState(
      editedNext.mpv,
      { fullscreen: false, maximized: false },
      'the automatic successor to inherit explicit windowed state over configured yes',
    );
    // With --vo=null a player has no window and reports its size as 0x0. The
    // predecessor started neither fullscreen nor maximized, so that report
    // reached the size rule, and 0x0 is not a size: nothing is passed. A real
    // windowed size cannot be produced in this venue, so only this side is
    // checked here. The external player only: the embedded launch is handed
    // no window state at all (as for the steps above).
    if (windowStates) {
      assert.equal(
        await editedNext.mpv.getProp('geometry'),
        '',
        'a 0x0 size report from a player with no window must not become --geometry',
      );
    }
    // mpv names its file before it has a position in it. In the external
    // player the window-state wait above used to give it that moment; in the
    // embedded one there is no such wait, so the position is waited for.
    const resumedAt = await pollUntil(
      () =>
        editedNext.mpv
          .getProp('time-pos')
          .then((t) => (typeof t === 'number' ? { t } : null))
          .catch(() => null),
      'the auto-advanced item to have a position',
    ).then(({ t }) => t);
    assert.ok(
      resumedAt >= 0.7 && resumedAt < 3,
      `auto-advanced in-progress item should silently resume near 1s, got ${resumedAt}s`,
    );
    await pollUntil(() => {
      try {
        const recent = readConfig().recents?.[0];
        return recent?.item?.ratingKey === 'jf-a:a2' && recent.ended_at_ms === 0;
      } catch {
        return false;
      }
    }, 'the dispatcher-owned open recent for A2');
    await driver.waitFor(
      `const cards = [...document.querySelectorAll('[aria-label="Continue watching"] .flowcard')];
       return document.querySelector('[aria-label="Continue watching"] .flowcard.center')?.getAttribute('title') === 'Alpha Edited Next'
         && cards.some((card) => card.getAttribute('title') === 'Alpha Edited Next')
         && !cards.some((card) => card.getAttribute('title') === 'Beta Start');`,
      'the dispatcher repaint with A2 rendered and completed B0 suppressed',
    );

    await setWindowState(
      editedNext.mpv,
      { fullscreen: true, maximized: false },
      'the first automatic successor window state before its clean EOF',
    );
    await releaseToEof(editedNext);
    const originalNext = await nextMpv(mockA.port, 'a1');
    await originalNext.mpv.setProp('pause', true);
    await waitForWindowState(
      originalNext.mpv,
      { fullscreen: true, maximized: false },
      'the next automatic successor to inherit the freshly published observation',
    );
    await pollUntil(() => {
      try {
        const config = readConfig();
        return (
          !(config.recents ?? []).some((entry) => entry.item?.ratingKey === 'jf-a:a2')
          && (config.hidden_from_continue ?? []).includes('jf-a:a2')
        );
      } catch {
        return false;
      }
    }, 'the clean A2 completion removal and tombstone');

    await releaseToEof(originalNext);
    const last = await nextMpv(mockB.port, 'b1');
    await last.mpv.setProp('pause', true);
    await pollUntil(() => {
      try {
        const recent = readConfig().recents?.[0];
        return recent?.item?.ratingKey === 'jf-b:b1' && recent.ended_at_ms === 0;
      } catch {
        return false;
      }
    }, 'the open dispatcher recent for the mixed-source final item');
    await releaseToEof(last);
    await pollUntil(() => {
      try {
        const config = readConfig();
        return (
          !(config.recents ?? []).some((entry) => entry.item?.ratingKey === 'jf-b:b1')
          && (config.hidden_from_continue ?? []).includes('jf-b:b1')
        );
      } catch {
        return false;
      }
    }, 'the final mixed-source completion curation');

    assert.deepEqual(fs.readFileSync(playlistFile), bytesAfterEdit, 'playback never writes playlists.json');
    assert.deepEqual(playbackInfoIds(mockA), ['a2', 'a1']);
    assert.deepEqual(playbackInfoIds(mockB), ['b0', 'b0', 'b1']);
    // The end of the playlist ends the last play: the page is the user's
    // again (the embedded player's view must not outlive it).
    await waitForPageUncovered(driver, 'at the end of the playlist');
    // Return to the durable playlist after the visible-Home guard so the
    // original unavailable-entry assertion remains part of this run.
    await driver.click(
      await driver.find(
        'xpath',
        `//button[contains(@class,'sideitem') and normalize-space(.)='Playlists']`,
      ),
    );
    await driver.waitFor(
      `return !!document.querySelector('section.playlists .playlistgrid button[aria-label^="Open Sequence,"]')`,
      'the Sequence playlist after playback',
    );
    await driver.click(
      await driver.find(
        'css selector',
        'section.playlists .playlistgrid button[aria-label^="Open Sequence,"]',
      ),
    );
    await driver.waitFor(
      `return document.querySelectorAll('section.playlists ol.entries > li').length === 6`,
      'the unchanged edited sequence after playback',
    );
    assert.equal(
      await driver.exec(
        `return !![...document.querySelectorAll('section.playlists ol.entries > li')]
          .find((row) => row.querySelector('.entrymeta strong')?.textContent.trim() === 'Unavailable Ghost')
          ?.classList.contains('unavailable')`,
      ),
      true,
      'skipping never removes the unavailable curated entry',
    );
    await screenshot('01-edited-sequence-finished');
    // The end of its proof of its rows in this player (proof.mjs).
    shown('5');
  },
};
