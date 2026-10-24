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

const posixBashUnavailable = process.platform === "win32" ? "requires a POSIX /bin/bash" : false;

test("an explicit Linux build collects only fresh requested bundles", { skip: posixBashUnavailable }, async (t) => {
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
  await fakeLinuxPackageTools(fakeBin);
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

// Fakes for every package manager build.sh knows, so the prerequisite check is
// hermetic whatever distro (or macOS) the suite runs on. Packages whose name
// matches `missingPattern` report as not installed; installs are logged.
async function fakeLinuxPackageTools(fakeBin, { missingPattern = "", callLog = "/dev/null" } = {}) {
  const missing = `case "$pkg" in *${missingPattern || "__nothing__"}*) exit 1 ;; esac`;
  const lastArg = `for pkg in "$@"; do :; done`;
  const log = `printf '%s %s\\n' "$(basename "$0")" "$*" >> "${callLog}"`;
  await executable(path.join(fakeBin, "cargo"), "#!/bin/sh\nexit 0\n");
  await executable(path.join(fakeBin, "id"), "#!/bin/sh\nprintf '1000\\n'\n");
  await executable(path.join(fakeBin, "sudo"), `#!/bin/sh\n${log}\n`);
  await executable(
    path.join(fakeBin, "pacman"),
    `#!/bin/sh\n[ "$1" = -Q ] || { ${log}; exit 0; }\n${lastArg}\n${missing}\n`,
  );
  await executable(
    path.join(fakeBin, "dpkg-query"),
    `#!/bin/sh\n${lastArg}\n${missing}\nprintf 'installed'\n`,
  );
  await executable(path.join(fakeBin, "rpm"), `#!/bin/sh\n${lastArg}\n${missing}\n`);
  await executable(path.join(fakeBin, "apt-get"), `#!/bin/sh\n${log}\n`);
  await executable(path.join(fakeBin, "dnf"), `#!/bin/sh\n${log}\n`);
}

async function linuxPrereqFixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-build-prereq-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const scripts = path.join(root, "scripts");
  const fakeBin = path.join(root, "fake-bin");
  const callLog = path.join(root, "calls.log");
  await Promise.all([
    mkdir(scripts, { recursive: true }),
    mkdir(fakeBin, { recursive: true }),
    mkdir(path.join(root, "node_modules"), { recursive: true }),
  ]);
  await copyFile(path.join(repoRoot, "scripts", "build.sh"), path.join(scripts, "build.sh"));
  await chmod(path.join(scripts, "build.sh"), 0o755);
  await writeFile(path.join(root, "node_modules", ".package-lock.json"), "{}\n");
  await writeFile(callLog, "");
  await executable(path.join(fakeBin, "uname"), "#!/bin/sh\nprintf 'Linux\\n'\n");
  await executable(path.join(fakeBin, "node"), "#!/bin/sh\nexit 0\n");
  await executable(
    path.join(fakeBin, "npm"),
    `#!/bin/sh
printf 'npm %s\\n' "$*" >> "${callLog}"
mkdir -p src-tauri/target/release/bundle/deb
printf 'fresh\\n' > src-tauri/target/release/bundle/deb/Vela_1.0.0_amd64.deb
`,
  );
  await fakeLinuxPackageTools(fakeBin, { missingPattern: "webkit", callLog });
  const run = (...args) =>
    spawnSync("/bin/bash", [path.join(scripts, "build.sh"), "--bundles", "deb", ...args], {
      cwd: root,
      encoding: "utf8",
      input: "",
      env: { ...process.env, PATH: `${fakeBin}:${process.env.PATH}` },
    });
  return { root, callLog, run };
}

test("a Linux build names missing system packages and stops before building", { skip: posixBashUnavailable }, async (t) => {
  const fixture = await linuxPrereqFixture(t);
  const run = fixture.run();
  assert.notEqual(run.status, 0);
  assert.match(run.stderr, /Missing build prerequisites: \S*webkit\S*\n/);
  assert.match(run.stderr, /Install with: sudo .*webkit/);
  assert.match(run.stderr, /--install-deps/);
  assert.equal(await readFile(fixture.callLog, "utf8"), "");
});

test("a Linux build with --install-deps installs only the missing packages, then builds", { skip: posixBashUnavailable }, async (t) => {
  const fixture = await linuxPrereqFixture(t);
  const run = fixture.run("--install-deps");
  assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
  const calls = (await readFile(fixture.callLog, "utf8")).trim().split("\n");
  assert.match(calls[0], /^sudo (pacman -S --needed|apt-get install -y|dnf install -y) \S*webkit\S*$/);
  assert.match(calls[1], /^npm run tauri -- build --bundles deb$/);
  assert.deepEqual(await readdir(path.join(fixture.root, "dist")), ["Vela_1.0.0_amd64.deb"]);
});

test("the native macOS build works with Bash 3 empty-array nounset semantics", { skip: posixBashUnavailable }, async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-build-native-"));
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
  ]);

  const source = await readFile(path.join(repoRoot, "scripts", "build.sh"), "utf8");
  const script = path.join(scripts, "build.sh");
  await writeFile(script, source);
  await chmod(script, 0o755);
  await writeFile(path.join(root, "node_modules", ".package-lock.json"), "{}\n");

  await executable(path.join(fakeBin, "uname"), "#!/bin/sh\nprintf 'Darwin\\n'\n");
  await executable(path.join(fakeBin, "node"), "#!/bin/sh\nexit 0\n");
  await executable(
    path.join(fakeBin, "npm"),
    `#!/bin/sh
set -eu
expected='run tauri -- build --bundles dmg'
[ "$*" = "$expected" ] || { printf 'unexpected npm command: %s\n' "$*" >&2; exit 99; }
mkdir -p src-tauri/target/release/bundle/dmg src-tauri/target/release/bundle/macos
printf 'dmg\n' > src-tauri/target/release/bundle/dmg/Vela_1.0.0_aarch64.dmg
mkdir -p src-tauri/target/release/bundle/macos/Vela.app
printf 'app\n' > src-tauri/target/release/bundle/macos/Vela.app/marker
`,
  );

  const run = spawnSync("/bin/bash", [script, "--native"], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, PATH: `${fakeBin}:${process.env.PATH}` },
  });
  assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
  assert.deepEqual((await readdir(path.join(root, "dist"))).sort(), [
    "Vela.app",
    "Vela_1.0.0_aarch64.dmg",
  ]);
});

