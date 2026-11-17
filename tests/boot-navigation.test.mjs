// What Vela does with a view chosen while it starts. The sidebar's Home and
// Playlists are on screen before the sources have loaded; the first Home
// load (loadHome sets the mode) must not put Home back over Playlists chosen
// in that time. App.svelte's own boot and navigation functions are taken
// from the page's script and run against a stubbed backend whose source list
// arrives when the test says.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import * as ts from 'typescript';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const page = fs.readFileSync(path.join(repoRoot, 'src', 'App.svelte'), 'utf8');
const script = /<script\b[^>]*>([\s\S]*?)<\/script>/.exec(page)?.[1];
assert.ok(script, 'App.svelte must contain its TypeScript script');
const sourceFile = ts.createSourceFile('App.svelte.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const functions = new Map();
(function visit(node) {
  if (ts.isFunctionDeclaration(node) && node.name && node.body) functions.set(node.name.text, node);
  ts.forEachChild(node, visit);
})(sourceFile);
const functionText = (name) => {
  const node = functions.get(name);
  assert.ok(node, `App.svelte must define ${name}`);
  return node.getText(sourceFile);
};

// The page's functions a boot and the two sidebar entries run, verbatim. The
// state they share is declared here as the page starts it; what they call
// beyond these (the view's error line, the context menus) is stubbed.
const PAGE_FUNCTIONS = [
  'bootNormal', 'loadSourceList', 'loadServerPlaylists', 'loadEverything',
  'loadSections', 'loadHome', 'openPlaylists', 'goHome',
];
const code = ts.transpileModule(`
  let mode = "home", navEpoch = 0, detailView = null, detailStatus = null;
  let homeGen = 0, loadGen = 0, sourceGen = 0, serverPlaylistGen = 0;
  let loadingMore = false, loading = false, searchTerm = "", personView = null;
  let active = null, activeType = null, crumbs = [], items = [], selectedServerPlaylist = null;
  let hubs = [], continueHubs = [], sections = [], recents = [], continueTombstones = [];
  let sources = [], authenticated = false, normalBooted = false, serverPlaylistGroups = [];
  let activeSource = null, mpvInfo = null, appInfo = null, continuePlayingMode = "only-tv";
  const errors = [];
  function setError(message) { if (message !== null) errors.push(message); }
  function closeMenu() {}
  function closeSectionMenu() {}
  ${PAGE_FUNCTIONS.map(functionText).join('\n')}
  return {
    bootNormal, openPlaylists, goHome,
    get view() { return { mode, hubs, sections, errors, authenticated }; },
  };
`, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;

const SECTIONS = [{ key: 'movies', title: 'Movies', type: 'movie' }];
const HUBS = [{ key: 'recent', title: 'Recently Added', items: [] }];

// A backend whose source list is held until the test lets it go; before it
// has gone, there are no libraries and Home has nothing, as with no sources.
function backend() {
  let release;
  let arrived = false;
  const sourceList = new Promise((resolve) => { release = resolve; });
  const calls = [];
  const replies = {
    get_sources: () => sourceList,
    get_server_playlists: async () => [],
    get_sections: async () => (arrived ? SECTIONS : []),
    get_hubs: async () => (arrived ? HUBS : []),
    get_recents: async () => [],
    get_continue_tombstones: async () => [],
    check_mpv: async () => ({ available: true }),
    get_app_info: async () => ({ version: '0' }),
    get_continue_playing: async () => 'only-tv',
  };
  return {
    calls,
    sourcesArrive: () => {
      arrived = true;
      release([{ id: 'jf', name: 'Mock', kind: 'jellyfin' }]);
    },
    invoke(command) {
      calls.push(command);
      const reply = replies[command];
      if (!reply) throw new Error(`the boot asked the backend for ${command}, which this test does not answer`);
      return reply();
    },
  };
}

const start = (server) => new Function('invoke', code)(server.invoke);
const settle = () => new Promise((resolve) => setImmediate(resolve));

test('Playlists chosen while the sources load stays, with the sidebar libraries loaded', async () => {
  const server = backend();
  const app = start(server);
  const booting = app.bootNormal();
  app.openPlaylists(); // the click, while the source list is still on its way
  assert.equal(app.view.mode, 'playlists');
  server.sourcesArrive();
  await booting;
  await settle();
  assert.equal(app.view.mode, 'playlists', 'the first Home load must not put Home back over Playlists');
  assert.equal(app.view.authenticated, true);
  assert.deepEqual(app.view.sections, SECTIONS, "the sidebar's libraries load all the same");
  assert.equal(server.calls.includes('get_hubs'), false, 'Home is not loaded behind Playlists');
  assert.deepEqual(app.view.errors, []);

  app.goHome();
  await settle();
  assert.equal(app.view.mode, 'home');
  assert.deepEqual(app.view.hubs, HUBS, 'Home loads when it is opened');
});

test('without a choice while the sources load, the boot opens Home as before', async () => {
  const server = backend();
  const app = start(server);
  const booting = app.bootNormal();
  server.sourcesArrive();
  await booting;
  await settle();
  assert.equal(app.view.mode, 'home');
  assert.deepEqual(app.view.hubs, HUBS);
  assert.deepEqual(app.view.sections, SECTIONS);
});

// Home chosen before the sources arrive loads an empty Home; the boot's own
// Home load, once they have, is what fills it.
test('Playlists and then Home chosen while the sources load: the boot loads Home', async () => {
  const server = backend();
  const app = start(server);
  const booting = app.bootNormal();
  app.openPlaylists();
  app.goHome();
  server.sourcesArrive();
  await booting;
  await settle();
  assert.equal(app.view.mode, 'home');
  assert.deepEqual(app.view.hubs, HUBS);
  assert.deepEqual(app.view.sections, SECTIONS);
});
