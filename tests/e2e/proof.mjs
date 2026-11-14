// What a scenario reaches while it runs, recorded for the runner (run.mjs),
// which checks it against what the scenario declares (scenario-loader.mjs,
// proofProblems) once run() has returned:
//
// - shown(row, ...): the scenario has reached the end of its proof of these
//   inventory rows in this run's player. A row it declares and does not
//   report fails the scenario, so a run() that returns early, or does
//   nothing, in one player cannot count for that player (guard finding atk-3
//   of the parity review); a row it reports and does not declare fails it
//   too (an over-claim is caught the same way: the claiming scenario never
//   reaches the proof).
// - testOnlyRoute(id): a step went through a route no user of this player
//   has (the ids are TEST_ONLY_ROUTES). The scenario must declare it in
//   `testOnly` for this player, and must take every route it declares there
//   (guard finding atk-5).
//
// Importing this module does nothing; the runner calls beginProof() before
// each scenario.

// The test-only routes, by id: where each is taken.
export const TEST_ONLY_ROUTES = {
  clickUnderPlay: 'helpers.mjs clickUnderPlay, in the embedded player: a click on the page under the player view',
  exitVelaFromPage: "helpers.mjs exitVelaFromPage: the page invokes exit_vela, the recovery screens' Exit Vela command",
  mpvInput: "helpers.mjs mpvInput: mpv's `mouse` and `keypress` commands over the IPC socket, into mpv's input layer",
};

let current = null;

export function beginProof() {
  current = { shown: new Set(), routes: new Set() };
}

export function endProof() {
  const proof = current ?? { shown: new Set(), routes: new Set() };
  current = null;
  return { shown: [...proof.shown], routes: [...proof.routes] };
}

export function shown(...rows) {
  for (const row of rows) current?.shown.add(row);
}

export function testOnlyRoute(id) {
  if (!(id in TEST_ONLY_ROUTES)) throw new Error(`unknown test-only route "${id}"`);
  current?.routes.add(id);
}
