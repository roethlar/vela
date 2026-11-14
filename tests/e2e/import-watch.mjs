// Importing a scenario must do nothing: the runner imports every one of them
// before it runs any. tests/e2e-parity.test.mjs runs this file as a process of
// its own, `node import-watch.mjs <dir> <prefix> [<dir> <prefix> ...]`, which
// replaces every way of acting on the machine it knows by one that records the
// call, then imports the scenarios of each directory through the runner's
// loader, and prints on its standard output, as one line of JSON, what it saw.
//
// The watch is never taken down. What an import puts off (a promise that
// settles later, a callback of a route not watched) runs after the import has
// returned, and a watch taken down at that point would not see it (review
// finding pa-4). So the report is written only when the event loop has
// nothing left to do ('beforeExit'): every put-off action has run by then,
// under the watch. Something that keeps the loop busy for good never lets the
// process end; the test that runs this file stops it and fails.
//
// Only Node's own modules are imported statically here: the loader and the
// scenarios are imported after the watch is in place, so no module of the
// suite can have taken a function before it was replaced.
import childProcess from 'node:child_process';
import cluster from 'node:cluster';
import dgram from 'node:dgram';
import dns from 'node:dns';
import fs from 'node:fs';
import http from 'node:http';
import https from 'node:https';
import module from 'node:module';
import net from 'node:net';
import timers from 'node:timers';
import timersPromises from 'node:timers/promises';
import workerThreads from 'node:worker_threads';

// Taken before the watch: how the report is written, and when.
const writeSync = fs.writeSync;
const report = (value) => writeSync(1, `${JSON.stringify(value)}\n`);
const pairs = process.argv.slice(2);

const calls = [];
const watch = (object, names, label) => {
  for (const name of names) {
    if (typeof object[name] !== 'function') continue;
    const original = object[name];
    object[name] = function watched(...args) {
      // Opening a file to read it is how modules are loaded at all.
      const readOnly = (name === 'openSync' || name === 'open') && [undefined, 'r', 0].includes(args[1]);
      if (!readOnly) calls.push(`${label}.${name}(${typeof args[0] === 'string' ? args[0] : ''})`);
      return original.apply(this, args);
    };
  }
};

// What an import could change in this process for every later module: the
// global object's own names, and the built-in prototypes' properties.
const shared = () => {
  const out = new Map([['globalThis', Object.getOwnPropertyNames(globalThis).sort().join(',')]]);
  for (const [name, proto] of Object.entries({
    Object: Object.prototype, Array: Array.prototype, String: String.prototype, Function: Function.prototype,
    Promise: Promise.prototype, Number: Number.prototype, RegExp: RegExp.prototype, Map: Map.prototype,
    Set: Set.prototype, Error: Error.prototype, JSON, Math, Reflect, Object_: Object, Array_: Array, Promise_: Promise,
  })) {
    for (const key of Reflect.ownKeys(proto)) out.set(`${name}.${String(key)}`, Object.getOwnPropertyDescriptor(proto, key));
  }
  return out;
};
const sameDescriptor = (a, b) =>
  !a || !b ? a === b : ['value', 'get', 'set', 'writable', 'enumerable', 'configurable'].every((k) => Object.is(a[k], b[k]));

let loaded = null;
let newHandles = null;
let failed = null;
let handlesBefore, envBefore, sharedBefore, listenersBefore;
// Every listener on the process, by event: a signal handler can be added
// through EventEmitter.prototype.on called on the process as well as through
// process.on, so the listeners themselves are compared, whatever the route.
const processListeners = () => new Map(process.eventNames().map((event) => [event, process.rawListeners(event)]));
let reported = false;
// `on`, not `once`: a once-listener is removed before it runs, which would
// read as a listener the import took away.
process.on('beforeExit', () => {
  if (reported) return;
  reported = true;
  if (failed !== null) return report({ error: failed, calls });
  if (loaded === null) return report({ error: 'the import never finished: something it awaited never settled', calls });
  // Read now, when everything the import put off has run, as well as what
  // the import did itself.
  const listenersAfter = processListeners();
  for (const event of new Set([...listenersBefore.keys(), ...listenersAfter.keys()])) {
    const [a, b] = [listenersBefore.get(event) ?? [], listenersAfter.get(event) ?? []];
    if (a.length !== b.length || a.some((listener, i) => listener !== b[i])) {
      calls.push(`process listeners of ${String(event)} changed`);
    }
  }
  const sharedAfter = shared();
  const sharedChanged = [];
  for (const key of new Set([...sharedBefore.keys(), ...sharedAfter.keys()])) {
    const [a, b] = [sharedBefore.get(key), sharedAfter.get(key)];
    if (key === 'globalThis' ? a !== b : !sameDescriptor(a, b)) sharedChanged.push(key);
  }
  report({ loaded, calls, newHandles, envChanged: JSON.stringify(process.env) !== envBefore, sharedChanged });
});

