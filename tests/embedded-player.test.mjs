// The embedded player's wiring (.agents/plans/embedded-player.md, Slices 3
// and 4).
//
// The Rust unit tests prove the pieces: the start-up decision for every
// combination of facts (player/embedded/startup.rs), the loader's refusals
// (libmpv.rs), the launch description (player/launch.rs), the option files
// (options.rs), the HDR arithmetic and the agreement between mpv and the
// surface (hdr.rs). These prove the pieces are connected the way the plan
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
const surface = read('src-tauri/src/player/embedded/surface.rs');
const hdr = read('src-tauri/src/player/embedded/hdr.rs');
const embeddedMod = read('src-tauri/src/player/embedded/mod.rs');
const cargo = read('src-tauri/Cargo.toml');
const tauriConf = JSON.parse(read('src-tauri/tauri.conf.json'));
const page = read('src/routes/+page.svelte');

const code = (source) =>
  source
    .split('\n')
    .filter((line) => !/^\s*\/\//.test(line))
    .join('\n');

// A method of an `impl` block: from its signature to its own closing brace.
function methodBody(source, signature, what) {
  const start = source.indexOf(`\n${signature}`);
  assert.notEqual(start, -1, `${what} must exist`);
  const end = source.indexOf('\n    }\n', start);
  assert.notEqual(end, -1, `${what} must have a closing brace`);
  return source.slice(start, end);
}

const count = (source, text) => source.split(text).length - 1;

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

  // The one thing that decision reads from disk, on every Linux start, is
  // read so that no settings path can make a start wait (review finding
  // ec-3); the unit tests in startup.rs prove the reader, this proves
  // nothing else reads.
  assert.match(
    startup,
    /fn saved_player_setting\(\) -> Option<String> \{\s+let path = crate::config::config_path\(\)\.ok\(\)\?;\s+player_setting_in\(&settings_text\(&path\)\?\)\s+\}/,
  );
  const shippedStartup = code(startup.slice(0, startup.indexOf('#[cfg(test)]')));
  assert.equal(count(shippedStartup, 'OpenOptions::new()'), 1, 'one place opens a file');
  // What it opened is read through the bounded reader and no other way.
  assert.match(shippedStartup, /if !file\.metadata\(\)\.ok\(\)\?\.file_type\(\)\.is_file\(\) \{\s+return None;\s+\}\s+bounded_text\(file\)\s+\}/);
  assert.equal(count(shippedStartup, 'read_to_string'), 1, 'one read, inside the bounded reader');
  assert.match(shippedStartup, /source\s+\.take\(SETTINGS_READ_LIMIT \+ 1\)\s+\.read_to_string\(&mut text\)/);
  for (const read of ['fs::read', 'File::open', 'read_to_end', 'load_config', 'load_json']) {
    assert.equal(count(shippedStartup, read), 0, `start-up never calls ${read}`);
  }
  assert.match(
    shippedStartup,
    /\.custom_flags\(libc::O_NONBLOCK \| libc::O_NOFOLLOW\)\s+\.open\(path\)\s+\.ok\(\)\?;\s+if !file\.metadata\(\)\.ok\(\)\?\.file_type\(\)\.is_file\(\) \{\s+return None;\s+\}/,
  );

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

// ---- Slice 4: HDR

