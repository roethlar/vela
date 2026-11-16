import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
// The patterns below spell line breaks as \n; a Windows checkout with
// core.autocrlf=true has CRLF, so normalize at read time.
const workflow = (
  await readFile(path.join(repoRoot, ".github", "workflows", "release.yml"), "utf8")
).replace(/\r\n/g, "\n");
const ci = (
  await readFile(path.join(repoRoot, ".github", "workflows", "ci.yml"), "utf8")
).replace(/\r\n/g, "\n");

function otherPlatformsJob() {
  const job = ci.match(/^  other-platforms:\n[\s\S]*?(?=^  [a-z][a-z0-9_-]*:\n|(?![\s\S]))/m)?.[0];
  assert.ok(job, "CI must keep its other-platforms job");
  return job.split("\n").filter((line) => !/^\s*#/.test(line)).join("\n");
}

test("CI installs both targets of the universal macOS release", () => {
  const job = otherPlatformsJob();
  assert.match(job, /^        os: \[macos-latest, windows-latest\]$/m);
  assert.match(job, /^    runs-on: \$\{\{ matrix\.os \}\}$/m);
  assert.match(job, /    defaults:\n      run:\n        working-directory: src-tauri\n/);
  assert.match(job, /^      fail-fast: false$/m);
  assert.doesNotMatch(job, /continue-on-error:/);
  assert.ok(
    job.includes(
      "      - uses: dtolnay/rust-toolchain@stable\n" +
      "        with:\n" +
      "          targets: ${{ matrix.os == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}\n",
    ),
    "the macOS toolchain must install both Darwin targets; Windows keeps its host target",
  );
});

for (const target of ["aarch64-apple-darwin", "x86_64-apple-darwin"]) {
  test(`CI explicitly checks ${target} on macOS`, () => {
    assert.match(
      otherPlatformsJob(),
      new RegExp(
        "^      - name: [^\\n]+\\n" +
        "        if: runner\\.os == 'macOS'\\n" +
        `        run: cargo \\+stable check --locked --target ${target}\\n`,
        "m",
      ),
      `CI must check ${target} explicitly, regardless of the macOS runner's host architecture`,
    );
  });
}

test("CI scopes the host-only compile check to Windows", () => {
  assert.match(
    otherPlatformsJob(),
    /      - name: [^\n]+\n        if: runner\.os == 'Windows'\n        run: cargo \+stable check --locked\n/,
  );
});

test("release artifacts fail closed through a final inventory", () => {
  assert.doesNotMatch(workflow, /if-no-files-found:\s*ignore/);
  assert.doesNotMatch(workflow, /find "\$root" -maxdepth 1/);
  assert.match(workflow, /find "\$root" -type f -name "\$pattern"/);
  assert.match(workflow, /inventory:\n[\s\S]*needs: \[bundle, arch\]/);
  assert.match(workflow, /uses: actions\/download-artifact@v8/);
  // Package jobs wait for audits and for the sets their platforms carry
  // (tests/libmpv-packaging.test.mjs also holds the Linux part).
  assert.match(
    workflow,
    /  bundle:\n    needs: \[npm-audit, cargo-audit, libmpv, libmpv-windows\]\n    permissions:\n      contents: write/,
  );
  assert.match(workflow, /  arch:\n    name: Arch package\n    needs: \[npm-audit, cargo-audit, libmpv\]\n/);
  // A known vulnerability fails the release as it fails CI, in either ecosystem.
  assert.match(workflow, /  npm-audit:\n[\s\S]*?- run: npm audit --audit-level=moderate\n/);
  assert.match(workflow, /  cargo-audit:\n[\s\S]*?- run: cargo audit --file src-tauri\/Cargo\.lock\n/);
  assert.match(workflow, /  libmpv:\n[\s\S]*?    needs: \[npm-audit, cargo-audit\]\n/);
  // The inventory takes what is released and nothing that merely travelled
  // between jobs, or SHA256SUMS would list the set's files.
  assert.match(workflow, /uses: actions\/download-artifact@v8\n\s+with:\n\s+pattern: vela-\*\n\s+path: artifacts\n/);
  assert.match(
    workflow,
    /find artifacts -type f ! -name SHA256SUMS -print0 \| sort -z/,
  );
  assert.match(workflow, /name: vela-sha256sums\n\s+if-no-files-found: error/);
  assert.match(
    workflow,
    /gh release upload "\$GITHUB_REF_NAME"[\s\S]*--repo "\$GITHUB_REPOSITORY"[\s\S]*--clobber/,
  );
});

function releaseJob(source, name) {
  const body = source.match(new RegExp(`^  ${name}:\\n[\\s\\S]*?(?=^  [a-z][a-z0-9_-]*:\\n|(?![\\s\\S]))`, 'm'))?.[0];
  assert.ok(body, `missing ${name} job`);
  return body.split('\n').filter((line) => !/^\s*#/.test(line)).join('\n');
}

function windowsReleaseContracts(source) {
  const set = releaseJob(source, 'libmpv-windows');
  const ciSet = releaseJob(ci, 'libmpv-windows');
  const cache = (job) => job.match(/^          key: (.+)$/m)?.[1];
  assert.match(set, /^    needs: \[npm-audit, cargo-audit\]$/m, 'Windows set waits for audits');
  assert.match(set, /^    runs-on: ubuntu-22\.04$/m, 'Windows set build baseline');
  assert.ok(cache(set), 'Windows cache key exists');
  assert.equal(cache(set), cache(ciSet), 'Windows release uses CI cache key');
  assert.match(set, /uses: actions\/cache@v6\n        with:\n          path: \.libmpv\/out\/libmpv-windows-x86_64\n/, 'Windows cache path');
  assert.match(set, /if: steps\.cache\.outputs\.cache-hit != 'true'\n        run: bash scripts\/build-libmpv-windows\.sh --container\n/, 'Windows cache miss builds from pins');
  assert.match(set, /- name: Check the manifest\n        run: python3 -B packaging\/libmpv\/check-libmpv-windows\.py \.libmpv\/out\/libmpv-windows-x86_64 --manifest-only\n/, 'cached and new Windows sets checked');
  assert.match(set, /uses: actions\/upload-artifact@v7\n        with:\n          name: libmpv-windows-x86_64\n          if-no-files-found: error\n          path: \.libmpv\/out\/libmpv-windows-x86_64\n/, 'Windows set handed off');
  assert.doesNotMatch(set, /continue-on-error|tauri-action/, 'Windows set must pass before packaging');

  const bundle = releaseJob(source, 'bundle');
  assert.match(bundle, /^    needs: \[npm-audit, cargo-audit, libmpv, libmpv-windows\]$/m, 'bundle waits for both sets');
  const download = '- name: Fetch the libmpv set (Windows)';
  assert.match(bundle, /- name: Fetch the libmpv set \(Windows\)\n        if: matrix\.os == 'windows-latest'\n        uses: actions\/download-artifact@v8\n        with:\n          name: libmpv-windows-x86_64\n          path: \.libmpv\/out\/libmpv-windows-x86_64\n/, 'Windows download feeds overlay source');
  assert.match(bundle, /- uses: actions\/setup-python@v6\n        if: runner\.os == 'Windows'\n        with:\n          python-version: '3\.13'\n          architecture: x64\n/, 'Windows playback requires x64 Python');
  assert.ok(bundle.includes("(matrix.os == 'windows-latest' && '--bundles nsis,msi --config src-tauri/tauri.libmpv-windows.conf.json' ||"), 'Windows build must pass overlay and both bundle types');
  const build = '- uses: tauri-apps/tauri-action@v1';
  const check = '- name: Require the libmpv set in every Windows installer';
  const upload = '- name: Upload the Windows installers';
  assert.match(bundle, /- name: Require the libmpv set in every Windows installer\n        if: runner\.os == 'Windows'\n        shell: pwsh\n        run: \|\n          python -B packaging\/libmpv\/check-package-windows\.py --set \.libmpv\/out\/libmpv-windows-x86_64 --bundle-dir '\$\{\{ matrix\.bundle_dir \}\}'\n/, 'every Windows installer must be checked');
  assert.ok(bundle.indexOf(download) < bundle.indexOf(build), 'download precedes build');
  assert.ok(bundle.indexOf(build) < bundle.indexOf(check), 'installer check follows build');
  assert.ok(bundle.indexOf(check) < bundle.indexOf(upload), 'installer check precedes upload');
  assert.doesNotMatch(bundle, /continue-on-error:|if:.*always\(/, 'package failures cannot be ignored');
  for (const [key, value] of [['tagName', 'github.ref_name'], ['releaseName', "format('Vela {0}', github.ref_name)"]]) {
    assert.ok(bundle.includes(`${key}: \${{ startsWith(github.ref, 'refs/tags/') && matrix.os == 'macos-latest' && ${value} || '' }}`), 'only macOS may attach before package checks');
  }
  const inventory = releaseJob(source, 'inventory');
  assert.match(inventory, /needs: \[bundle, arch\]/, 'inventory waits for package checks');
  assert.ok(inventory.includes("\\( -name '*.msi' -o -name '*-setup.exe' \\) -print0 | sort -z"), 'inventory collects both checked Windows formats');
  assert.ok(inventory.includes('[ "${#windows[@]}" -eq 2 ] || {'), 'inventory requires both Windows installers');
  assert.ok(inventory.includes('            "${windows[@]}" \\\n'), 'inventory attaches checked Windows installers');
}

test('Windows release builds once, packages both installers, and checks them before upload', () => {
  windowsReleaseContracts(workflow);
});

test('Windows release guards reject a missing overlay, download, check or dependency (red proof)', () => {
  const breaks = [
    ["--bundles nsis,msi --config src-tauri/tauri.libmpv-windows.conf.json", '--bundles nsis,msi', /must pass overlay/],
    ['- name: Fetch the libmpv set (Windows)', '- name: Removed Windows download', /download feeds/],
    ['python -B packaging/libmpv/check-package-windows.py --set .libmpv/out/libmpv-windows-x86_64 --bundle-dir', 'echo skipped --bundle-dir', /installer must be checked/],
    ['needs: [npm-audit, cargo-audit, libmpv, libmpv-windows]', 'needs: [npm-audit, cargo-audit, libmpv]', /waits for both sets/],
    ["matrix.os == 'macos-latest' && github.ref_name", "matrix.os != 'ubuntu-22.04' && github.ref_name", /only macOS may attach/],
    ['            "${windows[@]}" \\\n', '', /attaches checked Windows/],
  ];
  for (const [before, after, reason] of breaks) {
    assert.ok(workflow.includes(before), `mutation must apply: ${before}`);
    assert.throws(() => windowsReleaseContracts(workflow.replace(before, after)), reason);
  }
});

test("the Arch package is built without root and uploaded", () => {
  assert.match(workflow, /^  arch:\n/m);
  assert.match(workflow, /container:\n\s+image: archlinux:base-devel/);
  assert.match(workflow, /runuser -u vela-builder/);
  assert.match(workflow, /npm run build:arch/);
  assert.match(workflow, /name: vela-arch\n\s+if-no-files-found: error/);
});

test("every promised native artifact has an exact inventory guard", () => {
  const requiredContracts = [
    "require_one 'universal macOS DMG' artifacts/vela-macos '*.dmg'",
    "require_one 'Linux AppImage' artifacts/vela-linux '*.AppImage'",
    "require_one 'Debian package' artifacts/vela-linux '*.deb'",
    "require_one 'RPM package' artifacts/vela-linux '*.rpm'",
    "require_one 'Windows MSI' artifacts/vela-windows '*.msi'",
    "require_one 'Windows NSIS installer' artifacts/vela-windows '*-setup.exe'",
    "require_one 'Arch package' artifacts/vela-arch '*.pkg.tar.zst'",
  ];

  for (const contract of requiredContracts) {
    assert.ok(workflow.includes(contract), `missing release contract: ${contract}`);
  }
});

test('the macOS app says why it reaches the local network, and the release checks the built app says it', async () => {
  // macOS asks before an app talks to devices on the local network, where
  // Vela's servers usually are, and expects the app's Info.plist to say why
  // (NSLocalNetworkUsageDescription). Tauri merges src-tauri/Info.plist into
  // the app's; the release workflow reads the key back from the built app.
  const plist = await readFile(new URL('../src-tauri/Info.plist', import.meta.url), 'utf8');
  assert.match(plist, /<key>NSLocalNetworkUsageDescription<\/key>\s*<string>[^<]*Plex[^<]*Jellyfin[^<]*Emby[^<]*local network[^<]*<\/string>/);
  const release = await readFile(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8');
  assert.match(release, /\/usr\/libexec\/PlistBuddy -c 'Print :NSLocalNetworkUsageDescription' "\$app\/Contents\/Info\.plist"/);
});