watch(childProcess, ['spawn', 'spawnSync', 'exec', 'execSync', 'execFile', 'execFileSync', 'fork'], 'child_process');
watch(cluster, ['fork', 'setupPrimary', 'setupMaster'], 'cluster');
watch(net, ['createServer', 'createConnection', 'connect'], 'net');
watch(dgram, ['createSocket'], 'dgram');
watch(http, ['createServer', 'request', 'get'], 'http');
watch(https, ['createServer', 'request', 'get'], 'https');
// The same acts reached without those functions: a socket or a server made
// with `new` and then connected or listening (a TLS socket or an HTTP server
// is one of these too), a child made with `new ChildProcess()` and spawned,
// and a DNS query, of the module or of a resolver of its own.
watch(net.Socket.prototype, ['connect'], 'net.Socket');
watch(net.Server.prototype, ['listen'], 'net.Server');
watch(dgram.Socket.prototype, ['bind', 'connect', 'send'], 'dgram.Socket');
watch(childProcess.ChildProcess.prototype, ['spawn'], 'child_process.ChildProcess');
const DNS_QUERIES = [
  'lookup', 'lookupService', 'resolve', 'resolve4', 'resolve6', 'resolveAny', 'resolveCaa', 'resolveCname',
  'resolveMx', 'resolveNaptr', 'resolveNs', 'resolvePtr', 'resolveSoa', 'resolveSrv', 'resolveTlsa', 'resolveTxt',
  'reverse',
];
watch(dns, DNS_QUERIES, 'dns');
watch(dns.promises, DNS_QUERIES, 'dns.promises');
watch(dns.Resolver.prototype, DNS_QUERIES, 'dns.Resolver');
watch(dns.promises.Resolver.prototype, DNS_QUERIES, 'dns.promises.Resolver');
// Every fs call that changes a file, a directory or its metadata; opening
// is watched too, except to read.
const FS_WRITERS = [
  'writeFile', 'appendFile', 'write', 'writev', 'mkdir', 'mkdtemp', 'rm', 'rmdir', 'unlink', 'rename',
  'copyFile', 'cp', 'symlink', 'link', 'truncate', 'ftruncate', 'chmod', 'fchmod', 'lchmod', 'chown',
  'fchown', 'lchown', 'utimes', 'futimes', 'lutimes', 'open',
];
// A file watcher acts on nothing, but it is left running, and one made with
// `persistent: false` holds no handle.
watch(fs, [...FS_WRITERS, ...FS_WRITERS.map((name) => `${name}Sync`), 'createWriteStream', 'watch', 'watchFile'], 'fs');
watch(fs.promises, [...FS_WRITERS, 'watch'], 'fs.promises');
watch(globalThis, ['fetch', 'setTimeout', 'setInterval', 'setImmediate', 'queueMicrotask'], 'global');
// The timer modules are other routes to the same timers (review finding
// pa-4): `import { setTimeout } from 'node:timers'`, an awaited delay from
// node:timers/promises, its scheduler, and a signal that aborts itself
// after a delay. An unreferenced timer (`.unref()`) holds no handle, so only
// the call shows it.
watch(timers, ['setTimeout', 'setInterval', 'setImmediate'], 'timers');
watch(timersPromises, ['setTimeout', 'setInterval', 'setImmediate'], 'timers/promises');
watch(timersPromises.scheduler, ['wait', 'yield'], 'timers/promises.scheduler');
watch(AbortSignal, ['timeout'], 'AbortSignal');
// A wait on shared memory that settles later, with no timer and no handle.
watch(Atomics, ['waitAsync'], 'Atomics');
watch(process, ['on', 'once', 'addListener', 'prependListener', 'kill', 'exit', 'chdir', 'umask', 'nextTick'], 'process');
// A worker thread runs code of its own, which can do any of the above out of
// this thread's sight (review finding atk-8); its construction is the act.
const OriginalWorker = workerThreads.Worker;
workerThreads.Worker = class WatchedWorker extends OriginalWorker {
  constructor(...args) {
    calls.push(`worker_threads.Worker(${typeof args[0] === 'string' ? args[0].slice(0, 80) : ''})`);
    super(...args);
  }
};
// Named imports of the built-in modules (`import { spawnSync } from
// 'node:child_process'`) see the replacements only after this.
module.syncBuiltinESMExports();

handlesBefore = process.getActiveResourcesInfo().length;
envBefore = JSON.stringify(process.env);
sharedBefore = shared();
listenersBefore = processListeners();
try {
  const { loadScenarios } = await import('./scenario-loader.mjs');
  const names = [];
  for (let i = 0; i < pairs.length; i += 2) {
    for (const scenario of await loadScenarios(pairs[i], { prefix: pairs[i + 1] ?? '' })) names.push(scenario.name);
  }
  // What the import left running, counted as soon as it has returned.
  newHandles = process.getActiveResourcesInfo().length - handlesBefore;
  loaded = names;
} catch (err) {
  failed = String(err?.message ?? err);
}