test('mpv is told what it draws for in one place, together with the surface', () => {
  // The unit tests in hdr.rs prove that `hdr::carry_out` changes mpv and
  // the surface together or not at all. This proves it is the only way
  // either of them is changed.
  const rust = fs
    .readdirSync(path.join(repoRoot, 'src-tauri/src'), { recursive: true })
    .map(String)
    .filter((file) => file.endsWith('.rs'));
  assert.ok(rust.includes('player/embedded/hdr.rs') && rust.length > 20);
  for (const file of rust) {
    if (file === 'player/embedded/hdr.rs') continue;
    assert.doesNotMatch(
      code(read(path.join('src-tauri/src', file))),
      /target-(trc|prim|peak)/,
      `${file} must not name an mpv target option`,
    );
  }
  // mpv: the one call that changes an option of the running player is the
  // one `hdr` drives.
  assert.equal(count(code(libmpv), '(self.lib.set_property_string)('), 1);
  assert.match(
    libmpv,
    /impl super::hdr::Targetable for Player \{[\s\S]{0,300}fn set_target\(&self, option: &str, value: &str\) -> bool \{[\s\S]{0,500}\(self\.lib\.set_property_string\)\(self\.raw, c_option\.as_ptr\(\), c_value\.as_ptr\(\)\)[\s\S]{0,40}self\.lib\.check\(code\)\.is_ok\(\)\s+\}\s+\}/,
  );
  // The surface: a description is asked for, put on and taken off only
  // through the same trait.
  const describable = surface.slice(surface.indexOf('\nimpl Describable for VideoSurface {'));
  assert.ok(describable.length > 0 && describable.startsWith('\nimpl Describable for VideoSurface {'));
  const ends = describable.indexOf('\n}\n');
  for (const request of [
    'create_parametric_creator(',
    'set_mastering_luminance(',
    '.set_image_description(',
    '.unset_image_description(',
  ]) {
    assert.equal(count(code(surface), request), 1, `${request} is sent from one place`);
    const at = describable.indexOf(request);
    assert.ok(at !== -1 && at < ends, `${request} is sent from the Describable implementation`);
  }
  // And the session reaches both through `hdr::carry_out`, once.
  assert.equal(count(code(session), 'hdr::carry_out('), 1);
  assert.match(
    session,
    /fn carry_out\(\s+player: &Player,\s+surface: &mut VideoSurface,\s+tagging: &mut Tagging,\s+step: hdr::Step,\s+\) -> bool \{\s+let change = hdr::carry_out\(step, tagging, player, surface\);[\s\S]{0,600}change\.needs_a_frame\(\)\s+\}/,
  );
  for (const direct of ['.describe(', '.tag(', '.untag(', '.set_target(']) {
    assert.equal(count(code(session), direct), 0, `the session never calls ${direct} itself`);
  }
  // Nothing in the module is switched off from the compiler's view: a part
  // of this that nothing calls fails the build.
  for (const [name, source] of [['hdr.rs', hdr], ['mod.rs', embeddedMod], ['surface.rs', surface]]) {
    assert.doesNotMatch(source, /allow\(dead_code\)/, `${name} hides no unused code`);
  }
  assert.match(embeddedMod, /^mod hdr;$/m);
});

