import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const script = path.join(repoRoot, "scripts", "check-js-toolchain.mjs");
const posixShUnavailable = process.platform === "win32" ? "requires a POSIX /bin/sh" : false;

// Runs the real script on the suite's own Node with a PATH that holds nothing
// but (optionally) a fake npm reporting `npmVersion`.
async function runCheck(t, { npmVersion } = {}) {
  const bin = await mkdtemp(path.join(os.tmpdir(), "vela-toolchain-"));
  t.after(() => rm(bin, { recursive: true, force: true }));
  if (npmVersion !== undefined) {
    const npm = path.join(bin, "npm");
    await writeFile(npm, `#!/bin/sh\nprintf '${npmVersion}\\n'\n`);
    await chmod(npm, 0o755);
  }
  const env = { PATH: bin };
  for (const name of ["SystemRoot", "ComSpec"]) {
    if (process.env[name]) env[name] = process.env[name];
  }
  return spawnSync(process.execPath, [script], { encoding: "utf8", env });
}

function assertNoStackTrace(stderr) {
  assert.doesNotMatch(stderr, /\n\s+at /);
  assert.ok(!stderr.includes("node:internal"), stderr);
}

test("a missing npm is reported in one actionable line", async (t) => {
  const run = await runCheck(t);
  assert.equal(run.status, 1, run.stderr);
  assert.match(
    run.stderr,
    /^toolchain: npm was not found or could not run; install npm \S+ \(package\.json engines\) and put it on PATH\n$/,
  );
  assertNoStackTrace(run.stderr);
});

test("an out-of-range npm is reported in one line", { skip: posixShUnavailable }, async (t) => {
  const run = await runCheck(t, { npmVersion: "1.0.0" });
  assert.equal(run.status, 1, run.stderr);
  assert.match(run.stderr, /^toolchain: expected Node .*; got v\S+\/npm 1\.0\.0\n$/);
  assertNoStackTrace(run.stderr);
});

test("an in-range toolchain still passes", { skip: posixShUnavailable }, async (t) => {
  const run = await runCheck(t, { npmVersion: "99.0.0" });
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stdout, /^toolchain: Node v\S+\/npm 99\.0\.0\n$/);
  assert.equal(run.stderr, "");
});
