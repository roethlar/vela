// Windows venue plumbing. Importing this file performs no I/O (the scenario
// import watch loads it on Linux too). PowerShell is invoked with argument
// arrays, never with paths or scenario names interpolated into shell code.
import { execFile, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const isWindows = process.platform === 'win32';
const here = path.dirname(fileURLToPath(import.meta.url));
const scopes = new Map();

export function windowsToolAsync(action, args = []) {
  return new Promise((resolve, reject) => {
    execFile('pwsh.exe', ['-NoProfile', '-NonInteractive', '-File', path.join(here, 'windows.ps1'), action, ...args.map(String)], {
      encoding: 'utf8', timeout: 30000, windowsHide: true,
    }, (error, stdout, stderr) => {
      if (error) { reject(new Error(`Windows ${action}: ${stderr.trim() || error.message}`)); return; }
      try { resolve(JSON.parse(stdout.trim())); } catch (err) { reject(err); }
    });
  });
}

export function windowsTool(action, args = []) {
  const result = spawnSync('pwsh.exe', ['-NoProfile', '-NonInteractive', '-File', path.join(here, 'windows.ps1'), action, ...args.map(String)], {
    encoding: 'utf8', timeout: 30000, windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`Windows ${action}: ${result.error?.message ?? result.stderr?.trim() ?? `exit ${result.status}`}`);
  }
  return JSON.parse(result.stdout.trim());
}

// npm.cmd cannot be passed to spawn without a shell. npm already tells this
// Node process the CLI it ran; executing that JS with Node also preserves the
// Cargo JSON stream and handles paths containing spaces without cmd quoting.
export function npmCommand(args, env = process.env, platform = process.platform) {
  if (platform !== 'win32') return { command: 'npm', args };
  if (!env.npm_execpath || !fs.existsSync(env.npm_execpath)) {
    throw new Error('Windows E2E must be started through npm (npm_execpath is missing)');
  }
  return { command: process.execPath, args: [env.npm_execpath, ...args] };
}

export function windowsConfigPaths(roaming) {
  if (!path.win32.isAbsolute(roaming)) throw new Error('no absolute roaming AppData known folder');
  const root = path.win32.join(roaming, 'vela');
  // directories 6 ProjectDirs::from("com", "vela", "vela") on Windows:
  // organization / application / config, below FOLDERID_RoamingAppData.
  return { root, config: path.win32.join(root, 'vela', 'config') };
}

export function prepareWindowsConfig(configRoot) {
  if (process.env.VELA_E2E_DISPOSABLE_WINDOWS !== '1') {
    throw new Error('Windows E2E deletes the real roaming AppData\\vela. Use a disposable account and set VELA_E2E_DISPOSABLE_WINDOWS=1 (README, On Windows).');
  }
  const { roaming, running } = windowsTool('config-location');
  if (running.length) throw new Error(`refusing to clear Vela config while Vela is running (${running.join(', ')})`);
  const dirs = windowsConfigPaths(roaming);
  fs.rmSync(dirs.root, { recursive: true, force: true });
  fs.mkdirSync(dirs.config, { recursive: true });
  fs.mkdirSync(path.join(configRoot, 'config'), { recursive: true });
  // The scenario's existing paths point at the REAL config; no environment
  // variable pretends to redirect ProjectDirs. Junctions need no symlink privilege.
  fs.symlinkSync(dirs.config, path.join(configRoot, 'config', 'vela'), 'junction');
}

export function processAlive(pid) {
  try { process.kill(Number(pid), 0); return true; }
  catch (err) { if (err.code === 'ESRCH') return false; throw err; }
}

// A new session is scoped by descent from this scenario's tauri-driver AND
// by the exact executable the locator registered. A user's Vela cannot count.
export function ownedWindowsApps(processes, driverPid, appBinary) {
  // --skip-build joins Cargo's directory with forward slashes; CIM reports
  // backslashes. Compare Windows spellings without changing the path started.
  const canonical = (value) => path.win32.normalize(value.replace(/^\\\\\?\\(?=[a-z]:)/i, '')).toLowerCase();
  const byPid = new Map(processes.map((p) => [p.pid, p]));
  const descends = (p) => {
    const seen = new Set();
    while (p && !seen.has(p.pid)) {
      if (p.parent === driverPid) return true;
      seen.add(p.pid);
      p = byPid.get(p.parent);
    }
    return false;
  };
  return processes.filter((p) => p.exe && canonical(p.exe) === canonical(appBinary) && descends(p)).map((p) => p.pid);
}

export function registerWindowsApp(configRoot, driverPid, appBinary) {
  scopes.set(configRoot, { driverPid, appBinary, pids: [] });
  const pids = windowsAppPids(configRoot);
  if (pids.length !== 1) throw new Error(`expected one Vela under this scenario's driver, got [${pids}]`);
  scopes.get(configRoot).pids = pids;
  windowsTool('place-window', [pids[0]]);
}

export function windowsAppPids(configRoot) {
  const scope = scopes.get(configRoot);
  if (!scope) throw new Error('no Windows app scope registered');
  return ownedWindowsApps(windowsTool('processes'), scope.driverPid, scope.appBinary);
}

export function windowsPipePaths(names, pids) {
  const owners = new Set(pids.map(String));
  return names.filter((name) => owners.has(/^mpv-plex-(\d+)-\d+-\d+$/.exec(name)?.[1]))
    .map((name) => `\\\\.\\pipe\\${name}`);
}

export function windowsMpvPipes(configRoot) {
  const scope = scopes.get(configRoot);
  if (!scope) throw new Error('no Windows app scope registered');
  // No PowerShell process per poll: the session registered its actual app
  // pid before playback. A restart registers the new pid before any play.
  return windowsPipePaths(fs.readdirSync('\\\\.\\pipe\\'), scope.pids.filter(processAlive));
}

export function killWindowsTree(pid) {
  if (!pid || !processAlive(pid)) return;
  const result = spawnSync('taskkill.exe', ['/PID', String(pid), '/T', '/F'], { encoding: 'utf8', windowsHide: true });
  if (result.status !== 0 && processAlive(pid)) throw new Error(`taskkill failed for ${pid}: ${result.stderr}`);
}

export function stopWindowsApps(configRoot) {
  const scope = scopes.get(configRoot);
  // DELETE session can end EdgeDriver before its app. Keep the app pid we
  // already proved, as well as any still discoverable descendants.
  if (scope) for (const pid of new Set([...scope.pids, ...windowsAppPids(configRoot)])) killWindowsTree(pid);
}

export function forgetWindowsApp(configRoot) { scopes.delete(configRoot); }

export function processName(pid) {
  if (isWindows && processAlive(pid)) {
    for (const scope of scopes.values()) {
      if (scope.pids.includes(pid)) return path.win32.basename(scope.appBinary, '.exe').toLowerCase();
    }
  }
  return isWindows ? windowsTool('process-name', [pid]) : fs.readFileSync(`/proc/${pid}/comm`, 'utf8').trim();
}

export function processText(pid, what) {
  return isWindows ? windowsTool('process-text', [pid, what]) : fs.readFileSync(`/proc/${pid}/${what}`, 'utf8');
}

export function privateFile(file) {
  return isWindows ? windowsTool('private-file', [file]) : (fs.statSync(file).mode & 0o777) === 0o600;
}

// The standalone Lua fixtures start their own mpv, so they name their own
// unique pipes instead of looking for Vela's mpv-plex-* pipes.
export function fixtureIpcPath(dir, name) {
  return isWindows ? `\\\\.\\pipe\\vela-e2e-${process.pid}-${path.basename(dir)}-${name}` : path.join(dir, `${name}.sock`);
}

// Windows libmpv uses its GPU output in a native child of Vela. Linux uses
// the libmpv render API. Checking this AND the process identity excludes an
// external fallback and excludes vo=null on both venues.
export const embeddedVideoOutput = isWindows ? 'gpu-next' : 'libmpv';
