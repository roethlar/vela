#!/usr/bin/env node
// The parity verdict of two full runs (guard finding atk-3 of the parity
// review): what the parity test (tests/e2e-parity.test.mjs) checks from the
// scenarios' declarations, checked against what the runs did. Run it after a
// full external run (`npm run e2e`) and a full embedded run
// (`npm run e2e -- --embedded`) of the same commit, with that commit checked
// out and clean (it refuses runs of any other commit); each writes
// tests/e2e/results/<player>.json. It fails (exit 1) when a scenario failed
// other than as a recorded known failure, when a known failure stopped failing
// or failed differently, when a scenario did not run, or when a row is shown
// in a player by no scenario that passed there and reached the row's proof
// (rows exempted, or RED on a known failure, excepted). See tests/e2e/README.md,
// "Parity".
//
// Usage: npm run e2e:verdict [-- <external.json> <embedded.json>]
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { EXEMPTIONS, INVENTORY, KNOWN_FAILURES, PLAYERS } from './parity.mjs';
import { coverage, loadScenarios, redCoverage, testOnlyCoverage, verdictProblems } from './scenario-loader.mjs';

const e2eDir = path.dirname(fileURLToPath(import.meta.url));
const [externalFile, embeddedFile] = process.argv.slice(2);
const files = {
  external: externalFile ?? path.join(e2eDir, 'results', 'external.json'),
  embedded: embeddedFile ?? path.join(e2eDir, 'results', 'embedded.json'),
};
const results = {};
for (const player of PLAYERS) {
  try {
    results[player] = JSON.parse(fs.readFileSync(files[player], 'utf8'));
  } catch (err) {
    console.error(`parity verdict: cannot read the ${player} results (${files[player]}): ${err.message}`);
  }
}
const scenarios = await loadScenarios(path.join(e2eDir, 'scenarios'));

for (const player of PLAYERS) {
  const run = results[player];
  if (run) console.log(`${player}: ${run.full ? 'full' : 'NAMED'} run of ${run.head}${run.dirty ? ' (DIRTY)' : ''}${run.built === true ? '' : ' (NOT BUILT BY THE RUN)'}, finished ${run.finishedAt}`);
}
for (const k of KNOWN_FAILURES) {
  console.log(`known product failure: ${k.scenario} in the ${k.player} player: ${results[k.player]?.scenarios?.[k.scenario]?.status ?? 'not run'}`);
}
for (const { id, behaviour } of INVENTORY) {
  const parts = PLAYERS.map((player) => {
    if (EXEMPTIONS.some((e) => e.row === id && e.player === player)) return `${player} (exempted)`;
    const ran = results[player]?.scenarios ?? {};
    const passed = coverage(scenarios, id, player).filter(
      (name) => ran[name]?.status === 'pass' && (ran[name].shown ?? []).includes(id),
    );
    const viaTestOnly = testOnlyCoverage(scenarios, id, player);
    const red = redCoverage(scenarios, id, player);
    const note = passed.length > 0 && passed.every((name) => viaTestOnly.includes(name)) ? ' [ONLY VIA TEST-ONLY ROUTES]' : '';
    const redNote = red.length > 0 ? ` [RED: ${red.join(', ')}]` : '';
    return `${player} ${passed.join(', ') || 'NOT SHOWN'}${note}${redNote}`;
  });
  console.log(`row ${id} (${behaviour}): ${parts.join('; ')}`);
}

// The state this verdict reads its scenarios and records from; both runs must
// be of it (verdictProblems, review finding conf-1).
const git = (args) => {
  const res = spawnSync('git', args, { cwd: e2eDir, encoding: 'utf8' });
  return res.status === 0 ? res.stdout.trim() : null;
};
const checkedOut = git(['rev-parse', 'HEAD']);
const status = git(['status', '--porcelain']);
const checkout = checkedOut && status !== null ? { head: checkedOut, dirty: status !== '' } : null;
console.log(`checked out: ${checkout ? `${checkout.head}${checkout.dirty ? ' (DIRTY)' : ''}` : 'unknown'}`);

const problems = verdictProblems(scenarios, results, { checkout });
if (problems.length > 0) {
  console.error(`parity verdict: FAIL\n  ${problems.join('\n  ')}`);
  process.exit(1);
}
console.log('parity verdict: PASS');
