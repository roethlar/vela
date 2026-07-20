import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const libSource = fs.readFileSync(path.join(repoRoot, "src-tauri", "src", "lib.rs"), "utf8");
const commandsSource = fs.readFileSync(path.join(repoRoot, "src-tauri", "src", "commands.rs"), "utf8");

function maskRustNonCode(source) {
  const masked = source.split("");
  const blank = (index) => {
    if (masked[index] !== "\n" && masked[index] !== "\r") masked[index] = " ";
  };

  for (let index = 0; index < source.length; ) {
    if (source.startsWith("//", index)) {
      while (index < source.length && source[index] !== "\n") blank(index++);
      continue;
    }

    if (source.startsWith("/*", index)) {
      let depth = 0;
      do {
        if (source.startsWith("/*", index)) {
          blank(index++);
          blank(index++);
          depth += 1;
        } else if (source.startsWith("*/", index)) {
          blank(index++);
          blank(index++);
          depth -= 1;
        } else {
          blank(index++);
        }
      } while (index < source.length && depth > 0);
      assert.equal(depth, 0, "src-tauri/src/lib.rs must not contain an unterminated block comment");
      continue;
    }

    const raw = /^(?:br|r)(#+)?"/.exec(source.slice(index));
    if (raw) {
      const hashes = raw[1] ?? "";
      const terminator = `"${hashes}`;
      for (let count = 0; count < raw[0].length; count += 1) blank(index++);
      const end = source.indexOf(terminator, index);
      assert.notEqual(end, -1, "src-tauri/src/lib.rs must not contain an unterminated raw string");
      while (index < end + terminator.length) blank(index++);
      continue;
    }

    const stringPrefix = source.startsWith('b"', index) ? 2 : source[index] === '"' ? 1 : 0;
    if (stringPrefix > 0) {
      for (let count = 0; count < stringPrefix; count += 1) blank(index++);
      let escaped = false;
      let closed = false;
      while (index < source.length) {
        const char = source[index];
        blank(index++);
        if (escaped) {
          escaped = false;
        } else if (char === "\\") {
          escaped = true;
        } else if (char === '"') {
          closed = true;
          break;
        }
      }
      assert.ok(closed, "src-tauri/src/lib.rs must not contain an unterminated string");
      continue;
    }

    const charLiteral = /^(?:b)?'(?:\\.|[^'\\\r\n])'/.exec(source.slice(index));
    if (charLiteral) {
      for (let count = 0; count < charLiteral[0].length; count += 1) blank(index++);
      continue;
    }

    index += 1;
  }

  return masked.join("");
}

function balancedBlock(source, masked, openBrace) {
  assert.equal(masked[openBrace], "{", "the guarded Rust block must start at an opening brace");
  let depth = 0;
  for (let index = openBrace; index < masked.length; index += 1) {
    if (masked[index] === "{") depth += 1;
    if (masked[index] === "}") depth -= 1;
    if (depth === 0) {
      return {
        source: source.slice(openBrace + 1, index),
        masked: masked.slice(openBrace + 1, index),
      };
    }
  }
  assert.fail("the guarded Rust block must have a matching closing brace");
}

function allCodeMatches(source, masked, regex) {
  return [...source.matchAll(regex)].filter((match) => masked[match.index] !== " ");
}

function oneCodeMatch(source, masked, regex, message) {
  const matches = allCodeMatches(source, masked, regex);
  assert.equal(matches.length, 1, message);
  return matches[0];
}

function braceDepth(masked, offset) {
  let depth = 0;
  for (let index = 0; index < offset; index += 1) {
    if (masked[index] === "{") depth += 1;
    if (masked[index] === "}") depth -= 1;
  }
  return depth;
}

function rustFunction(source, name) {
  const masked = maskRustNonCode(source);
  const declarations = allCodeMatches(
    source,
    masked,
    new RegExp(`\\b(?:pub(?:\\(crate\\))?\\s+)?(?:async\\s+)?fn\\s+${name}\\s*\\(`, "g"),
  );
  assert.equal(declarations.length, 1, `commands.rs must define one ${name} function`);
  const openBrace = masked.indexOf("{", declarations[0].index);
  assert.notEqual(openBrace, -1, `${name} must have a body`);
  return balancedBlock(source, masked, openBrace);
}

