// Continue Playing settings persistence and immediate Off behavior. The mode
// changes without restarting, then a natural EOF must stop even though another
// item is visibly available in the exact Continue Watching flow.
import assert from 'node:assert/strict';
import fs from 'node:fs';
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

let mock;

function item(id, title, endedAt) {
  return {
    item: {
      ratingKey: `jf-mock:${id}`,
      title,
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

async function openPlayerSettings(driver) {
  await driver.waitFor(
    `return !!document.querySelector('button[aria-label="Settings"]')`,
    'the Settings button',
  );
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

async function finishNaturally(session) {
  await session.setProp('time-pos', 9.2);
  await session.setProp('pause', false);
  session.close();
}

const playbackInfoIds = () =>
  logicalPlaybackInfoIds(mock);

export default {
  name: 'continueoff',

  async seed({ configRoot }) {
    const mediaDir = makeClips(configRoot, ['off-a.mp4', 'off-b.mp4']);
    mock = await startMockJellyfin({
      movies: [
        {
          id: 'off-a',
          name: 'Off Alpha',
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'off-a.mp4'),
        },
        {
          id: 'off-b',
          name: 'Off Beta',
          runTimeTicks: 600_000_000,
          mediaFile: path.join(mediaDir, 'off-b.mp4'),
        },
      ],
    });
    seedConfig(configRoot, [mockSource(mock)], {
      continue_playing: 'on',
      recents: [item('off-a', 'Off Alpha', 200), item('off-b', 'Off Beta', 100)],
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    const readConfig = () => JSON.parse(fs.readFileSync(configFile, 'utf8'));

    await driver.waitFor(
      `return document.querySelectorAll('[aria-label="Continue watching"] .flowcard').length === 2`,
      'two Continue Watching choices',
    );
    await openPlayerSettings(driver);
    assert.equal(
      await driver.exec(`return document.querySelector('select#continue-playing')?.value`),
      'on',
      'the persisted starting mode must load',
    );
    await driver.exec(
      `const select = document.querySelector('select#continue-playing');
       select.value = 'off';
       select.dispatchEvent(new Event('change', { bubbles: true }));`,
    );
    await driver.click(
      await driver.find('xpath', `//button[normalize-space(.)='Save Continue Playing']`),
    );
    await pollUntil(
      () => readConfig().continue_playing === 'off',
      'the Off mode in config.json',
    );
    await driver.click(await driver.find('css selector', 'button[aria-label="Close"]'));

    // The callback from Settings must take effect immediately: no restart
    // before this clean EOF.
    const before = mpvSocketSnapshot();
    await driver.click(
      await driver.find('css selector', '[aria-label="Continue watching"] .flowcard.center'),
    );
    const socket = await waitForNewMpvSocket(before);
    const mpv = await MpvIpc.connect(socket);
    await mpv.setProp('pause', true);
    assert.ok(
      String(await mpv.getProp('path')).includes('/Videos/off-a/stream'),
      'the rendered head must start first',
    );
    await finishNaturally(mpv);
    await pollUntil(
      () =>
        mock.state.served.some(
          (response) =>
            response.method === 'POST' && response.path === '/Sessions/Playing/Stopped',
        ),
      'the final Stopped response',
    );
    await holdsFor(
      () =>
        playbackInfoIds().length > 1
          ? `unexpected continuation requests: ${playbackInfoIds().join(',')}`
          : false,
      2_500,
      'Off must remain stopped after clean EOF',
    );
    assert.deepEqual(playbackInfoIds(), ['off-a']);
    await screenshot('01-off-stopped');

    await restart();
    await openPlayerSettings(driver);
    assert.equal(
      await driver.exec(`return document.querySelector('select#continue-playing')?.value`),
      'off',
      'Off must survive an app restart',
    );
    await screenshot('02-off-persisted');
  },
};
