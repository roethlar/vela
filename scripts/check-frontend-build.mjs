#!/usr/bin/env node
// Post-build layout check for the frontend Tauri embeds. `npm run build` runs
// it after `vite build`; tests/frontend-shell.test.mjs guards that wiring.
// It reads build/ and exits non-zero, naming every failed assertion:
//   B1  exactly one module script, served from /assets/*.js, and it exists
//   B2  linked stylesheets exist and carry the Geist Variable @font-face, with
//       woff2 files under /assets/ that exist
//   B3  no font is inlined as a data: URI (the CSP has no font-src, so
//       default-src 'self' would block it)
//   B4  build/favicon.png exists
//   B5  the pre-paint theme script survived into build/index.html
//   B6  no stale SvelteKit output (build/_app) is left to be embedded
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const buildDir = path.join(repoRoot, "build");
const failures = [];
const fail = (id, message) => failures.push(`${id}: ${message}`);

// Map a root-absolute URL path ("/assets/x.js") to a file under build/.
function builtFile(urlPath) {
  const file = path.resolve(buildDir, `.${urlPath}`);
  return file.startsWith(buildDir + path.sep) ? file : null;
}

function attributes(tag) {
  const attrs = {};
  const body = tag.replace(/^<[a-z]+/i, "").replace(/\/?>$/, "");
  for (const m of body.matchAll(/([^\s=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?/g)) {
    attrs[m[1].toLowerCase()] = m[2] ?? m[3] ?? m[4] ?? "";
  }
  return attrs;
}

const indexFile = path.join(buildDir, "index.html");
if (!fs.existsSync(indexFile)) {
  console.error(`check-frontend-build: ${indexFile} is missing; run vite build first`);
  process.exit(1);
}
const html = fs.readFileSync(indexFile, "utf8");

// B1
const moduleScripts = [...html.matchAll(/<script\b[^>]*>/gi)]
  .map((m) => attributes(m[0]))
  .filter((a) => a.type === "module");
if (moduleScripts.length !== 1) {
  fail("B1", `build/index.html has ${moduleScripts.length} module scripts; expected exactly 1`);
} else {
  const src = moduleScripts[0].src;
  if (!src || !/^\/assets\/[^/]+\.js$/.test(src)) {
    fail("B1", `module script src ${JSON.stringify(src)} is not /assets/<name>.js`);
  } else if (!fs.existsSync(builtFile(src))) {
    fail("B1", `module script ${src} does not exist under build/`);
  }
}

// B2 and B3
const stylesheets = [...html.matchAll(/<link\b[^>]*>/gi)]
  .map((m) => attributes(m[0]))
  .filter((a) => a.rel?.split(/\s+/).includes("stylesheet"))
  .map((a) => a.href);
let css = "";
if (stylesheets.length === 0) fail("B2", "build/index.html links no stylesheet");
for (const href of stylesheets) {
  const file = href?.startsWith("/") ? builtFile(href) : null;
  if (!file || !fs.existsSync(file)) {
    fail("B2", `linked stylesheet ${JSON.stringify(href)} does not exist under build/`);
  } else {
    css += fs.readFileSync(file, "utf8") + "\n";
  }
}
if (!/@font-face\s*\{[^}]*font-family:\s*["']?Geist Variable/.test(css)) {
  fail("B2", "no @font-face with font-family Geist Variable in the linked CSS");
}
const urls = [...css.matchAll(/url\(\s*(["']?)([^"')]*)\1\s*\)/g)].map((m) => m[2].trim());
if (!urls.some((u) => /^\/assets\/[^/]+\.woff2$/.test(u))) {
  fail("B2", "the linked CSS references no /assets/*.woff2 font file");
}
for (const u of urls.filter((u) => u.startsWith("/assets/"))) {
  const file = builtFile(u);
  if (!file || !fs.existsSync(file)) fail("B2", `CSS url(${u}) does not exist under build/`);
}
const inlinedFonts = urls.filter((u) => /^data:font/i.test(u));
if (inlinedFonts.length > 0) {
  fail("B3", `${inlinedFonts.length} CSS url() values inline a font as data:font (blocked by the CSP)`);
}

// B4
if (!fs.existsSync(path.join(buildDir, "favicon.png"))) fail("B4", "build/favicon.png is missing");

// B5
if (!html.includes("var ids = [")) {
  fail("B5", "build/index.html lost the pre-paint theme script (`var ids = [`)");
}

// B6
if (fs.existsSync(path.join(buildDir, "_app"))) {
  fail("B6", "build/_app exists: stale SvelteKit output would be embedded");
}

if (failures.length > 0) {
  for (const f of failures) console.error(`check-frontend-build: ${f}`);
  process.exit(1);
}
console.log("check-frontend-build: B1-B6 passed");