async function signedMacFixture(t, { identityAvailable = true, keyMode = 0o600 } = {}) {
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-build-signed-"));
  t.after(() => rm(root, { recursive: true, force: true }));

  const scripts = path.join(root, "scripts");
  const fakeBin = path.join(root, "fake-bin");
  const signingDir = path.join(root, "Certificates");
  const keyPath = path.join(signingDir, "AuthKey_TEST123.p8");
  const callLog = path.join(root, "calls.log");
  await Promise.all([
    mkdir(scripts, { recursive: true }),
    mkdir(fakeBin, { recursive: true }),
    mkdir(signingDir, { recursive: true }),
  ]);

  await copyFile(
    path.join(repoRoot, "scripts", "build-signed-macos.sh"),
    path.join(scripts, "build-signed-macos.sh"),
  );
  await chmod(path.join(scripts, "build-signed-macos.sh"), 0o755);
  await writeFile(
    path.join(signingDir, "AppStoreConnectCertInfo.csv"),
    "\ufeffIssuerID,Name,KeyID,KeyFile\r\nissuer-value,Test Key,TEST123,AuthKey_TEST123.p8\r\n",
  );
  await writeFile(keyPath, "private-key-sentinel-that-must-never-be-printed\n");
  await chmod(keyPath, keyMode);
  await writeFile(
    path.join(signingDir, "pwlist"),
    "password-sentinel-that-must-never-be-printed\n",
  );

  await executable(path.join(fakeBin, "uname"), "#!/bin/sh\nprintf 'Darwin\\n'\n");
  await executable(
    path.join(fakeBin, "security"),
    identityAvailable
      ? `#!/bin/sh
printf '%s\n' '1) HASH "Developer ID Application: MICHAEL COELHO (27R2KCAHN7)"'
printf '%s\n' '1 valid identities found'
`
      : "#!/bin/sh\nprintf '0 valid identities found\\n'\n",
  );
  await executable(
    path.join(fakeBin, "xcrun"),
    `#!/bin/sh
set -eu
printf 'xcrun:%s\n' "$*" >> "$VELA_CALL_LOG"
case "$1 $2" in
  'notarytool history'|'stapler validate') exit 0 ;;
  *) exit 98 ;;
esac
`,
  );
  await executable(
    path.join(fakeBin, "codesign"),
    `#!/bin/sh
set -eu
printf 'codesign:%s\n' "$*" >> "$VELA_CALL_LOG"
printf '%s\n' 'Authority=Developer ID Application: MICHAEL COELHO (27R2KCAHN7)'
`,
  );
  await executable(
    path.join(fakeBin, "spctl"),
    `#!/bin/sh
set -eu
printf 'spctl:%s\n' "$*" >> "$VELA_CALL_LOG"
`,
  );
  await executable(
    path.join(fakeBin, "hdiutil"),
    `#!/bin/sh
set -eu
printf 'hdiutil:%s\n' "$*" >> "$VELA_CALL_LOG"
case "$1" in
  attach)
    mount=''
    while [ "$#" -gt 0 ]; do
      if [ "$1" = '-mountpoint' ]; then shift; mount="$1"; fi
      shift
    done
    [ -n "$mount" ] || exit 97
    mkdir -p "$mount/Vela.app/Contents/MacOS"
    printf 'plist\n' > "$mount/Vela.app/Contents/Info.plist"
    printf 'binary\n' > "$mount/Vela.app/Contents/MacOS/vela"
    ;;
  detach) ;;
  *) exit 96 ;;
esac
`,
  );
  await executable(
    path.join(fakeBin, "plutil"),
    `#!/bin/sh
set -eu
printf 'plutil:%s\n' "$*" >> "$VELA_CALL_LOG"
printf 'vela\n'
`,
  );
  await executable(
    path.join(fakeBin, "lipo"),
    `#!/bin/sh
set -eu
printf 'lipo:%s\n' "$*" >> "$VELA_CALL_LOG"
printf 'x86_64 arm64\n'
`,
  );
  await executable(
    path.join(fakeBin, "shasum"),
    `#!/bin/sh
set -eu
printf 'shasum:%s\n' "$*" >> "$VELA_CALL_LOG"
printf 'fakehash  %s\n' "$3"
`,
  );
  await executable(
    path.join(scripts, "build.sh"),
    `#!/bin/sh
set -eu
[ "$APPLE_SIGNING_IDENTITY" = 'Developer ID Application: MICHAEL COELHO (27R2KCAHN7)' ]
[ "$APPLE_API_ISSUER" = 'issuer-value' ]
[ "$APPLE_API_KEY" = 'TEST123' ]
[ "$APPLE_API_KEY_PATH" = "$VELA_EXPECTED_KEY_PATH" ]
[ "\${APPLE_CERTIFICATE+x}" != x ]
[ "\${APPLE_CERTIFICATE_PASSWORD+x}" != x ]
printf 'build\n' >> "$VELA_CALL_LOG"
mkdir -p dist
printf 'dmg\n' > dist/Vela_test_universal.dmg
`,
  );

  return {
    root,
    script: path.join(scripts, "build-signed-macos.sh"),
    callLog,
    env: {
      ...process.env,
      PATH: `${fakeBin}:${process.env.PATH}`,
      VELA_SIGNING_DIR: signingDir,
      VELA_EXPECTED_KEY_PATH: keyPath,
      VELA_CALL_LOG: callLog,
      APPLE_API_ISSUER: "",
      APPLE_API_KEY: "",
      APPLE_API_KEY_PATH: "",
      APPLE_CERTIFICATE: "must-be-unset",
      APPLE_CERTIFICATE_PASSWORD: "must-be-unset",
    },
  };
}

