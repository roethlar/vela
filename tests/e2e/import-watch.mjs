// Importing a scenario must do nothing: the runner imports every one of them
// before it runs any. tests/e2e/import-check.mjs runs this file as a process
// of its own, `node --permission-audit --allow-fs-read=* import-watch.mjs
// <dir> <prefix> [<dir> <prefix> ...]`, with a secret on its standard input
// and a pipe of its own on descriptor 3. It imports the scenarios of each
// directory through the runner's loader, under three nets, and reports what
// they saw on descriptor 3, as one line: the secret, a space, and JSON.
//
// The nets look where Node itself sees an act, not at a list of functions a
// module might call (review finding pa-4: a list of routes always misses one,
// and a replaced function is not the same function):
//  1. Every async resource Node creates from before the loader is imported to
//     the report is recorded by its type (node:async_hooks). A timer, an
//     immediate, a put-off callback, a socket, a file watcher, a DNS query,
//     a child's pipe: each is a resource, whatever JavaScript route made it.
//  2. Node's permission system runs in audit mode, with reading allowed: every
//     other check it makes (writing a file by path, a child process, a worker
//     thread, the network and DNS, an addon, WASI, the inspector) is
//     published on a diagnostics channel, which this file listens to. The
//     check is made inside Node, under any function a module can reach, and
//     an attempt inside a try/catch is published like any other. Audit mode
//     lets the act go ahead, so an import does here what it does in the
//     runner.
//  3. The few acts neither sees (a write to a descriptor already open, a
//     signal sent, the working directory or umask changed, a wait on shared
//     memory) go through recording replacements that keep everything the
//     original has (import-watch-record.mjs). The process's listeners, the
//     environment, the global object's names and the built-in prototypes are
//     compared at the end.
//
// The nets are never taken down. The report is written only when the event
// loop has nothing left to do ('beforeExit'), so an act an import puts off
// runs under them. A process that never gets there, or ends any other way,
// makes no report, and import-check.mjs fails it.
//
// Only Node's own modules, and the replacement helper, which imports nothing,
// are imported statically: the loader and the scenarios come after the nets.
import asyncHooks from 'node:async_hooks';
import diagnosticsChannel from 'node:diagnostics_channel';
import fs from 'node:fs';
import module from 'node:module';
import { instrument } from './import-watch-record.mjs';

// The secret that marks this run's report, read before anything else is
// imported, and kept in this module's scope only. The parent writes it and
// closes the pipe.
const readSecret = () => {
  const buffer = Buffer.alloc(256);
  let text = '';
  for (;;) {
    let n;
    try {
      n = fs.readSync(0, buffer);
    } catch (err) {
      if (err.code === 'EAGAIN') continue;
      throw err;
    }
    if (n === 0) break;
    text += buffer.toString('utf8', 0, n);
  }
  return text.trim();
};
const secret = readSecret();
if (!/^[0-9a-f]{64}$/.test(secret)) {
  fs.writeSync(2, 'import-watch: no secret on standard input\n');
  process.exit(2);
}
// Taken before the nets: how the report is written.
const writeSync = fs.writeSync;
const report = (value) => writeSync(3, `${secret} ${JSON.stringify(value)}\n`);
const pairs = process.argv.slice(2);

// Node's assert module, which the scenarios import, makes the process's
// standard-error stream when it is loaded, and that stream is a resource of
// its own (a PIPEWRAP when the parent reads it through a pipe). It is made
// here, before the first net, so that it is not counted against them. A
// write to it is still a resource (a TickObject), and standard output is
// left to be made by whoever first prints.
void process.stderr;

// Net 1. The only type of resource that importing the real scenarios makes
// (measured: about 4,000 of them, and nothing else, once the error stream
// exists) is a promise: every import is one, and every async function call.
// A promise acts on nothing; what one is put off to do is an act the nets
// see when it runs. Any other type is reported, Timeout and Immediate first
// among them, and TickObject and Microtask too: neither the loader nor any
// scenario puts a callback off with process.nextTick or queueMicrotask.
const ALLOWED_RESOURCES = new Set(['PROMISE']);
const resources = [];
const hook = asyncHooks.createHook({
  init(_id, type) {
    if (!ALLOWED_RESOURCES.has(type) && !resources.includes(type)) resources.push(type);
  },
});
hook.enable();

// Net 2.
const permission = [];
for (const scope of ['fs', 'net', 'child', 'worker', 'addon', 'wasi', 'ffi', 'inspector', 'openssl-store']) {
  diagnosticsChannel.subscribe(`node:permission-model:${scope}`, (message) => {
    permission.push(`${scope} ${message?.permission} ${message?.resource}`);
  });
}

// Net 3.
const calls = [];
const record = (label, args) => {
  const first = args[0];
  calls.push(`${label}(${typeof first === 'string' || typeof first === 'number' ? first : ''})`);
};
// Calls on a descriptor already open are not checked by the permission
// system (its documentation says so; fchmod, fchown, futimes, fsync and
// fdatasync are refused outright there, with nothing published, so a refusal
// caught by a module would not be seen either). Standard output and error,
// and the report's own descriptor, are open from the start.
const FD_CALLS = ['write', 'writev', 'ftruncate', 'fchmod', 'fchown', 'futimes', 'fsync', 'fdatasync'];
instrument(fs, [...FD_CALLS, ...FD_CALLS.map((name) => `${name}Sync`)], 'fs', record);
// Signals (process.kill calls process._kill, which a module can call itself;
// _debugProcess signals another process), the working directory and umask.
instrument(process, ['kill', '_kill', '_debugProcess', 'chdir', 'umask'], 'process', record);
// A wait on shared memory settles later through the engine alone: no
// resource, no check.
instrument(Atomics, ['waitAsync'], 'Atomics', record);
// Named imports of the built-in modules (`import { writeSync } from
// 'node:fs'`) see the replacements only after this.
module.syncBuiltinESMExports();

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
// Every listener on the process, by event: a signal handler can be added
// through process.on or through EventEmitter.prototype.on called on the
// process, so the listeners themselves are compared, whatever the route.
const processListeners = () => new Map(process.eventNames().map((event) => [event, process.rawListeners(event)]));

let loaded = null;
let newHandles = null;
let failed = null;
let reported = false;
// `on`, not `once`: a once-listener is removed before it runs, which would
// read as a listener the import took away.
process.on('beforeExit', () => {
  if (reported) return;
  reported = true;
  const seen = { calls, permission, resources };
  if (failed !== null) return report({ error: failed, ...seen });
  if (loaded === null) return report({ error: 'the import never finished: something it awaited never settled', ...seen });
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
  report({ loaded, ...seen, newHandles, envChanged: JSON.stringify(process.env) !== envBefore, sharedChanged });
});

const handlesBefore = process.getActiveResourcesInfo().length;
const envBefore = JSON.stringify(process.env);
const sharedBefore = shared();
const listenersBefore = processListeners();
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
