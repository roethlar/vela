#!/usr/bin/env node

import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
const nodeRange = packageJson.engines?.node;
const npmRange = packageJson.engines?.npm;

if (!nodeRange || !npmRange) {
  throw new Error("expected package.json engines.node and engines.npm ranges");
}

// A host condition (no npm, wrong versions) is the builder's to fix, so it gets
// one actionable line rather than an uncaught-exception stack trace.
function fail(message) {
  console.error(`toolchain: ${message}`);
  process.exit(1);
}

function parseVersion(text) {
  const match = /^v?(\d+)(?:\.(\d+))?(?:\.(\d+))?$/.exec(text);
  if (!match) {
    throw new Error(`unparseable version: ${text}`);
  }
  return [1, 2, 3].map((index) => Number(match[index] ?? 0));
}

function compare(left, right) {
  for (let index = 0; index < 3; index += 1) {
    if (left[index] !== right[index]) {
      return left[index] < right[index] ? -1 : 1;
    }
  }
  return 0;
}

// Supports only the space-separated `>=` / `<` comparators the repo uses; any
// other syntax fails closed rather than being guessed at.
function satisfies(version, range) {
  const actual = parseVersion(version);
  return range
    .trim()
    .split(/\s+/)
    .every((comparator) => {
      const match = /^(>=|<)(.+)$/.exec(comparator);
      if (!match) {
        throw new Error(`unsupported engines comparator: ${comparator}`);
      }
      const order = compare(actual, parseVersion(match[2]));
      return match[1] === ">=" ? order >= 0 : order < 0;
    });
}

// Not found, a non-zero exit, and a spawn error are all the same condition
// here. stderr is dropped so the shell's own "command not found" is not
// printed on top of the message below.
let npmVersion;
try {
  npmVersion = execSync("npm --version", {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  }).trim();
} catch {
  fail(
    `npm was not found or could not run; install npm ${npmRange} ` +
      `(package.json engines) and put it on PATH`,
  );
}

if (
  !satisfies(process.version, nodeRange) ||
  !satisfies(npmVersion, npmRange)
) {
  fail(
    `expected Node ${nodeRange}/npm ${npmRange}; got ${process.version}/npm ${npmVersion}`,
  );
}

console.log(`toolchain: Node ${process.version}/npm ${npmVersion}`);
