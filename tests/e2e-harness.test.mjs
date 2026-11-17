// The end-to-end harness's own helpers that can be checked without the app.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import vm from 'node:vm';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { appBinaryFromMetadata, appCommands, appExecutableFromMessages, appIdentity, locateAppBinary, MESSAGE_FORMAT_ARG, webdriverBuildArgs, webdriverConfig, writeWebdriverConfig } from './e2e/app-binary.mjs';
import { MpvIpc, mpvSocketSnapshot, setScenarioConfigRoot, socketOfScenario } from './e2e/mpv.mjs';
import { npmCommand, ownedWindowsApps, windowsConfigPaths, windowsPipePaths } from './e2e/windows.mjs';
import { resultsFileProblems, scenarioProblems, skipOutcome, verdictProblems } from './e2e/scenario-loader.mjs';
import { settingValue } from './e2e/helpers.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const harnessText = (file) => fs.readFileSync(path.join(repoRoot, 'tests/e2e', file), 'utf8');

for (const [selector, expected, loaded] of [['#continue-playing', 'only-tv', 'off'], ['#player-window', 'remember', 'default']]) {
  test(`settingValue waits for ${selector} to load before accepting its default`, async (t) => {
    let now = 0;
    t.mock.method(Date, 'now', () => now);
    let reads = 0;
    const driver = {
      async exec(script) {
        reads += 1;
        const busy = reads === 1;
        if (!busy) now = 10001; // Reach the helper's deadline without a ten-second test.
        const document = { querySelector(requested) {
          assert.equal(requested, selector);
          return { value: busy ? expected : loaded, getAttribute: (name) => name === 'aria-busy' ? String(busy) : null };
        } };
        return new Function('document', script)(document);
      },
    };
    await assert.rejects(settingValue(driver, selector, expected, 'loaded setting'),
      (err) => err.message.includes(`shows "${loaded}", not "${expected}"`));
    assert.equal(reads, 2, 'the busy default was not accepted');
    // A loaded default is valid too, once its busy indication clears.
    now = 0;
    reads = 0;
    driver.exec = async (script) => {
      reads += 1;
      return new Function('document', script)({ querySelector: () => ({
        value: expected, getAttribute: () => String(reads === 1),
      }) });
    };
    await settingValue(driver, selector, expected, 'loaded default');
    assert.equal(reads, 2);
  });
}

