import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

test("the released Wayland API uses the locally secured scanner", async () => {
  const cargoToml = await readFile(
    path.join(repoRoot, "src-tauri", "Cargo.toml"),
    "utf8",
  );
  const cargoLock = await readFile(
    path.join(repoRoot, "src-tauri", "Cargo.lock"),
    "utf8",
  );
  const vendorToml = await readFile(
    path.join(
      repoRoot,
      "src-tauri",
      "vendor",
      "wayland-scanner",
      "Cargo.toml",
    ),
    "utf8",
  );
  const vendorParser = await readFile(
    path.join(
      repoRoot,
      "src-tauri",
      "vendor",
      "wayland-scanner",
      "src",
      "parse.rs",
    ),
    "utf8",
  );
  const patchSection = cargoToml.match(
    /\[patch\.crates-io\]\n(?<body>[\s\S]*?)(?=\n\[|$)/,
  )?.groups?.body;
  assert.ok(patchSection, "Cargo.toml must retain the crates.io patch section");
  assert.match(
    patchSection,
    /^wayland-scanner = \{ path = "vendor\/wayland-scanner" \}$/m,
    "the vulnerable published scanner must be replaced locally",
  );
  assert.match(vendorToml, /^name = "wayland-scanner"$/m);
  assert.match(vendorToml, /^version = "0\.31\.10"$/m);
  assert.match(vendorToml, /^quick-xml = "0\.41"$/m);
  assert.match(vendorParser, /byte_ref\.xml10_content\(\)/);
  assert.doesNotMatch(vendorParser, /byte_ref\.xml_content\(\)/);

  for (const crate of ["wayland-backend", "wayland-client"]) {
    const packageBlock = cargoLock.match(
      new RegExp(
        `\\[\\[package\\]\\]\\nname = "${crate}"\\n[\\s\\S]*?(?=\\n\\[\\[package\\]\\]|$)`,
      ),
    )?.[0];
    assert.match(packageBlock ?? "", /^source = "registry\+/m);
  }

  const scannerBlock = cargoLock.match(
    /\[\[package\]\]\nname = "wayland-scanner"\n[\s\S]*?(?=\n\[\[package\]\]|$)/,
  )?.[0];
  assert.ok(scannerBlock, "the local scanner must be locked");
  assert.doesNotMatch(scannerBlock, /^source = /m);
  assert.match(scannerBlock, / "quick-xml",/);
});
