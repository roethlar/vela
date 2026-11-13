// The embedded player's wiring (.agents/plans/embedded-player.md, Slice 3).
//
// The Rust unit tests prove the pieces: the start-up decision for every
// combination of facts (player/embedded/startup.rs), the loader's refusals
// (libmpv.rs), the launch description (player/launch.rs), the option files
// (options.rs). These prove the pieces are connected the way the plan
// promises, above all its first promise: a user who has not chosen the
// embedded player gets exactly the Vela they had.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file) => fs.readFileSync(path.join(repoRoot, file), 'utf8').replace(/\r\n/g, '\n');
const main = read('src-tauri/src/main.rs');
const lib = read('src-tauri/src/lib.rs');
const commands = read('src-tauri/src/commands.rs');
const startup = read('src-tauri/src/player/embedded/startup.rs');
const host = read('src-tauri/src/player/embedded/host.rs');
const libmpv = read('src-tauri/src/player/embedded/libmpv.rs');
const session = read('src-tauri/src/player/embedded/session.rs');
const cargo = read('src-tauri/Cargo.toml');
const tauriConf = JSON.parse(read('src-tauri/tauri.conf.json'));
const page = read('src/routes/+page.svelte');

const code = (source) =>
  source
    .split('\n')
    .filter((line) => !/^\s*\/\//.test(line))
    .join('\n');

function functionBody(source, signature, what) {
  const start = source.indexOf(`\n${signature}`);
  assert.notEqual(start, -1, `${what} must exist`);
  const end = source.indexOf('\n}\n', start);
  assert.notEqual(end, -1, `${what} must have a closing brace`);
  return source.slice(start, end);
}

test("every start is today's unless embedded window mode was decided", () => {
  // The decision is the first thing main does, before anything can have
  // started a thread or a window.
  assert.match(
    code(main),
    /fn main\(\) \{\n\s+#\[cfg_attr\(not\(target_os = "linux"\), allow\(unused_variables\)\)\]\n\s+let embedded_window = vela_lib::prepare_startup\(\);/,
  );
  // WebKit's GPU renderer stays off, as it has been for everyone, unless
  // this run is in embedded window mode.
  assert.match(
    main,
    /#\[cfg\(target_os = "linux"\)\]\n\s+if !embedded_window && std::env::var_os\("WEBKIT_DISABLE_DMABUF_RENDERER"\)\.is_none\(\) \{\n\s+std::env::set_var\("WEBKIT_DISABLE_DMABUF_RENDERER", "1"\);\n\s+\}\n\n\s+vela_lib::run\(\)/,
  );
  assert.equal(code(main).split('set_var').length - 1, 1, 'main sets that one variable and no other');

  // What "decided" means: the start-up module's answer, on Linux only.
  assert.match(
    functionBody(lib, 'pub fn prepare_startup() -> bool {', 'prepare_startup'),
    /#\[cfg\(target_os = "linux"\)\]\s+\{\s+use player::embedded::startup;\s+startup::prepare\(\) == startup::WindowMode::Embedded\s+\}\s+#\[cfg\(not\(target_os = "linux"\)\)\]\s+\{\s+false\s+\}/,
  );

  // The window is transparent only in that mode. The configuration file,
  // which every other start uses untouched, says nothing about it.
  assert.match(
    lib,
    /if startup::decision\(\)\.mode == startup::WindowMode::Embedded \{[\s\S]{0,300}for window in &mut context\.config_mut\(\)\.app\.windows \{\s+window\.transparent = true;\s+\}\s+\}/,
  );
  assert.equal(code(lib).split('transparent').length - 1, 1, 'nothing else makes the window transparent');
  for (const window of tauriConf.app.windows) {
    assert.equal(window.transparent, undefined, 'the configured window is opaque');
  }
});

test('embedded window mode is entered only behind its safety marker, which the loaded page clears', () => {
  const prepare = functionBody(startup, 'pub(crate) fn prepare() -> WindowMode {', 'startup::prepare');
  // One decision, from the facts.
  assert.match(prepare, /let mut decision = decide\(facts\);/);
  assert.equal(prepare.split('decide(').length - 1, 1);
  // A marker that was found is removed, so the start after this one may try
  // again; and the mode is kept only if a new marker could be left.
  assert.match(
    prepare,
    /if marker_left_behind \{[\s\S]{0,200}std::fs::remove_file\(path\);[\s\S]{0,200}if decision\.mode == WindowMode::Embedded \{[\s\S]{0,300}let left = marker\s+\.as_ref\(\)\s+\.is_some_and\(\|path\| std::fs::write\(path, b""\)\.is_ok\(\)\);\s+if left \{[\s\S]{0,700}libc::mallopt\(libc::M_ARENA_MAX, 4\)[\s\S]{0,60}\} else \{\s+decision = Decision \{\s+mode: WindowMode::Today,\s+refused: Some\(EmbeddedUnavailable::FailedLastStart\),/,
  );
  assert.equal(code(startup).split('mallopt').length - 1, 1, 'the allocator is touched in that one place');
  assert.match(prepare, /let mode = decision\.mode;\s+let _ = DECISION\.set\(decision\);\s+mode$/);

  // The page reports that it loaded, and that is what clears the marker.
  assert.match(
    functionBody(commands, 'pub fn player_window_ready() {', 'player_window_ready'),
    /#\[cfg\(target_os = "linux"\)\]\s+crate::player::embedded::startup::window_came_up\(\);/,
  );
  assert.match(lib, /commands::player_window_ready,/);
  assert.match(page, /invoke\("player_window_ready"\)/);
});

test('an embedded play is offered only in a run that started in embedded window mode', () => {
  const library = functionBody(host, '    fn library(', 'Host::library');
  const gate = library.indexOf('if decided.mode != WindowMode::Embedded {');
  assert.notEqual(gate, -1);
  assert.match(
    library.slice(gate),
    /^if decided\.mode != WindowMode::Embedded \{\s+return Err\(decided\.refused\.unwrap_or\(EmbeddedUnavailable::NeedsRestart\)\);\s+\}/,
  );
  // Nothing about the machine is examined, and no library is loaded, before
  // that.
  for (const later of ['self.start_failed', 'self.checked.get_or_init']) {
    assert.ok(gate < library.indexOf(later), `${later} comes after the start-up check`);
  }
  // Both ways in go through it.
  assert.match(host, /pub\(crate\) fn availability\([\s\S]{0,120}self\.library\(app\)\.map\(\|_\| \(\)\)/);
  assert.match(host, /let lib = self\.library\(app\)\.map_err\(\|reason\| reason\.notice\(\)\)\?;/);
});

test("only Vela's own copy of the mpv library is ever loaded", () => {
  // Where: the resource directory, or in a development build the directory a
  // developer names. `cfg!(debug_assertions)` is false in every release.
  assert.match(
    host,
    /let directory = libmpv::set_directory\(\s+&resources,\s+cfg!\(debug_assertions\),\s+std::env::var_os\(DEVELOPMENT_SET_VARIABLE\)\.as_deref\(\),\s+\);\s+LibMpv::load\(&directory\.join\(libmpv::LIBRARY_FILE\)\)/,
  );
  // How: one call to the system's loader, after the path was checked to be
  // an exact file, with the flags that keep the set to itself.
  const load = functionBody(libmpv, '    pub(crate) fn load(', 'LibMpv::load');
  assert.ok(load.indexOf('refuse_unless_exact(path)?;') < load.indexOf('libc::dlopen('));
  assert.match(load, /let flags = libc::RTLD_NOW \| libc::RTLD_LOCAL \| libc::RTLD_DEEPBIND;/);
  assert.equal(code(libmpv).split('dlopen(').length - 1, 1);
  // Nothing links an mpv library at build time, and nothing else names one.
  assert.doesNotMatch(cargo, /^\s*(libmpv|mpv)[\w-]*\s*=/m, 'no mpv crate');
  for (const file of fs.readdirSync(path.join(repoRoot, 'src-tauri/src'), { recursive: true })) {
    if (!String(file).endsWith('.rs') || String(file).endsWith('embedded/libmpv.rs')) continue;
    assert.doesNotMatch(
      read(path.join('src-tauri/src', String(file))),
      /libmpv\.so/,
      `${file} must not name an mpv library file`,
    );
  }
});

test('a running embedded play is asked to stop, and waited for, before its window can go', () => {
  // The play draws into a child of Vela's window. Dropping its handle (the
  // exit sweep, a replaced play that is reaped) must not return while the
  // play still holds that surface.
  assert.match(
    session,
    /impl Drop for Session \{\s+fn drop\(&mut self\) \{[\s\S]{0,300}if self\.thread\.is_some\(\) \{\s+self\.stop_and_wait\(Duration::from_secs\(5\)\);/,
  );
  // Teardown order is field order: what draws goes before what it draws
  // with, and the surface last.
  assert.match(
    session,
    /struct Live \{\s+renderer: Renderer,\s+player: Player,\s+window: egl::Window,\s+surface: VideoSurface,/,
  );
});