test("the signed macOS wrapper loads local metadata and verifies every artifact boundary", { skip: posixBashUnavailable }, async (t) => {
  const fixture = await signedMacFixture(t);
  const run = spawnSync("/bin/bash", [fixture.script], {
    cwd: fixture.root,
    encoding: "utf8",
    env: fixture.env,
  });
  assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
  assert.match(run.stdout, /Signed, notarized universal DMG verified/);
  assert.doesNotMatch(
    `${run.stdout}\n${run.stderr}`,
    /private-key-sentinel|password-sentinel/,
  );

  const calls = await readFile(fixture.callLog, "utf8");
  for (const expected of [
    "xcrun:notarytool history",
    "build",
    "codesign:--verify --strict",
    "hdiutil:attach",
    "codesign:--verify --deep --strict",
    "xcrun:stapler validate",
    "spctl:--assess --type execute",
    "plutil:-extract CFBundleExecutable raw",
    "lipo:-archs",
    "hdiutil:detach",
    "shasum:-a 256",
  ]) {
    assert.match(calls, new RegExp(expected), `missing verification call: ${expected}`);
  }
});

test("the signed macOS wrapper fails closed before building", { skip: posixBashUnavailable }, async (t) => {
  await t.test("when the Developer ID identity is absent", async (t) => {
    const fixture = await signedMacFixture(t, { identityAvailable: false });
    const run = spawnSync("/bin/bash", [fixture.script], {
      cwd: fixture.root,
      encoding: "utf8",
      env: fixture.env,
    });
    assert.notEqual(run.status, 0);
    assert.match(run.stderr, /Keychain identity is unavailable/);
  });

  await t.test("when only part of an API credential override is supplied", async (t) => {
    const fixture = await signedMacFixture(t);
    const run = spawnSync("/bin/bash", [fixture.script], {
      cwd: fixture.root,
      encoding: "utf8",
      env: { ...fixture.env, APPLE_API_ISSUER: "override-only" },
    });
    assert.notEqual(run.status, 0);
    assert.match(run.stderr, /set APPLE_API_ISSUER.*together/);
  });

  await t.test("when the private API key is not owner-only", async (t) => {
    const fixture = await signedMacFixture(t, { keyMode: 0o644 });
    const run = spawnSync("/bin/bash", [fixture.script], {
      cwd: fixture.root,
      encoding: "utf8",
      env: fixture.env,
    });
    assert.notEqual(run.status, 0);
    assert.match(run.stderr, /private key must be owner-only/);
  });
});