test("nothing is sent to the compositor's colour manager that it did not offer", () => {
  // A request the compositor did not offer is a protocol error; it ends the
  // connection, and the connection is the one Vela's window lives on.
  assert.match(
    methodBody(surface, '    pub(crate) fn colour_support(&self) -> bool {', 'colour_support'),
    /let colour = self\.state\.colour;\s+self\.state\.colour_manager\.is_some\(\)\s+&& self\.feedback\.is_some\(\)\s+&& colour\.parametric\s+&& colour\.mastering\s+&& colour\.pq\s+&& colour\.bt2020\s+&& colour\.perceptual$/,
  );
  // Each of those is what the compositor announced, under the protocol's
  // own numbers.
  for (const [constant, value] of [
    ['FEATURE_PARAMETRIC', 1],
    ['FEATURE_MASTERING_DISPLAY', 5],
    ['TRANSFER_PQ', 11],
    ['PRIMARIES_BT2020', 6],
    ['INTENT_PERCEPTUAL', 0],
  ]) {
    assert.match(surface, new RegExp(`^const ${constant}: u32 = ${value};$`, 'm'));
  }
  assert.match(surface, /FEATURE_PARAMETRIC => state\.colour\.parametric = true,\s+FEATURE_MASTERING_DISPLAY => state\.colour\.mastering = true,/);
  assert.match(surface, /state\.colour\.pq \|= u32::from\(tf\) == TRANSFER_PQ;/);
  assert.match(surface, /state\.colour\.bt2020 \|= u32::from\(primaries\) == PRIMARIES_BT2020;/);
  assert.match(surface, /state\.colour\.perceptual \|= u32::from\(render_intent\) == INTENT_PERCEPTUAL;/);
  // The version whose events this code handles (`ready`, `preferred_changed`).
  assert.match(
    surface,
    /"wp_color_manager_v1" => state\.colour_manager = Some\(registry\.bind\(name, 1, qh, \(\)\)\),/,
  );

  // Asking for a description, and putting one on the surface, each begin by
  // checking; a description is put on the surface only once it was made.
  assert.match(
    methodBody(surface, '    fn describe(&mut self, target: &HdrTarget) {', 'describe'),
    /^\n    fn describe\(&mut self, target: &HdrTarget\) \{\n        if !self\.colour_support\(\) \{\n            return;\n        \}/,
  );
  assert.match(
    methodBody(surface, '    fn tag(&mut self) -> bool {', 'tag'),
    /^\n    fn tag\(&mut self\) -> bool \{\n        if !self\.colour_support\(\) \|\| !self\.state\.made \{\n            return false;\n        \}/,
  );
  assert.match(surface, /\(Described::Surface, Event::Ready \{ \.\. \}\) => \{\s+state\.answer = Some\(true\);\s+state\.made = true;\s+\}/);
  assert.match(surface, /self\.state\.answer = None;\s+self\.state\.made = false;\s+\/\/ Creating the description consumes the parameters object\.\s+self\.described = Some\(parameters\.create\(&qh, Described::Surface\)\);/);
  assert.equal(count(code(surface), 'made = true'), 1);
  // The output's description is asked for in its readable form only where
  // that form exists, and read only once it is ready.
  assert.match(
    methodBody(surface, '    pub(crate) fn ask_about_output(&mut self) {', 'ask_about_output'),
    /let \(Some\(feedback\), true\) = \(&self\.feedback, self\.state\.colour\.parametric\) else \{\s+return;\s+\};/,
  );
  assert.equal(count(code(surface), '.get_information('), 1);
  assert.match(surface, /\(Described::Output, Event::Ready \{ \.\. \}\) => \{[\s\S]{0,400}description\.get_information\(qh, \(\)\);/);
});

test("what the compositor says about the display is followed from a play's start to its end", () => {
  // One function acts on it...
  const follow = functionBody(session, 'fn follow_colour(', 'follow_colour');
  assert.match(
    follow,
    /if surface\.output_changed\(\) && capabilities\.hdr_possible\(\) \{\s+surface\.ask_about_output\(\);\s+\}\s+if let Some\(output\) = surface\.output_described\(\) \{\s+let step = tagging\.output_described\(hdr::presentation\(Some\(output\), capabilities\)\);\s+redraw \|= carry_out\(player, surface, tagging, step\);\s+\}\s+if let Some\(made\) = surface\.description_answer\(\) \{\s+let step = if made \{\s+tagging\.description_ready\(\)\s+\} else \{\s+tagging\.description_failed\(\)\s+\};\s+redraw \|= carry_out\(player, surface, tagging, step\);\s+\}\s+redraw$/,
  );
  assert.equal(count(code(session), 'hdr::presentation('), 1);
  assert.equal(count(code(session), 'follow_colour('), 3, 'its definition and its two uses');

  // ...before the first picture, once mpv has every other option, with what
  // this machine can do...
  const setUp = functionBody(session, 'fn set_up(', 'set_up');
  assert.match(
    setUp,
    /let capabilities = Capabilities \{\s+can_describe: surface\.colour_support\(\),\s+ten_bit_window: window\.depth == 10,\s+\};\s+let mut tagging = Tagging::new\(\);/,
  );
  const settle = setUp.search(
    /if capabilities\.hdr_possible\(\) \{\s+surface\.ask_about_output\(\);\s+for _ in 0\.\.COLOUR_EXCHANGES_AT_START \{\s+surface\.roundtrip\(\)\?;\s+follow_colour\(&player, &mut surface, capabilities, &mut tagging\);\s+if tagging\.settled\(\) \{\s+break;\s+\}\s+\}\s+\}/,
  );
  assert.notEqual(settle, -1);
  const optionsChecked = setUp.indexOf('return Err("mpv did not take the player\'s options".to_string());');
  const fileGiven = setUp.indexOf('.command(&["loadfile", &start.url])');
  assert.ok(optionsChecked !== -1 && fileGiven !== -1);
  assert.ok(optionsChecked < settle, "after the user's options and Vela's own");
  assert.ok(settle < fileGiven, 'before mpv has anything to draw');
  assert.equal(count(code(setUp), 'Capabilities {'), 1);
  assert.match(setUp, /capabilities,\s+tagging,\s+\}\)$/, 'and the running play keeps both');

  // ...and on every turn of the running play, drawing a frame when it
  // changed something.
  const run = methodBody(session, '    fn run(&mut self, shared: &Shared) {', 'Live::run');
  assert.match(
    run,
    /redraw \|= follow_colour\(\s+&self\.player,\s+&mut self\.surface,\s+self\.capabilities,\s+&mut self\.tagging,\s+\);/,
  );
  assert.ok(
    run.indexOf('redraw |= follow_colour(') < run.indexOf('if self.renderer.has_new_frame() || redraw {'),
  );
  // The compositor's change of mind is what sets it off.
  assert.match(
    surface,
    /if let wp_color_management_surface_feedback_v1::Event::PreferredChanged \{ \.\. \} = event \{\s+state\.output_changed = true;\s+\}/,
  );
});
