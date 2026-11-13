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
  // The path is never opened for reading to find out what it is (that is
  // an act on a pipe or a terminal). A handle is taken without opening,
  // asked what it is, and only a regular file is then opened, through the
  // handle. These are the only two opens start-up makes, in this order.
  assert.match(
    functionBody(shippedStartup, 'fn settings_text(path: &std::path::Path) -> Option<String> {', 'settings_text'),
    /\{\s+text_of\(&regular_file_at\(path\)\?\)$/,
  );
  assert.match(
    functionBody(shippedStartup, 'fn regular_file_at(path: &std::path::Path) -> Option<std::fs::File> {', 'regular_file_at'),
    /let held = std::fs::OpenOptions::new\(\)\s+\.read\(true\)\s+\.custom_flags\(libc::O_PATH \| libc::O_NOFOLLOW\)\s+\.open\(path\)\s+\.ok\(\)\?;\s+held\.metadata\(\)\.ok\(\)\?\.file_type\(\)\.is_file\(\)\.then_some\(held\)$/,
  );
  // The second half is given the handle and no path.
  assert.match(
    functionBody(shippedStartup, 'fn text_of(held: &std::fs::File) -> Option<String> {', 'text_of'),
    /let file = std::fs::OpenOptions::new\(\)\s+\.read\(true\)\s+\.custom_flags\(libc::O_NONBLOCK \| libc::O_NOCTTY\)\s+\.open\(format!\("\/proc\/self\/fd\/\{\}", held\.as_raw_fd\(\)\)\)\s+\.ok\(\)\?;\s+if !file\.metadata\(\)\.ok\(\)\?\.file_type\(\)\.is_file\(\) \{\s+return None;\s+\}\s+bounded_text\(file\)$/,
  );
  assert.equal(count(shippedStartup, 'OpenOptions::new()'), 2, 'those two opens and no other');
  assert.equal(count(shippedStartup, '.open(path)'), 1, 'the path itself is opened once, without reading');
  assert.equal(count(shippedStartup, 'read_to_string'), 1, 'one read, inside the bounded reader');
  assert.match(shippedStartup, /source\s+\.take\(SETTINGS_READ_LIMIT \+ 1\)\s+\.read_to_string\(&mut text\)/);
  for (const read of ['fs::read', 'File::open', 'read_to_end', 'load_config', 'load_json']) {
    assert.equal(count(shippedStartup, read), 0, `start-up never calls ${read}`);
  }

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
  assert.match(host, /let Ready \{ lib, guest \} = self\.library\(app\)\.map_err\(\|reason\| reason\.notice\(\)\)\?;/);
});

