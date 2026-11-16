// The one place scenarios are loaded and understood. The runner (run.mjs) and
// the parity test (tests/e2e-parity.test.mjs) both use it, so the two cannot
// disagree about what a scenario says. It REFUSES a scenario it does not
// understand: a key it does not know, a player it does not know, a missing or
// unknown row, a name that is not its file's. What a scenario declares is
// described in tests/e2e/README.md ("What a scenario declares").
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { APP_BINARY_FROM } from './app-binary.mjs';
import { EXEMPTIONS, INVENTORY, KNOWN_FAILURES, PLAYERS, ROW_IDS, SPLITS } from './parity.mjs';
import { TEST_ONLY_ROUTES } from './proof.mjs';

const FUNCTION_KEYS = ['seed', 'environment', 'cleanup', 'run'];
const DATA_KEYS = ['name', 'onlyPlayer', 'onlyPlayerWhy', 'rows', 'noRows', 'testOnly'];
export const SCENARIO_KEYS = [...DATA_KEYS, ...FUNCTION_KEYS];
const TEST_ONLY_KEYS = ['row', 'player', 'via', 'route', 'proves', 'unproven'];

const text = (value) => typeof value === 'string' && value.trim().length > 0;

// Everything wrong with one scenario's default export, as sentences; empty
// when it is understood. `expectedName` is what its file says its name is.
export function scenarioProblems(scenario, expectedName) {
  const where = `scenario file for "${expectedName}"`;
  if (scenario === null || typeof scenario !== 'object') {
    return [`${where}: the default export is not an object`];
  }
  const problems = [];
  for (const key of Object.keys(scenario)) {
    if (!SCENARIO_KEYS.includes(key)) problems.push(`${where}: unknown key "${key}"`);
  }
  if (scenario.name !== expectedName) {
    problems.push(`${where}: its name is ${JSON.stringify(scenario.name)}, not "${expectedName}" as its file says`);
  }
  for (const key of FUNCTION_KEYS) {
    if (key in scenario && typeof scenario[key] !== 'function') problems.push(`${where}: "${key}" is not a function`);
  }
  if (typeof scenario.run !== 'function') problems.push(`${where}: no run()`);

  if ('onlyPlayer' in scenario) {
    if (!PLAYERS.includes(scenario.onlyPlayer)) {
      problems.push(`${where}: unknown onlyPlayer ${JSON.stringify(scenario.onlyPlayer)} (one of ${PLAYERS.join(', ')})`);
    }
    if (!text(scenario.onlyPlayerWhy)) problems.push(`${where}: onlyPlayer without onlyPlayerWhy, the reason it is one player's`);
  } else if ('onlyPlayerWhy' in scenario) {
    problems.push(`${where}: onlyPlayerWhy without onlyPlayer`);
  }

  if (!('rows' in scenario)) {
    problems.push(`${where}: no "rows" (the inventory rows it stands for; [] with "noRows" when none)`);
  } else if (!Array.isArray(scenario.rows)) {
    problems.push(`${where}: "rows" is not an array`);
  } else {
    for (const row of scenario.rows) {
      if (!ROW_IDS.includes(row)) {
        problems.push(`${where}: unknown row ${JSON.stringify(row)} (rows are ${ROW_IDS.join(', ')}, as strings)`);
      }
    }
    if (new Set(scenario.rows).size !== scenario.rows.length) problems.push(`${where}: a row is named twice`);
    if (scenario.rows.length === 0 && !text(scenario.noRows)) {
      problems.push(`${where}: no rows and no "noRows" saying why it stands for none`);
    }
    if (scenario.rows.length > 0 && 'noRows' in scenario) problems.push(`${where}: rows and "noRows" both`);
  }

  if ('testOnly' in scenario) {
    if (!Array.isArray(scenario.testOnly) || scenario.testOnly.length === 0) {
      problems.push(`${where}: "testOnly" is not a non-empty array`);
    } else {
      for (const route of scenario.testOnly) {
        const about = `${where}: testOnly ${JSON.stringify(route?.route ?? route)}`;
        if (route === null || typeof route !== 'object') {
          problems.push(`${about} is not an object`);
          continue;
        }
        for (const key of Object.keys(route)) {
          if (!TEST_ONLY_KEYS.includes(key)) problems.push(`${about}: unknown key "${key}"`);
        }
        if (!Array.isArray(scenario.rows) || !scenario.rows.includes(route.row)) {
          problems.push(`${about}: row ${JSON.stringify(route.row)} is not one of the scenario's rows`);
        }
        if (!PLAYERS.includes(route.player)) {
          problems.push(`${about}: unknown player ${JSON.stringify(route.player)}`);
        } else if (scenario.onlyPlayer && scenario.onlyPlayer !== route.player) {
          problems.push(`${about}: player ${route.player}, but the scenario runs only in ${scenario.onlyPlayer}`);
        }
        // Own names only: `in` is also true of what every object inherits,
        // such as toString (review finding pa-8).
        if (typeof route.via !== 'string' || !Object.hasOwn(TEST_ONLY_ROUTES, route.via)) {
          problems.push(
            `${about}: "via" is ${JSON.stringify(route.via)}, not one of the test-only routes (${Object.keys(TEST_ONLY_ROUTES).join(', ')}; proof.mjs)`,
          );
        }
        for (const key of ['route', 'proves', 'unproven']) {
          if (!text(route[key])) problems.push(`${about}: no "${key}"`);
        }
      }
    }
  }
  return problems;
}

