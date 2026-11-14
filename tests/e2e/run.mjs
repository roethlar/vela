#!/usr/bin/env node
// E2E orchestrator (see .agents/plans/e2e-harness.md). Per scenario:
// fresh throwaway config dir → tauri-driver + vendored WebKitWebDriver →
// WebDriver session against the debug binary → screenshots into
// tests/e2e/artifacts/ → pass/fail summary.
//
// Usage: npm run e2e [-- --skip-build] [--embedded] [scenario-name ...]
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { appBinaryFromMetadata, cargoMetadataCwd, METADATA_ARGS } from './app-binary.mjs';
import { Driver } from './driver.mjs';
import { setScenarioConfigRoot } from './mpv.mjs';
import { beginProof, endProof } from './proof.mjs';
import { loadScenarios, proofProblems, recordedTree, runsIn } from './scenario-loader.mjs';

const e2eDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(e2eDir, '../..');
const artifactsDir = path.join(e2eDir, 'artifacts');
const resultsDir = path.join(e2eDir, 'results');
const driverPort = Number(process.env.VELA_E2E_PORT ?? 4444);
const driverUrl = `http://127.0.0.1:${driverPort}`;

const argv = process.argv.slice(2);
const skipBuild = argv.includes('--skip-build');
const nameFilter = argv.filter((a) => !a.startsWith('--'));
// `--embedded` runs the suite in the embedded player instead of the external
// one: every seeded config asks for it (helpers.mjs reads this variable), and
// the venue below is a Wayland compositor, which that player needs.
if (argv.includes('--embedded')) process.env.VELA_E2E_PLAYER = 'embedded';
const runPlayer = process.env.VELA_E2E_PLAYER || 'external';
if (!['external', 'embedded'].includes(runPlayer)) {
  console.error(`e2e: unknown player "${runPlayer}" (VELA_E2E_PLAYER is external or embedded)`);
  process.exit(1);
}

// Node skips 'exit' handlers when dying to a default signal disposition,
// and the detached driver group never sees the terminal's SIGINT — so an
// interrupted run must reap the group itself (eh-1).
const activeKills = new Set();
const activeCleanups = new Set();
let handlingSignal = false;
for (const [sig, code] of Object.entries({ SIGHUP: 129, SIGINT: 130, SIGTERM: 143 })) {
  process.on(sig, async () => {
    // A second signal is the operator's force-exit escape hatch. The first one
    // reaps the app, then gives externally-mutating live scenarios a chance to
    // restore state before Node exits (ordinary mocks clean up here too).
    if (handlingSignal) process.exit(code);
    handlingSignal = true;
    for (const kill of activeKills) kill();
    await Promise.allSettled([...activeCleanups].map((cleanup) => cleanup()));
    process.exit(code);
  });
}

function runStep(cmd, args, env = {}) {
  const res = spawnSync(cmd, args, { stdio: 'inherit', cwd: repoRoot, env: { ...process.env, ...env } });
  if (res.status !== 0) {
    console.error(`e2e: \`${cmd} ${args.join(' ')}\` failed`);
    process.exit(1);
  }
}

function resolveTauriDriver() {
  for (const dir of [...(process.env.PATH ?? '').split(':'), path.join(os.homedir(), '.cargo/bin')]) {
    const p = path.join(dir, 'tauri-driver');
    if (dir && fs.existsSync(p)) return p;
  }
  console.error('e2e: tauri-driver not found — install with `cargo install tauri-driver`');
  process.exit(1);
}

async function driverListening() {
  try {
    await fetch(`${driverUrl}/status`);
    return true; // any HTTP response means something is listening
  } catch {
    return false;
  }
}

