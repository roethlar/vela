// The parity guard of the two players (.agents/plans/embedded-player.md,
// "Slice 6"; tests/e2e/README.md, "Parity"). Every row of the behaviour
// inventory (tests/e2e/parity.mjs) must be shown by a scenario in each player,
// or carry a recorded exemption there; the scenarios say what they show on
// their own default export, read here through the loader the runner uses.
// Nothing reads JavaScript source: the scenarios are imported.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { importRefusal, importWatched, runBounded, stopUnlessCleared } from './e2e/import-check.mjs';
import { recording } from './e2e/import-watch-record.mjs';
import { EXEMPTIONS, INVENTORY, KNOWN_FAILURES, PLAYERS, SPLITS } from './e2e/parity.mjs';
import { beginProof, endProof, TEST_ONLY_ROUTES, testOnlyRoute } from './e2e/proof.mjs';
import {
  coverage,
  inventoryProblems,
  loadScenarios,
  parityProblems,
  planRowNumbers,
  proofProblems,
  recordedTree,
  redCoverage,
  scenarioProblems,
  testOnlyCoverage,
  verdictProblems,
} from './e2e/scenario-loader.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const scenarioDir = path.join(repoRoot, 'tests/e2e/scenarios');
const liveDir = path.join(repoRoot, 'tests/e2e/live');

// Importing a scenario must do nothing: the runner imports every one of them
// before it runs any. tests/e2e/import-watch.mjs imports them in a process of
// its own, under three nets (the async resources Node creates, the checks of
// Node's permission system, and recording replacements for the few acts
// neither sees), and reports what they saw once that process has nothing
// left to do, so that what an import puts off is seen as well as what it does
// at once (review finding pa-4). tests/e2e/import-check.mjs runs it and judges
// its report. This file imports the scenarios itself only once that report
// has cleared them: a scenario that fails the watch is never imported here,
// where it could hang the test or act in it.
const imported = await importWatched([scenarioDir, '', liveDir, 'live-']);
stopUnlessCleared(imported);
const scenarios = await loadScenarios(scenarioDir);
const live = await loadScenarios(liveDir, { prefix: 'live-' });

test('importing a scenario has no side effects', () => {
  assert.equal(imported.error, undefined, imported.error);
  // Every scenario was imported under the watch, so that an empty import
  // cannot pass.
  assert.deepEqual(imported.loaded, [...scenarios, ...live].map((s) => s.name));
  assert.deepEqual(imported.calls, [], 'a scenario module acted on the machine when it was imported');
  assert.deepEqual(imported.permission, [], 'a scenario module did what the permission system checks when it was imported');
  assert.deepEqual(imported.resources, [], 'a scenario module made a timer, a handle or a put-off callback when it was imported');
  assert.equal(imported.newHandles, 0, 'a scenario module left something running when it was imported');
  assert.equal(imported.envChanged, false, 'a scenario module changed the environment when it was imported');
  assert.deepEqual(imported.sharedChanged, [], 'a scenario module changed the global object or a built-in when it was imported');
});

