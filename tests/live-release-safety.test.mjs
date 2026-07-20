import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const livePlex = await readFile(path.join(repoRoot, "tests", "e2e", "live", "plex.mjs"), "utf8");

function sourceSlice(startNeedle, endNeedle) {
  const start = livePlex.indexOf(startNeedle);
  const end = livePlex.indexOf(endNeedle, start + 1);
  assert.ok(start >= 0 && end > start, `expected ${startNeedle} before ${endNeedle}`);
  return livePlex.slice(start, end);
}

test("every live Plex completion fixture is registered clean before playback", () => {
  assert.match(livePlex, /function registerCleanRestore\(item\)[\s\S]*item\.played, false/);
  assert.match(livePlex, /registerCleanRestore\(movie\)/);
  assert.match(livePlex, /registerCleanRestore\(episode\);\n\s*registerCleanRestore\(successor\)/);

  const run = sourceSlice("async run({ driver })", "// ── 3. Scan Library");
  assert.ok(
    run.indexOf("discoverFixtures(driver)") < run.indexOf("completeEpisodeAndContinue(driver, fixtures)"),
    "clean fixtures must be registered before the natural-EOF leg starts",
  );
});

test("the live Plex server is started before Vela receives its saved connection", () => {
  const seed = sourceSlice("async seed({ configRoot })", "async cleanup()");
  assert.ok(
    seed.indexOf('control("/plex/start")') >= 0 &&
      seed.indexOf('control("/plex/start")') < seed.indexOf("seedConfig(configRoot"),
    "Plex must be serving before the app launches and performs its initial library load",
  );
});

test("live Plex cleanup restores every registered item and verifies zero state", () => {
  const cleanup = sourceSlice("async function restoreTargetWatchState", "export default");
  assert.match(cleanup, /for \(const \[ratingKey, title\] of restoreItems\)/);
  assert.match(cleanup, /fetch\(restoreUrl\(ratingKey\)/);
  assert.match(cleanup, /fixtures\.episode, fixtures\.successor/);
  assert.match(cleanup, /item\?\.played === false && \(item\.viewOffsetMs \?\? 0\) === 0/);
  assert.ok(
    cleanup.indexOf("restoreItems.clear()") > cleanup.indexOf("every touched Plex item"),
    "cleanup registrations may clear only after the readback proof",
  );
});

test("the real completion leg proves EOF, Plex watched state, continuation, and refresh", () => {
  const completion = sourceSlice(
    "async function completeEpisodeAndContinue",
    "async function exerciseMovieDetailUi",
  );
  assert.match(completion, /natural EOF is the behavior under test/);
  assert.match(completion, /episode\?\.played === true/);
  assert.match(completion, /!completed && successor/);
  assert.match(completion, /fixtures\.successor\.title/);
  assert.match(completion, /without manual Refresh/);
});