async function waitUntil(fn, what, timeoutMs = 10000) {
  const deadline = Date.now() + timeoutMs;
  while (!(await fn())) {
    if (Date.now() > deadline) throw new Error(`e2e: timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 200));
  }
}

// Returns { err, proof }: the error it failed with (null when it passed) and
// what it reached (proof.mjs), which the results file records.
async function runScenario(scenario, tauriDriverBin) {
  let proof = { shown: [], routes: [] };
  const configRoot = fs.mkdtempSync(path.join(os.tmpdir(), `vela-e2e-${scenario.name}-`));
  // A play's socket is the scenario's only when its app made it (mpv.mjs).
  setScenarioConfigRoot(configRoot);
  // The app reads XDG_CONFIG_HOME=<configRoot>/config, so a seeded config
  // file belongs at <configRoot>/config/vela/config.json.
  if (scenario.seed) await scenario.seed({ configRoot, repoRoot });
  const scenarioEnv = scenario.environment
    ? await scenario.environment({ configRoot, repoRoot })
    : {};
  let cleanupPromise = null;
  const runCleanup = scenario.cleanup
    ? () => {
        if (!cleanupPromise) {
          // Start from an already-resolved promise so a synchronous cleanup
          // throw becomes a rejection too; both ordinary teardown and the
          // signal handler can then wait without skipping the remaining rails.
          cleanupPromise = Promise.resolve()
            .then(() => scenario.cleanup())
            .finally(() => {
              cleanupPromise = null;
            });
        }
        return cleanupPromise;
      }
    : null;
  if (runCleanup) activeCleanups.add(runCleanup);
  const logFd = fs.openSync(path.join(artifactsDir, `${scenario.name}-driver.log`), 'w');
  // detached ⇒ own process group, so killing -pid also takes down the
  // WebKitWebDriver child tauri-driver spawns.
  const proc = spawn(
    tauriDriverBin,
    ['--native-driver', path.join(e2eDir, 'wkdriver-wrapper.sh'), '--port', String(driverPort)],
    {
      cwd: repoRoot,
      env: {
        ...process.env,
        ...displayEnv,
        ...scenarioEnv,
        XDG_CONFIG_HOME: path.join(configRoot, 'config'),
      },
      stdio: ['ignore', logFd, logFd],
      detached: true,
    },
  );
  const killTree = () => {
    try {
      process.kill(-proc.pid, 'SIGTERM');
    } catch {}
  };
  process.on('exit', killTree);
  activeKills.add(killTree);

  const driver = new Driver(driverUrl);
  try {
    await waitUntil(driverListening, `tauri-driver on :${driverPort}`);
    await driver.newSession(appBinary);
    beginProof();
    await scenario.run({
      driver,
      repoRoot,
      configRoot,
      screenshot: (name) =>
        driver.screenshotTo(path.join(artifactsDir, `${scenario.name}-${name}.png`)),
      // Relaunch the app on the same throwaway config (restart-persistence
      // scenarios). WebKitWebDriver spawns a fresh app per session. The
      // optional `between` hook runs while the app is down — the only safe
      // window to edit the seeded config without racing its lock.
      restart: async (between) => {
        // Exit-path scenarios deliberately close the app from inside the
        // webview, so the old WebDriver session may already be gone.
        await driver.deleteSession().catch(() => {});
        if (between) await between();
        await driver.newSession(appBinary);
      },
    });
    // What it reached must be what it declares (proof.mjs).
    proof = endProof();
    const problems = proofProblems(scenario, runPlayer, proof);
    if (problems.length > 0) {
      throw new Error(`e2e: ${scenario.name} does not do what it declares:\n  ${problems.join('\n  ')}`);
    }
    return { err: null, proof };
  } catch (err) {
    return { err, proof };
  } finally {
    endProof();
    if (runCleanup) await runCleanup().catch(() => {});
    if (runCleanup) activeCleanups.delete(runCleanup);
    await driver.deleteSession().catch(() => {});
    killTree();
    activeKills.delete(killTree);
    process.removeListener('exit', killTree);
    fs.closeSync(logFd);
    await waitUntil(async () => !(await driverListening()), 'driver port to free').catch(() => {});
    fs.rmSync(configRoot, { recursive: true, force: true });
  }
}

// LIVE runs load from tests/e2e/live/ instead: those drive REAL servers, so they are not
// hermetic, they are not deterministic in the way the mocks are, and they must never gate
// an ordinary change. They are opt-in (`npm run e2e:live`) and never part of the default
// suite — a flaky test in the gate is worse than no test, because it teaches you to ignore
// red.
const live = argv.includes('--live');
const scenarioDir = path.join(e2eDir, live ? 'live' : 'scenarios');
// The same loader as the parity test's (scenario-loader.mjs): a scenario it
// does not understand (an unknown key or player, a missing or unknown row, a
// name that is not its file's) stops the run before anything starts, instead
// of being skipped or run on a guess.
let allScenarios;
try {
  allScenarios = await loadScenarios(scenarioDir, { prefix: live ? 'live-' : '' });
} catch (err) {
  console.error(err.message);
  process.exit(1);
}
// A typo'd name must fail loudly, not silently shrink the run (eh-2).
const unknown = nameFilter.filter((n) => !allScenarios.some((s) => s.name === n));
if (unknown.length > 0) {
  console.error(
    `e2e: unknown scenario(s): ${unknown.join(', ')} — available: ${allScenarios.map((s) => s.name).join(', ')}`,
  );
  process.exit(1);
}
const scenarios =
  nameFilter.length === 0 ? allScenarios : allScenarios.filter((s) => nameFilter.includes(s.name));
if (scenarios.length === 0) {
  console.error('e2e: no scenarios found');
  process.exit(1);
}

// The commit this run is of, and whether the worktree was clean, read BEFORE
// the build and the first scenario, for the results file below: read at the
// end, a tree changed during the run (an injection restored before the end,
// a commit landing mid-run) would be recorded as a clean run of a commit
// whose code it did not run (review finding conf-1). Read again after every
// scenario; any difference from the start marks the run dirty. (A change made
// and undone within one scenario is not seen; the scenario modules themselves
// were imported above, before this read.)
const git = (args) => spawnSync('git', args, { cwd: repoRoot, encoding: 'utf8' }).stdout?.trim() ?? '';
const readTree = () => ({ head: git(['rev-parse', 'HEAD']), status: git(['status', '--porcelain']) });
const treeAtStart = readTree();
const treeSamples = [];

if (await driverListening()) {
  console.error(`e2e: something is already listening on :${driverPort} — stop it or set VELA_E2E_PORT`);
  process.exit(1);
}

// The app binary is where Cargo builds, asked of Cargo itself with the
// environment the build gets: CARGO_TARGET_DIR or a Cargo configuration may
// move it from src-tauri/target, and a fixed path would then start an older
// binary than the one just built (review finding pa-6). The same for a run
// with --skip-build, which starts what is there. The build's environment is
// runStep's with nothing added: this process's own. Cargo is asked from
// src-tauri/, where tauri build runs it, so that a src-tauri/.cargo/config.toml
// and a relative CARGO_TARGET_DIR are read as the build reads them.
let appBinary;
{
  const buildEnv = { ...process.env };
  const res = spawnSync('cargo', METADATA_ARGS, {
    cwd: cargoMetadataCwd(repoRoot),
    env: buildEnv,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  try {
    if (res.error || res.status !== 0) {
      throw new Error(`\`cargo ${METADATA_ARGS.join(' ')}\` failed: ${res.error?.message || res.stderr?.trim() || `exit ${res.status}`}`);
    }
    appBinary = appBinaryFromMetadata(res.stdout, buildEnv);
  } catch (err) {
    console.error(`e2e: cannot tell where Cargo builds the app, so which binary to run: ${err.message}`);
    process.exit(1);
  }
}