test("only Vela's own copy of the mpv library is ever loaded", () => {
  // Where: the resource directory, or in a development build the directory a
  // developer names. `cfg!(debug_assertions)` is false in every release.
  assert.match(
    host,
    /let directory = libmpv::set_directory\(\s+&resources,\s+cfg!\(debug_assertions\),\s+std::env::var_os\(DEVELOPMENT_SET_VARIABLE\)\.as_deref\(\),\s+\);\s+let lib = LibMpv::load\(&directory\.join\(libmpv::LIBRARY_FILE\)\)/,
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
    /impl Drop for Session \{\s+fn drop\(&mut self\) \{[\s\S]{0,400}if self\.thread\.is_some\(\) && !self\.stop_and_wait\(self\.patience\) \{/,
  );
  assert.match(session, /^const STOP_PATIENCE: Duration = Duration::from_secs\(5\);$/m);
  assert.match(session, /thread: Some\(thread\),\s+header_include,\s+patience: STOP_PATIENCE,\s+leftovers: &LEFTOVERS,\s+\};/);
  // Teardown order is field order: what draws goes before what it draws
  // with, and the surface last.
  assert.match(
    session,
    /struct Live \{\s+renderer: Renderer,\s+player: Player,\s+window: egl::Window,\s+surface: VideoSurface,/,
  );
});

test("the window's Wayland connection is joined once for the whole run, never per play", () => {
  // Joining asks the compositor for a registry, which the protocol cannot
  // give back: the compositor keeps one per request until the connection
  // closes, and this connection is the window's (review finding ec-9).
  const shippedSurface = code(surface.slice(0, surface.indexOf('#[cfg(test)]\nmod tests')));
  assert.equal(count(shippedSurface, 'get_registry('), 1, 'one place asks for a registry');
  assert.equal(count(shippedSurface, 'registry.bind('), 5, 'and one place binds globals, five of them');
  assert.ok(
    shippedSurface.lastIndexOf('registry.bind(') < shippedSurface.indexOf('pub(crate) struct Guest {'),
    'all of them where the list of globals is read',
  );
  const join = methodBody(surface, '    pub(crate) unsafe fn join(display: WlDisplay) -> Result<Self, String> {', 'Guest::join');
  assert.ok(join.includes('Backend::from_foreign_display(') && join.includes('.get_registry('));
  // That place is reached once per process: from the availability check,
  // whose answer is kept.
  assert.equal(count(code(host), 'Guest::join('), 1);
  const check = methodBody(host, '    fn check(app: &tauri::AppHandle) -> Result<Ready, EmbeddedUnavailable> {', 'Host::check');
  assert.ok(check.includes('Guest::join(display)'));
  assert.equal(count(code(host), 'Self::check('), 1);
  assert.match(host, /self\.checked\.get_or_init\(\|\| Self::check\(app\)\)\.clone\(\)/);
  assert.match(host, /checked: OnceLock<Result<Ready, EmbeddedUnavailable>>,/);
  for (const file of [session, libmpv, read('src-tauri/src/player/embedded/egl.rs')]) {
    assert.equal(count(code(file), 'Guest::join('), 0);
    assert.equal(count(code(file), 'get_registry('), 0);
  }
  // A play makes its surface from those globals and asks the compositor's
  // registry nothing. It does so through a connection object of its own,
  // with a queue of its own: two threads sharing one take each other's
  // events off it, and a round trip can then wait for an answer the other
  // thread has already consumed (the first repair did; reopened).
  const create = surface.slice(surface.indexOf('    pub(crate) unsafe fn create('));
  const createBody = create.slice(0, create.indexOf('\n    }\n'));
  assert.match(
    createBody,
    /^    pub\(crate\) unsafe fn create\(\s+guest: &Arc<Guest>,\s+parent_surface: \*mut c_void,\s+width: i32,\s+height: i32,\s+\) -> Result<Self, String> \{\s+guest\.drain\(\);\s+(?:\/\/[^\n]*\n\s+)*let conn = Connection::from_backend\(unsafe \{\s+Backend::from_foreign_display\(guest\.display\.0\.cast\(\)\)\s+\}\);\s+let queue = conn\.new_event_queue\(\);/,
  );
  assert.equal(count(code(createBody), 'registry'), 0);
  for (const [type, field] of [
    ['WlCompositor', 'compositor'],
    ['WlSubcompositor', 'subcompositor'],
    ['WpViewporter', 'viewporter'],
  ]) {
    assert.ok(
      createBody.includes(`${type}::from_id(&conn, guest.${field}.id())`),
      `the play uses the process's ${field} from its own connection object`,
    );
  }
  assert.match(createBody, /\.map\(\|manager\| WpFractionalScaleManagerV1::from_id\(&conn, manager\.id\(\)\)\)/);
  assert.match(createBody, /\.map\(\|manager\| WpColorManagerV1::from_id\(&conn, manager\.id\(\)\)\)/);
  // Exactly two connection objects are ever made: the process's, and one for
  // each play. Each has one queue, and nothing reaches into the other's.
  assert.equal(count(shippedSurface, 'from_foreign_display('), 2);
  assert.equal(count(shippedSurface, 'new_event_queue()'), 2);
  assert.equal(count(shippedSurface, 'guest._conn'), 0, "a play never uses the process's connection object");
  assert.equal(count(shippedSurface, 'guest.announcements'), 0);
  assert.match(surface, /pub\(crate\) struct VideoSurface \{\s+(?:\/\/\/[^\n]*\n\s+)*guest: Arc<Guest>,\s+(?:\/\/\/[^\n]*\n\s+)*conn: Connection,\s+globals: Adopted,\s+queue: EventQueue<State>,/);
  // And what a play makes, it destroys by name: nothing is left for a
  // closing connection to sweep up, because this one never closes.
  const drop = surface.slice(surface.indexOf('impl Drop for VideoSurface {'));
  const dropBody = drop.slice(0, drop.indexOf('\n}\n'));
  const fields = surface.slice(surface.indexOf('pub(crate) struct VideoSurface {'));
  const made = [...fields.slice(0, fields.indexOf('\n}\n')).matchAll(/^    (\w+): (?:Option<)?(?:Wl|Wp)\w+>?,$/gm)].map((m) => m[1]);
  assert.deepEqual(
    made,
    ['surface', 'subsurface', 'viewport', 'fractional', 'feedback', 'colour_surface', 'output_query', 'described'],
    'the Wayland objects a play holds',
  );
  for (const field of made) {
    assert.match(
      dropBody,
      new RegExp(`(self\\.${field}\\.destroy\\(\\)|Some\\(\\w+\\) = &self\\.${field}\\b[\\s\\S]{0,40}\\.destroy\\(\\)|self\\.${field}\\.take\\(\\))`),
      `${field} is destroyed when the play ends`,
    );
  }
  assert.match(dropBody, /for description in \[self\.described\.take\(\), self\.output_query\.take\(\)\]\s+\.into_iter\(\)\s+\.flatten\(\)\s+\{\s+description\.destroy\(\);/);
  // What a play must never destroy is what it did not make: the process's
  // globals, which every later play needs.
  assert.equal(count(dropBody, 'self.globals'), 0);
  assert.equal(count(dropBody, 'self.guest'), 0);
  assert.equal(count(createBody, '.destroy()'), 1, 'making a surface destroys one thing: its throwaway region');
  assert.match(createBody, /let nothing = compositor\.create_region\(&qh, \(\)\);\s+surface\.set_input_region\(Some\(&nothing\)\);\s+nothing\.destroy\(\);/);
  assert.equal(count(shippedSurface, 'compositor.destroy()'), 0);
  assert.equal(count(shippedSurface, 'viewporter.destroy()'), 0);
  assert.equal(count(shippedSurface, 'manager.destroy()'), 0);
});

test('a play that outlasts the wait for it neither starts unannounced nor goes untracked', () => {
  // The pieces are proven in session.rs: who settles a start
  // (`StartRace`), the last step of one (`finish_start`), and where a thread
  // goes that will not end (`Leftovers`). This proves a real play uses them
  // (review finding ec-2).
  const shippedSession = code(session.slice(0, session.indexOf('#[cfg(test)]\nmod tests')));
  // Building a play does not hand mpv its file; the last step does, and
  // only a play that won its start is announced and drawn.
  const setUp = functionBody(session, 'fn set_up(', 'set_up');
  assert.equal(count(setUp, 'loadfile'), 0);
  assert.equal(count(setUp, 'live.load('), 0);
  assert.equal(count(shippedSession, '"loadfile"'), 1);
  assert.equal(count(shippedSession, 'live.load('), 1);
  assert.match(
    session,
    /match finish_start\(&shared\.race, \|\| live\.load\(&start\.url\)\) \{\s+Ok\(Started::Yes\) => \{\s+let _ = ready\.send\(Ok\(\(\)\)\);\s+\(start\.on_lifecycle\)\(Lifecycle::Opened\);\s+live\.run\(&shared\);\s+forget_surface\(\);\s+drop\(live\);\s+\(start\.on_lifecycle\)\(Lifecycle::Closed\);\s+\}\s+(?:\/\/[^\n]*\n\s+)*Ok\(Started::TooLate\) => \{\s+forget_surface\(\);\s+drop\(live\);\s+\}\s+Err\(reason\) => \{\s+forget_surface\(\);\s+drop\(live\);\s+let _ = ready\.send\(Err\(reason\)\);\s+\}\s+\}/,
  );
  for (const once of ['Lifecycle::Opened)', 'Lifecycle::Closed)', 'live.run(', 'ready.send(Ok(']) {
    assert.equal(count(shippedSession, once), 1, `${once} in that one arm`);
  }
  // The caller's side of the same decision: it reports "not in time" only
  // if it won; if the play won in the same instant, it is a started play.
  assert.match(
    session,
    /match started\.recv_timeout\(start_limit\) \{\s+Ok\(Ok\(\(\)\)\) => Ok\(session\),\s+Ok\(Err\(reason\)\) => Err\(reason\),\s+Err\(_\) if session\.shared\.race\.give_up\(\) => late\(\),/,
  );
  assert.equal(count(shippedSession, '.give_up()'), 1);
  // How long that wait is: twenty seconds, in every build a user gets.
  assert.match(session, /^pub\(crate\) const START_LIMIT: Duration = Duration::from_secs\(20\);$/m);
  assert.match(session, /let start_limit = start\.start_limit;/);
  assert.match(
    host,
    /start_limit: start_limit\(\s+cfg!\(debug_assertions\),\s+std::env::var_os\(DEVELOPMENT_START_LIMIT_VARIABLE\)\.as_deref\(\),\s+\),/,
  );
  assert.equal(count(shippedSession, '.claim_started()'), 1, 'claimed in finish_start only');
  // A thread's handle leaves a play in two ways only: joined once the play
  // has ended, or kept when it has not ended in time. Never just dropped.
  assert.equal(count(shippedSession, 'self.thread.take()'), 2);
  assert.match(session, /if let Some\(thread\) = self\.thread\.take\(\) \{\s+let _ = thread\.join\(\);\s+\}/);
  assert.match(
    session,
    /if self\.thread\.is_some\(\) && !self\.stop_and_wait\(self\.patience\) \{\s+if let Some\(thread\) = self\.thread\.take\(\) \{\s+eprintln!\([^)]*\);\s+self\.leftovers\.keep\(self\.shared\.clone\(\), thread\);/,
  );
  // While one is kept, no embedded play is started next to it; and Vela
  // gives them one more moment as it exits.
  const library = methodBody(host, '    fn library(', 'Host::library');
  const refusal = library.indexOf('if session::unfinished_plays() > 0 {\n            return Err(EmbeddedUnavailable::StartFailed);\n        }');
  assert.notEqual(refusal, -1);
  assert.ok(refusal < library.indexOf('self.checked.get_or_init'));
  assert.match(
    lib,
    /queue\.clear\(\);\s+\}\s+(?:\/\/[^\n]*\n\s+)*#\[cfg\(target_os = "linux"\)\]\s+player::embedded::wait_for_unfinished_plays\(std::time::Duration::from_secs\(2\)\);/,
  );
});

test("the window's own surface is used on the window's thread, at once, and never carried away", () => {
  // The window can be closed at any moment, on its own thread. A pointer to
  // its surface that is carried to the play's thread and used there later
  // may be to a surface that no longer exists (review finding ec-1). So the
  // one use of it, making the video surface its child, happens on the
  // window's thread, inside the same call that fetched it.
  const prepare = methodBody(host, '    pub(crate) fn prepare(', 'Host::prepare');
  assert.match(
    prepare,
    /let surface = on_window_thread\(app, \{\s+let app = app\.clone\(\);\s+let guest = guest\.clone\(\);\s+move \|\| \{\s+let facts = window_facts\(&app\)\?;\s+let rect = rect_in_window\(facts\.page, reported\.unwrap_or\(whole\(facts\.page\)\)\);[\s\S]{0,300}unsafe \{ VideoSurface::create\(&guest, facts\.surface, rect\.width, rect\.height\) \}/,
  );
  const embedded = ['egl', 'hdr', 'host', 'libmpv', 'mod', 'options', 'session', 'startup', 'surface'].map((name) => [
    name,
    read(`src-tauri/src/player/embedded/${name}.rs`),
  ]);
  const everywhere = embedded.map(([, source]) => code(source)).join('\n');
  assert.equal(count(everywhere, 'VideoSurface::create('), 1, 'the surface is made in that one place');
  assert.equal(count(code(host), 'facts.surface'), 1, 'and the pointer is read in that one place');
  // It cannot be carried away either: what holds it cannot be sent to
  // another thread, and the build fails if that changes.
  assert.match(host, /struct WindowFacts \{\s+display: WlDisplay,\s+surface: \*mut c_void,/);
  assert.match(
    host,
    /const _: fn\(\) = \|\| \{\s+trait AmbiguousIfSend<Marker> \{\s+fn check\(\) \{\}\s+\}\s+impl<T: \?Sized> AmbiguousIfSend<\(\)> for T \{\}\s+impl<T: \?Sized \+ Send> AmbiguousIfSend<u8> for T \{\}\s+let _ = <WindowFacts as AmbiguousIfSend<_>>::check;\s+\};/,
  );
  // The only raw pointer of the window's that IS allowed to travel is the
  // connection, which lives as long as the process.
  for (const [name, source] of embedded) {
    const sends = [...code(source).matchAll(/unsafe impl (?:Send|Sync) for (\w+)/g)].map((m) => m[1]);
    const allowed =
      {
        surface: ['WlDisplay', 'WlDisplay'],
        // Tables of functions in loaded libraries, and an mpv handle.
        egl: ['Api', 'Api'],
        libmpv: ['LibMpv', 'LibMpv', 'Player'],
      }[name] ?? [];
    assert.deepEqual(sends, allowed, `${name}.rs: what is declared safe to send`);
  }
  // Inside the surface module the window's surface is used for exactly the
  // two requests that make the new surface its child and put it below, and
  // the window's thread is never made to wait for the compositor.
  const create = surface.slice(surface.indexOf('    pub(crate) unsafe fn create('));
  const createBody = create.slice(0, create.indexOf('\n    }\n'));
  assert.equal(count(createBody, '&parent'), 2);
  assert.match(createBody, /subcompositor\.get_subsurface\(&surface, &parent, &qh, \(\)\);\s+subsurface\.place_below\(&parent\);/);
  for (const waits of ['roundtrip', 'blocking_dispatch', 'prepare_read', '.wait(']) {
    assert.equal(count(createBody, waits), 0, `making the surface never calls ${waits}`);
  }
  assert.equal(count(code(surface), 'parent_surface'), 2, 'the pointer is a parameter and is used once');
  // The play's own thread does that waiting, first thing.
  assert.match(
    session,
    /fn set_up\(start: &Start, mut surface: VideoSurface, shared: &Arc<Shared>\) -> Result<Live, String> \{\s+(?:\/\/[^\n]*\n\s+)*surface\.settle\(\)\?;/,
  );
  assert.match(
    surface,
    /pub\(crate\) fn settle\(&mut self\) -> Result<\(\), String> \{\s+self\.queue\.roundtrip\(&mut self\.state\)\.map_err\(describe\)\?;\s+self\.apply\(\);\s+Ok\(\(\)\)\s+\}/,
  );
});

test('both players give up a consumed stream-auth file the same way', () => {
  // `HeaderInclude::remove_consumed` keeps the file with its owner when the
  // removal fails (proven in player/launch.rs and, for the embedded play, in
  // session.rs). This proves each player's handle uses it and has no way of
  // its own (review finding ec-7: the embedded one had, and lost the file).
  const external = read('src-tauri/src/player/external.rs');
  const launch = read('src-tauri/src/player/launch.rs');
  assert.match(
    external,
    /pub\(crate\) fn remove_consumed_header_include\(&mut self\) -> Result<\(\), String> \{\s+HeaderInclude::remove_consumed\(&mut self\._header_include\)\s+\}/,
  );
  assert.match(
    session,
    /pub\(crate\) fn remove_consumed_header_include\(&mut self\) -> Result<\(\), String> \{\s+HeaderInclude::remove_consumed\(&mut self\.header_include\)\s+\}/,
  );
  // Nothing outside `launch.rs` removes one directly, and an include leaves
  // its owner in exactly one place.
  const shipped = (source) => code(source.slice(0, source.indexOf('#[cfg(test)]\nmod tests')));
  for (const [name, source] of [['external.rs', external], ['session.rs', session]]) {
    assert.equal(count(shipped(source), 'remove_now'), 0, `${name} never removes an include itself`);
  }
  assert.equal(count(shipped(session), 'header_include.take()'), 1, 'taken once: out of the start request');
  assert.match(session, /let header_include = start\.header_include\.take\(\);/);
  assert.equal(count(shipped(external), '_header_include.take()'), 0);
  assert.match(
    launch,
    /pub\(super\) fn remove_consumed\(owner: &mut Option<HeaderInclude>\) -> Result<\(\), String> \{\s+let Some\(mut include\) = owner\.take\(\) else \{\s+return Ok\(\(\)\);\s+\};\s+if include\.remove_now\(\)\.is_ok\(\) \{\s+return Ok\(\(\)\);\s+\}\s+\*owner = Some\(include\);\s+Err\(/,
  );
});

test('everything a launch wrote to disk has an owner in an embedded play', () => {
  // A launch may write two files: the stream-auth include and the marker
  // payload. The external launcher removes the payload itself when it cannot
  // start; the embedded one used to take the plan apart and drop the
  // payload's path (review finding ec-8). The plan is now taken apart by
  // name, with no "and the rest", and both files go into the start request.
  assert.match(
    host,
    /let LaunchPlan \{\s+args,\s+env: _,\s+url,\s+header_include,\s+marker_payload,\s+\} = plan;/,
  );
  const start = host.slice(host.indexOf('let start = Start {'));
  assert.ok(start.length > 0 && start.startsWith('let start = Start {'));
  const request = start.slice(0, start.indexOf('};'));
  assert.match(request, /\n            header_include,\n            _marker_payload: marker_payload\.map\(MarkerPayload::new\),\n/);
  // The request lives exactly as long as the play, or as its failed start:
  // the play's thread owns it, and it is nowhere copied or kept.
  assert.match(session, /\.spawn\(move \|\| \{\s+let shared = thread_shared;[\s\S]{0,500}?match set_up\(&start, surface, &shared\) \{/);
  assert.match(
    session,
    /impl Drop for MarkerPayload \{\s+fn drop\(&mut self\) \{[\s\S]{0,120}let _ = std::fs::remove_file\(&self\.0\);\s+\}\s+\}/,
  );
  const shippedSession = code(session.slice(0, session.indexOf('#[cfg(test)]\nmod tests')));
  assert.equal(count(shippedSession, 'std::mem::forget'), 0);
  assert.equal(count(code(host), 'std::mem::forget'), 0);
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
    /let colour = self\.guest\.colour;\s+self\.globals\.colour_manager\.is_some\(\)\s+&& self\.feedback\.is_some\(\)\s+&& colour\.parametric\s+&& colour\.mastering\s+&& colour\.pq\s+&& colour\.bt2020\s+&& colour\.perceptual$/,
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
    /let \(Some\(feedback\), true\) = \(&self\.feedback, self\.guest\.colour\.parametric\) else \{\s+return;\s+\};/,
  );
  assert.equal(count(code(surface), '.get_information('), 1);
  assert.match(surface, /\(Described::Output\(question\), Event::Ready \{ \.\. \}\) => \{[\s\S]{0,400}description\.get_information\(qh, \*question\);/);
});

test("every piece of the output's description is matched to the question it answers", () => {
  // `OutputAnswers` (hdr.rs, with its tests) puts a description together
  // from its pieces and hands on only the answer to the latest question.
  // This proves the surface gives it every piece with the right number: the
  // number travels on the protocol objects themselves, from the question to
  // the object that delivers its contents (review finding hd-1: one shared
  // slot let the tail of an old answer replace a new one).
  assert.match(
    surface,
    /let question = self\.state\.answers\.ask\(\);\s+self\.output_query = Some\(\s+feedback\.get_preferred_parametric\(&self\.queue\.handle\(\), Described::Output\(question\)\),\s+\);/,
  );
  assert.match(
    surface,
    /\(Described::Output\(question\), Event::Ready \{ \.\. \}\) => \{\s+(?:\/\/[^\n]*\n\s+)*if state\.answers\.ready\(\*question\) \{\s+description\.get_information\(qh, \*question\);\s+\}\s+\}/,
  );
  const from = surface.indexOf('impl Dispatch<WpImageDescriptionInfoV1, u32> for State {');
  assert.notEqual(from, -1, 'the object that delivers the contents carries the number');
  const contents = code(surface.slice(from, surface.indexOf('\n}\n', from)));
  assert.match(contents, /question: &u32,/);
  assert.match(contents, /state\.answers\.piece\(\*question, Piece::Reference\(reference_lum\)\)/);
  assert.match(contents, /state\.answers\.piece\(\s+\*question,\s+Piece::Range \{/);
  assert.match(contents, /state\.answers\.piece\(\s+\*question,\s+Piece::Primaries\(/);
  assert.match(contents, /Event::Done => state\.answers\.done\(\*question\),/);
  assert.equal(count(contents, 'state.answers.'), 4, 'three pieces and the end, nothing else');
  // Nothing else in the module keeps or collects a description.
  const shippedSurface = code(surface.slice(0, surface.indexOf('#[cfg(test)]\nmod tests')));
  assert.equal(count(shippedSurface, 'answers.'), 7, 'ask, ready, three pieces, done, take');
  assert.match(
    surface,
    /pub\(crate\) fn output_described\(&mut self\) -> Option<OutputDescription> \{\s+self\.state\.answers\.take\(\)\s+\}/,
  );
  assert.doesNotMatch(shippedSurface, /\barriving\b/);
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
  assert.notEqual(optionsChecked, -1);
  assert.ok(optionsChecked < settle, "after the user's options and Vela's own");
  // Before mpv has anything to draw: building a play gives mpv no file,
  // and the file is handed over only once the play is built.
  assert.equal(count(setUp, 'loadfile'), 0);
  const built = session.indexOf('match set_up(&start, surface, &shared) {');
  const fileGiven = session.indexOf('match finish_start(&shared.race, || live.load(&start.url)) {');
  assert.ok(built !== -1 && built < fileGiven);
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
