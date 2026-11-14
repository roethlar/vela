// The parity guard of the two players (.agents/plans/embedded-player.md,
// "Slice 6"; tests/e2e/README.md, "Parity"). Every row of the behaviour
// inventory (tests/e2e/parity.mjs) must be shown by a scenario in each player,
// or carry a recorded exemption there; the scenarios say what they show on
// their own default export, read here through the loader the runner uses.
// Nothing reads JavaScript source: the scenarios are imported.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
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
// its own, with every way of acting on the machine it knows replaced by one
// that records the call, and reports what it saw once that process has
// nothing left to do, so that what an import puts off is seen as well as what
// it does at once (review finding pa-4). `pairs` are directories and the
// prefix their scenarios are loaded with.
function importWatched(pairs, { timeout = 60000 } = {}) {
  const res = spawnSync(process.execPath, [path.join(repoRoot, 'tests/e2e/import-watch.mjs'), ...pairs], {
    encoding: 'utf8',
    timeout,
  });
  if (res.error?.code === 'ETIMEDOUT' || res.signal) {
    return { error: `the import never let its process end (${res.error?.code ?? res.signal}): it left something running`, calls: [] };
  }
  const line = res.stdout.trim().split('\n').at(-1) ?? '';
  try {
    return JSON.parse(line);
  } catch {
    return { error: `the watch reported nothing it could read (exit ${res.status}): ${res.stderr.trim()}`, calls: [] };
  }
}

const imported = importWatched([scenarioDir, '', liveDir, 'live-']);
const scenarios = await loadScenarios(scenarioDir);
const live = await loadScenarios(liveDir, { prefix: 'live-' });

test('importing a scenario has no side effects', () => {
  assert.equal(imported.error, undefined, imported.error);
  // Every scenario was imported under the watch, so that an empty import
  // cannot pass.
  assert.deepEqual(imported.loaded, [...scenarios, ...live].map((s) => s.name));
  assert.deepEqual(imported.calls, [], 'a scenario module acted on the machine when it was imported');
  assert.equal(imported.newHandles, 0, 'a scenario module left something running when it was imported');
  assert.equal(imported.envChanged, false, 'a scenario module changed the environment when it was imported');
  assert.deepEqual(imported.sharedChanged, [], 'a scenario module changed the global object or a built-in when it was imported');
});

// Each route to a timer, taken by a made-up scenario module at import, must be
// seen by the watch above (review finding pa-4). The modules are written to a
// directory of their own, never among the scenarios, and loaded through the
// loader the runner uses.
test('the import watch sees a timer started by any route, and what an import puts off', async () => {
  const routes = {
    timerssettimeout: ["import { setTimeout } from 'node:timers';", 'setTimeout(() => {}, 1);', 'timers.setTimeout()'],
    timersunref: ["import { setTimeout } from 'node:timers';", 'setTimeout(() => {}, 60000).unref();', 'timers.setTimeout()'],
    timersdefault: ["import timers from 'node:timers';", 'timers.setTimeout(() => {}, 60000).unref();', 'timers.setTimeout()'],
    timersrequire: [
      "import { createRequire } from 'node:module';",
      "createRequire(import.meta.url)('node:timers').setTimeout(() => {}, 60000).unref();",
      'timers.setTimeout()',
    ],
    timerssetinterval: [
      "import { setInterval, clearInterval } from 'node:timers';",
      'clearInterval(setInterval(() => {}, 60000));',
      'timers.setInterval()',
    ],
    timerssetimmediate: ["import { setImmediate } from 'node:timers';", 'setImmediate(() => {});', 'timers.setImmediate()'],
    promisesdelay: ["import { setTimeout as delay } from 'node:timers/promises';", 'await delay(1);', 'timers/promises.setTimeout()'],
    promisesoftimers: ["import { promises } from 'node:timers';", 'await promises.setTimeout(1);', 'timers/promises.setTimeout()'],
    promisesunawaited: [
      "import { setTimeout as delay } from 'node:timers/promises';",
      'delay(60000, undefined, { ref: false }).catch(() => {});',
      'timers/promises.setTimeout()',
    ],
    promisessetinterval: [
      "import { setInterval } from 'node:timers/promises';",
      'const ticks = setInterval(1); await ticks.next(); await ticks.return();',
      'timers/promises.setInterval()',
    ],
    promisessetimmediate: ["import { setImmediate } from 'node:timers/promises';", 'await setImmediate();', 'timers/promises.setImmediate()'],
    schedulerwait: ["import { scheduler } from 'node:timers/promises';", 'await scheduler.wait(1);', 'timers/promises.scheduler.wait()'],
    scheduleryield: ["import { scheduler } from 'node:timers/promises';", 'await scheduler.yield();', 'timers/promises.scheduler.yield()'],
    globalunref: ['', 'setTimeout(() => {}, 60000).unref();', 'global.setTimeout()'],
    globalsetinterval: ['', 'clearInterval(setInterval(() => {}, 60000));', 'global.setInterval()'],
    globalsetimmediate: ['', 'setImmediate(() => {});', 'global.setImmediate()'],
    aborttimeout: ['', 'AbortSignal.timeout(60000);', 'AbortSignal.timeout()'],
    // Callbacks put off without a timer, which run after the import too.
    nexttick: ['', 'process.nextTick(() => {});', 'process.nextTick()'],
    microtask: ['', 'queueMicrotask(() => {});', 'global.queueMicrotask()'],
    // A wait on shared memory, which settles later with no timer and no handle.
    atomicswaitasync: ['', 'Atomics.waitAsync(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1);', 'Atomics.waitAsync()'],
    // An act put off through a route the watch does not record (reading a
    // file), which runs after the import has returned: the watch must still
    // be up then. Two reads in a row put it well after the import.
    deferredwrite: [
      "import fs from 'node:fs';",
      'const self = new URL(import.meta.url);\n' +
        "fs.promises.readFile(self).then(() => fs.promises.readFile(self)).then(() => fs.writeFileSync(new URL('./written.txt', self), 'x'));",
      'fs.writeFileSync()',
    ],
  };
  seenByTheWatch(routes);
});