runStep(path.join(e2eDir, 'fetch-driver.sh'), []);
if (!skipBuild) {
  // --debug keeps compile times sane; tauri build (unlike plain cargo build)
  // embeds the built frontend, so no dev server is involved.
  runStep('npm', ['run', 'tauri', '--', 'build', '--debug', '--no-bundle']);
}
if (!fs.existsSync(appBinary)) {
  console.error(
    `e2e: app binary missing: ${appBinary}, where Cargo builds it (${skipBuild ? 'run without --skip-build' : 'the build did not leave it there'})`,
  );
  process.exit(1);
}
console.log(`e2e: running ${appBinary}`);

fs.rmSync(artifactsDir, { recursive: true, force: true });
fs.mkdirSync(artifactsDir, { recursive: true });

// Default to a private Xvfb display: on the live Wayland session a test
// window that opens unfocused/occluded stops getting frame callbacks and
// WebKit's snapshot then hangs — screenshots only render reliably when
// nothing contends for visibility. VELA_E2E_HEADED=1 runs on the real
// desktop instead (watchable, but screenshot timing depends on focus).
let displayEnv = {};
if (runPlayer === 'embedded') {
  // The embedded player loads the mpv library Vela ships. A debug build takes
  // it from wherever this variable points; by default, where
  // scripts/build-libmpv.sh leaves it. A relative path is taken from the
  // repository root, where it is checked here, and handed on whole: the
  // app and the library tests (which cargo runs from src-tauri/) would each
  // read it from somewhere else.
  const set = path.resolve(repoRoot, process.env.VELA_LIBMPV_DIR ?? '.libmpv/out/libmpv');
  if (!fs.existsSync(path.join(set, 'libmpv.so.2'))) {
    console.error(
      `e2e: no libmpv set at ${set} — build one with scripts/build-libmpv.sh --container, or point VELA_LIBMPV_DIR at one`,
    );
    process.exit(1);
  }
  displayEnv = { VELA_LIBMPV_DIR: set };
  // The tests against that library itself (`npm run test:libmpv`, ignored
  // in an ordinary `cargo test` because CI has no library): what the player
  // expects of mpv's answers, which no fake and no scenario here can show
  // (weston has no colour management, so no play here ever changes mpv's
  // target). Every full run of this player runs them first; a failure, or a
  // set they cannot load, fails the run.
  if (nameFilter.length === 0) {
    runStep('npm', ['run', 'test:libmpv'], { VELA_LIBMPV_DIR: set });
  } else {
    console.log('e2e: the libmpv tests run in a full embedded run (npm run test:libmpv runs them alone)');
  }
}
if (!process.env.VELA_E2E_HEADED && runPlayer === 'embedded') {
  // A private, headless Wayland compositor: Xvfb is X11, where the embedded
  // player cannot run. It draws with the GPU (the web view and the video
  // both need that) and shows nothing on any screen.
  const socket = `vela-e2e-${process.pid}`;
  const runtimeDir = process.env.XDG_RUNTIME_DIR;
  if (!runtimeDir) {
    console.error('e2e: XDG_RUNTIME_DIR is not set; a Wayland compositor needs it');
    process.exit(1);
  }
  const weston = spawn(
    'weston',
    // weston goes idle after 300 seconds without input, and input sent
    // through WebDriver does not count. Once idle it never draws a window
    // that opens afterwards, so every later scenario of a long run hung at
    // its first screenshot. Hence no idle time at all.
    // --fake-seat: without a seat the compositor never activates the app's
    // window, so the page never has focus (document.hasFocus() is false,
    // :focus and :focus-visible never match), a state no user's desktop is
    // ever in; GDK also logs three gdk_seat_get_keyboard criticals per start.
    // WebDriver still types without one, because WebKitWebDriver injects its
    // events inside the page, which is why only the focus styles showed it.
    [
      '--backend=headless',
      '--fake-seat',
      '--renderer=gl',
      `--socket=${socket}`,
      '--width=1280',
      '--height=800',
      '--idle-time=0',
    ],
    { stdio: 'ignore' },
  );
  weston.on('error', () => {
    console.error('e2e: weston not found — install it, or set VELA_E2E_HEADED=1 to use the desktop');
    process.exit(1);
  });
  process.on('exit', () => weston.kill());
  activeKills.add(() => weston.kill());
  await waitUntil(() => fs.existsSync(path.join(runtimeDir, socket)), `weston on ${socket}`);
  displayEnv = { ...displayEnv, GDK_BACKEND: 'wayland', WAYLAND_DISPLAY: socket, DISPLAY: '' };
} else if (!process.env.VELA_E2E_HEADED) {
  const display = process.env.VELA_E2E_DISPLAY ?? ':97';
  if (fs.existsSync(`/tmp/.X${display.slice(1)}-lock`)) {
    console.error(`e2e: display ${display} is taken — set VELA_E2E_DISPLAY or remove the stale lock`);
    process.exit(1);
  }
  const xvfb = spawn('Xvfb', [display, '-screen', '0', '1920x1080x24'], { stdio: 'ignore' });
  xvfb.on('error', () => {
    console.error('e2e: Xvfb not found — install it (xorg-server-xvfb) or set VELA_E2E_HEADED=1');
    process.exit(1);
  });
  process.on('exit', () => xvfb.kill());
  activeKills.add(() => xvfb.kill());
  await waitUntil(() => fs.existsSync(`/tmp/.X11-unix/X${display.slice(1)}`), `Xvfb on ${display}`);
  displayEnv = { ...displayEnv, GDK_BACKEND: 'x11', DISPLAY: display, WAYLAND_DISPLAY: '' };
}

