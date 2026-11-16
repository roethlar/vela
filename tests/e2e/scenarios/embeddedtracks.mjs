// In-page track lists must survive both playback updates and the bar's hide
// deadline. Run only in the embedded player, using real clicks/keys and mpv
// IPC to check the result of each choice.
import assert from 'node:assert/strict';
import path from 'node:path';
import { Driver } from '../driver.mjs';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import {
  DISPLAYLESS_MPV_ARGS,
  holdsFor,
  makeClips,
  mockSource,
  openDetailAndPlay,
  openLibraryGrid,
  pollUntil,
  recordPlayerEvents,
  seedConfig,
  showPlayerBar,
  stopPlayAsUser,
} from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock;
const click = async (driver, selector) => driver.click(await driver.find('css selector', selector));
const viewState = (driver) => driver.exec(`
  const player = document.querySelector('.player');
  const bar = player?.querySelector('.bar');
  const list = player?.querySelector('[role="listbox"]');
  const states = window.__playerEvents?.states ?? [];
  return {
    view: !!player,
    list: list?.getAttribute('aria-label') ?? null,
    highlight: list?.getAttribute('aria-activedescendant') ?? null,
    options: [...(list?.querySelectorAll('[role="option"]') ?? [])].map(el => ({
      id: el.id, label: el.textContent.trim(), selected: el.getAttribute('aria-selected'),
    })),
    bar: !!bar && !bar.classList.contains('hidden') && getComputedStyle(bar).visibility === 'visible',
    cursor: player ? getComputedStyle(player).cursor : null,
    focus: document.activeElement?.className ?? null,
    fullscreen: player?.querySelector('.fullscreen')?.getAttribute('aria-pressed') ?? null,
    size: [window.innerWidth, window.innerHeight],
    states: states.length,
    position: states.at(-1)?.position ?? null,
  };
`);

async function openList(driver, kind, label) {
  await showPlayerBar(driver);
  await click(driver, `.player button.${kind}`);
  await driver.waitFor(
    `const list = document.querySelector('.player [role="listbox"]');
     return list?.getAttribute('aria-label') === ${JSON.stringify(label)};`,
    `${label} list to open`,
    { timeoutMs: 3000 },
  );
}

async function chosen(driver, mpv, selector, property, expected) {
  await click(driver, selector);
  await driver.waitFor(
    `return !document.querySelector('.player [role="listbox"]')`,
    `the list to close after choosing ${property}=${expected}`,
    { timeoutMs: 3000 },
  );
  let last;
  try {
    await pollUntil(async () => {
      last = await mpv.getProp(property);
      return last === expected || (expected === 'no' && last === false);
    }, `mpv ${property}=${expected}`, { timeoutMs: 5000 });
  } catch (error) {
    error.message += `; saw ${property}=${JSON.stringify(last)}`;
    throw error;
  }
  assert.equal(await driver.exec(`return document.activeElement === document.querySelector('.player')`), true,
    `focus returned to the view after ${property}=${expected}`);
  console.log(`Chose ${property}=${expected}; mpv reports ${JSON.stringify(last)}`);
}