test('Windows: npm executes its JS CLI and Cargo metadata predicts the exe suffix', () => {
  const cli = path.join(repoRoot, 'package.json'); // existence only; never executed
  assert.deepEqual(npmCommand(['run', 'e2e'], { npm_execpath: cli }, 'win32'), {
    command: process.execPath, args: [cli, 'run', 'e2e'],
  });
  assert.throws(() => npmCommand([], {}, 'win32'), /npm_execpath/);
  assert.deepEqual(npmCommand(['run', 'e2e'], {}, 'linux'), { command: 'npm', args: ['run', 'e2e'] });
  const manifest = path.join(repoRoot, 'src-tauri/Cargo.toml');
  const metadata = JSON.stringify({ target_directory: '/target', packages: [{ manifest_path: manifest, targets: [{ kind: ['bin'], name: 'vela' }] }] });
  assert.equal(appBinaryFromMetadata(metadata, {}, manifest, 'win32'), '/target/debug/vela.exe');
  assert.equal(appBinaryFromMetadata(metadata, {}, manifest, 'linux'), '/target/debug/vela');
  assert.match(harnessText('app-binary.mjs'), /build: \{ \.\.\.npmCommand\(buildArgs, process\.env, platform\)/);
});

test("Windows: the suite's build opens the DevTools port Edge WebDriver waits for (probe W3)", () => {
  const tauriConf = JSON.parse(fs.readFileSync(path.join(repoRoot, 'src-tauri/tauri.conf.json'), 'utf8'));
  // Every window of the real configuration, copied whole, with wry's default
  // arguments kept and a port the browser picks.
  const overlay = webdriverConfig(tauriConf);
  assert.deepEqual(overlay.app.windows.map(({ additionalBrowserArgs, ...window }) => window), tauriConf.app.windows);
  for (const window of overlay.app.windows) {
    const args = window.additionalBrowserArgs.split(' ');
    assert.ok(args.includes('--remote-debugging-port=0'));
    assert.ok(args.includes('--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection'));
  }
  assert.throws(() => webdriverConfig({ app: {} }), /no window/);
  // The file goes to tauri build before the arguments Cargo gets.
  assert.deepEqual(webdriverBuildArgs('C:\\t\\webdriver.conf.json'),
    ['run', 'tauri', '--', 'build', '--config', 'C:\\t\\webdriver.conf.json', '--debug', '--no-bundle', '--', MESSAGE_FORMAT_ARG]);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-e2e-overlay-test-'));
  try {
    const file = writeWebdriverConfig(repoRoot, tmp);
    assert.ok(file.startsWith(tmp));
    assert.deepEqual(JSON.parse(fs.readFileSync(file, 'utf8')), overlay);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
  // Only Windows builds that way; elsewhere the build is unchanged.
  assert.match(harnessText('app-binary.mjs'),
    /const buildArgs = platform === 'win32' \? webdriverBuildArgs\(writeWebdriverConfig\(repoRoot\)\) : BUILD_ARGS;/);
  assert.deepEqual(appCommands('/r', 'linux').build.args, ['run', 'tauri', '--', 'build', '--debug', '--no-bundle', '--', MESSAGE_FORMAT_ARG]);
});

test('Windows: actual known-folder config is cleared and the existing scenario paths reach it', () => {
  assert.deepEqual(windowsConfigPaths('C:\\Users\\runner\\AppData\\Roaming'), {
    root: 'C:\\Users\\runner\\AppData\\Roaming\\vela', config: 'C:\\Users\\runner\\AppData\\Roaming\\vela\\vela\\config',
  });
  assert.throws(() => windowsConfigPaths('relative'), /absolute/);
  const win = harnessText('windows.mjs');
  assert.match(win, /VELA_E2E_DISPOSABLE_WINDOWS !== '1'/);
  assert.match(win, /if \(running\.length\) throw/);
  assert.match(win, /fs\.rmSync\(dirs\.root, \{ recursive: true, force: true \}\)/);
  assert.match(win, /fs\.symlinkSync\(dirs\.config, path\.join\(configRoot, 'config', 'vela'\), 'junction'\)/);
  assert.match(harnessText('windows.ps1'), /GetFolderPath\('ApplicationData'\)/);
  const run = harnessText('run.mjs');
  assert.ok(run.indexOf('prepareWindowsConfig(configRoot)') < run.indexOf('await scenario.seed'));
  assert.match(run, /if \(isWindows\) windowsTool\('protect-fixtures'\)/);
});

test('Windows: pipe ownership requires the located app descended from this driver', () => {
  const processes = [
    { pid: 1, parent: 99, exe: 'C:\\driver.exe' },
    { pid: 2, parent: 1, exe: 'C:\\msedgedriver.exe' },
    { pid: 3, parent: 2, exe: 'C:\\APP\\vela.exe' },
    { pid: 4, parent: 2, exe: 'C:\\other\\vela.exe' },
    { pid: 5, parent: 99, exe: 'C:\\app\\vela.exe' },
    { pid: 6, parent: 7, exe: 'C:\\app\\vela.exe' },
    { pid: 7, parent: 6, exe: 'C:\\app\\vela.exe' },
  ];
  assert.deepEqual(ownedWindowsApps(processes, 1, 'C:\\app\\vela.exe'), [3]);
  assert.deepEqual(ownedWindowsApps(processes, 1, 'C:\\app/vela.exe'), [3]);
  assert.deepEqual(ownedWindowsApps(processes, 1, '\\\\?\\C:\\app\\vela.exe'), [3]);
  assert.deepEqual(windowsPipePaths(['mpv-plex-3-123-0', 'mpv-plex-5-123-0', 'mpv-3-123-0.sock', 'mpv-plex-3-123-0-junk'], [3]), ['\\\\.\\pipe\\mpv-plex-3-123-0']);
  assert.match(harnessText('mpv.mjs'), /if \(isWindows\) return windowsMpvPipes\(scenarioConfigRoot\)/);
  assert.equal(harnessText('run.mjs').split('registerWindowsApp(configRoot, proc.pid, appBinary)').length - 1, 2, 'initial and restarted sessions register the same located executable');
  assert.match(harnessText('run.mjs'), /if \(isWindows\) forgetWindowsApp\(configRoot\)/);
});

test('Windows: connection waits count clients, and cleanup kills the driver tree', () => {
  assert.match(harnessText('helpers.mjs'), /if \(isWindows\) \{\s*await windowsToolAsync\('wait-clients', \[socketPath, connections\]\)/);
  const ps = harnessText('windows.ps1');
  assert.match(ps, /\$accepted = \$pipe\.NumberOfServerInstances - 2/);
  assert.match(ps, /if \(\$accepted -ge \[int\]\$Detail\)/);
  assert.match(harnessText('windows.mjs'), /'taskkill\.exe', \['\/PID', String\(pid\), '\/T', '\/F'\]/);
  assert.match(harnessText('run.mjs'), /if \(isWindows\) \{ killWindowsTree\(proc\.pid\); return; \}/);
});

test('Windows: native desktop, both screenshot kinds, and Windows libmpv preflight are wired', () => {
  const run = harnessText('run.mjs');
  assert.match(run, /if \(isWindows\) \{\s*console\.log\('e2e: using the Windows interactive desktop/);
  assert.match(run, /isWindows \? path\.join\(artifactsRoot, runPlayer\) : artifactsRoot/);
  assert.match(run, /await windowsToolAsync\('screenshot', \[`\$\{file\}-desktop\.png`\]\)/);
  assert.match(run, /await driver\.screenshotTo\(`\$\{file\}\.png`\)/);
  assert.match(harnessText('windows.ps1'), /\$graphics\.CopyFromScreen\(/);
  assert.match(run, /isWindows \? 'libmpv-2\.dll' : 'libmpv\.so\.2'/);
  assert.match(run, /runStep\('python', \['-B', 'packaging\/libmpv\/check-libmpv-windows\.py', set\]\)/);
});

test('Windows: exact WebView2 version and valid Microsoft signature precede driver execution', () => {
  const ps = harnessText('fetch-driver.ps1');
  assert.match(ps, /F3017226-FE2A-4295-8BDF-00C3A9A7E4C5/);
  assert.match(ps, /https:\/\/msedgedriver\.microsoft\.com\/\$version\/edgedriver_win64\.zip/);
  assert.match(ps, /\$signature\.Status -ne 'Valid'/);
  assert.match(ps, /O=Microsoft Corporation/);
  assert.ok(ps.indexOf('Get-AuthenticodeSignature') < ps.indexOf('& $driver --version'));
  // The driver's own version line must name exactly the runtime's version.
  assert.match(ps, /\(MSEdgeDriver\|Microsoft Edge WebDriver\) \$\(\[regex\]::Escape\(\$version\)\)\(\\s\|\$\)/);
  const run = harnessText('run.mjs');
  assert.match(run, /nativeDriver = windowsDriver\.driver/);
  assert.match(run, /WEBVIEW2_BROWSER_EXECUTABLE_FOLDER = windowsDriver\.runtimeDir/);
  assert.match(run, /\['--native-driver', nativeDriver,/);
});

test('Windows: private backups and process token checks retain native evidence', () => {
  const ps = harnessText('windows.ps1');
  assert.match(ps, /\$acl\.AreAccessRulesProtected -and \$rules\.Count -eq 3/);
  assert.match(ps, /\$rule\.IdentityReference\.Value -in \$expected/);
  assert.match(ps, /\$rule\.FileSystemRights -eq \[System\.Security\.AccessControl\.FileSystemRights\]::FullControl/);
  assert.match(harnessText('windows-native.cs'), /ReadProcessMemory\(process,/);
  assert.match(harnessText('windows-native.cs'), /throw new Win32Exception\(Marshal\.GetLastWin32Error\(\), "Cannot read the player's process memory"\)/);
  assert.match(harnessText('scenarios/multiplex.mjs'), /processText\(pid, what\)/);
  for (const name of ['configrecovery', 'connectionrecovery', 'combinedrecovery', 'recoveryresume']) {
    assert.match(harnessText(`scenarios/${name}.mjs`), /privateFile\(/);
  }
});

test('Windows: platform-specific scenarios keep the same assertions on native equivalents', () => {
  assert.match(harnessText('scenarios/windowsetting.mjs'), /assert\.equal\(await first\.mpv\.getProp\('screen'\), 0,/);
  assert.match(harnessText('scenarios/markerbutton.mjs'), /isWindows \? '--ao=null\\n--vo=gpu-next\\n--gpu-api=d3d11' : '--ao=null\\n--vo=x11'/);
  assert.match(harnessText('windows.mjs'), /embeddedVideoOutput = isWindows \? 'gpu-next' : 'libmpv'/);
  for (const name of ['embeddedplay', 'embeddedownconfig', 'mpvargs']) {
    assert.match(harnessText(`scenarios/${name}.mjs`), /processName\(pid\)/);
    assert.match(harnessText(`scenarios/${name}.mjs`), /embeddedVideoOutput[,)]/);
  }
  assert.match(harnessText('scenarios/multiplex.mjs'), /isWindows \? \/\^http:/);
  for (const name of ['markerpayload', 'cropscript']) assert.match(harnessText(`scenarios/${name}.mjs`), /const socket = fixtureIpcPath\(dir, name\)/);
});

test('Windows: skip reasons are validated, judged, and never count as parity coverage', () => {
  const scenario = { name: 'fallback', rows: [], noRows: 'venue-only', skipOnWindows: 'X11 requires Wayland', run() {} };
  assert.deepEqual(scenarioProblems(scenario, 'fallback'), []);
  assert.match(scenarioProblems({ ...scenario, skipOnWindows: '' }, 'fallback').join(' '), /non-empty reason/);
  assert.equal(skipOutcome(scenario, 'external', 'linux'), null);
  assert.deepEqual(skipOutcome(scenario, 'external', 'win32'), { status: 'skip', reason: scenario.skipOnWindows });
  const run = (player) => ({ player, platform: 'win32', head: 'commit', full: true, built: true, appBinary: 'C:\\target\\debug\\vela.exe', appBinaryFrom: "cargo's report of this run's build", dirty: false, changedDuringRun: false, finishedAt: '2026-11-16T00:00:00.000Z', scenarios: { fallback: skipOutcome(scenario, player, 'win32') } });
  const results = { external: run('external'), embedded: run('embedded') };
  const options = { inventory: [], exemptions: [], knownFailures: [], checkout: { head: 'commit', dirty: false } };
  assert.deepEqual(resultsFileProblems(results.external, 'external'), []);
  assert.deepEqual(verdictProblems([scenario], results, options), []);
  const bad = structuredClone(results);
  bad.external.scenarios.fallback.reason = 'invented';
  assert.match(verdictProblems([scenario], bad, options).join(' '), /not the skip/);
  bad.external.platform = 'linux';
  assert.match(resultsFileProblems(bad.external, 'external').join(' '), /unsupported platform/);
  delete bad.external.platform;
  assert.match(verdictProblems([scenario], bad, options).join(' '), /different platforms/);
  const claims = { ...scenario, rows: ['1'] };
  assert.match(verdictProblems([claims], results, { ...options, inventory: [{ id: '1' }] }).join(' '), /row 1 is shown by no scenario/);
});

test('Windows: workflow runs both suites independently and judges only two full runs', () => {
  const workflow = fs.readFileSync(path.join(repoRoot, '.github/workflows/e2e-windows.yml'), 'utf8');
  for (const required of ['workflow_dispatch:', 'runs-on: windows-latest', 'actions: read', '--workflow ci.yml --branch main --status success', 'gh run download', '--name libmpv-windows-x86_64', 'bash scripts/build-libmpv-windows.sh --container', 'choco install mpv ffmpeg openssl', 'npm run e2e:verdict', 'tests/e2e/artifacts/', 'tests/e2e/results/']) assert.ok(workflow.includes(required), required);
  assert.match(workflow, /!cancelled\(\) && steps\.ready\.outcome == 'success' && inputs\.suite != 'external'/);
  assert.match(workflow, /inputs\.suite == 'both' && inputs\.scenario == ''/);
  assert.doesNotMatch(workflow, /continue-on-error|contents: write|actions: write/);
});

// Which app binary the runner starts (review finding pa-6). A run that builds
// takes the executable from Cargo's own report of that build; a run with
// --skip-build predicts it from `cargo metadata` and is recorded as not built.
// Every test of that finding has "(pa-6)" in its name.

// A made-up project laid out as this one: <root>/src-tauri/Cargo.toml, a
// binary `madeup` that prints NEW, and with `workspace` a second member,
// `helper`, with a binary of its own.
function resourceFixture(crate, mapped = 'mpv-scripts/') {
  const source = path.join(crate, 'resources/mpv-scripts');
  fs.mkdirSync(source, { recursive: true });
  fs.writeFileSync(path.join(crate, 'tauri.conf.json'), JSON.stringify({ bundle: { resources: { 'resources/mpv-scripts/': mapped } } }));
  return source;
}

function madeupProject(root, { workspace = false, cargoConfig } = {}) {
  const crate = path.join(root, 'src-tauri');
  fs.mkdirSync(path.join(crate, 'src'), { recursive: true });
  resourceFixture(crate);
  fs.writeFileSync(
    path.join(crate, 'Cargo.toml'),
    `[package]\nname = "madeup"\nversion = "0.0.0"\nedition = "2021"\n\n[workspace]\n${workspace ? 'members = ["helper"]\n' : ''}`,
  );
  fs.writeFileSync(path.join(crate, 'src/main.rs'), 'fn main() { println!("NEW"); }\n');
  if (workspace) {
    fs.mkdirSync(path.join(crate, 'helper/src'), { recursive: true });
    fs.writeFileSync(path.join(crate, 'helper/Cargo.toml'), '[package]\nname = "helper"\nversion = "0.0.0"\nedition = "2021"\n');
    fs.writeFileSync(path.join(crate, 'helper/src/main.rs'), 'fn main() { println!("HELPER"); }\n');
  }
  if (cargoConfig) {
    fs.mkdirSync(path.join(crate, '.cargo'));
    fs.writeFileSync(path.join(crate, '.cargo/config.toml'), cargoConfig);
  }
  return crate;
}

// An older binary, where a guess would look: it prints OLD.
function plantOld(file) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, '#!/bin/sh\necho OLD\n', { mode: 0o755 });
}

const printed = (file) => spawnSync(file, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).stdout?.trim();
const { CARGO_TARGET_DIR: _t, CARGO_BUILD_TARGET_DIR: _b, CARGO_BUILD_TARGET: _bt, ...plainEnv } = process.env;
const cargoMissing = spawnSync('cargo', ['--version']).error?.code === 'ENOENT';

// Files that stand for two manifests, for the tests that need no Cargo.
function twoManifests() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-manifests-'));
  for (const p of ['app', 'other']) {
    fs.mkdirSync(path.join(dir, p));
    fs.writeFileSync(path.join(dir, p, 'Cargo.toml'), '');
  }
  resourceFixture(path.join(dir, 'app'));
  return { dir, app: path.join(dir, 'app/Cargo.toml'), other: path.join(dir, 'other/Cargo.toml') };
}

test("the app's executable is the one Cargo reported for the app's package, as Cargo printed it (pa-6)", () => {
  const { dir, app, other } = twoManifests();
  try {
    const artifact = (manifest, executable, kind = ['bin']) => ({
      reason: 'compiler-artifact',
      manifest_path: manifest,
      target: { kind, name: 'x' },
      executable,
    });
    const finished = { reason: 'build-finished', success: true };
    // Used as printed: a `..` after a symbolic link is not collapsed.
    assert.equal(
      appExecutableFromMessages([artifact(app, '/p/link/../target/debug/vela'), finished], app),
      '/p/link/../target/debug/vela',
    );
    // The app's manifest however it is spelt; another package's binary, a
    // library and a test binary are not the app.
    assert.equal(
      appExecutableFromMessages(
        [
          artifact(other, '/t/debug/helper'),
          artifact(app, null, ['rlib']),
          artifact(app, '/t/debug/deps/vela-abc', ['test']),
          artifact(`${dir}/app/../app/Cargo.toml`, '/t/debug/vela'),
          artifact(app, '/t/debug/vela'),
          finished,
        ],
        app,
      ),
      '/t/debug/vela',
    );
    assert.throws(() => appExecutableFromMessages([artifact(app, '/t/debug/vela')], app), /the build printed no report from Cargo \(no build-finished message/);
    assert.throws(
      () => appExecutableFromMessages([artifact(other, '/t/debug/helper'), finished], app),
      /Cargo reported no executable of the app's package/,
    );
    assert.throws(
      () => appExecutableFromMessages([artifact(app, '/t/debug/a'), artifact(app, '/t/debug/b'), finished], app),
      /more than one executable claims to be the app: \/t\/debug\/a, \/t\/debug\/b/,
    );
    assert.throws(() => appExecutableFromMessages([artifact(app, 'debug/vela'), finished], app), /at a relative path/);
    // The manifest is matched as a file, not as a string: spelt another way
    // it is still the app, and the only one here.
    assert.equal(
      appExecutableFromMessages([artifact(`${dir}/app/../app/Cargo.toml`, '/t/debug/by-another-spelling'), finished], app),
      '/t/debug/by-another-spelling',
    );
    // A manifest that is no file at all is not the app's.
    assert.equal(
      appExecutableFromMessages([artifact(path.join(dir, 'gone/Cargo.toml'), '/t/debug/gone'), artifact(app, '/t/debug/vela'), finished], app),
      '/t/debug/vela',
    );
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("a --skip-build run's prediction is the app package's one binary, joined without normalizing (pa-6)", () => {
  const { dir, app, other } = twoManifests();
  try {
    const metadata = (target, packages) => JSON.stringify({ target_directory: target, packages });
    const appPackage = { manifest_path: app, targets: [{ kind: ['staticlib', 'cdylib', 'rlib'], name: 'vela_lib' }, { kind: ['bin'], name: 'vela' }] };
    const helper = { manifest_path: other, targets: [{ kind: ['bin'], name: 'helper' }] };
    assert.equal(appBinaryFromMetadata(metadata('/elsewhere/target', [appPackage]), {}, app), '/elsewhere/target/debug/vela');
    assert.equal(appBinaryFromMetadata(metadata('/p/link/../target', [helper, appPackage]), {}, app), '/p/link/../target/debug/vela');
    assert.throws(() => appBinaryFromMetadata('warning: something\n', {}, app), /Cargo's metadata is not JSON/);
    assert.throws(() => appBinaryFromMetadata(metadata(undefined, [appPackage]), {}, app), /names no absolute target directory \(target_directory is undefined\)/);
    assert.throws(() => appBinaryFromMetadata(metadata('relative/target', [appPackage]), {}, app), /names no absolute target directory/);
    assert.throws(() => appBinaryFromMetadata(metadata('/t', [helper]), {}, app), /names 0 packages whose manifest is .*app\/Cargo\.toml, not one/);
    assert.throws(
      () => appBinaryFromMetadata(metadata('/t', [{ manifest_path: app, targets: [{ kind: ['bin'], name: 'a' }, { kind: ['bin'], name: 'b' }] }]), {}, app),
      /names 2 binary targets of the app's package, not one/,
    );
    assert.throws(
      () => appBinaryFromMetadata(metadata('/t', [appPackage]), { CARGO_BUILD_TARGET: 'x86_64-unknown-linux-gnu' }, app),
      /CARGO_BUILD_TARGET is "x86_64-unknown-linux-gnu": the binary would be under \/t\/x86_64-unknown-linux-gnu\/debug\//,
    );
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("the runner's build hands Cargo the message format, and Cargo is asked from src-tauri (pa-6)", () => {
  assert.deepEqual(appCommands('/r'), {
    appManifest: '/r/src-tauri/Cargo.toml',
    build: { command: 'npm', args: ['run', 'tauri', '--', 'build', '--debug', '--no-bundle', '--', '--message-format=json-render-diagnostics'], cwd: '/r' },
    metadata: { command: 'cargo', args: ['metadata', '--format-version', '1', '--no-deps', '--manifest-path', 'Cargo.toml'], cwd: '/r/src-tauri' },
  });
  assert.equal(MESSAGE_FORMAT_ARG, '--message-format=json-render-diagnostics');
});

// Each configuration the review used to make a guess start an older binary.
// A real `cargo build` of the made-up project stands in for `tauri build`
// (which runs `cargo build --bins` in src-tauri/ with the arguments after its
// own `--`). The path returned must be the file that build wrote: run, it
// prints NEW, while the older binary planted where a guess would look prints
// OLD. Then the same for a --skip-build run's prediction, where it follows.
const host = cargoMissing ? null : /host: (\S+)/.exec(spawnSync('rustc', ['-vV'], { encoding: 'utf8' }).stdout ?? '')?.[1];
const configurations = [
  {
    name: "a configuration file's build.target",
    setup: () => ({ cargoConfig: `[build]\ntarget = "${host}"\n` }),
    builtUnder: (root) => `${root}/src-tauri/target/${host}/debug/`,
    old: (root) => `${root}/src-tauri/target/debug/madeup`,
    // Not followed by the prediction (README.md says so): a --skip-build run
    // in this setup starts the older binary, and is recorded as not built.
    predictionFollows: false,
  },
  {
    name: "CARGO_TARGET_DIR through a symbolic link and '..'",
    setup: (root) => {
      fs.mkdirSync(path.join(root, 'physical/nested'), { recursive: true });
      fs.symlinkSync(path.join(root, 'physical/nested'), path.join(root, 'link'));
      return { env: { CARGO_TARGET_DIR: `${root}/link/../target` } };
    },
    builtUnder: (root) => `${root}/link/../target/debug/`,
    old: (root) => `${root}/target/debug/madeup`,
  },
  {
    name: 'CARGO_BUILD_TARGET_DIR, relative',
    setup: () => ({ env: { CARGO_BUILD_TARGET_DIR: 'rel-target' } }),
    builtUnder: (root) => `${root}/src-tauri/rel-target/debug/`,
    old: (root) => `${root}/src-tauri/target/debug/madeup`,
  },
  {
    name: 'CARGO_BUILD_TARGET_DIR, absolute',
    setup: (root) => ({ env: { CARGO_BUILD_TARGET_DIR: `${root}/abs-target` } }),
    builtUnder: (root) => `${root}/abs-target/debug/`,
    old: (root) => `${root}/src-tauri/target/debug/madeup`,
  },
  {
    name: "src-tauri's .cargo/config.toml target-dir",
    setup: (root) => ({ cargoConfig: `[build]\ntarget-dir = ${JSON.stringify(`${root}/configured-target`)}\n` }),
    builtUnder: (root) => `${root}/configured-target/debug/`,
    old: (root) => `${root}/src-tauri/target/debug/madeup`,
  },
  {
    name: 'a two-package workspace with another binary',
    setup: () => ({ workspace: true }),
    builtUnder: (root) => `${root}/src-tauri/target/debug/`,
  },
];
for (const c of configurations) {
  test(`a build's app executable is the file that build wrote: ${c.name} (pa-6)`, async (t) => {
    if (cargoMissing) {
      t.skip('cargo is not installed here');
      return;
    }
    // realpath: os.tmpdir() may itself be reached through a symbolic link,
    // and Cargo prints the directory it runs in as the system gives it.
    const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-pa6-')));
    try {
      const { env: extra = {}, workspace = false, cargoConfig } = c.setup(root);
      const crate = madeupProject(root, { workspace, cargoConfig });
      if (c.old) plantOld(c.old(root));
      const env = { ...plainEnv, ...extra };
      const commands = {
        ...appCommands(root),
        build: { command: 'cargo', args: ['build', MESSAGE_FORMAT_ARG, '--bins', ...(workspace ? ['--workspace'] : [])], cwd: crate },
      };
      const echoed = [];
      const built = await locateAppBinary({ skipBuild: false, env, commands, echo: (line) => echoed.push(line) });
      assert.equal(built.built, true, 'a run that builds is recorded as built');
      assert.equal(printed(built.appBinary), 'NEW', `${built.appBinary} is not the binary the build wrote`);
      assert.ok(built.appBinary.startsWith(c.builtUnder(root)), `${built.appBinary} is not under ${c.builtUnder(root)}: the build was not given the environment`);
      assert.deepEqual(echoed, [], "Cargo's JSON messages are not echoed");
      assert.equal(built.identity, appIdentity(built.appBinary), 'the identity the driver checks is that of the file the build wrote');
      assert.notEqual(built.identity, null);
      const predicted = await locateAppBinary({ skipBuild: true, env, commands });
      assert.equal(predicted.built, false, 'a --skip-build run is recorded as not built');
      if (c.predictionFollows === false) {
        assert.equal(predicted.appBinary, c.old(root), 'the prediction is documented as not following build.target; update README.md if it now does');
      } else {
        assert.equal(printed(predicted.appBinary), 'NEW', `${predicted.appBinary} is not the binary the build wrote`);
      }
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
}

test("a build that fails, or prints no report from Cargo, gives no app binary (pa-6)", async (t) => {
  if (cargoMissing) {
    t.skip('cargo is not installed here');
    return;
  }
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-pa6-')));
  try {
    const crate = madeupProject(root);
    const commands = appCommands(root);
    // Without the message format Cargo reports nothing on standard output.
    await assert.rejects(
      locateAppBinary({ skipBuild: false, env: plainEnv, commands: { ...commands, build: { command: 'cargo', args: ['build', '--bins'], cwd: crate } }, echo: () => {} }),
      /the build printed no report from Cargo/,
    );
    fs.writeFileSync(path.join(crate, 'src/main.rs'), 'fn main() { not rust }\n');
    await assert.rejects(
      locateAppBinary({ skipBuild: false, env: plainEnv, commands: { ...commands, build: { command: 'cargo', args: ['build', MESSAGE_FORMAT_ARG], cwd: crate } }, echo: () => {} }),
      /`cargo build --message-format=json-render-diagnostics` failed \(exit 101\)/,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

// A stand-in for the build, as a Node script: what `tauri build` prints on
// standard output (a line of the front end's build, a JSON line that is not
// Cargo's, then Cargo's report of the app), a diagnostic on standard error,
// and then the exit status asked for. It writes with write(2) on file
// descriptors 1 and 2, as Cargo does: in a sandbox that refuses sockets,
// what Node's console and process.stdout wrote to the piped standard output
// never arrived.
// The four documented Cargo envelopes, also observed in Cargo 1.99.1's real
// output (an offline crate with a build script and an unused-variable warning).
function cargoReport(manifest, executable, status) {
  const common = {
    package_id: `path+file://${manifest}#vela@0.0.0`,
    manifest_path: manifest,
    target: { kind: ['bin'], crate_types: ['bin'], name: 'vela', src_path: `${manifest}/../src/main.rs`, edition: '2021', doc: true, doctest: false, test: true },
  };
  return [
    { reason: 'compiler-message', ...common, message: { message: 'a warning', code: null, level: 'warning', spans: [], children: [], rendered: 'warning: a warning\n' } },
    { reason: 'build-script-executed', package_id: common.package_id, linked_libs: [], linked_paths: [], cfgs: [], env: [['PA6_PROBE', '1']], out_dir: '/t/out' },
    { reason: 'compiler-artifact', ...common, profile: { opt_level: '0', debuginfo: 2, debug_assertions: true, overflow_checks: true, test: false }, features: [], filenames: [executable], executable, fresh: false },
    { reason: 'build-finished', success: status === '0' },
  ];
}
const otherBuildOutput = [
  'vite: building for production',
  '{"vite":"a JSON line without a reason"}',
  '{"reason":"dependency-warning","message":"a non-Cargo diagnostic"}',
  // A familiar reason alone is not a Cargo envelope either.
  ...['compiler-message', 'compiler-artifact', 'build-script-executed', 'build-finished'].map((reason) => JSON.stringify({ reason, message: 'not a Cargo envelope' })),
];
const fakeBuild = `
const { writeSync } = require('node:fs');
const [manifest, executable, status] = process.argv.slice(1);
const out = (line) => writeSync(1, line + '\\n');
const cargoReport = ${cargoReport.toString()};
const other = ${JSON.stringify(otherBuildOutput)};
for (const line of other.slice(0, 3)) out(line);
for (const message of cargoReport(manifest, executable, status)) out(JSON.stringify(message));
for (const line of other.slice(3)) out(line);
writeSync(2, 'DIAG: rendered by the build\\n');
process.exit(Number(status));
`;
function fakeBuildCommands(dir, status, executable = path.join(dir, 'target/debug/vela')) {
  const appManifest = path.join(dir, 'app/Cargo.toml');
  fs.mkdirSync(path.dirname(executable), { recursive: true });
  return {
    appManifest,
    build: { command: process.execPath, args: ['-e', fakeBuild, appManifest, executable, String(status)], cwd: dir },
  };
}

for (const mapped of ['mpv-scripts/', 'custom/scripts/']) {
  test(`a reported executable gets missing and different mpv resources at ${mapped}`, async () => {
    const { dir, app } = twoManifests();
    try {
      const source = resourceFixture(path.dirname(app), mapped);
      const files = new Map([
        ['markers.lua', Buffer.from('-- markers\n\0')],
        ['autocrop.lua', Buffer.from('-- new crop\n')],
        ['LICENSE.GPL', Buffer.from('license')],
        ['nested/extra.lua', Buffer.from('-- nested')],
      ]);
      for (const [name, contents] of files) {
        fs.mkdirSync(path.dirname(path.join(source, name)), { recursive: true });
        fs.writeFileSync(path.join(source, name), contents);
      }
      // Cargo can report a target reached through a symlink and `..` (pa-6).
      // Normalizing that string would put the scripts in a different tree.
      const physical = path.join(dir, 'physical');
      fs.mkdirSync(path.join(physical, 'nested'), { recursive: true });
      fs.symlinkSync(path.join(physical, 'nested'), path.join(dir, 'link'), 'dir');
      const executable = `${dir}/link/../separate-target/triple/debug/vela`;
      const destination = path.join(physical, 'separate-target/triple/debug', mapped);
      fs.mkdirSync(destination, { recursive: true });
      fs.writeFileSync(executable, 'made-up executable');
      fs.writeFileSync(path.join(destination, 'autocrop.lua'), '-- old crop\n');
      const unchanged = path.join(destination, 'LICENSE.GPL');
      fs.writeFileSync(unchanged, files.get('LICENSE.GPL'));
      fs.utimesSync(unchanged, 123, 123);
      const unchangedTime = fs.statSync(unchanged).mtimeMs;
      const echoed = [];
      const commands = fakeBuildCommands(dir, 0, executable);
      const located = await locateAppBinary({ skipBuild: false, env: process.env, commands, echo: (line) => echoed.push(line) });
      assert.equal(located.appBinary, executable);
      for (const [name, contents] of files) assert.deepEqual(fs.readFileSync(path.join(destination, name)), contents, name);
      assert.equal(fs.statSync(unchanged).mtimeMs, unchangedTime, 'identical files are left alone');
      assert.deepEqual(echoed.filter((line) => line.startsWith('e2e:')), [
        `e2e: copied 3 missing or different mpv resource file(s) to ${destination}`,
      ], 'the run discloses the repair');
      echoed.length = 0;
      await locateAppBinary({ skipBuild: false, env: process.env, commands, echo: (line) => echoed.push(line) });
      assert.deepEqual(echoed.filter((line) => line.startsWith('e2e:')), [], 'no repair is reported when all resources match');
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
}

test('a build that fails after Cargo reported the app gives no app binary (pa-6)', async () => {
  const { dir } = twoManifests();
  try {
    await assert.rejects(
      locateAppBinary({ skipBuild: false, env: process.env, commands: fakeBuildCommands(dir, 101), echo: () => {} }),
      /failed \(exit 101\)/,
      "a build's failure stops the run even when Cargo had already named the app's executable",
    );
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("a build's other output reaches the terminal: its standard output's other lines are echoed, its standard error passes through (pa-6)", async () => {
  const { dir } = twoManifests();
  try {
    const echoed = [];
    const located = await locateAppBinary({ skipBuild: false, env: process.env, commands: fakeBuildCommands(dir, 0), echo: (line) => echoed.push(line) });
    assert.equal(located.appBinary, path.join(dir, 'target/debug/vela'));
    assert.deepEqual(
      echoed,
      otherBuildOutput,
      "every line of standard output that is not one of Cargo's messages is echoed",
    );
    // Standard error is the terminal's: seen from a child process, whose
    // standard error is a pipe this test reads.
    const child = spawnSync(
      process.execPath,
      [
        '--input-type=module',
        '-e',
        `const { locateAppBinary } = await import(${JSON.stringify(new URL('./e2e/app-binary.mjs', import.meta.url).href)});
         await locateAppBinary({ skipBuild: false, env: process.env, commands: JSON.parse(process.argv[1]), echo: () => {} });`,
        JSON.stringify(fakeBuildCommands(dir, 0)),
      ],
      { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
    );
    assert.equal(child.status, 0, child.stderr);
    assert.match(child.stderr, /DIAG: rendered by the build/, "the build's standard error reaches the terminal");
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('non-Cargo JSON with a reason, including malformed Cargo envelopes, is echoed (pa-6)', async () => {
  const { dir } = twoManifests();
  try {
    const echoed = [];
    await locateAppBinary({ skipBuild: false, env: process.env, commands: fakeBuildCommands(dir, 0), echo: (line) => echoed.push(line) });
    for (const line of otherBuildOutput.slice(2)) {
      assert.ok(echoed.includes(line), `non-Cargo JSON with a reason disappeared: ${line}`);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

for (const reason of ['compiler-message', 'compiler-artifact', 'build-script-executed', 'build-finished']) {
  test(`Cargo's ${reason} output is consumed, not echoed (pa-6)`, async () => {
    const { dir, app } = twoManifests();
    try {
      const echoed = [];
      await locateAppBinary({ skipBuild: false, env: process.env, commands: fakeBuildCommands(dir, 0), echo: (line) => echoed.push(line) });
      const message = cargoReport(app, '/t/debug/vela', '0').find((m) => m.reason === reason);
      assert.ok(!echoed.includes(JSON.stringify(message)), `${reason} was echoed as non-Cargo output`);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
}

test('the app artifact is found among mixed Cargo and non-Cargo output (pa-6)', async () => {
  const { dir } = twoManifests();
  try {
    const located = await locateAppBinary({ skipBuild: false, env: process.env, commands: fakeBuildCommands(dir, 0), echo: () => {} });
    assert.equal(located.appBinary, path.join(dir, 'target/debug/vela'), 'mixed output lost the app artifact');
    assert.equal(located.built, true);
    assert.equal(located.appBinaryFrom, "cargo's report of this run's build");
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// The real project: Cargo asked as a --skip-build run asks it, with
// CARGO_TARGET_DIR set and without.
test('a --skip-build run asks Cargo about the real project, and CARGO_TARGET_DIR moves the answer (pa-6)', async (t) => {
  if (cargoMissing) {
    t.skip('cargo is not installed here');
    return;
  }
  const commands = appCommands(repoRoot);
  const plain = await locateAppBinary({ skipBuild: true, env: plainEnv, commands });
  // src-tauri/target/debug/vela, unless a Cargo configuration of this
  // machine moves it.
  assert.match(plain.appBinary, /^\/.*\/debug\/vela$/);
  const moved = await locateAppBinary({ skipBuild: true, env: { ...plainEnv, CARGO_TARGET_DIR: '/nowhere/vela-target' }, commands });
  assert.equal(moved.appBinary, '/nowhere/vela-target/debug/vela');
});

// Runtime owns session selection, regardless of the caller's names. Text
// checks retain only the runner's build/environment, registration and results
// wiring, plus a tripwire for literal WebDriver requests outside the driver.
// The latter cannot prevent code making its own HTTP request (README, Parity).
test("the runner registers and records locateAppBinary's result, and builds nothing itself (pa-6)", () => {
  const run = fs.readFileSync(path.join(repoRoot, 'tests/e2e/run.mjs'), 'utf8');
  const count = (text) => run.split(text).length - 1;
  const forms = [
    ["import { appCommands, locateAppBinary } from './app-binary.mjs';", 1],
    ["import { Driver, setRunApp } from './driver.mjs';", 1],
    ['let located;', 1],
    ['located = await locateAppBinary({ skipBuild, env: process.env, commands: appCommands(repoRoot) });', 1],
    ['setRunApp(located);', 1],
    ['if (isWindows) registerWindowsApp(configRoot, proc.pid, appBinary);', 2],
    ['const { appBinary, built, appBinaryFrom } = located;', 1],
    ['if (!fs.existsSync(appBinary)) {', 1],
    [
      "`e2e: app binary missing: ${appBinary}, from ${appBinaryFrom} (${skipBuild ? 'run without --skip-build' : 'the build did not leave it there'})`,",
      1,
    ],
    ['console.log(`e2e: running ${appBinary} (from ${appBinaryFrom})`);', 1],
    ['        built,\n        appBinary,\n        appBinaryFrom,\n        ...tree,\n', 1],
  ];
  let rest = run;
  for (const [form, times] of forms) {
    assert.equal(count(form), times, `the runner has \`${form}\` exactly ${times} time(s)`);
    rest = rest.split(form).join('');
  }
  for (const word of ['appBinary', 'located', 'locateAppBinary', 'appCommands', 'setRunApp', 'app-binary\\.mjs']) {
    assert.doesNotMatch(rest, new RegExp(`\\b${word}\\b`), `${word.replace(/\\/g, '')} appears in the runner only in the accepted forms`);
  }
  assert.doesNotMatch(rest, /\bbuilt\s*[:=]/, '`built` is recorded only as located says');
  assert.doesNotMatch(run, /['"`]cargo['"`]|['"`]build['"`]/, 'the runner runs no Cargo and no build of its own');
  assert.doesNotMatch(run, /target\/debug|['"`]target['"`]|['"`]debug['"`]/, 'the runner puts no target directory together itself');
  assert.ok(run.indexOf('setRunApp(located);') < run.indexOf('const { err, proof } = await runScenario(scenario, tauriDriverBin);'), 'the app is registered before scenarios run');
  // Literal protocol requests outside Driver are suspicious; this is not a
  // security boundary. New Driver objects and newSession calls are safe now.
  const e2eDir = path.join(repoRoot, 'tests/e2e');
  const modules = fs
    .readdirSync(e2eDir, { recursive: true })
    .filter((f) => /\.m?js$/.test(f) && f !== 'driver.mjs' && !f.startsWith('node_modules'));
  assert.ok(modules.length > 10, 'the harness modules are found');
  for (const file of modules) {
    const text = fs.readFileSync(path.join(e2eDir, file), 'utf8');
    assert.doesNotMatch(text, /\/session\b|tauri:options/, `${file} makes no WebDriver session of its own`);
  }
});

// Each case is a fresh process: the run app has no reset API, including for
// tests. All imports in that process share the real driver module, and fetch
// is stubbed before any session, so no server, display or socket is involved.
function driverCase(source) {
  const { dir } = twoManifests();
  try {
    const app = path.join(dir, 'vela');
    const old = path.join(dir, 'old-vela');
    plantOld(old);
    const driverModule = new URL('./e2e/driver.mjs', import.meta.url).href;
    const helper = path.join(dir, 'other-client.mjs');
    fs.writeFileSync(helper, `
      import { Driver as Client } from ${JSON.stringify(driverModule)};
      export const otherClient = (url, old) => new Client(url, old);
      export class Override extends Client {
        async newSession() { throw new Error('subclass override ran'); }
      }
    `);
    const child = spawnSync(process.execPath, ['--input-type=module', '-e', `
      import assert from 'node:assert/strict';
      import fs from 'node:fs';
      import { appIdentity, locateAppBinary } from ${JSON.stringify(new URL('./e2e/app-binary.mjs', import.meta.url).href)};
      import { Driver, setRunApp } from ${JSON.stringify(driverModule)};
      import { otherClient, Override } from ${JSON.stringify(helper)};
      const driverModule = ${JSON.stringify(driverModule)};
      const app = ${JSON.stringify(app)};
      const old = ${JSON.stringify(old)};
      const commands = ${JSON.stringify(fakeBuildCommands(dir, 0, app))};
      const locate = () => locateAppBinary({ skipBuild: false, env: process.env, commands, echo: () => {} });
      const when = new Date(1_700_000_000_000);
      const original = 'the reported build';
      const write = (file, text) => { fs.writeFileSync(file, text); fs.utimesSync(file, when, when); };
      write(app, original);
      const url = 'http://driver.invalid';
      const sent = [];
      globalThis.fetch = async (url, init) => {
        sent.push(JSON.parse(init.body));
        return new Response(JSON.stringify({ value: { sessionId: 's' + sent.length } }), { status: 200 });
      };
      const applications = () => sent.map((body) => body.capabilities.alwaysMatch['tauri:options'].application);
      ${source}
    `], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
    assert.equal(child.status, 0, child.stderr || child.error?.message);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

test("Windows: every session of a scenario's driver shares one WebView2 data folder", () => {
  driverCase(`
    const located = await locate();
    setRunApp(located);
    const shared = new Driver(url, { webviewUserDataFolder: 'C:\\\\cfg\\\\webview2' });
    await shared.newSession();
    await shared.newSession();
    await new Driver(url).newSession();
    const options = sent.map((body) => body.capabilities.alwaysMatch['tauri:options']);
    assert.deepEqual(options.map((o) => o.webviewOptions), [
      { userDataFolder: 'C:\\\\cfg\\\\webview2' }, { userDataFolder: 'C:\\\\cfg\\\\webview2' }, undefined,
    ], 'a restart must reuse the folder; a driver without one asks for none');
    assert.deepEqual(applications(), [app, app, app], 'the folder never chooses the app');
  `);
  // The runner gives one per scenario, on Windows only.
  assert.match(harnessText('run.mjs'),
    /new Driver\(driverUrl, isWindows \? \{ webviewUserDataFolder: path\.join\(configRoot, 'webview2'\) \} : \{\}\)/);
});

test("a scenario's temporary folder is removed with retries, and a leftover never ends the run", () => {
  const run = harnessText('run.mjs');
  // The WebView2 browser can hold its data folder (configRoot/webview2) for a
  // moment after the app has gone; fs.rmSync retries EPERM and EBUSY.
  assert.match(run, /try \{\s*fs\.rmSync\(configRoot, \{ recursive: true, force: true, maxRetries: 20, retryDelay: 250 \}\);\s*\} catch \(removeError\) \{\s*console\.error\(/);
  assert.equal(run.split('fs.rmSync(configRoot').length - 1, 1, 'no other removal of it can throw');
});

test('a click a fixed overlay intercepts is retried once with the element centred', async () => {
  const { Driver } = await import('./e2e/driver.mjs');
  const original = globalThis.fetch;
  const run = async (answers) => {
    const calls = [];
    globalThis.fetch = async (url, init) => {
      const body = init.body === undefined ? undefined : JSON.parse(init.body);
      calls.push({ path: new URL(url).pathname, body });
      const [status, value] = answers.shift();
      return new Response(JSON.stringify({ value }), { status });
    };
    const driver = new Driver('http://driver.invalid');
    driver.sessionId = 's';
    try {
      await driver.click('el-1');
      return { calls, error: null };
    } catch (error) {
      return { calls, error };
    }
  };
  const intercepted = [400, { error: 'element click intercepted', message: 'Other element would receive the click' }];
  try {
    const rect = [200, { x: 700, y: 630, width: 100, height: 30 }];
    const retried = await run([intercepted, rect, [200, 'BUTTON'], [200, true], [200, null]]);
    assert.equal(retried.error, null);
    assert.deepEqual(retried.calls.map((c) => c.path), [
      '/session/s/element/el-1/click', '/session/s/element/el-1/rect', '/session/s/element/el-1/name',
      '/session/s/execute/sync', '/session/s/element/el-1/click',
    ]);
    // Found again at its centre, by its tag, among all the elements there.
    assert.match(retried.calls[3].body.script, /document\.elementsFromPoint\(x - window\.scrollX, y - window\.scrollY\)/);
    assert.match(retried.calls[3].body.script, /scrollIntoView\(\{ block: 'center'/);
    assert.deepEqual(retried.calls[3].body.args, [750, 645, 'button']);
    // The script itself, run as WebDriver would (its body a function whose
    // arguments are the args), against an overlay over the covered button:
    // the button scrolls, not the overlay (tp-8).
    const scrolled = [];
    const element = (tag) => ({ tagName: tag.toUpperCase(), scrollIntoView: () => scrolled.push(tag) });
    const fakeDocument = {
      elementsFromPoint: (x, y) => {
        assert.deepEqual([x, y], [740, 615], 'viewport coordinates: the centre less the window scroll');
        return [element('footer'), element('div'), element('button'), element('section')];
      },
    };
    const script = new Function('document', 'window', 'args', `return (function () {${retried.calls[3].body.script}}).apply(null, args);`);
    assert.equal(script(fakeDocument, { scrollX: 10, scrollY: 30 }, retried.calls[3].body.args), true);
    assert.deepEqual(scrolled, ['button'], 'only the element with the tag scrolls');
    // Nothing with that tag at the point, or nothing at all: it says so and
    // scrolls nothing, so the click is not retried (tp-9).
    for (const found of [[], [element('footer'), element('div')]]) {
      scrolled.length = 0;
      const nothing = { elementsFromPoint: () => found };
      assert.equal(script(nothing, { scrollX: 0, scrollY: 0 }, retried.calls[3].body.args), false);
      assert.deepEqual(scrolled, []);
    }
    const stillCovered = await run([intercepted, rect, [200, 'BUTTON'], [200, true], intercepted]);
    assert.equal(stillCovered.error?.code, 'element click intercepted', 'a second interception fails the click');
    const notFound = await run([intercepted, rect, [200, 'BUTTON'], [200, false]]);
    assert.equal(notFound.error?.code, 'element click intercepted', 'an element not found again fails the click');
    assert.equal(notFound.calls.length, 4, 'and is not clicked again');
    const other = await run([[404, { error: 'no such element', message: 'gone' }]]);
    assert.equal(other.error?.code, 'no such element');
    assert.equal(other.calls.length, 1, 'any other error is not retried');
  } finally {
    globalThis.fetch = original;
  }
});

test("Windows: a scenario's mock CA is trusted for that scenario only, on a disposable runner", () => {
  const run = harnessText('run.mjs');
  const trust = run.indexOf("trustedCa = windowsTool('trust-ca', [scenarioEnv.SSL_CERT_FILE]).thumbprint;");
  assert.ok(trust > 0, 'trusted when the scenario starts');
  assert.match(run, /if \(isWindows && scenarioEnv\.SSL_CERT_FILE\) \{\s*trustedCa = /, 'only on Windows, only for a scenario that names a CA');
  // Inside the scenario's try, so a failed trust still tears down the scenario.
  const body = run.slice(run.indexOf('async function runScenario('), run.indexOf('\n}\n', run.indexOf('async function runScenario(')));
  assert.ok(body.indexOf('  try {\n    if (isWindows && scenarioEnv.SSL_CERT_FILE)') > 0);
  const tool = harnessText('windows.ps1');
  assert.match(tool, /'trust-ca' \{[\s\S]*?if \(\$env:VELA_E2E_DISPOSABLE_WINDOWS -ne '1'\) \{\s*throw/, 'never on a machine that is not disposable');
  assert.match(tool, /'untrust-ca' \{\s*if \(\$Value -notmatch '\^\[0-9A-F\]\{40\}\$'\) \{ throw/);
});

for (const signal of [null, 'SIGINT', 'SIGTERM']) {
  for (const failure of ['none', 'sync-cleanup', 'async-cleanup', 'teardown']) {
    test(`Windows: test CA removed once on ${signal ?? 'normal teardown'} with ${failure}`, async () => {
      const source = harnessText('run.mjs');
      const signals = source.slice(source.indexOf('const activeKills ='), source.indexOf('\nfunction runStep('));
      const start = source.indexOf('async function runScenario(');
      const scenarioCode = source.slice(start, source.indexOf('\n}\n', start) + 2);
      const events = [];
      const handlers = new Map();
      let reached, proceed;
      const waiting = new Promise((resolve) => { reached = resolve; });
      const resume = new Promise((resolve) => { proceed = resolve; });
      let waits = 0;
      const scenario = {
        name: 'ca-probe',
        environment: () => ({ SSL_CERT_FILE: '/mock/ca.pem' }),
        run: async () => {},
      };
      // Also exercise a CA-backed scenario that has no cleanup of its own.
      if (failure !== 'none') scenario.cleanup = () => {
        events.push('scenario-cleanup');
        if (failure === 'sync-cleanup') throw new Error('synchronous cleanup failed');
        if (failure === 'async-cleanup') return Promise.reject(new Error('async cleanup failed'));
      };
      const context = vm.createContext({
        scenario, path, os, console,
        isWindows: true, repoRoot: '/repo', artifactsDir: '/artifacts', nativeDriver: 'native',
        driverPort: 4444, driverUrl: 'http://driver.invalid', displayEnv: {}, appBinary: '/app', runPlayer: 'external',
        fs: { mkdtempSync: () => '/config', openSync: () => 9, closeSync() {}, rmSync() {} },
        process: { env: {}, on: (name, fn) => handlers.set(name, fn), removeListener() {}, exit: (code) => events.push(`exit:${code}`) },
        spawn: () => ({ pid: 123 }),
        Driver: class { async newSession() {} async deleteSession() {} },
        prepareWindowsConfig() {}, setScenarioConfigRoot() {}, registerWindowsApp() {}, forgetWindowsApp() {},
        killWindowsTree: () => events.push('kill-tree'),
        stopWindowsApps() { if (failure === 'teardown') throw new Error('app teardown failed'); },
        windowsTool(operation, args) {
          if (operation === 'protect-fixtures') return;
          events.push(`${operation}:${args[0]}`);
          return { thumbprint: 'test-thumbprint' };
        },
        beginProof() {}, endProof: () => ({}), proofProblems: () => [], driverListening() {},
        waitUntil: async () => { if (++waits === 1) { reached(); await resume; } },
      });
      vm.runInContext(`${signals}\n${scenarioCode}`, context);
      const running = vm.runInContext("runScenario(scenario, 'tauri-driver')", context);
      await waiting; // trust-ca has succeeded; no driver session has started yet.
      if (signal) {
        await handlers.get(signal)();
        assert.ok(events.indexOf('untrust-ca:test-thumbprint') > events.indexOf('trust-ca:/mock/ca.pem'));
        assert.ok(events.indexOf('untrust-ca:test-thumbprint') < events.indexOf(`exit:${signal === 'SIGINT' ? 130 : 143}`), 'CA removed before signal exit');
      }
      proceed();
      if (failure === 'teardown') await assert.rejects(running, /app teardown failed/);
      else await running;
      assert.equal(events.filter((e) => e === 'untrust-ca:test-thumbprint').length, 1, 'normal and signal cleanup share exactly one CA removal');
      assert.equal(events.filter((e) => e === 'scenario-cleanup').length, failure === 'none' ? 0 : 1);
    });
  }
}

test('an aliased second driver in another module cannot start another app (pa-6)', () => {
  driverCase(`
    const located = await locate();
    setRunApp(located);
    const driver = new Driver(url);
    await driver.newSession();
    // The reviewer's helper imports Driver as Client, supplies OLD, and
    // shadows driver in the existing try block, including the restart call.
    try {
      const driver = otherClient(url, { appBinary: old, identity: appIdentity(old) });
      await driver.newSession();
      await driver.newSession();
    } finally {}
    assert.deepEqual(applications(), [app, app, app], 'the aliased second driver started OLD instead of the located app');
  `);
});

test("subclasses and callers cannot replace the driver's session start (pa-6)", () => {
  driverCase(`
    setRunApp(await locate());
    const driver = new Override(url, { appBinary: old, identity: appIdentity(old) });
    await driver.newSession();
    assert.deepEqual(applications(), [app], 'the subclass did not start the located app');
    assert.throws(() => { driver.newSession = () => {}; }, TypeError, 'newSession must not be writable');
    assert.throws(() => Object.defineProperty(driver, 'newSession', { value: () => {} }), TypeError, 'newSession must not be configurable');
  `);
});

test("separate driver module URLs share the run's one app selection (pa-6)", () => {
  driverCase(`
    setRunApp(await locate());
    const { Driver: Client, setRunApp: setOtherApp } = await import(driverModule + '?another-import');
    commands.build.args[3] = old;
    const another = await locate();
    assert.throws(() => setOtherApp(another), /already been set/, 'a second driver module URL must not select another app');
    await new Client(url, another).newSession();
    assert.deepEqual(applications(), [app], 'a second driver module URL must start the run app');
  `);
});

test("the run's app can be set only once (pa-6)", () => {
  driverCase(`
    const located = await locate();
    setRunApp(located);
    assert.throws(() => setRunApp(located), /already been set/, 'even setting the same app twice must throw');
    commands.build.args[3] = old;
    const another = await locate();
    assert.equal(another.appBinary, old);
    assert.throws(() => setRunApp(another), /already been set/, 'a second located app must not replace the first');
    await new Driver(url).newSession();
    assert.deepEqual(applications(), [app], 'a rejected second setting changed the app');
  `);
});

test("only an unchanged locateAppBinary result can set the run's app (pa-6)", () => {
  driverCase(`
    const located = await locate();
    for (const forged of [{ appBinary: old, identity: appIdentity(old) }, { ...located }, Object.create(located), new Proxy(located, {}), undefined, null]) {
      assert.throws(() => setRunApp(forged), /must be a locateAppBinary result/, 'a forged or copied result must be refused');
    }
    for (const [key, value] of Object.entries({ appBinary: old, identity: appIdentity(old), built: false, appBinaryFrom: 'forged' })) {
      assert.throws(() => { located[key] = value; }, TypeError, 'the located result must be immutable: ' + key);
    }
    setRunApp(located);
    await new Driver(url).newSession();
    assert.deepEqual(applications(), [app]);
  `);
});

test("a session before the run's app is set is refused (pa-6)", () => {
  driverCase(`
    const located = await locate();
    const driver = otherClient(url, located);
    await assert.rejects(driver.newSession(), /the run app has not been set/, 'a session before registration must be refused');
    assert.equal(sent.length, 0, 'an unregistered session reached fetch');
    setRunApp(located);
    await driver.newSession();
    assert.deepEqual(applications(), [app], 'a refused early session must not poison later registration');
  `);
});

test("a located result without a file cannot set the run's app (pa-6)", () => {
  driverCase(`
    fs.rmSync(app);
    const missing = await locate();
    assert.throws(() => setRunApp(missing), /the located app has no file to start/, 'a missing file must not be registered');
    write(app, original);
    setRunApp(await locate());
    await new Driver(url).newSession();
    assert.deepEqual(applications(), [app]);
  `);
});

test("a --skip-build locator result selects the run's app without claiming a build (pa-6)", () => {
  driverCase(`
    const target = app + '-target';
    fs.mkdirSync(target + '/debug', { recursive: true });
    fs.copyFileSync(app, target + '/debug/vela');
    const metadata = { target_directory: target, packages: [{ manifest_path: commands.appManifest, targets: [{ kind: ['bin'], name: 'vela' }] }] };
    commands.metadata = { command: process.execPath, args: ['-e', 'require("node:fs").writeSync(1, process.argv[1])', JSON.stringify(metadata)], cwd: commands.build.cwd };
    const predicted = await locateAppBinary({ skipBuild: true, env: process.env, commands });
    assert.equal(predicted.built, false, 'a prediction must not claim a build');
    assert.equal(predicted.appBinaryFrom, 'cargo metadata (nothing was built)');
    setRunApp(predicted);
    await new Driver(url).newSession();
    assert.deepEqual(applications(), [target + '/debug/vela']);
  `);
});

test("the driver starts only the run's located app, and not once the file was replaced (pa-6)", () => {
  driverCase(`
    const located = await locate();
    setRunApp(located);
    const driver = new Driver(url);
    await driver.newSession();
    assert.deepEqual(applications(), [app], 'the session starts the located app');
    await assert.rejects(driver.newSession('/tmp/old-vela'), /newSession takes no path/, 'a path handed to newSession is refused');
    // A different device cannot be made with a rename on this filesystem;
    // change only the reported device, retaining the real inode/size/time.
    const statSync = fs.statSync;
    fs.statSync = (file, options) => {
      const stat = statSync(file, options);
      return file === app ? { ...stat, dev: stat.dev + 1n } : stat;
    };
    await assert.rejects(driver.newSession(), /is not the file it was when the run located it/, 'a file on a different device is refused');
    fs.statSync = statSync;
    assert.equal(appIdentity(app), located.identity);
    // Change one part at a time, then put back the original file/state so
    // an earlier mismatch cannot make a later identity check vacuously pass.
    write(app, 'an older build, of another size');
    await assert.rejects(driver.newSession(), /is not the file it was when the run located it/, 'a file written over (a new size) is refused');
    write(app, original);
    assert.equal(appIdentity(app), located.identity);
    fs.renameSync(app, app + '.saved');
    write(app, original);
    await assert.rejects(new Driver(url).newSession(), /is not the file it was when the run located it/, 'a file put in its place (a new inode) is refused');
    fs.renameSync(app + '.saved', app);
    assert.equal(appIdentity(app), located.identity);
    fs.utimesSync(app, new Date(0), new Date(0));
    await assert.rejects(driver.newSession(), /is not the file it was when the run located it/, 'a file with a new modification time is refused');
    fs.utimesSync(app, when, when);
    assert.equal(appIdentity(app), located.identity);
    fs.rmSync(app);
    await assert.rejects(driver.newSession(), /now no file/);
    assert.equal(sent.length, 1, 'nothing refused reached the driver');
  `);
});

// Another process of this user that makes a socket of a play's name pattern
// (a `cargo test` binary does) is not the scenario's play, however fresh its
// socket: a scenario that took it hung on a socket that never answers (venue
// finding atk-1). Here this test process stands in for that stranger.
test("a socket of the play pattern that the scenario's app did not make is not listed", async () => {
  const dir = fs.mkdtempSync('/tmp/vela-harness-');
  const stranger = path.join(dir, `mpv-${process.pid}-${Date.now()}-1.sock`);
  const server = net.createServer();
  await new Promise((resolve) => server.listen(stranger, resolve));
  const configRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-e2e-harness-'));
  try {
    setScenarioConfigRoot(configRoot);
    assert.ok(fs.existsSync(stranger));
    assert.equal(mpvSocketSnapshot().has(stranger), false, "a stranger's socket was taken for the scenario's play");
  } finally {
    await new Promise((resolve) => server.close(resolve));
    fs.rmSync(dir, { recursive: true, force: true });
    fs.rmSync(configRoot, { recursive: true, force: true });
  }
});

// The owner is the Vela process the socket is named after, and it must carry
// the scenario's own XDG_CONFIG_HOME: a Vela of the user, or of another
// scenario, does not count.
test("a socket belongs to the scenario when its named owner is the scenario's Vela", () => {
  const proc = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-harness-proc-'));
  try {
    const fakeProcess = (pid, comm, env) => {
      fs.mkdirSync(path.join(proc, String(pid)));
      fs.writeFileSync(path.join(proc, String(pid), 'comm'), `${comm}\n`);
      fs.writeFileSync(path.join(proc, String(pid), 'environ'), env.map((e) => `${e}\0`).join(''));
    };
    const root = '/tmp/vela-e2e-x-123';
    fakeProcess(10, 'vela', ['HOME=/h', `XDG_CONFIG_HOME=${root}/config`, 'PATH=/bin']);
    fakeProcess(11, 'vela', ['XDG_CONFIG_HOME=/home/someone/.config']);
    fakeProcess(12, 'vela_lib-ec1a75', [`XDG_CONFIG_HOME=${root}/config`]);
    fakeProcess(13, 'vela', [`XDG_CONFIG_HOME=${root}/config-other`]);
    assert.equal(socketOfScenario('mpv-10-1700000000000-1.sock', root, proc), true);
    assert.equal(socketOfScenario('mpv-11-1700000000000-1.sock', root, proc), false, "the user's own Vela");
    assert.equal(socketOfScenario('mpv-12-1700000000000-1.sock', root, proc), false, 'a cargo test binary');
    assert.equal(socketOfScenario('mpv-13-1700000000000-1.sock', root, proc), false, 'another config root');
    assert.equal(socketOfScenario('mpv-14-1700000000000-1.sock', root, proc), false, 'an owner that is gone');
    assert.equal(socketOfScenario('mpv-10.sock', root, proc), false, 'not the pattern');
  } finally {
    fs.rmSync(proc, { recursive: true, force: true });
  }
});

// A play that has ended leaves its socket file behind, and connecting to it is
// refused. Linux refuses a connection to any file that nothing listens on, so a
// plain file left where a socket was stands in for it here.
test('a socket file nothing listens on is reported as a play that has already ended', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-harness-'));
  try {
    const leftBehind = path.join(dir, 'mpv-1-2-3.sock');
    const server = net.createServer();
    await new Promise((resolve) => server.listen(leftBehind, resolve));
    // Closing the server unlinks the file; put a file back where it was.
    await new Promise((resolve) => server.close(resolve));
    fs.writeFileSync(leftBehind, '');
    await assert.rejects(MpvIpc.connect(leftBehind, { timeoutMs: 300 }), (err) => {
      assert.equal(err.code, 'ECONNREFUSED');
      assert.match(err.message, /mpv-1-2-3\.sock/);
      assert.match(err.message, /the play has already ended/);
      return true;
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('a socket that answers is connected to', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-harness-'));
  const socket = path.join(dir, 'mpv.sock');
  const server = net.createServer((conn) => conn.end());
  await new Promise((resolve) => server.listen(socket, resolve));
  try {
    const mpv = await MpvIpc.connect(socket, { timeoutMs: 300 });
    mpv.close();
  } finally {
    await new Promise((resolve) => server.close(resolve));
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
