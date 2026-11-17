// The E2E harness's connection to mpv (tests/e2e/mpv.mjs, MpvIpc).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { MpvIpc } from './e2e/mpv.mjs';

// A peer that serves one connection the way mpv 0.42's IPC client thread
// does (input/ipc-unix.c, client_thread): each turn it first writes the
// events it has pending, and when a write fails it drops the client without
// reading what the client sent; only then does it read. Here an event is
// pending once the client has sent its last bytes and closed, as in the
// `markers` failure of 2026-11-17 (a seek's events going out as the scenario
// quit and closed).
function mpvLikePeer(socketPath) {
  let settle;
  const outcome = new Promise((resolve) => {
    settle = resolve;
  });
  const server = net.createServer((sock) => {
    sock.on('error', () => {});
    let received = '';
    // Its thread is busy elsewhere while the client writes and closes.
    setTimeout(() => {
      sock.write('{"event":"playback-restart"}\n', (error) => {
        if (error) {
          settle({ dropped: true, received });
          sock.destroy();
          return;
        }
        sock.on('data', (chunk) => {
          received += chunk;
        });
        sock.on('end', () => {
          settle({ dropped: false, received });
          sock.end();
        });
        sock.resume();
      });
    }, 200);
  });
  return new Promise((resolve) => server.listen(socketPath, () => resolve({ server, outcome })));
}

test(
  'a quit sent just before the connection closes reaches an mpv that writes before it reads',
  { skip: process.platform === 'win32' && 'the peer is a Unix socket' },
  async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-mpvipc-'));
    const { server, outcome } = await mpvLikePeer(path.join(dir, 'mpv.sock'));
    try {
      const ipc = await MpvIpc.connect(path.join(dir, 'mpv.sock'));
      ipc.quit();
      ipc.close();
      const { dropped, received } = await outcome;
      assert.equal(dropped, false, 'mpv could not write its event, so it dropped the connection unread');
      assert.match(received, /\{"command":\["quit"\]\}\n/, 'and so the quit never reached it');
    } finally {
      server.close();
      fs.rmSync(dir, { recursive: true, force: true });
    }
  },
);
