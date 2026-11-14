// The parity guard of the two players (.agents/plans/embedded-player.md,
// "Slice 6"; tests/e2e/README.md, "Parity"). Every row of the behaviour
// inventory (tests/e2e/parity.mjs) must be shown by a scenario in each player,
// or carry a recorded exemption there; the scenarios say what they show on
// their own default export, read here through the loader the runner uses.
// Nothing reads JavaScript source: the scenarios are imported.
import assert from 'node:assert/strict';
import childProcess from 'node:child_process';
import cluster from 'node:cluster';
import dgram from 'node:dgram';
import fs from 'node:fs';
import http from 'node:http';
import https from 'node:https';
import module from 'node:module';
import net from 'node:net';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import workerThreads from 'node:worker_threads';
import { EXEMPTIONS, INVENTORY, KNOWN_FAILURES, PLAYERS } from './e2e/parity.mjs';
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
// before it runs any, and this test imports them all. So the first import of
// them in this process happens with every way of acting on the machine
// replaced by one that records the call, and with the process's live handles
// counted before and after.
async function importWatched() {
  const calls = [];
  const patched = [];
  const watch = (object, names, label) => {
    for (const name of names) {
      if (typeof object[name] !== 'function') continue;
      const original = object[name];
      patched.push(() => {
        object[name] = original;
      });
      object[name] = function watched(...args) {
        // Opening a file to read it is how modules are loaded at all.
        const readOnly = (name === 'openSync' || name === 'open') && [undefined, 'r', 0].includes(args[1]);
        if (!readOnly) calls.push(`${label}.${name}(${typeof args[0] === 'string' ? args[0] : ''})`);
        return original.apply(this, args);
      };
    }
  };
  watch(childProcess, ['spawn', 'spawnSync', 'exec', 'execSync', 'execFile', 'execFileSync', 'fork'], 'child_process');
  watch(cluster, ['fork', 'setupPrimary', 'setupMaster'], 'cluster');
  watch(net, ['createServer', 'createConnection', 'connect'], 'net');
  watch(dgram, ['createSocket'], 'dgram');
  watch(http, ['createServer', 'request', 'get'], 'http');
  watch(https, ['createServer', 'request', 'get'], 'https');
  // Every fs call that changes a file, a directory or its metadata; opening
  // is watched too, except to read.
  const FS_WRITERS = [
    'writeFile', 'appendFile', 'write', 'writev', 'mkdir', 'mkdtemp', 'rm', 'rmdir', 'unlink', 'rename',
    'copyFile', 'cp', 'symlink', 'link', 'truncate', 'ftruncate', 'chmod', 'fchmod', 'lchmod', 'chown',
    'fchown', 'lchown', 'utimes', 'futimes', 'lutimes', 'open',
  ];
  watch(fs, [...FS_WRITERS, ...FS_WRITERS.map((name) => `${name}Sync`), 'createWriteStream'], 'fs');
  watch(fs.promises, FS_WRITERS, 'fs.promises');
  watch(globalThis, ['fetch', 'setTimeout', 'setInterval', 'setImmediate', 'queueMicrotask'], 'global');
  watch(process, ['on', 'once', 'addListener', 'prependListener', 'kill', 'exit', 'chdir', 'umask'], 'process');
  // A worker thread runs code of its own, which can do any of the above out of
  // this thread's sight (review finding atk-8); its construction is the act.
  const OriginalWorker = workerThreads.Worker;
  patched.push(() => {
    workerThreads.Worker = OriginalWorker;
  });
  workerThreads.Worker = class WatchedWorker extends OriginalWorker {
    constructor(...args) {
      calls.push(`worker_threads.Worker(${typeof args[0] === 'string' ? args[0].slice(0, 80) : ''})`);
      super(...args);
    }
  };
  // Named imports of the built-in modules (`import { spawnSync } from
  // 'node:child_process'`) see the replacements only after this.
  module.syncBuiltinESMExports();
  const handlesBefore = process.getActiveResourcesInfo().length;
  const envBefore = JSON.stringify(process.env);
  // What an import could change in this process for every later module: the
  // global object's own names, and the built-in prototypes' properties.
  const shared = () => {
    const out = new Map([['globalThis', Object.getOwnPropertyNames(globalThis).sort().join(',')]]);
    for (const [name, proto] of Object.entries({
      Object: Object.prototype, Array: Array.prototype, String: String.prototype, Function: Function.prototype,
      Promise: Promise.prototype, Number: Number.prototype, RegExp: RegExp.prototype, Map: Map.prototype,
      Set: Set.prototype, Error: Error.prototype, JSON, Math, Reflect, Object_: Object, Array_: Array, Promise_: Promise,
    })) {
      for (const key of Reflect.ownKeys(proto)) {
        const d = Object.getOwnPropertyDescriptor(proto, key);
        out.set(`${name}.${String(key)}`, d);
      }
    }
    return out;
  };
  const sharedBefore = shared();
  let loaded;
  try {
    loaded = {
      scenarios: await loadScenarios(scenarioDir),
      live: await loadScenarios(liveDir, { prefix: 'live-' }),
    };
  } finally {
    for (const restore of patched) restore();
    module.syncBuiltinESMExports();
  }
  const sharedAfter = shared();
  const sharedChanged = [];
  for (const key of new Set([...sharedBefore.keys(), ...sharedAfter.keys()])) {
    const [a, b] = [sharedBefore.get(key), sharedAfter.get(key)];
    if (key === 'globalThis' ? a !== b : !sameDescriptor(a, b)) sharedChanged.push(key);
  }
  return {
    ...loaded,
    calls,
    newHandles: process.getActiveResourcesInfo().length - handlesBefore,
    envChanged: JSON.stringify(process.env) !== envBefore,
    sharedChanged,
  };
}