export default {
  name: 'embeddedtracks',
  onlyPlayer: 'embedded',
  onlyPlayerWhy: 'the in-page Audio and Subtitles controls exist only in the embedded player',
  rows: [],
  noRows: 'track-list interaction and persistence during playback; no whole row of the player behaviour inventory is shown',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['tracks.mkv'], { withTracks: true });
    mock = await startMockJellyfin({
      movies: [{
        id: 'm1', name: 'Mock Movie', year: 2020,
        runTimeTicks: 60 * 10_000_000,
        mediaFile: path.join(mediaDir, 'tracks.mkv'),
      }],
    });
    seedConfig(configRoot, [mockSource(mock)], {
      mpv_extra_args: `${DISPLAYLESS_MPV_ARGS}\n--aid=1\n--sid=no`,
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    await openLibraryGrid(driver);
    await recordPlayerEvents(driver);
    const beforeSockets = mpvSocketSnapshot();
    await openDetailAndPlay(driver, 'button.poster[aria-label^="Mock Movie"]');
    const socketPath = await waitForNewMpvSocket(beforeSockets);
    const mpv = await MpvIpc.connect(socketPath);
    try {
      await pollUntil(() => mpv.getProp('time-pos').then(t => t > 0.5).catch(() => false), 'the clip to play');
      await driver.waitFor(
        `return ['audio', 'subtitles'].every(kind => {
          const button = document.querySelector('.player button.' + kind);
          return button && !button.disabled;
        });`,
        'both track buttons to be enabled',
      );
      assert.equal(await mpv.getProp('pause'), false, 'open Audio while PLAYING');
      await openList(driver, 'audio', 'Audio track');
      const before = await viewState(driver);
      const positionBefore = await mpv.getProp('time-pos');
      assert.equal(before.options.length, 2, `two audio tracks: ${JSON.stringify(before)}`);
      await holdsFor(async () => {
        const seen = await viewState(driver);
        return seen.list !== 'Audio track' || !seen.bar || seen.cursor === 'none' || seen.highlight !== before.highlight
          ? JSON.stringify(seen) : false;
      }, 4000, 'Audio list, highlight, bar and pointer stay present while playing');
      const after = await viewState(driver);
      const positionAfter = await mpv.getProp('time-pos');
      assert.equal(await mpv.getProp('pause'), false, 'the wait did not pause playback');
      assert.ok(positionAfter > positionBefore + 2, `mpv position advanced: ${positionBefore} -> ${positionAfter}`);
      assert.ok(after.states > before.states && after.position > before.position,
        `player-state events advanced during the hold: ${JSON.stringify({ before, after })}`);
      console.log(`Audio stayed open for 4s: mpv ${positionBefore} -> ${positionAfter}; states ${before.states} -> ${after.states}`);
      await screenshot('01-audio-still-open');
      await chosen(driver, mpv, '.player #player-track-2', 'aid', 2);

      await openList(driver, 'subtitles', 'Subtitles');
      await chosen(driver, mpv, '.player #player-track-1', 'sid', 1);
      await openList(driver, 'subtitles', 'Subtitles');
      await chosen(driver, mpv, '.player #player-track-no', 'sid', 'no');

      // Escape must dismiss only the list, including in fullscreen, where
      // the player's ordinary Escape would otherwise leave fullscreen.
      const windowed = await viewState(driver);
      await driver.pressKeys('f');
      await driver.waitFor(
        `return document.querySelector('.player .fullscreen')?.getAttribute('aria-pressed') === 'true'
          && window.innerHeight > ${windowed.size[1]};`,
        'the window to enter fullscreen before the Escape check',
      );
      await openList(driver, 'audio', 'Audio track');
      const beforeEscape = await viewState(driver);
      await driver.pressKeys(Driver.KEY.Escape);
      await driver.waitFor(`return !document.querySelector('.player [role="listbox"]')`, 'Escape to close the list');
      // Allow a mistaken fullscreen/stop request time to arrive.
      await holdsFor(async () => {
        const seen = await viewState(driver);
        return !seen.view || seen.list !== null || seen.fullscreen !== beforeEscape.fullscreen
          || JSON.stringify(seen.size) !== JSON.stringify(beforeEscape.size)
          // The view's class list also carries Svelte's scope class.
          || !String(seen.focus ?? '').split(' ').includes('player')
          ? JSON.stringify(seen) : false;
      }, 1000, 'Escape leaves the player and fullscreen window intact, with focus on the view');
      console.log(`Escape closed only the list; fullscreen ${beforeEscape.size.join('x')} unchanged`);
      await stopPlayAsUser(driver, mpv, socketPath, 'track controls checked', { key: 'q' });
    } catch (error) {
      const page = await viewState(driver).catch(e => ({ error: e.message }));
      const playback = {};
      for (const property of ['time-pos', 'pause', 'aid', 'sid']) {
        playback[property] = await mpv.getProp(property).catch(e => e.message);
      }
      error.message += `\nObserved page: ${JSON.stringify(page)}\nObserved mpv: ${JSON.stringify(playback)}`;
      throw error;
    } finally {
      mpv.close();
    }
  },
};