// Everything wrong with what one scenario reached in a run of `player` (its
// `proof`, from proof.mjs: { shown, routes }) against what it declares, as
// sentences; empty when they agree. The runner fails the scenario on any.
export function proofProblems(scenario, player, proof) {
  const problems = [];
  for (const row of scenario.rows) {
    if (!proof.shown.includes(row)) {
      problems.push(`declares row ${row} but did not reach the end of its proof in the ${player} player (no shown('${row}'))`);
    }
  }
  for (const row of new Set(proof.shown)) {
    if (!scenario.rows.includes(row)) problems.push(`reported row ${row} as shown, which it does not declare`);
  }
  const declared = (scenario.testOnly ?? []).filter((r) => r.player === player).map((r) => r.via);
  for (const route of proof.routes) {
    if (!declared.includes(route)) {
      problems.push(
        `took the test-only route ${route} in the ${player} player without declaring it in "testOnly" for that player`,
      );
    }
  }
  for (const route of new Set(declared)) {
    if (!proof.routes.includes(route)) {
      problems.push(`declares the test-only route ${route} for the ${player} player, but did not take it`);
    }
  }
  return problems;
}

// What a run's results file records of the tree it ran (run.mjs): the commit
// and `git status --porcelain` read before the build and the first scenario
// (`start`) and again after each scenario and at the end (`samples`). The head
// recorded is the start's, the one whose code the run built (a run made with
// --skip-build built nothing, records that, and is refused by the verdict);
// the run is dirty when the tree was dirty at the start or differed from it at
// any sample (review finding conf-1).
export function recordedTree(start, samples) {
  const changedDuringRun = samples.some((s) => s.head !== start.head || s.status !== start.status);
  return { head: start.head, dirty: start.status !== '' || changedDuringRun, changedDuringRun };
}

// A value from a results file, for a sentence about it: short enough to read.
const said = (value) => {
  const text = JSON.stringify(value) ?? String(value);
  return text.length > 200 ? `${text.slice(0, 200)}…` : text;
};

// One description of the results file the runner writes (run.mjs), in three
// parts, each of which says what is wrong as sentences or the end of one. The
// verdict applies them to the files it is given, and the runner applies them
// to the file it has just written (resultsFileProblems), so that what is
// written and what is read cannot drift apart unseen. A file is taken only as
// the runner could have written it; nothing in it gets the benefit of the
// doubt (review finding pa-5). What no rule here can tell is a file made by
// hand that is exactly like one of the runner's.

