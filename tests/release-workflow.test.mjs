import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workflow = await readFile(
  path.join(repoRoot, ".github", "workflows", "release.yml"),
  "utf8",
);

test("release artifacts fail closed through a final inventory", () => {
  assert.doesNotMatch(workflow, /if-no-files-found:\s*ignore/);
  assert.doesNotMatch(workflow, /find "\$root" -maxdepth 1/);
  assert.match(workflow, /find "\$root" -type f -name "\$pattern"/);
  assert.match(workflow, /inventory:\n[\s\S]*needs: \[bundle, arch\]/);
  assert.match(workflow, /uses: actions\/download-artifact@v8/);
  assert.match(
    workflow,
    /  bundle:\n    needs: npm-audit\n    permissions:\n      contents: write/,
  );
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
