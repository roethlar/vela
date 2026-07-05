// Shared scenario plumbing for the seeded-local-source scenarios. The
// playback scenario keeps its own detailed inline flow — it IS the mpv-IPC
// probe test; these leaner helpers serve scenarios that use playback as a
// means (curation, resume).
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from './mpv.mjs';

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
