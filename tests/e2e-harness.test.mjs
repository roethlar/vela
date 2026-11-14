// The end-to-end harness's own helpers that can be checked without the app.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { appBinaryFromMetadata, cargoMetadataCwd, METADATA_ARGS } from './e2e/app-binary.mjs';
import { MpvIpc, mpvSocketSnapshot, setScenarioConfigRoot, socketOfScenario } from './e2e/mpv.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// The runner starts the binary where Cargo says it builds, not a fixed
// src-tauri/target/debug/vela (review finding pa-6).
test("the app binary is taken from Cargo's target directory", () => {
  const metadata = (target, targets = [{ kind: ['staticlib', 'cdylib', 'rlib'], name: 'vela_lib' }, { kind: ['bin'], name: 'vela' }]) =>
    JSON.stringify({ target_directory: target, packages: [{ name: 'vela', targets }] });
  assert.equal(appBinaryFromMetadata(metadata('/elsewhere/target')), '/elsewhere/target/debug/vela');
  assert.equal(appBinaryFromMetadata(metadata('/w/src-tauri/target')), '/w/src-tauri/target/debug/vela');
  // When Cargo cannot say, the runner stops instead of guessing.
  assert.throws(() => appBinaryFromMetadata('warning: something\n'), /Cargo's metadata is not JSON/);
  assert.throws(() => appBinaryFromMetadata(metadata(undefined)), /names no absolute target directory \(target_directory is undefined\)/);
  assert.throws(() => appBinaryFromMetadata(metadata('relative/target')), /names no absolute target directory/);
  assert.throws(() => appBinaryFromMetadata(metadata('/t', [])), /names 0 binary targets, not the one app binary/);
  assert.throws(
    () => appBinaryFromMetadata(metadata('/t', [{ kind: ['bin'], name: 'a' }, { kind: ['bin'], name: 'b' }])),
    /names 2 binary targets/,
  );
  // A build for a named target writes under <target>/<triple>/debug/.
  assert.throws(
    () => appBinaryFromMetadata(metadata('/t'), { CARGO_BUILD_TARGET: 'x86_64-unknown-linux-gnu' }),
    /CARGO_BUILD_TARGET is "x86_64-unknown-linux-gnu": the build would write its binary under \/t\/x86_64-unknown-linux-gnu/,
  );
});

// What Cargo really answers, with CARGO_TARGET_DIR set and without: the
// question run.mjs asks, in the environment it asks it in.
test('Cargo is asked where it builds, and CARGO_TARGET_DIR moves the answer', (t) => {
  const ask = (env) =>
    spawnSync('cargo', METADATA_ARGS, { cwd: cargoMetadataCwd(repoRoot), env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  const { CARGO_TARGET_DIR: _unset, CARGO_BUILD_TARGET_DIR: _unset2, ...plain } = process.env;
  const res = ask(plain);
  if (res.error?.code === 'ENOENT') {
    t.skip('cargo is not installed here');
    return;
  }
  assert.equal(res.status, 0, res.stderr);
  // src-tauri/target/debug/vela, unless a Cargo configuration of this
  // machine moves it.
  assert.match(appBinaryFromMetadata(res.stdout, plain), /^\/.*\/debug\/vela$/);
  const moved = ask({ ...plain, CARGO_TARGET_DIR: '/nowhere/vela-target' });
  assert.equal(moved.status, 0, moved.stderr);
  assert.equal(appBinaryFromMetadata(moved.stdout, plain), '/nowhere/vela-target/debug/vela');
});

// tauri build runs `cargo build` in src-tauri/, so Cargo reads a
// src-tauri/.cargo/config.toml there and takes a relative CARGO_TARGET_DIR
// from there. The runner must ask from the same place, or it starts an older
// binary than the one just built. A made-up project laid out as this one is
// (<root>/src-tauri/Cargo.toml, with its own .cargo/config.toml) shows what
// the runner's question answers (review finding pa-6).
test("Cargo is asked from where the build runs it, so src-tauri's own Cargo configuration counts", (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-cargo-dir-'));
  try {
    const crate = path.join(root, 'src-tauri');
    fs.mkdirSync(path.join(crate, 'src'), { recursive: true });
    fs.mkdirSync(path.join(crate, '.cargo'));
    fs.writeFileSync(
      path.join(crate, 'Cargo.toml'),
      '[package]\nname = "madeup"\nversion = "0.0.0"\nedition = "2021"\n\n[workspace]\n',
    );
    fs.writeFileSync(path.join(crate, 'src/main.rs'), 'fn main() {}\n');
    const configured = path.join(root, 'configured-target');
    fs.writeFileSync(path.join(crate, '.cargo/config.toml'), `[build]\ntarget-dir = ${JSON.stringify(configured)}\n`);
    const { CARGO_TARGET_DIR: _unset, CARGO_BUILD_TARGET_DIR: _unset2, ...plain } = process.env;
    const ask = (env) =>
      spawnSync('cargo', METADATA_ARGS, { cwd: cargoMetadataCwd(root), env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    const res = ask(plain);
    if (res.error?.code === 'ENOENT') {
      t.skip('cargo is not installed here');
      return;
    }
    assert.equal(res.status, 0, res.stderr);
    assert.equal(appBinaryFromMetadata(res.stdout, plain), path.join(configured, 'debug/madeup'));
    // A relative CARGO_TARGET_DIR is taken from src-tauri/, as the build takes it.
    const relative = ask({ ...plain, CARGO_TARGET_DIR: 'alt' });
    assert.equal(relative.status, 0, relative.stderr);
    assert.equal(appBinaryFromMetadata(relative.stdout, plain), path.join(crate, 'alt/debug/madeup'));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

// run.mjs asks Cargo through these helpers, from that directory: the test
// above shows what the question answers, this that the runner asks it so.
test('the runner asks Cargo from the directory the build runs it in', () => {
  const run = fs.readFileSync(path.join(repoRoot, 'tests/e2e/run.mjs'), 'utf8');
  assert.match(run, /spawnSync\('cargo', METADATA_ARGS, \{\s*cwd: cargoMetadataCwd\(repoRoot\),/);
  assert.equal(cargoMetadataCwd('/r'), '/r/src-tauri');
});

// And the binary it starts is that answer, taken nowhere else. Asking Cargo
// and then starting a fixed path anyway passed every test above (found when
// the fix was checked before it landed), so the runner's own text is held to
// it: one value is ever given to appBinary, Cargo's; every session is started
// with it; and the runner names no target directory of its own.
test("the runner takes the app binary from Cargo's answer, and starts that one", () => {
  const run = fs.readFileSync(path.join(repoRoot, 'tests/e2e/run.mjs'), 'utf8');
  assert.deepEqual(
    [...run.matchAll(/\bappBinary\s*=(?!=)\s*([^;]*);/g)].map((m) => m[1].trim()),
    ['appBinaryFromMetadata(res.stdout, buildEnv)'],
    'appBinary is given one value, the path made from what Cargo answered',
  );
  assert.equal((run.match(/\b(?:let|const|var)\s+appBinary\b/g) ?? []).length, 1, 'appBinary is declared once');
  const sessions = run.match(/\.newSession\(([^()]|\([^()]*\))*\)/g) ?? [];
  assert.ok(sessions.length > 0, 'the runner starts a session');
  assert.deepEqual([...new Set(sessions)], ['.newSession(appBinary)'], 'every session is started with that binary');
  assert.doesNotMatch(
    run,
    /target\/debug|['"`]target['"`]|['"`]debug['"`]/,
    'the runner puts no target directory together itself',
  );
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
