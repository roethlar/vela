// Server-stream playback probed over mpv's JSON IPC: seed a mock Jellyfin
// server backed by a generated clip, walk the flipped navigation (card click
// → info page → Play), assert mpv loads the mock stream URL, seek, quit —
// then assert Vela stamped the position into recents and the hero shows the
// item as Continue Watching.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';
import { makeClips, mockSource, seedConfig, pollUntil } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock;

export default {
  name: 'playback',

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
    });
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot }) {
    // Seeded config ⇒ authenticated view with the mock library as a sidebar
    // section. Wait for that specific button: boot loads sections async,
    // and the pre-boot Welcome screen would satisfy any generic render wait.
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Mock Library')`,
      'seeded library in the sidebar',
    );
    const section = await driver.find('xpath', `//button[contains(@class,'sideitem') and normalize-space(.)='Mock Library']`);
    await driver.click(section);

    await screenshot('01-grid');
    const card = await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="Mock Movie"]')`,
      'movie card in the grid',
    ).catch(async (err) => {
      const state = await driver.exec(
        `return {posters: [...document.querySelectorAll('button.poster')].map(b => b.getAttribute('aria-label')), text: document.body.innerText.slice(0, 400)}`,
      );
      err.message += ` — posters: ${JSON.stringify(state.posters)}; page: ${JSON.stringify(state.text)}`;
      throw err;
    }).then(() => driver.find('css selector', 'button.poster[aria-label^="Mock Movie"]'));

    // Nav flip (74ff385): a library card click opens the INFO PAGE, it does
    // not play. Assert the flip held, then play from the detail page.
    const socketsBefore = mpvSocketSnapshot();
    await driver.click(card);
    await driver.waitFor(
      `return !!document.querySelector('.detail button.playwide')`,
      'info page with a Play button after the card click',
    );
    await screenshot('02-detail');
    const play = await driver.find('css selector', '.detail button.playwide');
    await driver.click(play);

    // mpv side: the loaded path is the mock stream; seek to 6s; let Vela's
    // progress poll observe it; quit.
    const socketPath = await waitForNewMpvSocket(socketsBefore).catch(async (err) => {
      const text = await driver.exec(`return document.body.innerText.slice(0, 400)`).catch(() => '?');
      const { execSync } = await import('node:child_process');
      let mpvProcs = '';
      try { mpvProcs = execSync('pgrep -a -x mpv || true').toString().trim(); } catch {}
      let recents = '?';
      try {
        recents = JSON.stringify(JSON.parse(fs.readFileSync(path.join(configRoot, 'config', 'vela', 'config.json'), 'utf8')).recents ?? []);
      } catch {}
      err.message += ` — page: ${JSON.stringify(text)}; mpv procs: ${JSON.stringify(mpvProcs)}; recents: ${recents}`;
      throw err;
    });
    const mpv = await MpvIpc.connect(socketPath);
    try {
      const loaded = await pollUntil(
        () => mpv.getProp('path').catch(() => null),
        'mpv to load the stream',
      );
      assert.ok(
        loaded.startsWith(`http://127.0.0.1:${mock.port}/Videos/m1/stream`),
        `mpv must play the mock stream URL, got ${loaded}`,
      );
      await pollUntil(
        () => mpv.getProp('time-pos').then((t) => t > 0.5).catch(() => false),
        'playback to progress past 0.5s',
      );
      await mpv.setProp('time-pos', 6);
      // Vela polls the same socket for progress; give it a beat to observe ≥6s.
      await new Promise((r) => setTimeout(r, 1500));
      mpv.quit();
    } finally {
      mpv.close();
    }
    // Prove the quit acted (eh-7): the IPC socket must stop accepting well
    // before the 10s clip could reach natural EOF. The socket FILE is only
    // unlinked when Vela cleans its runtime dir, so probe connectability.
    const { createConnection } = await import('node:net');
    await pollUntil(
      () =>
        new Promise((resolve) => {
          const probe = createConnection(socketPath);
          probe.once('connect', () => {
            probe.destroy();
            resolve(false); // still accepting: mpv alive
          });
          probe.once('error', () => resolve(true)); // refused: mpv gone
        }),
      'mpv socket to stop accepting after quit',
      { timeoutMs: 4000 },
    );

    // Vela side: mpv exit stamps the final position into recents…
    const configFile = path.join(configRoot, 'config', 'vela', 'config.json');
    // record_recent creates the entry at play START (offset null); finish()
    // stamps the position at mpv exit — wait for the stamp, not the entry.
    const recent = await pollUntil(() => {
      try {
        const cfg = JSON.parse(fs.readFileSync(configFile, 'utf8'));
        const r = cfg.recents?.[0];
        return (r?.item?.viewOffsetMs ?? 0) > 0 ? r : null; // Item serializes camelCase
      } catch {
        return null; // mid-write / lock churn
      }
    }, 'the recents position stamp after mpv exit');
    assert.equal(recent.item.title, 'Mock Movie');
    const offset = recent.item.viewOffsetMs;
    // Upper bound 8000: seek 6s + ≤1.5s observed playback + margin. A
    // natural-EOF stamp (~10s) must NOT pass — that's the eh-7 failure mode.
    assert.ok(
      offset >= 3000 && offset <= 8000,
      `expected a mid-clip resume position (3000..8000), got ${offset}ms`,
    );

    // …and the hero cover-flow shows it as Continue Watching.
    await driver.find('xpath', `//button[contains(@class,'sideitem') and normalize-space(.)='Home']`).then((el) => driver.click(el));
    await driver.waitFor(
      `return !!document.querySelector('[aria-label="Continue watching"] [aria-label^="Resume Mock Movie"]')`,
      'movie in the Continue Watching hero',
    ).catch(async (err) => {
      const state = await driver.exec(
        `return {hero: [...document.querySelectorAll('[aria-label="Continue watching"] [aria-label]')].map(e => e.getAttribute('aria-label')), text: document.body.innerText.slice(0, 300)}`,
      ).catch(() => '?');
      err.message += ` — ${JSON.stringify(state)}`;
      throw err;
    });
    await screenshot('03-hero');
  },
};