// The top of the file: exactly these names, each of its kind.
const RESULTS_FILE_KEYS = ['appBinary', 'appBinaryFrom', 'built', 'changedDuringRun', 'dirty', 'finishedAt', 'full', 'head', 'player', 'scenarios'];
export function resultsTopProblems(run, player) {
  const about = `the ${player} results`;
  if (run === null || typeof run !== 'object' || Array.isArray(run)) return [`${about} are not a results file: ${said(run)}`];
  const problems = [];
  const keys = Object.keys(run).sort().join(', ');
  if (keys !== RESULTS_FILE_KEYS.join(', ')) problems.push(`${about} hold ${keys || 'nothing'}, not ${RESULTS_FILE_KEYS.join(', ')}`);
  if (run.player !== player) problems.push(`${about} are of a ${run.player} run`);
  for (const flag of ['full', 'built', 'dirty', 'changedDuringRun']) {
    if (typeof run[flag] !== 'boolean') problems.push(`${about} give "${flag}" as ${said(run[flag])}, not true or false`);
  }
  // recordedTree: a tree that changed during the run is a dirty run.
  if (run.changedDuringRun === true && run.dirty !== true) {
    problems.push(`${about} say the tree changed during the run and that the run was clean`);
  }
  if (typeof run.head !== 'string' || run.head === '') problems.push(`${about} name no commit`);
  if (typeof run.appBinary !== 'string' || !path.isAbsolute(run.appBinary)) {
    problems.push(`${about} name no app binary by its full path (${said(run.appBinary)})`);
  }
  // locateAppBinary: a run that built took the binary from Cargo's report of
  // the build, and one that did not, from the prediction.
  if (typeof run.built === 'boolean' && run.appBinaryFrom !== (run.built ? APP_BINARY_FROM.build : APP_BINARY_FROM.prediction)) {
    problems.push(`${about} say the app binary came from ${said(run.appBinaryFrom)}, which a run with "built" ${run.built} does not write`);
  }
  if (typeof run.finishedAt !== 'string' || Number.isNaN(Date.parse(run.finishedAt)) || new Date(run.finishedAt).toISOString() !== run.finishedAt) {
    problems.push(`${about} give no time of finishing as the runner writes it (${said(run.finishedAt)})`);
  }
  if (run.scenarios === null || typeof run.scenarios !== 'object' || Array.isArray(run.scenarios)) {
    problems.push(`${about} hold no scenarios by name`);
  }
  return problems;
}

// One scenario's result, whatever the scenario: `{ status: 'skip' }` and
// nothing else, or status 'pass' or 'fail' with exactly `error`, `shown` and
// `routes`. The two lists are what proof.mjs kept in sets: text, each entry
// once, and a route only if it is a registered one. A failure's error is
// text; a pass has none. Returns why the result is not one the runner
// writes, as the end of a sentence; null when it is.
export function outcomeShapeProblem(result) {
  if (result === null || typeof result !== 'object' || Array.isArray(result)) return `it is ${said(result)}, not a result`;
  const keys = Object.keys(result).sort().join(', ');
  if (result.status === 'skip') return keys === 'status' ? null : `it is a skip that holds ${keys}, not status alone`;
  if (result.status !== 'pass' && result.status !== 'fail') return `its status is ${said(result.status)}, not pass, fail or skip`;
  if (keys !== 'error, routes, shown, status') return `it holds ${keys || 'nothing'}, not status, error, shown and routes`;
  for (const list of ['shown', 'routes']) {
    if (!Array.isArray(result[list]) || result[list].some((entry) => typeof entry !== 'string')) {
      return `its "${list}" is ${said(result[list])}, not a list of text`;
    }
    const twice = result[list].find((entry, i) => result[list].indexOf(entry) !== i);
    if (twice !== undefined) return `its "${list}" names ${said(twice)} twice, and the runner writes each once`;
  }
  const unregistered = result.routes.find((route) => !Object.hasOwn(TEST_ONLY_ROUTES, route));
  if (unregistered !== undefined) return `its "routes" names ${said(unregistered)}, which is no test-only route`;
  if (result.status === 'fail') {
    return typeof result.error === 'string' ? null : `it failed with an error that is not text (${said(result.error)})`;
  }
  return result.error === null ? null : `it passed with an error (${said(result.error)}), and a pass has none`;
}

// A pass is also held to the runner's own rule for one: a scenario whose
// proof has a problem (proofProblems) is failed by the runner, so a pass with
// such a proof is no result of a run. For a result outcomeShapeProblem has
// cleared.
function passProofProblem(scenario, player, result) {
  const unproven = proofProblems(scenario, player, result);
  return unproven.length > 0 ? `it passed, but it ${unproven.join('; and it ')}: the runner fails such a scenario` : null;
}

