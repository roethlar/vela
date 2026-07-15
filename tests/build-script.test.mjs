import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

async function executable(file, body) {
  await writeFile(file, body);
  await chmod(file, 0o755);
}

test("an explicit Linux build collects only fresh requested bundles", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-build-script-"));
  t.after(() => rm(root, { recursive: true, force: true }));

  const scripts = path.join(root, "scripts");
  const fakeBin = path.join(root, "fake-bin");
  const bundleRoot = path.join(
    root,
    "src-tauri",
    "target",
    "release",
    "bundle",
  );
  await Promise.all([
    mkdir(scripts, { recursive: true }),
    mkdir(fakeBin, { recursive: true }),
    mkdir(path.join(root, "node_modules"), { recursive: true }),
    mkdir(path.join(bundleRoot, "appimage"), { recursive: true }),
    mkdir(path.join(bundleRoot, "deb"), { recursive: true }),
    mkdir(path.join(bundleRoot, "rpm"), { recursive: true }),
  ]);

  const source = await readFile(path.join(repoRoot, "scripts", "build.sh"), "utf8");
  const testScript = source.replace(
    "extra_args=()",
    "extra_args=(--test-placeholder)",
  );
  assert.notEqual(
    testScript,
    source,
    "the fixture must isolate the recorded Bash 3 empty-array issue",
  );
  const script = path.join(scripts, "build.sh");
  await writeFile(script, testScript);
  await chmod(script, 0o755);

  await writeFile(path.join(root, "node_modules", ".package-lock.json"), "{}\n");
  await writeFile(
    path.join(bundleRoot, "appimage", "Vela_0.1.39_aarch64.AppImage"),
    "stale\n",
  );
  await writeFile(
    path.join(bundleRoot, "deb", "Vela_0.1.50_arm64.deb"),
    "stale\n",
  );
  await writeFile(
    path.join(bundleRoot, "rpm", "Vela-0.1.50-1.aarch64.rpm"),
    "stale\n",
  );

  await executable(path.join(fakeBin, "uname"), "#!/bin/sh\nprintf 'Linux\\n'\n");
  await executable(path.join(fakeBin, "node"), "#!/bin/sh\nexit 0\n");
  await executable(
    path.join(fakeBin, "npm"),
    `#!/bin/sh
set -eu
expected='run tauri -- build --bundles deb,rpm --test-placeholder'
[ "$*" = "$expected" ] || { printf 'unexpected npm command: %s\\n' "$*" >&2; exit 99; }
mkdir -p src-tauri/target/release/bundle/deb src-tauri/target/release/bundle/rpm
printf 'fresh\\n' > src-tauri/target/release/bundle/deb/Vela_0.1.51_arm64.deb
printf 'fresh\\n' > src-tauri/target/release/bundle/rpm/Vela-0.1.51-1.aarch64.rpm
`,
  );

  const run = spawnSync("/bin/bash", [script, "--bundles", "deb,rpm"], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, PATH: `${fakeBin}:${process.env.PATH}` },
  });
  assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);

  assert.deepEqual((await readdir(path.join(root, "dist"))).sort(), [
    "Vela-0.1.51-1.aarch64.rpm",
    "Vela_0.1.51_arm64.deb",
  ]);
  assert.deepEqual(await readdir(path.join(bundleRoot, "deb")), [
    "Vela_0.1.51_arm64.deb",
  ]);
  assert.deepEqual(await readdir(path.join(bundleRoot, "rpm")), [
    "Vela-0.1.51-1.aarch64.rpm",
  ]);
  assert.deepEqual(await readdir(path.join(bundleRoot, "appimage")), [
    "Vela_0.1.39_aarch64.AppImage",
  ]);
});