function sameDescriptor(a, b) {
  if (!a || !b) return a === b;
  return ['value', 'get', 'set', 'writable', 'enumerable', 'configurable'].every((k) => Object.is(a[k], b[k]));
}

const imported = await importWatched();
const { scenarios, live } = imported;

test('importing a scenario has no side effects', () => {
  assert.deepEqual(imported.calls, [], 'a scenario module acted on the machine when it was imported');
  assert.equal(imported.newHandles, 0, 'a scenario module left something running when it was imported');
  assert.equal(imported.envChanged, false, 'a scenario module changed the environment when it was imported');
  assert.deepEqual(imported.sharedChanged, [], 'a scenario module changed the global object or a built-in when it was imported');
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

test('the inventory check refuses a dropped, a made-up and a half-split row', () => {
  const plan = ['1', '2', '3'];
  const rows = (...ids) => ids.map((id) => ({ id }));
  assert.deepEqual(inventoryProblems(rows('1', '2a', '2b', '3'), plan), []);
  assert.match(inventoryProblems(rows('1', '3'), plan).join('\n'), /plan row 2 is not in the inventory/);
  assert.match(inventoryProblems(rows('1', '2', '3', '4'), plan).join('\n'), /"4" is no row of the plan/);
  assert.match(inventoryProblems(rows('1', '2', '2a', '3'), plan).join('\n'), /both whole and split/);
  assert.match(inventoryProblems(rows('1', '2', '3'), []).join('\n'), /table was not found/);
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
    external: run('external', { ...passing('a', ['1']), b: { status: 'fail', error: 'AssertionError: the known text', shown: [] } }),
    embedded: run('embedded', { ...passing('a', ['1']), ...passing('b', ['2']) }),
  };
  const checkout = { head: 'h', dirty: false };
  const verdict = (results, knownFailures = [known], at = checkout) =>
    verdictProblems([a, b], results, { inventory, exemptions: [], knownFailures, checkout: at }).join('\n');
  assert.equal(verdict(good), '');
  // A failure that is not the known one.
  assert.match(
    verdict({ ...good, embedded: run('embedded', { ...passing('a', ['1']), b: { status: 'fail', error: 'boom', shown: [] } }) }),
    /b failed in the embedded player: boom/,
  );
  // The known failure failing differently, or not at all.
  assert.match(
    verdict({ ...good, external: run('external', { ...passing('a', ['1']), b: { status: 'fail', error: 'other', shown: [] } }) }),
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
    verdict({ ...good, external: run('external', { ...passing('a', ['1']), b: { status: 'fail', error: 'other', shown: [] } }) }, [race]),
    /not with its known failure/,
  );
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
