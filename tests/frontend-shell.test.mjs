// The app shell without SvelteKit: Vite config, tsconfig, index.html,
// src/main.ts and tauri.conf.json must agree on where the build goes, which
// dev port Tauri loads, where `$lib` is defined, and how the app mounts.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { parse } from "svelte/compiler";
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

function assertAppTarget(html) {
  // Reuse the installed parser for complete attribute names and quoted values.
  // This is the explicit, static HTML shell, not a Svelte component. Template
  // contents are detached; raw-text/RCDATA contents (and noscript with JS on)
  // cannot contribute elements to document.getElementById().
  const { fragment } = parse(html, { modern: true });
  const documentElement = fragment.nodes.find((node) => node.name?.toLowerCase() === "html");
  const bodies = documentElement?.fragment.nodes.filter((node) => node.name?.toLowerCase() === "body") ?? [];
  assert.equal(bodies.length, 1, "index.html must have exactly one explicit body inside html");
  const textOrDetached = new Set([
    "script", "style", "textarea", "title", "xmp", "iframe", "noembed", "noframes", "noscript", "template",
  ]);
  const appTargets = [];
  let plaintext = false;
  function visit(nodes, inBody = false) {
    for (const node of nodes) {
      if (plaintext) return; // Unlike other raw text, plaintext has no end tag.
      if (node.type === "Text" || node.type === "Comment") continue;
      assert.ok(node.attributes, "index.html must contain static HTML, not Svelte blocks or expressions");
      const name = node.name.toLowerCase();
      const insideBody = inBody || node === bodies[0];
      // HTML attribute names are case-insensitive; values are case-sensitive.
      // In particular, data-id, aria-id, xid and namespaced names are not id.
      const id = node.attributes.find((attr) => attr.type === "Attribute" && attr.name.toLowerCase() === "id");
      if (Array.isArray(id?.value) && id.value.length === 1 && id.value[0].type === "Text" && id.value[0].data === "app") {
        appTargets.push({ insideBody });
      }
      if (name === "plaintext") plaintext = true;
      if (textOrDetached.has(name)) {
        // Svelte accepts these as self-closing, but an HTML browser keeps
        // consuming their contents; do not mistake following text for a target.
        assert.ok(!html.slice(node.start, node.end).endsWith("/>"), "text/template elements must have closing tags");
        continue;
      }
      visit(node.fragment?.nodes ?? [], insideBody);
    }
  }
  visit(fragment.nodes);
  assert.equal(appTargets.length, 1, 'index.html must contain exactly one id="app" element');
  assert.ok(appTargets[0].insideBody, 'the id="app" element must be in the body');
}

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
  assertAppTarget(indexHtml);
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

const shell = (body, head = "") => `<!doctype html><html><head>${head}</head><body>${body}</body></html>`;
const mountCases = [
  ["the real index.html", indexHtml, true],
  ["the reviewer's data-id mutation of index.html", indexHtml.replace('id="app"', 'data-id="app"'), false],
  ...["data-id", "aria-id", "xid", "some-id", "xml:id", "_id"].map((name) => [
    `${name} is not id`, shell(`<div ${name}="app"></div>`), false,
  ]),
  ["id text in a single-quoted attribute", shell(`<div title='id="app"'></div>`), false],
  ["id text in a double-quoted attribute", shell(`<div title="id='app'"></div>`), false],
  ["a fake tag in an attribute, including >", shell(`<div title='<span id="app">'></div>`), false],
  ["id text in a comment", shell('<!-- id="app" -->'), false],
  ["a commented-out target", shell('<!-- <div id="app"></div> -->'), false],
  ["id text outside a tag", shell('id="app"'), false],
  ["script contents", shell(`<script>const markup = '<div id="app"></div>';</script>`), false],
  ["style contents", shell(`<style>div::before { content: '<div id="app"></div>'; }</style>`), false],
  ...["textarea", "title", "xmp", "iframe", "noembed", "noframes", "noscript", "template"].map((name) => [
    `a target inside ${name}`, shell(`<${name}><div id="app"></div></${name}>`), false,
  ]),
  ["plaintext consumes the rest of the document", shell('<plaintext></plaintext><div id="app"></div>'), false],
  ["a self-closing script does not end raw text", shell('<script/><div id="app"></div>'), false, /must have closing tags/],
  ["a self-closing template does not attach its contents", shell('<template/><div id="app"></div>'), false, /must have closing tags/],
  ["Svelte syntax cannot hide another HTML target", shell('<div id="app"></div>{#if false}<div id="app"></div>{/if}'), false, /exactly one id="app" element|must contain static HTML/],
  ["no target", shell("<div></div>"), false],
  ["a different id", shell('<div id="other"></div>'), false],
  ["a later duplicate id attribute cannot override the first", shell('<div id="other" ID="app"></div>'), false],
  ["id values are case-sensitive", shell('<div id="APP"></div>'), false],
  ["id values are not trimmed", shell('<div id=" app "></div>'), false],
  ["two body targets", shell('<div id="app"></div><main id="app"></main>'), false],
  ["a second target using single quotes and spaces", shell('<div id="app"></div><main id = \'app\'></main>'), false],
  ["a second target with an uppercase attribute", shell('<div id="app"></div><main ID="app"></main>'), false],
  ["a second target with an unquoted value", shell('<div id="app"></div><main id=app></main>'), false],
  ["a second target with an encoded value", shell('<div id="app"></div><main id="a&#112;p"></main>'), false],
  ["a target in the head", shell("", '<meta id="app">'), false],
  ["a head target as well as a body target", shell('<div id="app"></div>', '<meta id="app">'), false],
  ["a target on html", '<html id="app"><head></head><body></body></html>', false],
  ["a target on head", '<html><head id="app"></head><body></body></html>', false],
  ["a nested body target", shell('<main><div id="app"></div></main>'), true],
  ["spaces around equals", shell('<div id = "app"></div>'), true],
  ["single quotes", shell("<div id = 'app'></div>"), true],
  ["newlines and tabs around equals", shell("<div\nid\t=\n'app'></div>"), true],
  ["uppercase attribute name", shell('<div ID="app"></div>'), true],
  ["the first duplicate id attribute keeps its value", shell('<div ID="app" id="other"></div>'), true],
  ["an unquoted value", shell('<div id=app></div>'), true],
  ["an encoded value", shell('<div id="a&#112;p"></div>'), true],
  ["a quoted > before the id", shell('<div title=">" id="app"></div>'), true],
  ["a real id next to a suffix attribute", shell('<div data-id="app" id="app"></div>'), true],
  ["a real id next to id text in another value", shell(`<div title='id="app"' id="app"></div>`), true],
  ["a real target alongside a comment", shell('<!-- <div id="app"></div> --><div id="app"></div>'), true],
  ["a real target alongside a detached template", shell('<template><div id="app"></div></template><div id="app"></div>'), true],
  ["a real target alongside script text", shell(`<script>const markup = '<div id="app"></div>';</script><div id="app"></div>`), true],
];

for (const [name, html, accepted, error = /exactly one id="app" element|must be in the body/] of mountCases) {
  test(`S4 mount target: ${name}`, () => {
    if (accepted) {
      assert.doesNotThrow(() => assertAppTarget(html));
    } else {
      assert.throws(() => assertAppTarget(html), error);
    }
  });
}
