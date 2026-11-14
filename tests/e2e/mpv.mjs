// Minimal mpv JSON-IPC client (newline-delimited JSON over a unix socket)
// plus discovery of the socket Vela creates per play session
// (/tmp/vela-<random>/mpv-<pid>-<ms>-<n>.sock, owner-only dirs).
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';

// The scenario whose plays the sockets below may belong to: the runner sets
// its throwaway config root before each scenario (run.mjs).
let scenarioConfigRoot = null;
export function setScenarioConfigRoot(configRoot) {
  scenarioConfigRoot = configRoot;
}

// Whether the socket file `name` (mpv-<pid>-<ms>-<n>.sock) belongs to a play
// of THIS scenario's app. Vela names every play's socket after its own pid
// (player/launch.rs ipc_socket_path), in either player, so the owner is the
// Vela process with that pid whose environment carries the scenario's
// XDG_CONFIG_HOME. Any other process of this user makes sockets of the same
// pattern too: a `cargo test` binary of another worktree (playback.rs
// ListeningLauncher, control.rs, report.rs), the user's own Vela, another
// run's app. Such a socket, taken for the scenario's play, accepts the
// connection and never answers, and the scenario failed with "no reply" or
// "timed out waiting for ... stream" (review finding atk-1 of the venue).
export function socketOfScenario(name, configRoot, procRoot = '/proc') {
  const owner = /^mpv-(\d+)-\d+-\d+\.sock$/.exec(name)?.[1];
  if (!owner) return false;
  const mine = `XDG_CONFIG_HOME=${path.join(configRoot, 'config')}\0`;
  try {
    return (
      fs.readFileSync(path.join(procRoot, owner, 'comm'), 'utf8').trim() === 'vela' &&
      fs.readFileSync(path.join(procRoot, owner, 'environ'), 'utf8').includes(mine)
    );
  } catch {
    return false; // its owner is gone, or is not ours to read
  }
}

function listMpvSockets() {
  if (!scenarioConfigRoot) throw new Error('mpv sockets: no scenario config root set (run.mjs sets it)');
  const sockets = [];
  for (const entry of fs.readdirSync('/tmp')) {
    if (!entry.startsWith('vela-')) continue;
    const dir = path.join('/tmp', entry);
    let names;
    try {
      names = fs.readdirSync(dir); // other users' dirs are 0700 → skipped by throw
    } catch {
      continue;
    }
    for (const name of names) {
      if (socketOfScenario(name, scenarioConfigRoot)) sockets.push(path.join(dir, name));
    }
  }
  return sockets;
}

// Snapshot before triggering playback, then await the socket the new mpv adds.
export function mpvSocketSnapshot() {
  return new Set(listMpvSockets());
}

export async function waitForNewMpvSocket(before, { timeoutMs = 15000 } = {}) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const fresh = listMpvSockets().filter((s) => !before.has(s));
    if (fresh.length > 0) return fresh[0];
    if (Date.now() > deadline) throw new Error('no new mpv IPC socket appeared');
    await new Promise((r) => setTimeout(r, 200));
  }
}

// A socket file that is there but refuses every connection for the whole
// wait is a play that has already ended: mpv leaves its socket file behind
// when it quits, and nothing listens on it any more. Say so, instead of a
// bare ECONNREFUSED. The usual reason is a stream that would not open (a
// play whose stream fails to load ends at once, and in either player without
// a word on the page); the play's own log says why (mpv's `--log-file`).
export function explainConnectFailure(err, socketPath, timeoutMs) {
  let why;
  if (err?.code === 'ECONNREFUSED') {
    why =
      'the socket file is there but nothing listens on it: the play has already ended (mpv leaves its socket file behind when it quits). ' +
      'A stream that would not open ends a play at once, in either player without a message on the page; ' +
      "give mpv a log (--log-file in the scenario's mpv_extra_args) to see why it ended";
  } else if (err?.code === 'ENOENT') {
    why = 'the socket file is gone: the run directory it was in has been removed';
  } else {
    why = 'the connection kept failing';
  }
  const explained = new Error(
    `mpv IPC: could not connect to ${socketPath} within ${timeoutMs} ms (${err?.code ?? err?.message}): ${why}`,
  );
  explained.code = err?.code;
  explained.cause = err;
  return explained;
}

export class MpvIpc {
  #sock;
  #nextId = 1;
  #pending = new Map();

  static async connect(socketPath, { timeoutMs = 10000 } = {}) {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      try {
        return await new Promise((resolve, reject) => {
          const sock = net.createConnection(socketPath);
          sock.once('connect', () => resolve(new MpvIpc(sock)));
          sock.once('error', reject);
        });
      } catch (err) {
        if (Date.now() > deadline) throw explainConnectFailure(err, socketPath, timeoutMs);
        await new Promise((r) => setTimeout(r, 200));
      }
    }
  }

  constructor(sock) {
    this.#sock = sock;
    let buf = '';
    sock.on('data', (chunk) => {
      buf += chunk;
      let nl;
      while ((nl = buf.indexOf('\n')) >= 0) {
        const line = buf.slice(0, nl);
        buf = buf.slice(nl + 1);
        if (!line.trim()) continue;
        let msg;
        try {
          msg = JSON.parse(line);
        } catch {
          continue;
        }
        if (msg.request_id !== undefined && this.#pending.has(msg.request_id)) {
          const { resolve, reject } = this.#pending.get(msg.request_id);
          this.#pending.delete(msg.request_id);
          if (msg.error === 'success') resolve(msg.data);
          else reject(new Error(`mpv: ${msg.error}`));
        }
        // events (no request_id) are ignored — assertions poll properties
      }
    });
    sock.on('error', () => {}); // quit() races the socket teardown by design
  }

  cmd(...command) {
    const request_id = this.#nextId++;
    return new Promise((resolve, reject) => {
      this.#pending.set(request_id, { resolve, reject });
      this.#sock.write(JSON.stringify({ command, request_id }) + '\n');
      setTimeout(() => {
        if (this.#pending.delete(request_id)) reject(new Error(`mpv: no reply to ${command[0]}`));
      }, 10000);
    });
  }

  getProp(name) {
    return this.cmd('get_property', name);
  }

  async setProp(name, value) {
    // mpv names its file before it can seek in it, and answers a position
    // set in that moment with "property unavailable". Scenarios seek as soon
    // as a play's socket answers, so that failed one or another of them now
    // and then. A seek therefore waits until mpv reports a position.
    if (name === 'time-pos') await this.#untilSeekable();
    return this.cmd('set_property', name, value);
  }

  async #untilSeekable() {
    const deadline = Date.now() + 10000;
    while (Date.now() < deadline) {
      const position = await this.getProp('time-pos').catch(() => undefined);
      if (typeof position === 'number') return;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    // Not seekable in time: the seek itself says what is wrong.
  }

  // fire-and-forget: mpv may close the socket before replying
  quit() {
    this.#sock.write(JSON.stringify({ command: ['quit'] }) + '\n');
  }

  close() {
    this.#sock.destroy();
  }
}