const tauriDriverBin = resolveTauriDriver();
let failures = 0;
let skipped = 0;
// What each scenario did, for the results file below.
const outcomes = {};
for (const scenario of scenarios) {
  process.stdout.write(`e2e: ${scenario.name} … `);
  // A scenario may be about one player (`onlyPlayer`). The whole run is in
  // one player: the external one unless it was started with `--embedded`
  // (see README.md). In a run of the other player the scenario is skipped by
  // name, never silently; asked for by name, it fails. (The loader has
  // already refused an `onlyPlayer` that names no player.)
  if (!runsIn(scenario, runPlayer)) {
    const needs =
      scenario.onlyPlayer === 'external'
        ? 'needs a run of the external player (without --embedded)'
        : 'needs a run of the embedded player (--embedded)';
    if (nameFilter.includes(scenario.name)) {
      failures += 1;
      console.log('FAIL');
      console.error(`e2e: ${scenario.name} ${needs}`);
    } else {
      skipped += 1;
      console.log(`SKIP (${needs})`);
    }
    outcomes[scenario.name] = { status: 'skip' };
    continue;
  }
  const { err, proof } = await runScenario(scenario, tauriDriverBin);
  treeSamples.push(readTree());
  outcomes[scenario.name] = { status: err ? 'fail' : 'pass', error: err ? String(err?.message ?? err) : null, ...proof };
  if (err) {
    failures += 1;
    console.log('FAIL');
    console.error(err);
  } else {
    console.log('PASS');
  }
}