// The whole file, as the runner checks the one it has just written: its top,
// and every result in it by itself. (What a result means for its scenario,
// in its player, is the verdict's to judge: verdictProblems.)
export function resultsFileProblems(run, player) {
  const problems = resultsTopProblems(run, player);
  const outcomes = run !== null && typeof run === 'object' && run.scenarios !== null && typeof run.scenarios === 'object' && !Array.isArray(run.scenarios) ? run.scenarios : {};
  for (const [name, result] of Object.entries(outcomes)) {
    const why = outcomeShapeProblem(result);
    if (why) problems.push(`${name} has a result in the ${player} run that the runner does not write: ${why}`);
  }
  return problems;
}

// Everything wrong with the parity as two full runs showed it, one of each
// player (`results`: { external, embedded }, each the results file run.mjs
// writes: { player, full, built, head, dirty, scenarios: { <name>: { status,
// error, shown, routes } } }). The parity test reads the declarations only; this
// reads what happened (guard finding atk-3): every scenario of the player ran;
// each passed, except a known failure, which must fail with its recorded text
// (and is reported once it passes); and every row is shown in each player by
// a scenario that passed there and reported reaching the row's proof, unless
// the row is exempted or RED there (claimed only by known failures).
//
// `checkout` ({ head, dirty }) is the state the verdict itself reads its
// scenarios, KNOWN_FAILURES and exemptions from: both runs must be of exactly
// that commit and it must be clean, or the verdict would judge old evidence by
// new records (a weakened scenario or a new known failure passing on runs that
// never ran it; review finding conf-1). It is required: without it nothing
// ties the runs to what is judged.
export function verdictProblems(
  scenarios,
  results,
  { inventory = INVENTORY, exemptions = EXEMPTIONS, knownFailures = KNOWN_FAILURES, checkout } = {},
) {
  const problems = [];
  const heads = new Set();
  if (!checkout?.head) {
    problems.push('the checked-out commit is unknown, so the runs cannot be tied to the scenarios and records judged');
  } else if (checkout.dirty) {
    problems.push(`the checkout (${checkout.head}) has uncommitted changes: the scenarios and records judged are not a commit's`);
  }
  for (const player of PLAYERS) {
    const run = results?.[player];
    if (!run) {
      problems.push(`no results of the ${player} run`);
      continue;
    }
    // The top of the file, as the runner writes it: its names, their kinds,
    // and what the runner cannot write together.
    problems.push(...resultsTopProblems(run, player));
    // Each of these is judged by what the runner writes and by nothing that
    // merely reads as it: `full: true`, `built: true`, `dirty: false`. A file
    // that does not say so does not get the benefit of the doubt.
    if (run.full !== true) problems.push(`the ${player} results are of a run of named scenarios, not the full suite`);
    // A run that built nothing ran whatever binary was there: its head says
    // which scenarios ran, not which product code (review finding conf2-1).
    if (run.built !== true) {
      problems.push(`the ${player} run did not build the app it ran (--skip-build), so nothing ties its binary to commit ${run.head}`);
    }
    if (run.dirty !== false) problems.push(`the ${player} run was of a worktree with uncommitted changes, or one that changed during the run`);
    if (checkout?.head && run.head !== checkout.head) {
      problems.push(`the ${player} run is of commit ${run.head}, not of the checked-out ${checkout.head}`);
    }
    heads.add(run.head);
    // The runner writes its results by scenario name, in an object. A name
    // is looked up among the object's own only: every object also answers
    // to names it inherits (review finding pa-5, the repair).
    const byName = run.scenarios !== null && typeof run.scenarios === 'object' && !Array.isArray(run.scenarios);
    const ran = byName ? run.scenarios : {};
    const resultOf = (name) => (Object.hasOwn(ran, name) ? ran[name] : undefined);
    for (const name of Object.keys(ran)) {
      if (!scenarios.some((s) => s.name === name)) problems.push(`the ${player} results name "${name}", which is no scenario now`);
    }
    // Every scenario's result is judged only as the runner writes it
    // (run.mjs), whole, before it counts for anything: `{ status: 'skip' }`
    // for a scenario that does not run in this player; otherwise status
    // 'pass' or 'fail' with `error` (null for a pass, text for a failure) and
    // the proof's `shown` and `routes`, both lists of text, and for a pass a
    // proof the runner itself would have passed. Anything else is a problem
    // in its own words and counts for nothing: not as a pass, not as a known
    // failure, not as showing a row (review finding pa-5).
    const passed = new Map();
    for (const scenario of scenarios) {
      const result = resultOf(scenario.name);
      const known = knownFailures.find((k) => k.scenario === scenario.name && k.player === player);
      const record = result !== null && typeof result === 'object' && !Array.isArray(result);
      if (!runsIn(scenario, player)) {
        // A full run names every scenario, the ones it skips too.
        if (result === undefined) {
          problems.push(`${scenario.name} is not in the ${player} results, which name every scenario, a skipped one too`);
        } else if (!record || result.status !== 'skip' || Object.keys(result).length !== 1) {
          problems.push(
            `${scenario.name} does not run in the ${player} player, and its result there is not the skip the runner writes: ${said(result)}`,
          );
        }
      } else if (result === undefined) {
        problems.push(`${scenario.name} did not run in the ${player} run`);
      } else if (!record || (result.status !== 'pass' && result.status !== 'fail')) {
        // Only 'pass' and 'fail' are results of a scenario that ran. A skip,
        // no status, or a status nobody writes is not a failure seen,
        // whatever error text it carries.
        problems.push(
          record && result.status === 'skip'
            ? `${scenario.name} was skipped in the ${player} run, so it has no result there`
            : `${scenario.name} has no result in the ${player} run: its status is ${JSON.stringify(record ? result.status : undefined)}, neither pass nor fail`,
        );
      } else if (outcomeShapeProblem(result) ?? (result.status === 'pass' ? passProofProblem(scenario, player, result) : null)) {
        const why = outcomeShapeProblem(result) ?? passProofProblem(scenario, player, result);
        problems.push(`${scenario.name} has a result in the ${player} run that the runner does not write: ${why}`);
      } else if (result.status === 'pass') {
        passed.set(scenario.name, result);
        if (known && !known.intermittent) {
          problems.push(`${scenario.name} passed in the ${player} player: its known failure is gone; take it out of KNOWN_FAILURES`);
        }
      } else if (!known) {
        problems.push(`${scenario.name} failed in the ${player} player: ${result.error}`);
      } else if (!result.error.includes(known.failsWith)) {
        problems.push(`${scenario.name} failed in the ${player} player, but not with its known failure: ${result.error}`);
      }
    }
    for (const { id } of inventory) {
      if (exemptions.some((e) => e.row === id && e.player === player)) continue;
      if (redCoverage(scenarios, id, player, knownFailures).length > 0 && coverage(scenarios, id, player, knownFailures).length === 0) continue;
      // Only a result that stood as a pass above shows a row.
      const showing = coverage(scenarios, id, player, knownFailures).filter((name) => passed.get(name)?.shown.includes(id));
      if (showing.length === 0) problems.push(`row ${id} is shown by no scenario that passed in the ${player} run`);
    }
  }
  if (heads.size > 1) problems.push(`the two runs are of different commits (${[...heads].join(', ')})`);
  return problems;
}

