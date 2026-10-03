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

const npmVersion = execSync("npm --version", { encoding: "utf8" }).trim();

if (
  !satisfies(process.version, nodeRange) ||
  !satisfies(npmVersion, npmRange)
) {
  throw new Error(
    `expected Node ${nodeRange}/npm ${npmRange}; got ${process.version}/npm ${npmVersion}`,
  );
}

console.log(`toolchain: Node ${process.version}/npm ${npmVersion}`);
