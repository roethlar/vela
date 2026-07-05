// Local-source playback probed over mpv's JSON IPC: seed a folder with a
// generated clip, play it through a real UI card click, assert the loaded
// path, seek, quit — then assert Vela stamped the position into recents and
// the hero shows the item as Continue Watching.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from '../mpv.mjs';

const CLIP = 'E2E Clip.mp4';

async function pollUntil(fn, what, { timeoutMs = 15000, intervalMs = 250 } = {}) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await fn();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, intervalMs));
  }
}

export default {
  name: 'playback',

  async seed({ configRoot }) {
    const mediaDir = path.join(configRoot, 'media');
    fs.mkdirSync(mediaDir, { recursive: true });
    const ff = spawnSync('ffmpeg', [
      '-f', 'lavfi', '-i', 'testsrc=duration=10:size=320x180:rate=24',
      '-f', 'lavfi', '-i', 'sine=frequency=440:duration=10',
      '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest',
      path.join(mediaDir, CLIP),
    ], { stdio: 'ignore' });
    if (ff.status !== 0) throw new Error('ffmpeg is required to generate the test clip');

    const configDir = path.join(configRoot, 'config', 'vela');
    fs.mkdirSync(configDir, { recursive: true });
    fs.writeFileSync(
      path.join(configDir, 'config.json'),
      JSON.stringify({
        local_folders: [{ id: 'e2e-local', name: 'E2E Media', path: mediaDir, kind: 'movie' }],
        // --vo=null/--ao=null override Vela's render defaults (mpv takes the
        // last value), keeping playback displayless and silent under Xvfb.
        // NB: Vela parses this field one option per LINE.
        mpv_extra_args: '--vo=null\n--ao=null',
      }),
    );
  },

  async run({ driver, screenshot, configRoot }) {
    // Seeded config ⇒ authenticated view with the folder as a sidebar
    // section. Wait for that specific button: boot loads sections async,
    // and the pre-boot Welcome screen would satisfy any generic render wait.
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'E2E Media')`,
      'seeded source in the sidebar',
    );
    const section = await driver.find('xpath', `//button[contains(@class,'sideitem') and normalize-space(.)='E2E Media']`);
    const socketsBefore = mpvSocketSnapshot();
    await driver.click(section);

    await screenshot('01-grid');
    const card = await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="E2E Clip"]')`,
      'clip card in the grid',
    ).catch(async (err) => {
      const state = await driver.exec(
        `return {posters: [...document.querySelectorAll('button.poster')].map(b => b.getAttribute('aria-label')), text: document.body.innerText.slice(0, 400)}`,
      );
      err.message += ` — posters: ${JSON.stringify(state.posters)}; page: ${JSON.stringify(state.text)}`;
      throw err;
    }).then(() => driver.find('css selector', 'button.poster[aria-label^="E2E Clip"]'));
    await driver.click(card);

    // mpv side: the loaded path is our clip; seek to 6s; let Vela's progress
    // poll observe it; quit.
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
        'mpv to load the clip',
      );
      assert.equal(loaded, path.join(configRoot, 'media', CLIP));
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
    assert.equal(recent.item.title, 'E2E Clip');
    const offset = recent.item.viewOffsetMs;
    assert.ok(
      offset >= 3000 && offset < 10000,
      `expected a mid-clip resume position, got ${offset}ms`,
    );

    // …and the hero cover-flow shows it as Continue Watching.
    await driver.find('xpath', `//button[contains(@class,'sideitem') and normalize-space(.)='Home']`).then((el) => driver.click(el));
    await driver.waitFor(
      `return !!document.querySelector('[aria-label="Continue watching"] [aria-label^="Play E2E Clip"]')`,
      'clip in the Continue Watching hero',
    ).catch(async (err) => {
      const state = await driver.exec(
        `return {hero: [...document.querySelectorAll('[aria-label="Continue watching"] [aria-label]')].map(e => e.getAttribute('aria-label')), text: document.body.innerText.slice(0, 300)}`,
      ).catch(() => '?');
      err.message += ` — ${JSON.stringify(state)}`;
      throw err;
    });
    await screenshot('02-hero');
  },
};