// The scenarios that show `row` in `player` only through a test-only route:
// they declare one for that row and player.
export const testOnlyCoverage = (scenarios, row, player, knownFailures = KNOWN_FAILURES) =>
  coverage(scenarios, row, player, knownFailures).filter((name) =>
    (scenarios.find((s) => s.name === name).testOnly ?? []).some((r) => r.row === row && r.player === player),
  );

// Imports every *.mjs in `dir` (sorted) and returns their default exports.
// Throws, listing every problem in every file, when any is not understood.
// A scenario's name is its file's name, with `prefix` in front (the live
// scenarios are named live-<file>).
export async function loadScenarios(dir, { prefix = '' } = {}) {
  const scenarios = [];
  const problems = [];
  for (const file of fs.readdirSync(dir).filter((f) => f.endsWith('.mjs')).sort()) {
    const { default: scenario } = await import(pathToFileURL(path.join(dir, file)).href);
    problems.push(...scenarioProblems(scenario, `${prefix}${file.slice(0, -'.mjs'.length)}`));
    scenarios.push(scenario);
  }
  if (problems.length > 0) {
    throw new Error(`e2e: scenarios that are not understood (tests/e2e/README.md, "What a scenario declares"):\n  ${problems.join('\n  ')}`);
  }
  return scenarios;
}

// Whether `scenario` runs in a run of `player`.
export const runsIn = (scenario, player) => !scenario.onlyPlayer || scenario.onlyPlayer === player;

// Whether `scenario` is recorded as failing in `player` on a product defect.
export const knownToFail = (scenario, player, knownFailures = KNOWN_FAILURES) =>
  knownFailures.some((k) => k?.scenario === scenario.name && k?.player === player);

