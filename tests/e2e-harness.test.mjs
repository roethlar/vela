// The end-to-end harness's own helpers that can be checked without the app.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { appBinaryFromMetadata, appCommands, appExecutableFromMessages, appIdentity, locateAppBinary, MESSAGE_FORMAT_ARG } from './e2e/app-binary.mjs';
import { Driver } from './e2e/driver.mjs';
import { MpvIpc, mpvSocketSnapshot, setScenarioConfigRoot, socketOfScenario } from './e2e/mpv.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// Which app binary the runner starts (review finding pa-6). A run that builds
// takes the executable from Cargo's own report of that build; a run with
// --skip-build predicts it from `cargo metadata` and is recorded as not built.
// Every test of that finding has "(pa-6)" in its name.

// A made-up project laid out as this one: <root>/src-tauri/Cargo.toml, a
// binary `madeup` that prints NEW, and with `workspace` a second member,
// `helper`, with a binary of its own.
function madeupProject(root, { workspace = false, cargoConfig } = {}) {
  const crate = path.join(root, 'src-tauri');
  fs.mkdirSync(path.join(crate, 'src'), { recursive: true });
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
const fakeBuild = `
const { writeSync } = require('node:fs');
const [manifest, executable, status] = process.argv.slice(1);
const out = (line) => writeSync(1, line + '\\n');
out('vite: building for production');
out('{"vite":"a JSON line without a reason"}');
out(JSON.stringify({ reason: 'compiler-artifact', manifest_path: manifest, target: { kind: ['bin'], name: 'vela' }, executable }));
out(JSON.stringify({ reason: 'build-finished', success: status === '0' }));
writeSync(2, 'DIAG: rendered by the build\\n');
process.exit(Number(status));
`;
function fakeBuildCommands(dir, status) {
  const appManifest = path.join(dir, 'app/Cargo.toml');
  return {
    appManifest,
    build: { command: process.execPath, args: ['-e', fakeBuild, appManifest, '/t/debug/vela', String(status)], cwd: dir },
  };
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
    assert.equal(located.appBinary, '/t/debug/vela');
    assert.deepEqual(
      echoed,
      ['vite: building for production', '{"vite":"a JSON line without a reason"}'],
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

// The runner starts exactly the path locateAppBinary returned. The driver is
// made with that result and keeps it where nothing else can change it; its
// newSession takes no path, and refuses to start a file that is no longer the
// one located (the test after this one). What is left to the runner's text is
// pinned here, because earlier forms of this test let other paths through: a
// session started with a path of its own in a nested call, the
// target-directory variables taken out of Cargo's environment, a parameter of
// runScenario whose default re-bound the name, a computed method name, a
// different driver module, a scenario starting a session of its own, and an
// open-ended accepted form that let any expression follow it. So each
// accepted form is a whole statement or a closed expression, occurs exactly as
// many times as it does today, and no other mention of the names it carries
// is allowed anywhere in the runner; and no harness module other than the
// driver can make a WebDriver session or a Driver.
test('the runner starts the one path locateAppBinary returned, and builds nothing itself (pa-6)', () => {
  const run = fs.readFileSync(path.join(repoRoot, 'tests/e2e/run.mjs'), 'utf8');
  const count = (text) => run.split(text).length - 1;
  const forms = [
    ["import { appCommands, locateAppBinary } from './app-binary.mjs';", 1],
    ["import { Driver } from './driver.mjs';", 1],
    ['let located;', 1],
    ['located = Object.freeze(await locateAppBinary({ skipBuild, env: process.env, commands: appCommands(repoRoot) }));', 1],
    ['const { appBinary, built, appBinaryFrom } = located;', 1],
    ['if (!fs.existsSync(appBinary)) {', 1],
    [
      "`e2e: app binary missing: ${appBinary}, from ${appBinaryFrom} (${skipBuild ? 'run without --skip-build' : 'the build did not leave it there'})`,",
      1,
    ],
    ['console.log(`e2e: running ${appBinary} (from ${appBinaryFrom})`);', 1],
    ['const driver = new Driver(driverUrl, located);', 1],
    ['await driver.newSession();', 2],
    ['        built,\n        appBinary,\n        appBinaryFrom,\n        ...tree,\n', 1],
  ];
  let rest = run;
  for (const [form, times] of forms) {
    assert.equal(count(form), times, `the runner has \`${form}\` exactly ${times} time(s)`);
    rest = rest.split(form).join('');
  }
  for (const word of ['appBinary', 'located', 'locateAppBinary', 'appCommands', 'newSession', 'Driver', 'driver\\.mjs', 'app-binary\\.mjs']) {
    assert.doesNotMatch(rest, new RegExp(`\\b${word}\\b`), `${word.replace(/\\/g, '')} appears in the runner only in the accepted forms`);
  }
  assert.doesNotMatch(rest, /\bbuilt\s*[:=]/, '`built` is recorded only as located says');
  assert.doesNotMatch(run, /['"`]cargo['"`]|['"`]build['"`]/, 'the runner runs no Cargo and no build of its own');
  assert.doesNotMatch(run, /target\/debug|['"`]target['"`]|['"`]debug['"`]/, 'the runner puts no target directory together itself');
  // Only the driver makes a WebDriver session or a Driver; scenarios may use
  // Driver.KEY.
  const e2eDir = path.join(repoRoot, 'tests/e2e');
  const modules = fs
    .readdirSync(e2eDir, { recursive: true })
    .filter((f) => /\.m?js$/.test(f) && f !== 'driver.mjs' && !f.startsWith('node_modules'));
  assert.ok(modules.length > 10, 'the harness modules are found');
  for (const file of modules) {
    const text = fs.readFileSync(path.join(e2eDir, file), 'utf8');
    assert.doesNotMatch(text, /\/session\b|tauri:options/, `${file} makes no WebDriver session of its own`);
    if (file !== 'run.mjs') {
      assert.doesNotMatch(text, /new\s+Driver\b|\bDriver\s*\.\s*prototype\b|\bnewSession\b/, `${file} makes no driver and starts no session`);
    }
  }
});

// What the driver starts: the path it was made with, whatever newSession is
// given, and only while that path is the file it was when it was located.
// fetch is replaced here, so no driver or socket is involved.
test('the driver starts only the app it was made with, and not once the file was replaced (pa-6)', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-pa6-driver-'));
  const realFetch = globalThis.fetch;
  try {
    const app = path.join(dir, 'vela');
    // Each replacement below changes one part of the file's identity only:
    // the modification time is put back to this whole second after each
    // write (a time set to the second is kept exactly, to the nanosecond).
    const when = new Date(1_700_000_000_000);
    const write = (file, text) => {
      fs.writeFileSync(file, text, { mode: 0o755 });
      fs.utimesSync(file, when, when);
    };
    write(app, 'the reported build');
    const sent = [];
    globalThis.fetch = async (url, init) => {
      sent.push(JSON.parse(init.body));
      return new Response(JSON.stringify({ value: { sessionId: `s${sent.length}` } }), { status: 200 });
    };
    const located = { appBinary: app, identity: appIdentity(app) };
    const driver = new Driver('http://driver.invalid', located);
    await driver.newSession();
    assert.deepEqual(
      sent.map((body) => body.capabilities.alwaysMatch['tauri:options'].application),
      [app],
      'the session starts the path the driver was made with',
    );
    await assert.rejects(driver.newSession('/tmp/old-vela'), /newSession takes no path/, 'a path handed to newSession is refused');
    await assert.rejects(new Driver('http://driver.invalid').newSession(), /made without an app to start/);
    // Written over in place with other contents: the same inode and time, a
    // new size.
    write(app, 'an older build, of another size');
    await assert.rejects(driver.newSession(), /is not the file it was when the run located it/, 'a file written over (a new size) is refused');
    // Put in its place: the same size and time, a new inode.
    const again = new Driver('http://driver.invalid', { appBinary: app, identity: appIdentity(app) });
    write(`${app}.other`, 'an older build, of another size');
    fs.renameSync(`${app}.other`, app);
    await assert.rejects(again.newSession(), /is not the file it was when the run located it/, 'a file put in its place (a new inode) is refused');
    // The same inode and size, touched: a new modification time.
    const touched = new Driver('http://driver.invalid', { appBinary: app, identity: appIdentity(app) });
    fs.utimesSync(app, new Date(0), new Date(0));
    await assert.rejects(touched.newSession(), /is not the file it was when the run located it/, 'a file with a new modification time is refused');
    fs.rmSync(app);
    await assert.rejects(touched.newSession(), /now no file/);
    assert.equal(sent.length, 1, 'nothing refused reached the driver');
  } finally {
    globalThis.fetch = realFetch;
    fs.rmSync(dir, { recursive: true, force: true });
  }
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
