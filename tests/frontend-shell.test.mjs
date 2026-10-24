// The app shell without SvelteKit: Vite config, tsconfig, index.html,
// src/main.ts and tauri.conf.json must agree on where the build goes, which
// dev port Tauri loads, where `$lib` is defined, and how the app mounts.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { resolveConfig } from "vite";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (...parts) => fs.readFileSync(path.join(repoRoot, ...parts), "utf8");

const config = await resolveConfig(
  { root: repoRoot, configFile: path.join(repoRoot, "vite.config.js"), logLevel: "silent" },
  "build",
  "production",
);
const tauri = JSON.parse(read("src-tauri", "tauri.conf.json"));
const pkg = JSON.parse(read("package.json"));
const tsconfig = JSON.parse(read("tsconfig.json"));
const indexHtml = read("index.html");
const mainTs = read("src", "main.ts");

test("S1: the Vite build output is the directory Tauri embeds", () => {
  assert.equal(
    path.resolve(repoRoot, config.build.outDir),
    path.resolve(repoRoot, "src-tauri", tauri.build.frontendDist),
    "Vite build.outDir must be tauri.conf.json build.frontendDist",
  );
  assert.equal(config.build.emptyOutDir, true, "build.emptyOutDir must be true so stale output is cleared");
  assert.equal(tauri.build.beforeBuildCommand, "npm run build", "Tauri must build the frontend with npm run build");
  assert.equal(
    pkg.scripts.build,
    "vite build && node scripts/check-frontend-build.mjs",
    "npm run build must run the post-build layout check after vite build",
  );
});

test("S2: the dev server is the strict port Tauri loads", () => {
  assert.equal(config.server.port, 1420, "Vite dev server port must be 1420");
  assert.equal(config.server.strictPort, true, "Vite dev server must fail rather than move off 1420");
  assert.equal(new URL(tauri.build.devUrl).port, "1420", "tauri.conf.json devUrl must point at port 1420");
});

test("S3: $lib is defined once, in tsconfig paths, and Vite reads it", () => {
  assert.equal(config.resolve.tsconfigPaths, true, "Vite must resolve $lib through resolve.tsconfigPaths");
  const paths = tsconfig.compilerOptions?.paths ?? {};
  assert.deepEqual(paths["$lib"], ["./src/lib"], 'tsconfig paths["$lib"] must be ["./src/lib"]');
  assert.deepEqual(paths["$lib/*"], ["./src/lib/*"], 'tsconfig paths["$lib/*"] must be ["./src/lib/*"]');
});

test("S4: index.html loads src/main.ts, which mounts the root component on #app", () => {
  const appTargets = indexHtml.match(/\bid\s*=\s*["']app["']/g) ?? [];
  assert.equal(appTargets.length, 1, 'index.html must contain exactly one id="app" element');
  const entryTag = '<script type="module" src="/src/main.ts"></script>';
  assert.equal(indexHtml.split(entryTag).length - 1, 1, `index.html must contain exactly one ${entryTag}`);
  const prePaint = indexHtml.indexOf("var ids = [");
  assert.ok(prePaint >= 0, "index.html must keep the pre-paint theme script");
  assert.ok(prePaint < indexHtml.indexOf(entryTag), "the pre-paint theme script must run before the module entry");

  assert.ok(mainTs.includes('getElementById("app")'), 'src/main.ts must mount on getElementById("app")');
  assert.ok(mainTs.includes("mount("), "src/main.ts must mount the root component with mount()");
  const cssImport = mainTs.search(/^import\s+["']\.\/app\.css["'];?\s*$/m);
  const rootImport = mainTs.search(/^import\s+App\s+from\s+["'][^"']+\.svelte["'];?\s*$/m);
  assert.ok(cssImport >= 0, 'src/main.ts must import "./app.css"');
  assert.ok(rootImport >= 0, "src/main.ts must import the root component as App");
  assert.ok(cssImport < rootImport, "src/main.ts must import ./app.css before the root component");
});