// The scenarios that show `row` in `player`: they run there, claim the row,
// and are not recorded as failing there (a scenario that fails shows nothing).
export const coverage = (scenarios, row, player, knownFailures = KNOWN_FAILURES) =>
  scenarios
    .filter((s) => runsIn(s, player) && s.rows.includes(row) && !knownToFail(s, player, knownFailures))
    .map((s) => s.name);

// The scenarios that claim `row` in `player` but are recorded as failing there.
export const redCoverage = (scenarios, row, player, knownFailures = KNOWN_FAILURES) =>
  scenarios
    .filter((s) => runsIn(s, player) && s.rows.includes(row) && knownToFail(s, player, knownFailures))
    .map((s) => s.name);

// The row numbers of the plan's behaviour inventory: the table under
// "## Behaviour inventory" in .agents/plans/embedded-player.md.
export function planRowNumbers(planText) {
  const start = planText.indexOf('## Behaviour inventory');
  if (start < 0) return [];
  const rest = planText.slice(start);
  const end = rest.indexOf('\n## ');
  const section = end < 0 ? rest : rest.slice(0, end);
  return [...section.matchAll(/^\|\s*(\d+)\s*\|/gm)].map((m) => m[1]);
}

// Everything wrong with the inventory against the plan's rows: every plan row
// must be in it, whole (`'7'`) or, where `splits` (parity.mjs, SPLITS) says it
// is split, as every one of its approved parts (`'8a'`, `'8b'`); it may hold
// nothing else. So a row cannot be dropped, half of a split row dropped, or a
// row or part made up, by editing parity.mjs's INVENTORY alone (guard finding
// atk-4; review finding pa-3).
export function inventoryProblems(inventory, planNumbers, splits = SPLITS) {
  const problems = [];
  if (planNumbers.length === 0) problems.push("the plan's behaviour inventory table was not found");
  const ids = inventory.map((row) => row.id);
  for (const n of Object.keys(splits)) {
    if (planNumbers.length > 0 && !planNumbers.includes(n)) problems.push(`split row ${n} is no row of the plan`);
  }
  for (const n of planNumbers) {
    const approved = Object.hasOwn(splits, n) ? splits[n] : null;
    if (approved) {
      if (ids.includes(n)) problems.push(`plan row ${n} is in the inventory both whole and split (its parts are ${approved.join(', ')})`);
      for (const part of approved) {
        if (!ids.includes(part)) problems.push(`part ${part} of split plan row ${n} is not in the inventory`);
      }
    } else if (!ids.includes(n)) {
      problems.push(`plan row ${n} is not in the inventory`);
    }
  }
  for (const id of ids) {
    const n = /^(\d+)[a-z]?$/.exec(id)?.[1];
    if (!n || !planNumbers.includes(n)) {
      problems.push(`inventory row ${JSON.stringify(id)} is no row of the plan`);
    } else if (id !== n && !(Object.hasOwn(splits, n) && splits[n].includes(id))) {
      problems.push(`inventory row ${JSON.stringify(id)} is no approved part of plan row ${n} (SPLITS in parity.mjs)`);
    }
  }
  return problems;
}

