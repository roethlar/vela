#!/usr/bin/env node

import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";

const nodeVersion = readFileSync(
  new URL("../.node-version", import.meta.url),
  "utf8",
).trim();
const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
const npmMatch = /^npm@(.+)$/.exec(packageJson.packageManager ?? "");

if (!nodeVersion || !npmMatch) {
  throw new Error(
    "expected .node-version and package.json packageManager npm@<version> pins",
  );
}

const expectedNode = `v${nodeVersion}`;
const expectedNpm = npmMatch[1];
const npmVersion = execSync("npm --version", { encoding: "utf8" }).trim();

if (process.version !== expectedNode || npmVersion !== expectedNpm) {
  throw new Error(
    `expected Node ${expectedNode}/npm ${expectedNpm}; got ${process.version}/npm ${npmVersion}`,
  );
}

console.log(`toolchain: Node ${process.version}/npm ${npmVersion}`);
