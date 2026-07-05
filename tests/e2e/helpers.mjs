// Shared scenario plumbing for the seeded-local-source scenarios. The
// playback scenario keeps its own detailed inline flow — it IS the mpv-IPC
// probe test; these leaner helpers serve scenarios that use playback as a
// means (curation, resume).
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from './mpv.mjs';

// Generate 10s test clips into <configRoot>/media; returns the dir.
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

// Seed the throwaway config with a local movie folder containing one
// ffmpeg-generated 10s clip per name, and displayless mpv args (one option
// per LINE — that is how Vela parses mpv_extra_args).
export function seedLocalMedia(configRoot, clipNames) {
  const mediaDir = makeClips(configRoot, clipNames);
  const configDir = path.join(configRoot, 'config', 'vela');
  fs.mkdirSync(configDir, { recursive: true });
  fs.writeFileSync(
    path.join(configDir, 'config.json'),
    JSON.stringify({
      local_folders: [{ id: 'e2e-local', name: 'E2E Media', path: mediaDir, kind: 'movie' }],
      mpv_extra_args: '--vo=null\n--ao=null',
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

export async function openLibraryGrid(driver) {
  await driver.waitFor(
    `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'E2E Media')`,
    'seeded source in the sidebar',
  );
  const section = await driver.find(
    'xpath',
    `//button[contains(@class,'sideitem') and normalize-space(.)='E2E Media']`,
  );
  await driver.click(section);
  await driver.waitFor(
    `return !!document.querySelector('button.poster[aria-label^="E2E Clip"]')`,
    'clip card in the grid',
  );
}

export async function goHome(driver) {
  const home = await driver.find(
    'xpath',
    `//button[contains(@class,'sideitem') and normalize-space(.)='Home']`,
  );
  await driver.click(home);
}

// Click `clickTarget`, then drive the resulting mpv session: seek to 6s, let
// Vela observe it, quit, and prove the quit acted. Returns the first
// time-pos sample observed after load (the resume-position evidence).
export async function playAndQuit(driver, clickTarget) {
  const before = mpvSocketSnapshot();
  await driver.click(clickTarget);
  const socketPath = await waitForNewMpvSocket(before);
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