// Each made-up module, `name: [imports, act, expected]`, is written to a
// directory of its own under a temporary one, never among the scenarios, and
// imported under the watch through the loader the runner uses. What the watch
// saw is listed as its calls, its permission checks (`<scope> <permission>
// <resource>`), and the types of resource it saw made (`resource <type>`);
// one of them must be `expected` or begin with it.
async function seenByTheWatch(routes) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-parity-watch-'));
  try {
    for (const [name, [imports, act, expected]] of Object.entries(routes)) {
      const one = path.join(dir, name);
      fs.mkdirSync(one);
      fs.writeFileSync(
        path.join(one, `${name}.mjs`),
        `${imports}\n${act}\nexport default { name: '${name}', rows: [], noRows: 'made up', async run() {} };\n`,
      );
      const watched = await importWatched([one, '']);
      assert.equal(watched.error, undefined, `${name}: ${watched.error}`);
      assert.deepEqual(watched.loaded, [name]);
      const seen = [...watched.calls, ...watched.permission, ...watched.resources.map((type) => `resource ${type}`)];
      assert.ok(
        seen.some((entry) => (expected instanceof RegExp ? expected.test(entry) : entry === expected || entry.startsWith(`${expected} `))),
        `${name}: ${JSON.stringify(seen)} should include ${expected}`,
      );
      assert.notEqual(importRefusal(watched), null, `${name}: the report cleared it`);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

// What a module writes, at a path of its own directory.
const writeBeside = "fs.writeFileSync(new URL('./written.txt', import.meta.url), 'x')";

// Each route to a timer, taken by a made-up scenario module at import, must be
// seen by the watch (review finding pa-4). A timer is a resource of Node's,
// whichever function made it, and an unreferenced one is made all the same.
test('the import watch sees a timer started by any route, and what an import puts off', async () => {
  await seenByTheWatch({
    timerssettimeout: ["import { setTimeout } from 'node:timers';", 'setTimeout(() => {}, 1);', 'resource Timeout'],
    timersunref: ["import { setTimeout } from 'node:timers';", 'setTimeout(() => {}, 60000).unref();', 'resource Timeout'],
    timersdefault: ["import timers from 'node:timers';", 'timers.setTimeout(() => {}, 60000).unref();', 'resource Timeout'],
    timersrequire: [
      "import { createRequire } from 'node:module';",
      "createRequire(import.meta.url)('node:timers').setTimeout(() => {}, 60000).unref();",
      'resource Timeout',
    ],
    timerssetinterval: [
      "import { setInterval, clearInterval } from 'node:timers';",
      'clearInterval(setInterval(() => {}, 60000));',
      'resource Timeout',
    ],
    timerssetimmediate: ["import { setImmediate } from 'node:timers';", 'setImmediate(() => {});', 'resource Immediate'],
    promisesdelay: ["import { setTimeout as delay } from 'node:timers/promises';", 'await delay(1);', 'resource Timeout'],
    promisesoftimers: ["import { promises } from 'node:timers';", 'await promises.setTimeout(1);', 'resource Timeout'],
    promisesunawaited: [
      "import { setTimeout as delay } from 'node:timers/promises';",
      'delay(60000, undefined, { ref: false }).catch(() => {});',
      'resource Timeout',
    ],
    promisessetinterval: [
      "import { setInterval } from 'node:timers/promises';",
      'const ticks = setInterval(1); await ticks.next(); await ticks.return();',
      'resource Timeout',
    ],
    promisessetimmediate: ["import { setImmediate } from 'node:timers/promises';", 'await setImmediate();', 'resource Immediate'],
    schedulerwait: ["import { scheduler } from 'node:timers/promises';", 'await scheduler.wait(1);', 'resource Timeout'],
    scheduleryield: ["import { scheduler } from 'node:timers/promises';", 'await scheduler.yield();', 'resource Immediate'],
    globalunref: ['', 'setTimeout(() => {}, 60000).unref();', 'resource Timeout'],
    globalsetinterval: ['', 'clearInterval(setInterval(() => {}, 60000));', 'resource Timeout'],
    globalsetimmediate: ['', 'setImmediate(() => {});', 'resource Immediate'],
    aborttimeout: ['', 'AbortSignal.timeout(60000);', 'resource Timeout'],
    // Callbacks put off without a timer, which run after the import too.
    nexttick: ['', 'process.nextTick(() => {});', 'resource TickObject'],
    microtask: ['', 'queueMicrotask(() => {});', 'resource Microtask'],
    // A wait on shared memory, which settles later with no timer and no handle.
    atomicswaitasync: ['', 'Atomics.waitAsync(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1);', 'Atomics.waitAsync()'],
    // An act put off through a route that is no act (reading a file), which
    // runs after the import has returned: the watch must still be up then.
    // Two reads in a row put it well after the import.
    deferredwrite: [
      "import fs from 'node:fs';",
      `const self = new URL(import.meta.url);\nfs.promises.readFile(self).then(() => fs.promises.readFile(self)).then(() => ${writeBeside});`,
      'fs FileSystemWrite',
    ],
  });
});

// The reviewer's demonstrations against the first fix (review finding pa-4,
// verification round 1), as near to verbatim as a module can put them: a
// timer through the scheduler's prototype, and through setTimeout's own
// promisified form, each writing a file when it fires. Neither was seen,
// because only named functions were replaced, and the replacement of
// setTimeout had lost `util.promisify.custom`, which changed what the import
// did. Now the timer is seen as made, and the promisified form still works.
test("the import watch sees the reviewer's timer routes, and changes nothing they do", async () => {
  await seenByTheWatch({
    prototypewait: [
      "import fs from 'node:fs';\nimport { scheduler } from 'node:timers/promises';",
      `Object.getPrototypeOf(scheduler).wait.call(scheduler, 500, { ref: false }).then(() => ${writeBeside});`,
      'resource Timeout',
    ],
    promisifycustom: [
      "import fs from 'node:fs';\nimport { promisify } from 'node:util';",
      `setTimeout[promisify.custom]?.(300, undefined, { ref: false }).then(() => ${writeBeside});`,
      'resource Timeout',
    ],
    // Awaited at import, so that the import fails (and the probe with it)
    // unless the promisified setTimeout resolves under the watch as it does
    // in the runner.
    promisifyworks: [
      "import { promisify } from 'node:util';",
      "if (setTimeout[promisify.custom] === undefined) throw new Error('setTimeout has no promisify.custom');\n" +
        "if ((await promisify(setTimeout)(1, 'v')) !== 'v') throw new Error('promisify(setTimeout) did not resolve to its value');",
      'resource Timeout',
    ],
    // A function the watch does replace keeps what promisify reads from it:
    // fs.write resolves to an object of two fields, which only Node's own
    // symbol on the function gives.
    replacedkeepsproperties: [
      "import fs from 'node:fs';\nimport { promisify } from 'node:util';",
      "const r = await promisify(fs.write)(2, '');\n" +
        "if (r === null || typeof r !== 'object' || !('buffer' in r)) throw new Error(`promisify(fs.write) gave ${JSON.stringify(r)}`);\n" +
        "if (fs.write.name !== 'write') throw new Error(`fs.write is named ${fs.write.name}`);",
      'fs.write(2)',
    ],
  });
});

// Acts reached other than through the module functions the watch first
// covered, each found unseen once (review finding pa-4): a signal handler
// added through the EventEmitter prototype, a DNS query, a file watcher that
// holds no handle, and a socket, server or child made with `new`; and acts
// that an error stops or a try/catch hides. Every address is this machine's,
// and each is closed or cancelled at once. Each probe takes a refusal as well
// as a success: where no socket is allowed (a sandbox answers EPERM to a
// listen on this machine's own address) the probe still ends and the attempt
// is seen, since the permission system's check comes before the attempt.
test('the import watch sees a signal handler, a DNS query, a file watcher, and a socket or child made with new', async () => {
  await seenByTheWatch({
    prototypelistener: [
      "import { EventEmitter } from 'node:events';",
      "EventEmitter.prototype.on.call(process, 'SIGUSR2', () => {});",
      'process listeners of SIGUSR2 changed',
    ],
    processon: ['', "process.on('SIGUSR2', () => {});", 'process listeners of SIGUSR2 changed'],
    dnslookup: ["import dns from 'node:dns';", "dns.lookup('localhost', () => {});", 'net Net localhost'],
    dnspromises: ["import dns from 'node:dns';", "await dns.promises.lookup('localhost').catch(() => {});", 'net Net localhost'],
    dnsresolver: [
      "import { Resolver } from 'node:dns';",
      "const r = new Resolver(); r.setServers(['127.0.0.1']); r.resolve4('vela.invalid', () => {}); r.cancel();",
      'net Net vela.invalid',
    ],
    dnspromisesresolver: [
      "import dns from 'node:dns';",
      "const r = new dns.promises.Resolver(); r.setServers(['127.0.0.1']); const q = r.resolve4('vela.invalid').catch(() => {}); r.cancel(); await q;",
      'net Net vela.invalid',
    ],
    watchfile: [
      "import fs from 'node:fs';",
      'const self = new URL(import.meta.url); fs.watchFile(self, { persistent: false }, () => {}); fs.unwatchFile(self);',
      'resource STATWATCHER',
    ],
    fswatch: ["import fs from 'node:fs';", 'try { fs.watch(new URL(import.meta.url), { persistent: false }).close(); } catch {}', 'resource FSEVENTWRAP'],
    socketnew: [
      "import net from 'node:net';",
      "const s = new net.Socket(); s.on('error', () => {}); s.connect(9, '127.0.0.1'); s.destroy();",
      // Destroyed before it got to connect: the socket was made all the same.
      'resource TCPWRAP',
    ],
    servernew: [
      "import net from 'node:net';",
      "const s = new net.Server(); await new Promise((r) => { s.on('error', r); s.listen(0, '127.0.0.1', r); }); s.close();",
      // A listen is checked with no resource named.
      'net Net',
    ],
    udpnew: [
      "import dgram from 'node:dgram';",
      "const s = new dgram.Socket('udp4'); await new Promise((r) => { s.on('error', r); s.bind(0, '127.0.0.1', r); }); s.close();",
      'net Net 127.0.0.1',
    ],
    childnew: [
      "import { ChildProcess } from 'node:child_process';",
      "const c = new ChildProcess(); c.on('error', () => {}); try { c.spawn({ file: '/nonexistent/vela-probe', args: ['/nonexistent/vela-probe'], envPairs: [], stdio: 'ignore' }); } catch {}",
      'child ChildProcess',
    ],
    // A write that fails (its directory does not exist), inside a try/catch:
    // the permission system's check comes first, and is seen.
    caughtwrite: ["import fs from 'node:fs';", "try { fs.writeFileSync('/nonexistent/vela-probe/x', 'x'); } catch {}", 'fs FileSystemWrite /nonexistent/vela-probe/x'],
    worker: [
      "import { Worker } from 'node:worker_threads';",
      "await new Worker('', { eval: true }).terminate();",
      'worker WorkerThreads',
    ],
    // A signal, to this process, of a kind that does nothing.
    signal: ['', 'process.kill(process.pid, 0);', /^process\.kill\(\d+\)$/],
    // A write to a descriptor that is already open.
    fdwrite: ["import { writeSync } from 'node:fs';", "writeSync(2, '');", 'fs.writeSync(2)'],
  });
});

// The report is taken only from the watch's own channel, with this run's
// secret, from a process that ended by itself with status 0 (review finding
// pa-4, verification round 1: a module printed a clean report and ended the
// process, and passed). Each module below writes a file, which the watch
// would see, and then tries to keep it from reporting.
test('a module that forges the report, ends the process, or takes the report away fails', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-parity-watch-'));
  // A clean report as the watch writes it, on standard output.
  const forged = (name) =>
    `process.stdout.write(${JSON.stringify(
      JSON.stringify({ loaded: [name], calls: [], permission: [], resources: [], newHandles: 0, envChanged: false, sharedChanged: [] }),
    )} + '\\n');`;
  const cases = {
    exitseven: [`${forged('exitseven')}\nprocess.exit(7);`, /^the watch's process ended with status 7, not 0/],
    exitzero: [`${forged('exitzero')}\nprocess.exit(0);`, /^the watch's process ended \(status 0\) without making its report: .*process\.exit.*beforeExit listener/],
    nobeforeexit: [`${forged('nobeforeexit')}\nprocess.removeAllListeners('beforeExit');`, /^the watch's process ended \(status 0\) without making its report/],
    // On the report's own descriptor, without the secret.
    forgedchannel: [
      `fs.writeSync(3, 'not-the-secret ' + ${JSON.stringify(JSON.stringify({ loaded: ['forgedchannel'], calls: [] }))} + '\\n');`,
      /^the watch's report channel held something other than one report carrying this run's secret/,
    ],
    // The one line there, shaped as the watch writes it, with a made-up
    // secret, and the process ended before the watch could add its own.
    forgedsecret: [
      `fs.writeSync(3, '0'.repeat(64) + ' ' + ${JSON.stringify(
        JSON.stringify({ loaded: ['forgedsecret'], calls: [], permission: [], resources: [], newHandles: 0, envChanged: false, sharedChanged: [] }),
      )} + '\\n');\nprocess.exit(0);`,
      /^the watch's report channel held something other than one report carrying this run's secret/,
    ],
    // An exit status left for the end, after the report.
    exitcode: ['process.exitCode = 3;', /^the watch's process ended with status 3, not 0/],
    // Ended by a signal it sends itself.
    signalled: ["process.kill(process.pid, 'SIGTERM');", /^the watch's process was ended by the signal SIGTERM/],
  };
  try {
    for (const [name, [act, expected]] of Object.entries(cases)) {
      const one = path.join(dir, name);
      fs.mkdirSync(one);
      fs.writeFileSync(
        path.join(one, `${name}.mjs`),
        `import fs from 'node:fs';\n${writeBeside};\n${act}\nexport default { name: '${name}', rows: [], noRows: 'made up', async run() {} };\n`,
      );
      const watched = await importWatched([one, '']);
      assert.match(watched.error ?? '', expected, `${name}: ${JSON.stringify(watched)}`);
      assert.match(importRefusal(watched) ?? '', /^the watched import failed: /, name);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// The watch reports only once its process has nothing left to do; an import
// that keeps it busy for good must fail, not hang the test or pass. The
// process is killed with SIGKILL at the deadline, so one that ignores SIGTERM
// or has stopped itself fails as soon (review finding pa-4, verification
// round 1: both outlived a deadline that sent SIGTERM).
test('an import that keeps its process busy for good, or stops it, fails at the deadline', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-parity-watch-'));
  const deadline = 1000;
  const cases = {
    busy: 'setInterval(() => {}, 1000);',
    ignoresterm: "process.on('SIGTERM', () => {});\nconst until = Date.now() + 60000;\nwhile (Date.now() < until);",
    stopsitself: "process.kill(process.pid, 'SIGSTOP');",
  };
  try {
    for (const [name, act] of Object.entries(cases)) {
      const one = path.join(dir, name);
      fs.mkdirSync(one);
      fs.writeFileSync(path.join(one, `${name}.mjs`), `${act}\nexport default { name: '${name}', rows: [], noRows: 'made up', async run() {} };\n`);
      const started = Date.now();
      const watched = await importWatched([one, ''], { deadline });
      const took = Date.now() - started;
      assert.match(watched.error ?? '', /^the import never let its process end: it was still running after 1000 ms and was killed \(SIGKILL\)/, name);
      assert.ok(took < deadline + 2000, `${name}: the failure took ${took} ms, past the deadline of ${deadline} ms`);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// A process an import starts in a group of its own is not killed with the
// watch's, and holds the pipes the check reads for as long as it lives. The
// check must say so a moment after the deadline, not wait for it to end.
// (The process started here ends by itself eight seconds on.)
test('a process the import starts outside its group, holding the pipes, does not hang the check', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-parity-watch-'));
  const deadline = 1000;
  try {
    fs.writeFileSync(
      path.join(dir, 'holdspipes.mjs'),
      "import { spawn } from 'node:child_process';\n" +
        "spawn(process.execPath, ['-e', 'setTimeout(() => {}, 8000)'], { detached: true, stdio: 'inherit' }).unref();\n" +
        "export default { name: 'holdspipes', rows: [], noRows: 'made up', async run() {} };\n",
    );
    const started = Date.now();
    const watched = await importWatched([dir, ''], { deadline });
    const took = Date.now() - started;
    assert.match(
      watched.error ?? '',
      /^the watch's process ended, but something the import started still held its pipes 1000 ms after the start/,
    );
    assert.ok(took < deadline + 2000, `the failure took ${took} ms, past the deadline of ${deadline} ms`);
    assert.match(importRefusal(watched) ?? '', /^the watched import failed: the watch's process ended, but something/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// This file's own top: the scenarios are imported into this process only
// after the watched import of the same two directories has cleared them.
// The test further down runs a copy of that arrangement; this holds the
// arrangement itself.
test('this file imports the scenarios only after the watched import has cleared them', () => {
  const own = fs.readFileSync(fileURLToPath(import.meta.url), 'utf8');
  const cleared = own.indexOf('\nstopUnlessCleared(imported);\n');
  const firstImport = own.search(/\bawait loadScenarios\(/);
  assert.ok(cleared > 0, 'the top of the file stops unless the watched import cleared the scenarios');
  assert.ok(firstImport > cleared, 'no scenario is imported before that');
  assert.ok(
    own.slice(0, cleared).endsWith("\nconst imported = await importWatched([scenarioDir, '', liveDir, 'live-']);"),
    'what is cleared is the watched import of the two scenario directories',
  );
  assert.equal(own.split('\nstopUnlessCleared(imported);\n').length, 2, 'and it is asked once, there');
});

// The decision this file takes before it imports any scenario itself, on
// made-up reports: only a report of an import that loaded something and saw
// nothing clears it.
test('the scenarios are imported here only on a clean report of the watched import', () => {
  const clean = { loaded: ['a'], calls: [], permission: [], resources: [], newHandles: 0, envChanged: false, sharedChanged: [] };
  assert.equal(importRefusal(clean), null);
  assert.equal(stopUnlessCleared(clean), clean);
  const refused = (report, pattern) => {
    assert.match(importRefusal(report) ?? '', pattern, JSON.stringify(report));
    const because = 'no scenario was imported into this process, because ';
    assert.throws(
      () => stopUnlessCleared(report),
      (err) => err.message.startsWith(because) && pattern.test(err.message.slice(because.length)),
    );
  };
  refused({ error: 'the import never let its process end: x' }, /^the watched import failed: the import never let its process end: x$/);
  refused({ ...clean, error: 'boom' }, /^the watched import failed: boom$/);
  refused(null, /^the watch made no report/);
  refused({ ...clean, loaded: [] }, /it loaded no scenario/);
  refused({ ...clean, loaded: undefined }, /it loaded no scenario/);
  refused({ ...clean, calls: ['fs.writeSync(2)'] }, /it acted on the machine: fs\.writeSync\(2\)/);
  refused({ ...clean, permission: ['fs FileSystemWrite /x'] }, /it did what the permission system checks: fs FileSystemWrite \/x/);
  refused({ ...clean, resources: ['Timeout'] }, /it made async resources other than promises: Timeout/);
  refused({ ...clean, resources: undefined }, /its report has no list of resources/);
  refused({ ...clean, sharedChanged: ['globalThis'] }, /changed the global object or a built-in: globalThis/);
  refused({ ...clean, newHandles: 1 }, /it left 1 handles open/);
  refused({ ...clean, envChanged: true }, /it changed the environment/);
  refused({ ...clean, envChanged: undefined }, /it changed the environment/);
});

// The same arrangement as the top of this file, run in a process of its own
// over a made-up scenario that keeps its process busy for good (the
// reviewer's injection, a referenced setInterval): it must stop at the
// refusal, without importing the scenario, and end, not hang.
test('a watched import that fails stops the file before it imports any scenario', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-parity-watch-'));
  try {
    const scenariosHere = path.join(dir, 'scenarios');
    fs.mkdirSync(scenariosHere);
    fs.writeFileSync(
      path.join(scenariosHere, 'busy.mjs'),
      "setInterval(() => {}, 1000);\nexport default { name: 'busy', rows: [], noRows: 'made up', async run() {} };\n",
    );
    const url = (file) => JSON.stringify(new URL(file, import.meta.url).href);
    fs.writeFileSync(
      path.join(dir, 'arrangement.mjs'),
      `import { importWatched, stopUnlessCleared } from ${url('./e2e/import-check.mjs')};\n` +
        `import { loadScenarios } from ${url('./e2e/scenario-loader.mjs')};\n` +
        `const imported = await importWatched([${JSON.stringify(scenariosHere)}, ''], { deadline: 1000 });\n` +
        'stopUnlessCleared(imported);\n' +
        `await loadScenarios(${JSON.stringify(scenariosHere)});\n` +
        "console.log('the scenarios were imported into this process');\n",
    );
    const run = await runBounded([path.join(dir, 'arrangement.mjs')], { deadline: 15000 });
    assert.equal(run.killed, false, `the arrangement hung until it was killed after ${run.ms} ms: ${run.stdout}${run.stderr}`);
    assert.equal(run.status, 1, run.stderr);
    assert.match(run.stderr, /no scenario was imported into this process, because the watched import failed: the import never let its process end/);
    assert.doesNotMatch(run.stdout, /imported into this process/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// The recording replacement keeps what the function is: its name, its length,
// its prototype chain, every own property (symbols too), a function-valued
// property replaced by one that records and calls the original, and `new`.
test('a recording replacement keeps everything the original function has', () => {
  const custom = Symbol('custom');
  function original(a, b) {
    return this?.k ?? a + b;
  }
  const helper = (x) => x * 2;
  original[custom] = helper;
  original.data = [1];
  Object.defineProperty(original, 'hidden', { value: 'h', enumerable: false, writable: false, configurable: true });
  original.prototype.method = function () {
    return 'm';
  };
  const calls = [];
  const watched = recording(original, 'x.original', (label, args) => calls.push(`${label}:${args.join(',')}`));
  assert.equal(watched.name, 'original');
  assert.equal(watched.length, 2);
  assert.deepEqual(Reflect.ownKeys(watched).sort((a, b) => String(a).localeCompare(String(b))), Reflect.ownKeys(original).sort((a, b) => String(a).localeCompare(String(b))));
  assert.equal(watched.data, original.data);
  assert.deepEqual(Object.getOwnPropertyDescriptor(watched, 'hidden'), Object.getOwnPropertyDescriptor(original, 'hidden'));
  assert.equal(Object.getPrototypeOf(watched), Object.getPrototypeOf(original));
  assert.equal(watched(1, 2), 3);
  assert.equal(watched.call({ k: 'this' }, 1, 2), 'this');
  assert.equal(watched[custom](4), 8);
  assert.notEqual(watched[custom], helper);
  assert.equal(new watched(1, 2).method(), 'm');
  assert.ok(new watched(1, 2) instanceof original);
  assert.deepEqual(calls, ['x.original:1,2', 'x.original:1,2', 'x.original.Symbol(custom):4', 'x.original:1,2', 'x.original:1,2']);
  // A function without a prototype of its own gets a replacement without one.
  const arrow = recording(() => 1, 'arrow', () => {});
  assert.equal(Object.hasOwn(arrow, 'prototype'), false);
});

test('every scenario says which rows it stands for, in a way the loader understands', () => {
  // loadScenarios has refused anything it does not understand; this names
  // the counts so that an empty directory cannot pass.
  assert.ok(scenarios.length >= 40, `only ${scenarios.length} scenarios were loaded`);
  assert.equal(live.length, 3);
  for (const scenario of [...scenarios, ...live]) assert.ok(Array.isArray(scenario.rows), scenario.name);
});

test('every row is shown in both players, or exempted where it is not', (t) => {
  // What stands in for a scenario, listed on every run so that nobody takes
  // it for coverage.
  for (const e of EXEMPTIONS) {
    t.diagnostic(
      `EXEMPTION row ${e.row} (${INVENTORY.find((r) => r.id === e.row)?.behaviour}) in the ${e.player} player: ${e.reason}` +
        (e.heldBy ? ` Held by: ${e.heldBy.map((h) => `${h.file} (${h.tests.join(', ')})`).join('; ')}.` : '') +
        (e.ownerStep ? ` Owner's step: ${e.ownerStep}` : ''),
    );
  }
  for (const s of scenarios) {
    for (const r of s.testOnly ?? []) {
      t.diagnostic(
        `TEST-ONLY ROUTE in ${s.name}, row ${r.row}, ${r.player} player (via ${r.via}): ${r.route}. Proves ${r.proves}. Does not prove ${r.unproven}.`,
      );
    }
  }
  for (const k of KNOWN_FAILURES) {
    t.diagnostic(
      `KNOWN PRODUCT FAILURE: ${k.scenario} fails in the ${k.player} player with "${k.failsWith}", and shows none of its rows there. ${k.defect}`,
    );
  }
  for (const row of INVENTORY) {
    t.diagnostic(
      `row ${row.id}: ${PLAYERS.map((p) => {
        const shownBy = coverage(scenarios, row.id, p).join(', ');
        const red = redCoverage(scenarios, row.id, p);
        const exempted = EXEMPTIONS.some((e) => e.row === row.id && e.player === p);
        const redNote = red.length > 0 ? ` [RED, not shown: ${red.join(', ')} fails on a product defect]` : '';
        const onlyTestOnly = shownBy && testOnlyCoverage(scenarios, row.id, p).length === coverage(scenarios, row.id, p).length;
        if (shownBy) return `${p} ${shownBy}${onlyTestOnly ? ' [ONLY VIA TEST-ONLY ROUTES]' : ''}${redNote}`;
        if (red.length > 0) return `${p} NOT SHOWN${redNote}`;
        return `${p} ${exempted ? '(exempted)' : '(NOTHING)'}`;
      }).join('; ')}`,
    );
  }
  t.diagnostic(
    `not counted (opt-in live venue, part of neither run): ${live.map((s) => `${s.name} [${s.rows.join(', ')}]`).join(', ')}`,
  );
  assert.deepEqual(parityProblems(scenarios, { repoRoot }), []);
});

// The inventory is the plan's, not whatever parity.mjs says: dropping an
// inconvenient row from it (and its exemptions) would otherwise pass.
test("the inventory holds every row of the plan's behaviour inventory, and nothing else", () => {
  const plan = fs.readFileSync(path.join(repoRoot, '.agents/plans/embedded-player.md'), 'utf8');
  const numbers = planRowNumbers(plan);
  assert.deepEqual(numbers, ['1', '2', '3', '4', '5', '6', '7', '8', '9', '10', '11', '12', '13', '14']);
  assert.deepEqual(inventoryProblems(INVENTORY, numbers), []);
});

// The list of rows itself, pinned here: dropping or adding a row, or a part of
// a split one, takes a change to this literal as well as to parity.mjs and the
// plan (review finding pa-3).
test('the inventory is exactly these rows, split exactly so', () => {
  assert.deepEqual(
    INVENTORY.map((row) => row.id),
    ['1', '2', '3', '4', '5', '6', '7', '8a', '8b', '9a', '9b', '9c', '10', '11', '12a', '12b', '13a', '13b', '14'],
  );
  assert.deepEqual(SPLITS, { 8: ['8a', '8b'], 9: ['9a', '9b', '9c'], 12: ['12a', '12b'], 13: ['13a', '13b'] });
});

// Row 9 is split by mode, so that a mode one player's scenario does not
// exercise is a part not shown there (review finding pa-2). The scenarios'
// declarations are copied, never edited: with the claim of 9a (Off) taken
// from the one scenario that shows it in a player, the parity check must name
// that row and player.
const withoutClaim = (name, row) => {
  const one = scenarios.find((s) => s.name === name);
  assert.ok(one?.rows.includes(row), `${name} claims row ${row}`);
  return scenarios.map((s) => (s === one ? { ...s, rows: s.rows.filter((r) => r !== row) } : s));
};

test("each mode of row 9 is a part of its own: autocrop's Off taken away leaves 9a unshown in the external player", () => {
  assert.deepEqual(parityProblems(withoutClaim('autocrop', '9a'), { repoRoot }), [
    'row 9a is shown in the embedded player but neither shown nor exempted in the external player',
  ]);
});

test("each mode of row 9 is a part of its own: embeddedcrop's Off taken away leaves 9a unshown in the embedded player", () => {
  assert.deepEqual(parityProblems(withoutClaim('embeddedcrop', '9a'), { repoRoot }), [
    'row 9a is shown in the external player but neither shown nor exempted in the embedded player',
  ]);
});

test('a mode of row 9 dropped from the inventory, or row 9 made whole again, is refused', () => {
  const numbers = planRowNumbers(fs.readFileSync(path.join(repoRoot, '.agents/plans/embedded-player.md'), 'utf8'));
  assert.deepEqual(inventoryProblems(INVENTORY.filter((row) => row.id !== '9a'), numbers), [
    'part 9a of split plan row 9 is not in the inventory',
  ]);
  const whole = INVENTORY.flatMap((row) =>
    row.id === '9a' ? [{ id: '9', behaviour: 'Autocrop off / manual / auto' }] : row.id.startsWith('9') ? [] : [row],
  );
  assert.deepEqual(inventoryProblems(whole, numbers), [
    'plan row 9 is in the inventory both whole and split (its parts are 9a, 9b, 9c)',
    'part 9a of split plan row 9 is not in the inventory',
    'part 9b of split plan row 9 is not in the inventory',
    'part 9c of split plan row 9 is not in the inventory',
  ]);
});

// The reviewer's injection: row 8b (the skip button) deleted from the
// inventory together with the scenarios that claim it. Every other check
// still passes on that; the inventory check must not (review finding pa-3).
test('half of a split row deleted with its scenarios is refused', () => {
  const plan = fs.readFileSync(path.join(repoRoot, '.agents/plans/embedded-player.md'), 'utf8');
  const without8b = INVENTORY.filter((row) => row.id !== '8b');
  const notClaiming8b = scenarios.filter((s) => !s.rows.includes('8b'));
  assert.ok(notClaiming8b.length < scenarios.length, 'no scenario claims row 8b');
  assert.deepEqual(parityProblems(notClaiming8b, { inventory: without8b, repoRoot }), []);
  assert.deepEqual(inventoryProblems(without8b, planRowNumbers(plan)), [
    'part 8b of split plan row 8 is not in the inventory',
  ]);
});

test('the inventory check refuses a dropped, a made-up and a half-split row', () => {
  const plan = ['1', '2', '3'];
  const rows = (...ids) => ids.map((id) => ({ id }));
  const split2 = { 2: ['2a', '2b'] };
  assert.deepEqual(inventoryProblems(rows('1', '2a', '2b', '3'), plan, split2), []);
  assert.deepEqual(inventoryProblems(rows('1', '2', '3'), plan, {}), []);
  assert.match(inventoryProblems(rows('1', '3'), plan, {}).join('\n'), /plan row 2 is not in the inventory/);
  assert.match(inventoryProblems(rows('1', '2', '3', '4'), plan, {}).join('\n'), /"4" is no row of the plan/);
  assert.match(inventoryProblems(rows('1', '2', '2a', '2b', '3'), plan, split2).join('\n'), /both whole and split/);
  assert.match(inventoryProblems(rows('1', '2', '3'), [], {}).join('\n'), /table was not found/);
  // A split row with a part missing, either one, or both (review finding pa-3).
  assert.deepEqual(inventoryProblems(rows('1', '2a', '3'), plan, split2), ['part 2b of split plan row 2 is not in the inventory']);
  assert.deepEqual(inventoryProblems(rows('1', '2b', '3'), plan, split2), ['part 2a of split plan row 2 is not in the inventory']);
  assert.deepEqual(inventoryProblems(rows('1', '3'), plan, split2), [
    'part 2a of split plan row 2 is not in the inventory',
    'part 2b of split plan row 2 is not in the inventory',
  ]);
  // A part that is not approved: of an unsplit row, or a third part of a split one.
  assert.deepEqual(inventoryProblems(rows('1', '2', '3a', '3'), plan, {}), [
    'inventory row "3a" is no approved part of plan row 3 (SPLITS in parity.mjs)',
  ]);
  assert.deepEqual(inventoryProblems(rows('1', '2a', '2b', '2c', '3'), plan, split2), [
    'inventory row "2c" is no approved part of plan row 2 (SPLITS in parity.mjs)',
  ]);
  // An unsplit row in parts, which SPLITS does not approve.
  assert.deepEqual(inventoryProblems(rows('1', '2a', '2b', '3'), plan, {}), [
    'plan row 2 is not in the inventory',
    'inventory row "2a" is no approved part of plan row 2 (SPLITS in parity.mjs)',
    'inventory row "2b" is no approved part of plan row 2 (SPLITS in parity.mjs)',
  ]);
  // A split recorded for a row the plan does not have.
  assert.match(inventoryProblems(rows('1', '2', '3'), plan, { 4: ['4a', '4b'] }).join('\n'), /split row 4 is no row of the plan/);
  assert.deepEqual(
    planRowNumbers('# P\n\n## Behaviour inventory (x)\n\n| # | B |\n| --- | --- |\n| 1 | a |\n| 2 | b |\n\n## Slices\n\n| 9 | not a row |\n'),
    ['1', '2'],
  );
});

// The rules themselves, on made-up scenarios, so that a rule cannot quietly
// stop applying while the real data happens to obey it.
const ok = (extra = {}) => ({ name: 'one', rows: ['1'], async run() {}, ...extra });

test('the loader refuses a scenario it does not understand', () => {
  assert.deepEqual(scenarioProblems(ok(), 'one'), []);
  const refused = (scenario, pattern) => {
    const problems = scenarioProblems(scenario, 'one');
    assert.ok(problems.some((p) => pattern.test(p)), `${JSON.stringify(problems)} should match ${pattern}`);
  };
  refused(ok({ onlyPlayer: 'embeded', onlyPlayerWhy: 'x' }), /unknown onlyPlayer "embeded"/);
  refused(ok({ onlyPlayer: 'embedded' }), /onlyPlayer without onlyPlayerWhy/);
  refused(ok({ onlyPlayerWhy: 'x' }), /onlyPlayerWhy without onlyPlayer/);
  const { rows, ...noRowsAtAll } = ok();
  assert.ok(rows);
  refused(noRowsAtAll, /no "rows"/);
  refused(ok({ rows: '1' }), /"rows" is not an array/);
  refused(ok({ rows: [1] }), /unknown row 1/);
  refused(ok({ rows: ['15'] }), /unknown row "15"/);
  refused(ok({ rows: ['12'] }), /unknown row "12"/);
  refused(ok({ rows: ['1', '1'] }), /named twice/);
  refused(ok({ rows: [] }), /no rows and no "noRows"/);
  refused(ok({ noRows: 'x' }), /rows and "noRows" both/);
  refused(ok({ rowz: ['1'] }), /unknown key "rowz"/);
  refused(ok({ name: 'other' }), /not "one" as its file says/);
  refused(ok({ run: undefined }), /no run\(\)/);
  const route = { row: '1', player: 'embedded', via: 'clickUnderPlay', route: 'r', proves: 'p', unproven: 'u' };
  assert.deepEqual(scenarioProblems(ok({ testOnly: [route] }), 'one'), []);
  refused(ok({ testOnly: [{ ...route, row: '2' }] }), /not one of the scenario's rows/);
  refused(ok({ testOnly: [{ ...route, player: 'both' }] }), /unknown player "both"/);
  refused(ok({ onlyPlayer: 'external', onlyPlayerWhy: 'x', testOnly: [route] }), /runs only in external/);
  refused(ok({ testOnly: [{ ...route, unproven: '' }] }), /no "unproven"/);
  refused(ok({ testOnly: [] }), /not a non-empty array/);
  refused(ok({ testOnly: [{ ...route, via: 'somethingElse' }] }), /"via" is "somethingElse", not one of the test-only routes/);
  refused(ok({ testOnly: [{ ...route, via: undefined }] }), /"via" is undefined/);
  // Names every object has are no route (review finding pa-8).
  for (const inherited of ['toString', 'constructor', '__proto__', 'hasOwnProperty']) {
    refused(
      ok({ testOnly: [{ ...route, via: inherited }] }),
      new RegExp(`"via" is "${inherited}", not one of the test-only routes`),
    );
  }
});

// The recorder (proof.mjs) takes only the routes it names, and no name every
// object inherits (review finding pa-8).
test('the recorder refuses a test-only route it does not name, an inherited name included', () => {
  beginProof();
  try {
    for (const id of Object.keys(TEST_ONLY_ROUTES)) testOnlyRoute(id);
    for (const inherited of ['toString', 'constructor', '__proto__', 'hasOwnProperty', 'somethingElse']) {
      assert.throws(() => testOnlyRoute(inherited), new RegExp(`unknown test-only route "${inherited}"`));
    }
  } finally {
    assert.deepEqual(endProof().routes.sort(), Object.keys(TEST_ONLY_ROUTES).sort());
  }
});

test('the parity check refuses a one-sided row, a row with nothing, and a wrong exemption', () => {
  const inventory = [{ id: '1' }, { id: '2' }];
  const both = { name: 'both', rows: ['1', '2'] };
  const exemption = { row: '2', player: 'embedded', reason: 'why', ownerStep: 'step' };
  const check = (scenarioSet, exemptions, knownFailures = []) =>
    parityProblems(scenarioSet, { inventory, exemptions, knownFailures });
  assert.deepEqual(check([both], []), []);
  // A row shown for one player only, with nothing for the other.
  const externalOnly = { name: 'ext', onlyPlayer: 'external', rows: ['2'] };
  assert.deepEqual(check([{ name: 'a', rows: ['1'] }, externalOnly], []), [
    'row 2 is shown in the external player but neither shown nor exempted in the embedded player',
  ]);
  // ... which an exemption for that player answers.
  assert.deepEqual(check([{ name: 'a', rows: ['1'] }, externalOnly], [exemption]), []);
  // A row with nothing at all.
  assert.deepEqual(check([{ name: 'a', rows: ['1'] }], []), [
    'row 2 is neither shown nor exempted in the external player',
    'row 2 is neither shown nor exempted in the embedded player',
  ]);
  // Exemptions for what does not exist, or for what a scenario already shows.
  assert.match(check([both], [{ ...exemption, row: '3' }]).join('\n'), /no such row/);
  assert.match(check([both], [{ ...exemption, player: 'both' }]).join('\n'), /no such player/);
  assert.match(check([both], [exemption]).join('\n'), /stale, row 2 is shown in the embedded player by both/);
  assert.match(
    check([{ name: 'a', rows: ['1'] }, externalOnly], [exemption, exemption]).join('\n'),
    /recorded twice/,
  );
  assert.match(check([externalOnly, { name: 'a', rows: ['1'] }], [{ ...exemption, reason: '' }]).join('\n'), /no reason/);
  assert.match(
    check([externalOnly, { name: 'a', rows: ['1'] }], [{ row: '2', player: 'embedded', reason: 'why' }]).join('\n'),
    /names neither the tests that hold it/,
  );
  assert.match(
    parityProblems([externalOnly, { name: 'a', rows: ['1'] }], {
      inventory,
      exemptions: [{ ...exemption, heldBy: [{ file: 'no/such/file.rs', tests: ['t'] }] }],
      knownFailures: [],
      repoRoot,
    }).join('\n'),
    /no\/such\/file\.rs does not exist/,
  );
  const heldBy = (tests) =>
    parityProblems([externalOnly, { name: 'a', rows: ['1'] }], {
      inventory,
      exemptions: [{ ...exemption, heldBy: [{ file: 'src-tauri/src/automatic.rs', tests }] }],
      knownFailures: [],
      repoRoot,
    }).join('\n');
  assert.equal(heldBy(['a_sustained_drop_storm_steps_down']), '');
  assert.match(heldBy(['no_such_test_at_all']), /automatic\.rs has no test named "no_such_test_at_all"/);
  // A part of a name is not the name.
  assert.match(heldBy(['a_sustained_drop_storm']), /has no test named "a_sustained_drop_storm"/);
  assert.match(check([both, both], []).join('\n'), /two scenarios are named "both"/);
});

// What the runner checks once a scenario's run() has returned (proof.mjs).
test('a scenario must take exactly the test-only routes it declares for the player it ran in', () => {
  const route = { row: '1', player: 'embedded', via: 'clickUnderPlay', route: 'r', proves: 'p', unproven: 'u' };
  const declaring = ok({ testOnly: [route] });
  const proof = (routes) => ({ shown: ['1'], routes });
  assert.deepEqual(proofProblems(declaring, 'embedded', proof(['clickUnderPlay'])), []);
  assert.deepEqual(proofProblems(ok(), 'external', proof([])), []);
  // Taken without being declared (for that player).
  assert.match(
    proofProblems(ok(), 'embedded', proof(['clickUnderPlay'])).join('\n'),
    /took the test-only route clickUnderPlay in the embedded player without declaring it/,
  );
  assert.match(proofProblems(declaring, 'external', proof(['clickUnderPlay'])).join('\n'), /without declaring it/);
  // Declared for a player and not taken there: a declaration that lies.
  assert.match(
    proofProblems(declaring, 'embedded', proof([])).join('\n'),
    /declares the test-only route clickUnderPlay for the embedded player, but did not take it/,
  );
});

// The guard against a row claimed and not shown: a run() that returns early
// in one player, does nothing, or never gets to the row (an over-claim).
test('a scenario must reach the proof of every row it declares, and of no other', () => {
  const proof = (shown) => ({ shown, routes: [] });
  assert.deepEqual(proofProblems(ok({ rows: ['1', '2'] }), 'embedded', proof(['1', '2'])), []);
  assert.match(
    proofProblems(ok({ rows: ['1', '2'] }), 'embedded', proof(['1'])).join('\n'),
    /declares row 2 but did not reach the end of its proof in the embedded player/,
  );
  assert.match(proofProblems(ok(), 'external', proof([])).join('\n'), /declares row 1 but did not reach/);
  assert.match(proofProblems(ok(), 'external', proof(['1', '12b'])).join('\n'), /reported row 12b as shown, which it does not declare/);
  assert.deepEqual(proofProblems(ok({ rows: [], noRows: 'x' }), 'external', proof([])), []);
});

test('the verdict of two full runs fails on a failure, a missing run, and a row no passing scenario showed', () => {
  const inventory = [{ id: '1' }, { id: '2' }];
  const a = { name: 'a', rows: ['1'] };
  const b = { name: 'b', rows: ['2'] };
  const known = { scenario: 'b', player: 'external', failsWith: 'the known text', defect: 'd' };
  const passing = (name, shown) => ({ [name]: { status: 'pass', error: null, shown, routes: [] } });
  const run = (player, scenarios, extra = {}) => ({ player, full: true, built: true, head: 'h', dirty: false, scenarios, ...extra });
  const good = {
    external: run('external', { ...passing('a', ['1']), b: { status: 'fail', error: 'AssertionError: the known text', shown: [], routes: [] } }),
    embedded: run('embedded', { ...passing('a', ['1']), ...passing('b', ['2']) }),
  };
  const checkout = { head: 'h', dirty: false };
  const verdict = (results, knownFailures = [known], at = checkout) =>
    verdictProblems([a, b], results, { inventory, exemptions: [], knownFailures, checkout: at }).join('\n');
  assert.equal(verdict(good), '');
  // A failure that is not the known one.
  assert.match(
    verdict({ ...good, embedded: run('embedded', { ...passing('a', ['1']), b: { status: 'fail', error: 'boom', shown: [], routes: [] } }) }),
    /b failed in the embedded player: boom/,
  );
  // The known failure failing differently, or not at all.
  assert.match(
    verdict({ ...good, external: run('external', { ...passing('a', ['1']), b: { status: 'fail', error: 'other', shown: [], routes: [] } }) }),
    /not with its known failure/,
  );
  assert.match(
    verdict({ ...good, external: run('external', { ...passing('a', ['1']), ...passing('b', ['2']) }) }),
    /its known failure is gone/,
  );
  // An intermittent known failure (a race) may pass; it may not fail otherwise.
  const race = { ...known, intermittent: true };
  assert.equal(verdict({ ...good, external: run('external', { ...passing('a', ['1']), ...passing('b', ['2']) }) }, [race]), '');
  assert.equal(verdict(good, [race]), '');
  assert.match(
    verdict({ ...good, external: run('external', { ...passing('a', ['1']), b: { status: 'fail', error: 'other', shown: [], routes: [] } }) }, [race]),
    /not with its known failure/,
  );
  // A known failure counts only when it failed: skipped, or with no status or
  // one nobody writes, it was not seen to fail, whatever text it carries
  // (review finding pa-5).
  const knownAs = (status) =>
    verdict({
      ...good,
      external: run('external', { ...passing('a', ['1']), b: { status, error: 'AssertionError: the known text', shown: [], routes: [] } }),
    });
  assert.match(knownAs('skip'), /^b was skipped in the external run, so it has no result there$/m);
  assert.match(knownAs(undefined), /^b has no result in the external run: its status is undefined, neither pass nor fail$/m);
  assert.match(knownAs('FAIL'), /^b has no result in the external run: its status is "FAIL", neither pass nor fail$/m);
  assert.match(knownAs('failed'), /its status is "failed", neither pass nor fail/);
  assert.equal(knownAs('fail'), '');
  // The same for a scenario that is no known failure: only 'pass' passes.
  const otherAs = (status, error = null) =>
    verdict({ ...good, embedded: run('embedded', { ...passing('b', ['2']), a: { status, error, shown: ['1'], routes: [] } }) });
  assert.match(otherAs('skip'), /^a was skipped in the embedded run, so it has no result there$/m);
  assert.match(otherAs(undefined), /^a has no result in the embedded run: its status is undefined, neither pass nor fail$/m);
  assert.match(otherAs('passed'), /^a has no result in the embedded run: its status is "passed", neither pass nor fail$/m);
  assert.match(otherAs('fail', 'boom'), /^a failed in the embedded player: boom$/m);
  // A failure with no error text is not one the runner wrote.
  assert.match(
    otherAs('fail'),
    /^a has a result in the embedded run that the runner does not write: it failed with an error that is not text \(null\)$/m,
  );
  assert.equal(otherAs('pass'), '');
  // And a known failure counts only as the runner could have written it:
  // its error is text. A list holding the text, the text inside an object,
  // or no error at all is not the known failure seen (review finding pa-5,
  // from its first verification).
  const knownErr = (error) =>
    verdict({ ...good, external: run('external', { ...passing('a', ['1']), b: { status: 'fail', error, shown: [], routes: [] } }) });
  for (const error of [['AssertionError: the known text'], [['the known text']], { message: 'the known text' }, null, undefined, 7]) {
    assert.match(
      knownErr(error),
      /^b has a result in the external run that the runner does not write: it failed with an error that is not text \(.*\)$/m,
      JSON.stringify(error),
    );
  }
  assert.equal(knownErr('the known text'), '');
  // A row counts as shown only from the list the runner writes: text in its
  // place would match one row's id inside another's.
  const textShown = { status: 'pass', error: null, shown: '12', routes: [] };
  assert.equal(
    verdict({ ...good, embedded: run('embedded', { a: textShown, b: textShown }) }),
    [
      'a has a result in the embedded run that the runner does not write: its "shown" is "12", not a list of text',
      'b has a result in the embedded run that the runner does not write: its "shown" is "12", not a list of text',
      'row 1 is shown by no scenario that passed in the embedded run',
      'row 2 is shown by no scenario that passed in the embedded run',
    ].join('\n'),
  );
  // From the second verification: a result is judged whole, as the runner
  // writes it, before it counts for anything. A pass carries no error.
  const embeddedA = (result) => verdict({ ...good, embedded: run('embedded', { ...passing('b', ['2']), a: result }) });
  const pass = { status: 'pass', error: null, shown: ['1'], routes: [] };
  assert.equal(embeddedA(pass), '');
  assert.match(
    embeddedA({ ...pass, error: ['scenario failed'] }),
    /^a has a result in the embedded run that the runner does not write: it passed with an error \(\["scenario failed"\]\), and a pass has none$/m,
  );
  assert.match(embeddedA({ ...pass, error: '' }), /it passed with an error \(""\), and a pass has none/);
  // A `shown` that is no list of text is refused where it stands, also when
  // another scenario shows the same row: a2 covers row 1 as well as a.
  const a2 = { name: 'a2', rows: ['1'] };
  const overlapping = (shown) =>
    verdictProblems(
      [a, a2, b],
      {
        external: run('external', { ...good.external.scenarios, ...passing('a2', ['1']) }),
        embedded: run('embedded', { a: { ...pass, shown }, ...passing('a2', ['1']), ...passing('b', ['2']) }),
      },
      { inventory, exemptions: [], knownFailures: [known], checkout },
    ).join('\n');
  assert.equal(overlapping(['1']), '');
  for (const shown of ['13', 7, {}, null, [1], ['1', 7]]) {
    assert.match(
      overlapping(shown),
      /^a has a result in the embedded run that the runner does not write: its "shown" is .*, not a list of text$/,
      JSON.stringify(shown),
    );
  }
  // The same for `routes`, for a result with a part missing or one too many,
  // and for a pass the runner would have failed: one that does not show a
  // row it declares, or shows one it does not.
  assert.match(embeddedA({ ...pass, routes: 'mpvInput' }), /its "routes" is "mpvInput", not a list of text/);
  const { routes: _noRoutes, ...withoutRoutes } = pass;
  assert.match(embeddedA(withoutRoutes), /it holds error, shown, status, not status, error, shown and routes/);
  assert.match(embeddedA({ ...pass, extra: 1 }), /it holds error, extra, routes, shown, status, not status, error, shown and routes/);
  assert.match(
    overlapping([]),
    /^a has a result in the embedded run that the runner does not write: it passed, but it declares row 1 but did not reach the end of its proof in the embedded player \(no shown\('1'\)\): the runner fails such a scenario$/,
  );
  assert.match(embeddedA({ ...pass, shown: ['1', '2'] }), /it passed, but it reported row 2 as shown, which it does not declare: the runner fails such a scenario/);
  assert.match(embeddedA({ ...pass, routes: ['mpvInput'] }), /it passed, but it took the test-only route mpvInput in the embedded player without declaring it/);
  // A scenario that does not run in a player is named there as skipped, and
  // as nothing else.
  const onlyEmbedded = { name: 'c', rows: ['2'], onlyPlayer: 'embedded' };
  const withC = (external) =>
    verdictProblems(
      [a, b, onlyEmbedded],
      {
        external: run('external', { ...good.external.scenarios, ...(external === undefined ? {} : { c: external }) }),
        embedded: run('embedded', { ...good.embedded.scenarios, ...passing('c', ['2']) }),
      },
      { inventory, exemptions: [], knownFailures: [known], checkout },
    ).join('\n');
  assert.equal(withC({ status: 'skip' }), '');
  assert.equal(withC(undefined), 'c is not in the external results, which name every scenario, a skipped one too');
  assert.match(withC({ ...pass, shown: ['2'] }), /^c does not run in the external player, and its result there is not the skip the runner writes: \{"status":"pass"/);
  assert.match(withC({ status: 'skip', error: 'x' }), /^c does not run in the external player, and its result there is not the skip the runner writes/);
  assert.match(withC('skip'), /^c does not run in the external player, and its result there is not the skip the runner writes: "skip"$/);
  // Results that name no commit.
  assert.match(verdict({ ...good, embedded: run('embedded', good.embedded.scenarios, { head: undefined }) }), /^the embedded results name no commit$/m);
  // Results that are not by scenario name hold none.
  assert.match(verdict({ ...good, embedded: run('embedded', [{ status: 'pass' }]) }), /^the embedded results hold no scenarios by name$/m);
  assert.match(verdict({ ...good, embedded: run('embedded', null) }), /^the embedded results hold no scenarios by name$/m);
  // A scenario whose name every object answers to has no result unless
  // the file holds one of its own.
  const inherited = { name: 'constructor', rows: ['1'] };
  assert.match(
    verdictProblems([inherited, b], good, { inventory, exemptions: [], knownFailures: [known], checkout }).join('\n'),
    /^constructor did not run in the external run$/m,
  );
  // A file that does not say the run was full, or clean, is not taken
  // as either.
  const embeddedWith = (extra) => verdict({ ...good, embedded: run('embedded', good.embedded.scenarios, extra) });
  assert.equal(embeddedWith({}), '');
  assert.match(embeddedWith({ dirty: undefined }), /^the embedded run was of a worktree with uncommitted changes/m);
  assert.match(embeddedWith({ dirty: 0 }), /^the embedded run was of a worktree with uncommitted changes/m);
  assert.match(embeddedWith({ full: 'yes' }), /^the embedded results are of a run of named scenarios, not the full suite$/m);
  // Row 2 in the external player is RED (only b claims it, a known failure);
  // without the record it is a failure like any other.
  assert.match(verdict(good, []), /b failed in the external player/);
  // A scenario that did not run, a named run, a dirty one, different commits.
  assert.match(verdict({ ...good, embedded: run('embedded', passing('a', ['1'])) }), /b did not run in the embedded run/);
  assert.match(verdict({ ...good, embedded: { ...good.embedded, full: false } }), /not the full suite/);
  assert.match(verdict({ ...good, embedded: { ...good.embedded, dirty: true } }), /uncommitted changes/);
  // A run that built nothing (--skip-build), or does not say (review finding
  // conf2-1): its binary may be of any code.
  assert.match(
    verdict({ ...good, embedded: { ...good.embedded, built: false } }),
    /the embedded run did not build the app it ran \(--skip-build\), so nothing ties its binary to commit h/,
  );
  const { built: _unsaid, ...unsaid } = good.external;
  assert.match(verdict({ ...good, external: unsaid }), /the external run did not build the app it ran/);
  assert.match(verdict({ ...good, embedded: { ...good.embedded, head: 'other' } }), /different commits/);
  assert.match(verdict({ external: good.external }), /no results of the embedded run/);
  // Two good runs of one commit judged at another, or at an uncommitted tree,
  // or with nothing saying what is checked out (review finding conf-1): the
  // records judged are not those the runs ran.
  assert.match(verdict(good, [known], { head: 'later', dirty: false }), /the external run is of commit h, not of the checked-out later/);
  assert.match(verdict(good, [known], { head: 'later', dirty: false }), /the embedded run is of commit h, not of the checked-out later/);
  assert.match(verdict(good, [known], { head: 'h', dirty: true }), /the checkout \(h\) has uncommitted changes/);
  assert.match(
    verdictProblems([a, b], good, { inventory, exemptions: [], knownFailures: [known] }).join('\n'),
    /the checked-out commit is unknown/,
  );
  assert.match(verdict(good, [known], null), /the checked-out commit is unknown/);
  // A passing scenario that did not report the row it claims shows nothing.
  assert.match(
    verdict({ ...good, embedded: run('embedded', { ...passing('a', []), ...passing('b', ['2']) }) }),
    /row 1 is shown by no scenario that passed in the embedded run/,
  );
});

test('a run records the commit it started at, and is dirty when its tree changed during the run (conf-1)', () => {
  const clean = { head: 'h', status: '' };
  assert.deepEqual(recordedTree(clean, [clean, clean]), { head: 'h', dirty: false, changedDuringRun: false });
  // Dirty at the start, and unchanged.
  const edited = { head: 'h', status: ' M tests/e2e/scenarios/a.mjs' };
  assert.deepEqual(recordedTree(edited, [edited]), { head: 'h', dirty: true, changedDuringRun: false });
  // An injection still in place at the end, and one restored before the end
  // but seen after a scenario.
  assert.deepEqual(recordedTree(clean, [clean, edited]), { head: 'h', dirty: true, changedDuringRun: true });
  assert.deepEqual(recordedTree(clean, [edited, clean]), { head: 'h', dirty: true, changedDuringRun: true });
  // A commit landing mid-run: the head is the start's, whose code was built
  // and run, and the run is dirty, clean as the tree is at both ends.
  const later = { head: 'later', status: '' };
  assert.deepEqual(recordedTree(clean, [later, later]), { head: 'h', dirty: true, changedDuringRun: true });
});

test('the parity printout marks a row shown only through test-only routes', () => {
  const route = { row: '1', player: 'embedded', via: 'clickUnderPlay', route: 'r', proves: 'p', unproven: 'u' };
  const viaRoute = { name: 'a', rows: ['1'], testOnly: [route] };
  assert.deepEqual(testOnlyCoverage([viaRoute], '1', 'embedded', []), ['a']);
  assert.deepEqual(testOnlyCoverage([viaRoute], '1', 'external', []), []);
  assert.deepEqual(testOnlyCoverage([viaRoute, { name: 'b', rows: ['1'] }], '1', 'embedded', []), ['a']);
});

test('a scenario recorded as failing on a product defect shows nothing in that player', () => {
  const inventory = [{ id: '1' }];
  const known = { scenario: 'both', player: 'external', failsWith: 'the text', defect: 'the defect' };
  const both = { name: 'both', rows: ['1'] };
  // It is not coverage there, and not in the other player's way.
  assert.deepEqual(coverage([both], '1', 'external', [known]), []);
  assert.deepEqual(redCoverage([both], '1', 'external', [known]), ['both']);
  assert.deepEqual(coverage([both], '1', 'embedded', [known]), ['both']);
  // A row that only it claims is RED there, which the check lets stand (it is
  // recorded) and the printout above names as NOT SHOWN; another scenario
  // showing the row makes it shown again.
  assert.deepEqual(parityProblems([both], { inventory, exemptions: [], knownFailures: [known] }), []);
  const other = { name: 'other', rows: ['1'] };
  assert.deepEqual(coverage([both, other], '1', 'external', [known]), ['other']);
  // What a record must say.
  const problems = (k) => parityProblems([both], { inventory, exemptions: [], knownFailures: [k] }).join('\n');
  assert.match(problems({ ...known, scenario: 'nope' }), /no such scenario/);
  assert.match(problems({ ...known, player: 'both' }), /no such player/);
  assert.match(problems({ ...known, failsWith: '' }), /no failsWith/);
  assert.match(problems({ ...known, defect: '' }), /no defect/);
  assert.match(problems({ ...known, kind: 'x' }), /unknown key "kind"/);
  assert.equal(problems({ ...known, intermittent: true }), '');
  assert.match(problems({ ...known, intermittent: 'yes' }), /intermittent is true or absent/);
  assert.match(
    parityProblems([{ ...both, onlyPlayer: 'embedded' }], { inventory, exemptions: [], knownFailures: [known] }).join('\n'),
    /does not run in that player/,
  );
});
