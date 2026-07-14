// Shared scenario plumbing for the mock-server scenarios. The playback
// scenario keeps its own detailed inline flow — it IS the mpv-IPC probe
// test; these leaner helpers serve scenarios that use playback as a means
// (curation, resume, queue, search).
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from './mpv.mjs';

// Generate 10s test clips into <configRoot>/media; returns the dir. The
// clips back the mock servers' Range-capable /Videos/{id}/stream routes.
export function makeClips(configRoot, clipNames) {
  const mediaDir = path.join(configRoot, 'media');
  fs.mkdirSync(mediaDir, { recursive: true });
  for (const name of clipNames) {
    const ff = spawnSync('ffmpeg', [
      '-f', 'lavfi', '-i', 'testsrc=duration=10:size=320x180:rate=24',
      '-f', 'lavfi', '-i', 'sine=frequency=440:duration=10',
      '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest',
      path.join(mediaDir, name),
    ], { stdio: 'ignore' });
    if (ff.status !== 0) throw new Error('ffmpeg is required to generate the test clips');
  }
  return mediaDir;
}

// Config `sources` entry for a started mock server.
export function mockSource(mock, { id = 'jf-mock', name = 'Mock JF' } = {}) {
  return {
    id,
    kind: 'jellyfin',
    name,
    base_url: `http://127.0.0.1:${mock.port}`,
    access_token: 'mock-token',
    user_id: mock.userId,
    device_id: 'e2e-device',
  };
}

// Seed the throwaway config: mock server sources + displayless mpv args
// (one option per LINE — that is how Vela parses mpv_extra_args).
export function seedConfig(configRoot, sources, extra = {}) {
  const configDir = path.join(configRoot, 'config', 'vela');
  fs.mkdirSync(configDir, { recursive: true });
  fs.writeFileSync(
    path.join(configDir, 'config.json'),
    JSON.stringify({
      sources,
      mpv_extra_args: '--vo=null\n--ao=null',
      ...extra,
    }),
  );
}

export async function pollUntil(fn, what, { timeoutMs = 15000, intervalMs = 250 } = {}) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await fn();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, intervalMs));
  }
}

// Sidebar → the mock library's grid.
export async function openLibraryGrid(driver, { section = 'Mock Library', cardPrefix = 'Mock Movie' } = {}) {
  await driver.waitFor(
    `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === '${section}')`,
    'seeded library in the sidebar',
  );
  const btn = await driver.find(
    'xpath',
    `//button[contains(@class,'sideitem') and normalize-space(.)='${section}']`,
  );
  await driver.click(btn);
  await driver.waitFor(
    `return !!document.querySelector('button.poster[aria-label^="${cardPrefix}"]')`,
    'movie card in the grid',
  );
}

// Every request matching `endsWith` has had its response SENT — nothing armed is still
// parked in the mock. Waiting for a request to ARRIVE proves only that the client asked;
// it says nothing about when the client was handed the answer and ran the code under
// test. An assertion that something bad did NOT happen passes just as well by asking too
// early, so it needs this (codex + grok, r21).
export const allDelivered = (mock, endsWith) => {
  const asked = mock.state.requests.filter((r) => r.path.endsWith(endsWith)).length;
  const answered = mock.state.served.filter((s) => s.path.endsWith(endsWith)).length;
  return asked > 0 && answered >= asked;
};

// Hold a condition open for a window, failing the MOMENT it breaks — rather than
// sleeping and sampling once at the end, which cannot tell "it never happened" from
// "it has not happened yet".
export async function holdsFor(check, ms, what) {
  const deadline = Date.now() + ms;
  for (;;) {
    const broke = await check();
    if (broke) throw new Error(`${what} — broke during the hold: ${broke}`);
    if (Date.now() >= deadline) return;
    await new Promise((r) => setTimeout(r, 200));
  }
}

export async function goHome(driver) {
  const home = await driver.find(
    'xpath',
    `//button[contains(@class,'sideitem') and normalize-space(.)='Home']`,
  );
  await driver.click(home);
}

// The nav flip (74ff385) routes library card clicks to the info page;
// playback goes through the detail page's Play/Resume button.
export async function openDetailAndPlay(driver, cardSelector) {
  const card = await driver.find('css selector', cardSelector);
  await driver.click(card);
  await driver.waitFor(
    `return !!document.querySelector('.detail button.playwide')`,
    'detail page Play button',
  );
  const play = await driver.find('css selector', '.detail button.playwide');
  await driver.click(play);
}

// Run `start()` (whatever triggers playback), then drive the resulting mpv
// session: seek to 6s, let Vela observe it, quit, and prove the quit acted.
// Returns the first time-pos sample observed after load (the
// resume-position evidence).
export async function playAndQuit(driver, start) {
  const before = mpvSocketSnapshot();
  await start();
  const socketPath = await waitForNewMpvSocket(before).catch(async (err) => {
    // The play error banner (play() sets `error`) is the usual culprit —
    // surface the page state instead of a bare socket timeout.
    const text = await driver.exec(`return document.body.innerText.slice(0, 400)`).catch(() => '?');
    err.message += ` — page: ${JSON.stringify(text)}`;
    throw err;
  });
  const mpv = await MpvIpc.connect(socketPath);
  let firstPos;
  try {
    firstPos = await pollUntil(
      () => mpv.getProp('time-pos').catch(() => null).then((t) => (t == null ? null : t)),
      'first time-pos sample',
    );
    await pollUntil(
      () => mpv.getProp('time-pos').then((t) => t > 0.5).catch(() => false),
      'playback to progress',
    );
    await mpv.setProp('time-pos', 6);
    await new Promise((r) => setTimeout(r, 1500)); // let Vela observe ≥6s
    mpv.quit();
  } finally {
    mpv.close();
  }
  const { createConnection } = await import('node:net');
  await pollUntil(
    () =>
      new Promise((resolve) => {
        const probe = createConnection(socketPath);
        probe.once('connect', () => {
          probe.destroy();
          resolve(false);
        });
        probe.once('error', () => resolve(true));
      }),
    'mpv socket to stop accepting after quit',
    { timeoutMs: 4000 },
  );
  return firstPos;
}