// Each made-up module, `name: [imports, act, expected]`, is written to a
// directory of its own under a temporary one and imported under the watch,
// which must record `expected`.
function seenByTheWatch(routes) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-parity-watch-'));
  try {
    for (const [name, [imports, act, expected]] of Object.entries(routes)) {
      const one = path.join(dir, name);
      fs.mkdirSync(one);
      fs.writeFileSync(
        path.join(one, `${name}.mjs`),
        `${imports}\n${act}\nexport default { name: '${name}', rows: [], noRows: 'made up', async run() {} };\n`,
      );
      const watched = importWatched([one, '']);
      assert.equal(watched.error, undefined, `${name}: ${watched.error}`);
      assert.deepEqual(watched.loaded, [name]);
      assert.ok(watched.calls.includes(expected), `${name}: ${JSON.stringify(watched.calls)} should include ${expected}`);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

// Acts reached other than through the module functions the watch first
// covered, each found unseen once (review finding pa-4): a signal handler
// added through the EventEmitter prototype, a DNS query, a file watcher that
// holds no handle, and a socket, server or child made with `new`. Every
// address is this machine's, and each is closed or cancelled at once. The
// call is what the watch records, so each probe takes a refusal as well as a
// success: where no socket is allowed (a sandbox answers EPERM to a listen
// on this machine's own address) the probe still ends and the call is seen.
test('the import watch sees a signal handler, a DNS query, a file watcher, and a socket or child made with new', () => {
  seenByTheWatch({
    prototypelistener: [
      "import { EventEmitter } from 'node:events';",
      "EventEmitter.prototype.on.call(process, 'SIGUSR2', () => {});",
      'process listeners of SIGUSR2 changed',
    ],
    processon: ['', "process.on('SIGUSR2', () => {});", 'process.on(SIGUSR2)'],
    dnslookup: ["import dns from 'node:dns';", "dns.lookup('localhost', () => {});", 'dns.lookup(localhost)'],
    dnspromises: ["import dns from 'node:dns';", "await dns.promises.lookup('localhost').catch(() => {});", 'dns.promises.lookup(localhost)'],
    dnsresolver: [
      "import { Resolver } from 'node:dns';",
      "const r = new Resolver(); r.setServers(['127.0.0.1']); r.resolve4('vela.invalid', () => {}); r.cancel();",
      'dns.Resolver.resolve4(vela.invalid)',
    ],
    dnspromisesresolver: [
      "import dns from 'node:dns';",
      "const r = new dns.promises.Resolver(); r.setServers(['127.0.0.1']); const q = r.resolve4('vela.invalid').catch(() => {}); r.cancel(); await q;",
      'dns.promises.Resolver.resolve4(vela.invalid)',
    ],
    watchfile: [
      "import fs from 'node:fs';",
      'const self = new URL(import.meta.url); fs.watchFile(self, { persistent: false }, () => {}); fs.unwatchFile(self);',
      'fs.watchFile()',
    ],
    fswatch: ["import fs from 'node:fs';", 'try { fs.watch(new URL(import.meta.url), { persistent: false }).close(); } catch {}', 'fs.watch()'],
    socketnew: [
      "import net from 'node:net';",
      "const s = new net.Socket(); s.on('error', () => {}); s.connect(9, '127.0.0.1'); s.destroy();",
      'net.Socket.connect()',
    ],
    servernew: [
      "import net from 'node:net';",
      "const s = new net.Server(); await new Promise((r) => { s.on('error', r); s.listen(0, '127.0.0.1', r); }); s.close();",
      'net.Server.listen()',
    ],
    udpnew: [
      "import dgram from 'node:dgram';",
      "const s = new dgram.Socket('udp4'); await new Promise((r) => { s.on('error', r); s.bind(0, '127.0.0.1', r); }); s.close();",
      'dgram.Socket.bind()',
    ],
    childnew: [
      "import { ChildProcess } from 'node:child_process';",
      "const c = new ChildProcess(); c.on('error', () => {}); try { c.spawn({ file: '/nonexistent/vela-probe', args: ['/nonexistent/vela-probe'], envPairs: [], stdio: 'ignore' }); } catch {}",
      'child_process.ChildProcess.spawn()',
    ],
  });
});

// The watch reports only once its process has nothing left to do; an import
// that keeps it busy for good must fail, not hang the test or pass.
test('an import that keeps its process busy for good fails', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-parity-watch-'));
  try {
    fs.writeFileSync(
      path.join(dir, 'busy.mjs'),
      "setInterval(() => {}, 1000);\nexport default { name: 'busy', rows: [], noRows: 'made up', async run() {} };\n",
    );
    const watched = importWatched([dir, ''], { timeout: 3000 });
    assert.match(watched.error ?? '', /^the import never let its process end/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
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
    ['1', '2', '3', '4', '5', '6', '7', '8a', '8b', '9', '10', '11', '12a', '12b', '13a', '13b', '14'],
  );
  assert.deepEqual(SPLITS, { 8: ['8a', '8b'], 12: ['12a', '12b'], 13: ['13a', '13b'] });
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
