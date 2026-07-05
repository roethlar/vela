#!/usr/bin/env node
// E2E orchestrator (see .agents/plans/e2e-harness.md). Per scenario:
// fresh throwaway config dir → tauri-driver + vendored WebKitWebDriver →
// WebDriver session against the debug binary → screenshots into
// tests/e2e/artifacts/ → pass/fail summary.
//
// Usage: npm run e2e [-- --skip-build] [scenario-name ...]
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { Driver } from './driver.mjs';

const e2eDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(e2eDir, '../..');
const artifactsDir = path.join(e2eDir, 'artifacts');
const appBinary = path.join(repoRoot, 'src-tauri/target/debug/vela');
const driverPort = Number(process.env.VELA_E2E_PORT ?? 4444);
const driverUrl = `http://127.0.0.1:${driverPort}`;

const argv = process.argv.slice(2);
const skipBuild = argv.includes('--skip-build');
const nameFilter = argv.filter((a) => !a.startsWith('--'));

function runStep(cmd, args) {
  const res = spawnSync(cmd, args, { stdio: 'inherit', cwd: repoRoot });
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

async function runScenario(scenario, tauriDriverBin) {
  const configRoot = fs.mkdtempSync(path.join(os.tmpdir(), `vela-e2e-${scenario.name}-`));
  const logFd = fs.openSync(path.join(artifactsDir, `${scenario.name}-driver.log`), 'w');
  // detached ⇒ own process group, so killing -pid also takes down the
  // WebKitWebDriver child tauri-driver spawns.
  const proc = spawn(
    tauriDriverBin,
    ['--native-driver', path.join(e2eDir, 'wkdriver-wrapper.sh'), '--port', String(driverPort)],
    {
      cwd: repoRoot,
      env: { ...process.env, XDG_CONFIG_HOME: path.join(configRoot, 'config') },
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

  const driver = new Driver(driverUrl);
  try {
    await waitUntil(driverListening, `tauri-driver on :${driverPort}`);
    await driver.newSession(appBinary);
    await scenario.run({
      driver,
      repoRoot,
      configRoot,
      screenshot: (name) =>
        driver.screenshotTo(path.join(artifactsDir, `${scenario.name}-${name}.png`)),
    });
    return null;
  } catch (err) {
    return err;
  } finally {
    await driver.deleteSession().catch(() => {});
    killTree();
    process.removeListener('exit', killTree);
    fs.closeSync(logFd);
    await waitUntil(async () => !(await driverListening()), 'driver port to free').catch(() => {});
    fs.rmSync(configRoot, { recursive: true, force: true });
  }
}

if (await driverListening()) {
  console.error(`e2e: something is already listening on :${driverPort} — stop it or set VELA_E2E_PORT`);
  process.exit(1);
}

runStep(path.join(e2eDir, 'fetch-driver.sh'), []);
if (!skipBuild) {
  // --debug keeps compile times sane; tauri build (unlike plain cargo build)
  // embeds the built frontend, so no dev server is involved.
  runStep('npm', ['run', 'tauri', '--', 'build', '--debug', '--no-bundle']);
}
if (!fs.existsSync(appBinary)) {
  console.error(`e2e: app binary missing: ${appBinary} (run without --skip-build)`);
  process.exit(1);
}

fs.rmSync(artifactsDir, { recursive: true, force: true });
fs.mkdirSync(artifactsDir, { recursive: true });

const scenarioDir = path.join(e2eDir, 'scenarios');
const scenarios = [];
for (const file of fs.readdirSync(scenarioDir).filter((f) => f.endsWith('.mjs')).sort()) {
  const { default: scenario } = await import(pathToFileURL(path.join(scenarioDir, file)).href);
  if (nameFilter.length === 0 || nameFilter.includes(scenario.name)) scenarios.push(scenario);
}
if (scenarios.length === 0) {
  console.error(`e2e: no scenarios match [${nameFilter.join(', ')}]`);
  process.exit(1);
}

const tauriDriverBin = resolveTauriDriver();
let failures = 0;
for (const scenario of scenarios) {
  process.stdout.write(`e2e: ${scenario.name} … `);
  const err = await runScenario(scenario, tauriDriverBin);
  if (err) {
    failures += 1;
    console.log('FAIL');
    console.error(err);
  } else {
    console.log('PASS');
  }
}

console.log(`e2e: ${scenarios.length - failures}/${scenarios.length} passed; artifacts in ${path.relative(repoRoot, artifactsDir)}/`);
process.exit(failures === 0 ? 0 : 1);
