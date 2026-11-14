// The end-to-end harness's own helpers that can be checked without the app.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { MpvIpc, mpvSocketSnapshot, setScenarioConfigRoot, socketOfScenario } from './e2e/mpv.mjs';

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