function joinedCompletionDispatcher(source) {
  const marker = "// Playback sequence dispatcher:";
  assert.equal(source.split(marker).length - 1, 1, "lib.rs must define one playback sequence dispatcher");

  const masked = maskRustNonCode(source);
  const markerIndex = source.indexOf(marker);
  const setupEnd = masked.indexOf(".invoke_handler", markerIndex);
  assert.ok(setupEnd > markerIndex, "the playback dispatcher must remain inside Tauri setup");

  const setupSlice = source.slice(markerIndex, setupEnd);
  const maskedSetupSlice = masked.slice(markerIndex, setupEnd);
  const spawn = oneCodeMatch(
    setupSlice,
    maskedSetupSlice,
    /tauri\s*::\s*async_runtime\s*::\s*spawn\s*\(\s*async\s+move\s*\{/g,
    "the dispatcher section must contain one async runtime task",
  );
  const spawnBrace = markerIndex + spawn.index + spawn[0].lastIndexOf("{");
  const spawnBlock = balancedBlock(source, masked, spawnBrace);

  const loops = allCodeMatches(spawnBlock.source, spawnBlock.masked, /\bloop\s*\{/g);
  assert.equal(loops.length, 1, "the dispatcher task must contain one joined-completion loop");
  const loopBrace = loops[0].index + loops[0][0].lastIndexOf("{");
  const loopBlock = balancedBlock(spawnBlock.source, spawnBlock.masked, loopBrace);
  assert.equal(
    allCodeMatches(loopBlock.source, loopBlock.masked, /advance_notify\s*\.\s*next\s*\(\s*\)\s*\.\s*await/g)
      .length,
    1,
    "the guarded loop must be the clean-EOF/final-tracker join consumer",
  );
  assert.equal(
    allCodeMatches(loopBlock.source, loopBlock.masked, /commands\s*::\s*admit_clean_completion\s*\(/g)
      .length,
    1,
    "the guarded loop must admit each joined clean completion once",
  );
  return loopBlock;
}

test("the joined clean-EOF dispatcher refreshes only after played state settles", () => {
  const dispatcher = joinedCompletionDispatcher(libSource);
  const advance = oneCodeMatch(
    dispatcher.source,
    dispatcher.masked,
    /commands\s*::\s*advance_playlist\s*\(\s*&state\s*,\s*&completion\.session_id\s*\)\s*\.\s*await/g,
    "the dispatcher must advance the admitted playlist exactly once",
  );
  const continuation = oneCodeMatch(
    dispatcher.source,
    dispatcher.masked,
    /app_handle\s*\.\s*emit\s*\(\s*"continue-playing"/g,
    "the dispatcher must contain one terminal continue-playing emit",
  );
  const mark = oneCodeMatch(
    dispatcher.source,
    dispatcher.masked,
    /let\s+watch_result\s*=\s*commands\s*::\s*mark_clean_completion_played\s*\(\s*&state\s*,\s*&completion\s*\)\s*\.\s*await/g,
    "the dispatcher must await one all-backing played-state attempt",
  );
  const markFailure = oneCodeMatch(
    dispatcher.source,
    dispatcher.masked,
    /if\s+watch_result\.failed_sources\s*>\s*0/g,
    "the dispatcher must handle a partial or total backing failure",
  );
  const safeLog = oneCodeMatch(
    dispatcher.source,
    dispatcher.masked,
    /eprintln!\s*\(\s*"vela: automatic played-state update reached \{\} source\(s\); \{\} failed"/g,
    "the dispatcher must log only safe aggregate played-state counts",
  );
  const refreshes = allCodeMatches(
    dispatcher.source,
    dispatcher.masked,
    /app_handle\s*\.\s*emit\s*\(\s*"playback-ended"/g,
  );
  assert.equal(refreshes.length, 1, "the dispatcher must emit playback-ended exactly once");
  const refresh = refreshes[0];

  assert.ok(advance.index < mark.index, "playlist advance must precede the played-state await");
  assert.ok(continuation.index < mark.index, "terminal continuation must precede the played-state await");
  assert.ok(mark.index < refresh.index, "the awaited played-state attempt must settle before playback-ended");
  assert.ok(markFailure.index < refresh.index, "played-state failure handling must finish before playback-ended");
  assert.ok(safeLog.index < refresh.index, "safe played-state logging must finish before playback-ended");
  assert.equal(
    braceDepth(dispatcher.masked, markFailure.index),
    braceDepth(dispatcher.masked, refresh.index),
    "playback-ended must remain unconditional outside the played-state warning branch",
  );
});

test("manual and clean-EOF watched mutations share the all-backing boundary", () => {
  const manual = rustFunction(commandsSource, "set_watched");
  const collect = oneCodeMatch(
    manual.source,
    manual.masked,
    /watched_state_backings\s*\(\s*&item\s*\)/g,
    "the manual command must derive every title backing from the submitted item",
  );
  const prepare = oneCodeMatch(
    manual.source,
    manual.masked,
    /prepare_watch_mutations\s*\(\s*&state\.registry\s*,\s*&backings\s*\)\s*\.\s*await/g,
    "the manual command must route every backing before provider awaits",
  );
  const curate = oneCodeMatch(
    manual.source,
    manual.masked,
    /recents\s*::\s*hide_with_undo\s*\(/g,
    "the manual command must retain curate-before-network undo state",
  );
  const execute = oneCodeMatch(
    manual.source,
    manual.masked,
    /execute_watch_mutations\s*\(\s*prepared\s*,\s*played\s*\)\s*\.\s*await/g,
    "the manual command must execute the prepared all-backing mutation",
  );
  assert.ok(collect.index < prepare.index && prepare.index < curate.index && curate.index < execute.index);

  const failure = oneCodeMatch(
    manual.source,
    manual.masked,
    /if\s+all_watch_mutations_failed\s*\(\s*&result\s*\)\s*\{/g,
    "the manual command must distinguish zero success from partial success",
  );
  const failureBrace = failure.index + failure[0].lastIndexOf("{");
  const failureBlock = balancedBlock(manual.source, manual.masked, failureBrace);
  assert.equal(
    allCodeMatches(failureBlock.source, failureBlock.masked, /recents\s*::\s*restore_hidden\s*\(/g).length,
    1,
    "only zero success may restore local curation",
  );
  assert.equal(
    allCodeMatches(failureBlock.source, failureBlock.masked, /return\s+Err\s*\(/g).length,
    1,
    "zero success must return failure",
  );
  assert.equal(
    allCodeMatches(manual.source, manual.masked, /\bOk\s*\(\s*result\s*\)/g).length,
    1,
    "partial or complete success must return its structured result",
  );

  const clean = rustFunction(commandsSource, "mark_clean_completion_played");
  for (const [regex, message] of [
    [/completion_watch_backings\s*\(\s*completion\s*\)/g, "clean EOF must use its immutable backing set"],
    [
      /prepare_watch_mutations\s*\(\s*&state\.registry\s*,\s*&backings\s*\)\s*\.\s*await/g,
      "clean EOF must route every captured backing",
    ],
    [
      /execute_watch_mutations\s*\(\s*prepared\s*,\s*true\s*\)\s*\.\s*await/g,
      "clean EOF must mark every prepared backing played",
    ],
  ]) {
    oneCodeMatch(clean.source, clean.masked, regex, message);
  }
  assert.match(
    commandsSource,
    /#\[serde\(skip\)\]\s*pub watch_backings: Vec<BackingRef>/,
    "completion backing identities must remain backend-only",
  );
  assert.equal(
    (commandsSource.match(/let watch_backings = watched_state_backings\(item\);/g) ?? []).length,
    1,
    "playback launch must capture one immutable backing set",
  );
  assert.equal(
    (commandsSource.match(/watch_backings: watch_backings\.clone\(\)/g) ?? []).length,
    1,
    "the exact completion must carry the captured backing set",
  );
});