// Everything wrong with the parity of a set of (already understood)
// scenarios, as sentences; empty when every row is shown in both players,
// exempted where it is not, or RED there: claimed only by scenarios recorded
// as failing on a product defect (KNOWN_FAILURES), which the parity test
// prints as not shown.
export function parityProblems(
  scenarios,
  { inventory = INVENTORY, exemptions = EXEMPTIONS, knownFailures = KNOWN_FAILURES, repoRoot = null } = {},
) {
  const problems = [];
  const rowIds = inventory.map((row) => row.id);
  if (new Set(rowIds).size !== rowIds.length) problems.push('the inventory names a row twice');
  const names = scenarios.map((s) => s.name);
  for (const name of new Set(names)) {
    if (names.filter((n) => n === name).length > 1) problems.push(`two scenarios are named "${name}"`);
  }
  const seenFailures = new Set();
  for (const known of knownFailures) {
    const about = `known failure ${JSON.stringify({ scenario: known?.scenario, player: known?.player })}`;
    for (const k of Object.keys(known ?? {})) {
      if (!['scenario', 'player', 'failsWith', 'defect', 'intermittent'].includes(k)) problems.push(`${about}: unknown key "${k}"`);
    }
    const scenario = scenarios.find((s) => s.name === known?.scenario);
    if (!scenario) problems.push(`${about}: no such scenario`);
    if (!PLAYERS.includes(known?.player)) problems.push(`${about}: no such player`);
    else if (scenario && !runsIn(scenario, known.player)) problems.push(`${about}: the scenario does not run in that player`);
    if (!text(known?.failsWith)) problems.push(`${about}: no failsWith, the text its failure carries`);
    if (!text(known?.defect)) problems.push(`${about}: no defect, the product defect and its evidence`);
    if ('intermittent' in (known ?? {}) && known.intermittent !== true) problems.push(`${about}: intermittent is true or absent`);
    const key = `${known?.scenario}/${known?.player}`;
    if (seenFailures.has(key)) problems.push(`${about}: recorded twice`);
    seenFailures.add(key);
  }
  const seen = new Set();
  for (const exemption of exemptions) {
    const about = `exemption ${JSON.stringify({ row: exemption?.row, player: exemption?.player })}`;
    if (!rowIds.includes(exemption?.row)) problems.push(`${about}: no such row`);
    if (!PLAYERS.includes(exemption?.player)) problems.push(`${about}: no such player`);
    const key = `${exemption?.row}/${exemption?.player}`;
    if (seen.has(key)) problems.push(`${about}: recorded twice`);
    seen.add(key);
    for (const k of Object.keys(exemption ?? {})) {
      if (!['row', 'player', 'reason', 'heldBy', 'ownerStep'].includes(k)) problems.push(`${about}: unknown key "${k}"`);
    }
    if (!text(exemption?.reason)) problems.push(`${about}: no reason`);
    const heldBy = exemption?.heldBy ?? [];
    if (!Array.isArray(heldBy)) problems.push(`${about}: heldBy is not an array`);
    if (!text(exemption?.ownerStep) && (!Array.isArray(heldBy) || heldBy.length === 0)) {
      problems.push(`${about}: names neither the tests that hold it (heldBy) nor the owner's step (ownerStep)`);
    }
    if ('ownerStep' in (exemption ?? {}) && !text(exemption.ownerStep)) problems.push(`${about}: an empty ownerStep`);
    for (const held of Array.isArray(heldBy) ? heldBy : []) {
      if (!text(held?.file) || !Array.isArray(held?.tests) || held.tests.length === 0 || !held.tests.every(text)) {
        problems.push(`${about}: a heldBy entry needs a file and the names of its tests`);
      } else if (repoRoot && !fs.existsSync(path.join(repoRoot, held.file))) {
        problems.push(`${about}: ${held.file} does not exist`);
      } else if (repoRoot) {
        // A renamed or removed test must not go on being named as holding
        // the row (guard finding atk-9). The name must occur in the file as
        // a whole word (a Rust fn, a node:test title, a scenario's name).
        const source = fs.readFileSync(path.join(repoRoot, held.file), 'utf8');
        for (const name of held.tests) {
          const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
          if (!new RegExp(`(^|[^\\w])${escaped}($|[^\\w])`).test(source)) {
            problems.push(`${about}: ${held.file} has no test named "${name}"`);
          }
        }
      }
    }
    if (rowIds.includes(exemption?.row) && PLAYERS.includes(exemption?.player)) {
      const covering = coverage(scenarios, exemption.row, exemption.player, knownFailures);
      if (covering.length > 0) {
        problems.push(`${about}: stale, row ${exemption.row} is shown in the ${exemption.player} player by ${covering.join(', ')}`);
      }
    }
  }
  for (const id of rowIds) {
    const byPlayer = Object.fromEntries(
      PLAYERS.map((player) => [
        player,
        coverage(scenarios, id, player, knownFailures).length > 0 ||
          redCoverage(scenarios, id, player, knownFailures).length > 0 ||
          exemptions.some((e) => e?.row === id && e?.player === player),
      ]),
    );
    for (const player of PLAYERS) {
      if (byPlayer[player]) continue;
      const others = PLAYERS.filter((p) => p !== player && byPlayer[p]);
      problems.push(
        others.length > 0
          ? `row ${id} is shown in the ${others.join(' and ')} player but neither shown nor exempted in the ${player} player`
          : `row ${id} is neither shown nor exempted in the ${player} player`,
      );
    }
  }
  return problems;
}