// The results of this run, for the parity verdict of two full runs, one of
// each player (`npm run e2e:verdict`; parity-verdict.mjs). A live run counts
// for neither player and writes none.
if (!live) {
  const tree = recordedTree(treeAtStart, [...treeSamples, readTree()]);
  if (tree.changedDuringRun) console.error('e2e: the worktree changed during the run; its results are recorded as dirty');
  fs.mkdirSync(resultsDir, { recursive: true });
  fs.writeFileSync(
    path.join(resultsDir, `${runPlayer}.json`),
    JSON.stringify(
      {
        player: runPlayer,
        full: nameFilter.length === 0,
        // Whether this run built the app it ran: it ran the build, then the
        // binary at the path Cargo gave for that same environment (above).
        // With --skip-build the binary is whatever was built last, of any
        // code; the verdict refuses it. What `built` does not prove: see
        // README.md, "Parity".
        built: !skipBuild,
        appBinary,
        ...tree,
        finishedAt: new Date().toISOString(),
        scenarios: outcomes,
      },
      null,
      2,
    ),
  );
}

const ran = scenarios.length - skipped;
console.log(
  `e2e: ${ran - failures}/${ran} passed${skipped ? `, ${skipped} skipped` : ''}; artifacts in ${path.relative(repoRoot, artifactsDir)}/`,
);
process.exit(failures === 0 ? 0 : 1);
