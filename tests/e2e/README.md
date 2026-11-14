# E2E harness

Drives the real debug Vela binary through `tauri-driver` + a vendored
WebKitWebDriver, on a throwaway config dir (`XDG_CONFIG_HOME` points into a
temp dir per scenario — the owner's `~/.config/vela` is never touched).
Design and deviations: `.agents/plans/e2e-harness.md`.

```sh
npm run e2e                    # build debug app, run all scenarios
npm run e2e -- --skip-build    # reuse the existing debug binary
npm run e2e -- smoke           # run one scenario by name
npm run e2e -- --embedded      # the same suite, played in the embedded player
```

- A run that builds starts the executable Cargo's own report of that build
  names for the app (the build passes `--message-format=json-render-diagnostics`
  through `tauri build` to Cargo, and the runner takes the compiler-artifact
  message of the `src-tauri/Cargo.toml` package's binary, path as printed),
  wherever the environment or a Cargo configuration sent it. It stops when no
  such message arrives or more than one executable claims to be the app.
- A run with `--skip-build` builds nothing, so nothing reports an artifact. It
  starts a prediction: `<target_directory>/debug/<the app package's binary>`,
  with `target_directory` from `cargo metadata` asked in `src-tauri/` with the
  runner's environment (so `CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR` and a
  configuration's `target-dir` are followed, and the path is not normalized).
  It does not follow a Cargo configuration file's `build.target`: in that
  setup it may start an older binary at `<target directory>/debug/`, or stop
  with "app binary missing" when there is none. `CARGO_BUILD_TARGET` in the
  environment is refused. Such a run is recorded as not built.
- The runner prints the path it runs and where the path came from, and
  records both in the results file (`appBinary`, `appBinaryFrom`).
- Every session starts that one path: the WebDriver client (`driver.mjs`) is
  made with it and starts nothing else. When the path was found, its device,
  inode, size and modification time were noted; before each session starts
  (a restart included), the client checks them again and refuses to start a
  file that was written over or put in its place since.
- Requires Linux, `tauri-driver` (`cargo install tauri-driver`), `Xvfb`
  (`xorg-server-xvfb`), `bsdtar`, `curl`, `openssl`, and — for the playback-driving
  scenarios — `ffmpeg` and `mpv`. Server flows run against in-process mock
  Jellyfin servers (`mockjf.mjs`) whose Range-capable streams serve
  ffmpeg-generated clips; no real server or network is touched.
- Runs headless on a private Xvfb display by default — screenshots on the
  live Wayland desktop hang whenever the test window is occluded/unfocused
  (no frame callbacks). `VELA_E2E_HEADED=1` opts into the real desktop to
  watch a run; `VELA_E2E_DISPLAY=:N` picks the Xvfb display;
  `VELA_E2E_DEBUG=1` logs each WebDriver call with timing.
- First run downloads the vendored driver via `fetch-driver.sh` into
  `vendor/` (gitignored, sha256-pinned).
- Screenshots and driver logs land in `artifacts/` (gitignored) for owner
  flip-through.
- Scenarios live in `scenarios/*.mjs`: `export default { name, rows, async
  run(ctx) }` (see "What a scenario declares" below) with `ctx = { driver,
  screenshot(name), restart(between), repoRoot, configRoot }`; assert with
  `node:assert/strict`.

## What a scenario declares

The runner and the parity test load scenarios through one loader,
`scenario-loader.mjs`, which refuses a scenario it does not understand
before anything runs (the whole run stops, naming the file and the
problem). A scenario's default export may hold only these keys:

- `name`: the file's name without `.mjs` (`live-<file>` for `live/`).
- `seed`, `environment`, `cleanup`, `run`: functions; `run` is required.
- `rows`: required. The rows of the players' behaviour inventory
  (`parity.mjs`, from `.agents/plans/embedded-player.md`) the scenario
  shows, as strings: `'1'` … `'14'`, with row 8 split into `'8a'` (automatic
  skip) and `'8b'` (the skip button), row 9 by mode into `'9a'` (Off),
  `'9b'` (Manual) and `'9c'` (Auto), row 12 into `'12a'` (a new play
  stops the previous one) and `'12b'` (app exit stops playback), and row 13
  into `'13a'` (the Advanced mpv arguments) and `'13b'` (own-config mode). A
  scenario that shows none says `rows: []` and why, in `noRows`. A scenario
  that declares rows calls `shown(<rows>)` (from `proof.mjs`) where its proof
  of them ends: mostly the last statement of `run()`, and in a scenario
  whose legs prove different rows (`autocrop`, `embeddedcrop`), the end of
  each row's last leg, never before its last check; the runner fails a
  scenario that returns without having reported every row it declares, or
  reports one it does not declare. So a `run()` that returns early in one
  player, or does nothing, fails there instead of counting.
- `onlyPlayer`: `'external'` or `'embedded'` for a scenario of one player,
  with `onlyPlayerWhy`, the reason. Anything else is refused (a misspelt
  player used to make a scenario skip in both runs).
- `testOnly`: the steps of the scenario that go through a route a user of
  that player does not have, one entry per row and player: `{ row, player,
  via, route, proves, unproven }`. `via` names the route's helper, one of
  `TEST_ONLY_ROUTES` in `proof.mjs` (`clickUnderPlay`, `exitVelaFromPage`,
  `mpvInput`); each helper records that it was taken, and the runner fails a
  scenario that took a route it did not declare for the player it ran in, or
  declared one it did not take. The parity test prints every one on each
  run, and marks a row shown in a player only through such routes as ONLY VIA
  TEST-ONLY ROUTES: it is not shown the way a user of that player would.

Importing a scenario must do nothing: the runner imports all of them before
it runs any. The watch on that is there to catch a mistake: a scenario module
that acts on the machine when it is imported, or puts an act off past its
import, by any use of Node's API however it is spelled. The parity test
imports the scenarios in a process of its own (`import-watch.mjs`, run by
`import-check.mjs`) under three nets, which are in place before the loader or
any scenario is imported and are never taken down:

- Resources. Every async resource Node creates is recorded by its type
  (`node:async_hooks`), whatever JavaScript route made it: a timer
  (`Timeout`, referenced or not, from the globals, `node:timers`,
  `node:timers/promises` or its scheduler, `AbortSignal.timeout`, or a
  promisified `setTimeout`), an `Immediate`, a callback put off with
  `process.nextTick` (`TickObject`) or `queueMicrotask` (`Microtask`), a
  socket, a server, a file watcher, a DNS query, a child, a worker. Promises
  are the one type allowed: importing the real scenarios makes thousands of
  them and nothing else (measured), and a promise acts on nothing.
- Node's permission system, in audit mode with reading allowed
  (`--permission-audit --allow-fs-read=*`). Node checks inside itself, under
  any function a module can reach, every write, creation, removal or change
  of a file by path, every child process, worker thread, network use and DNS
  query, addon, WASI instance and inspector, and publishes each on a
  diagnostics channel the watch listens to. An attempt that fails, or sits
  inside a try/catch, is published like any other, because the check comes
  first. Audit mode lets the act go ahead, so an import does under the watch
  what it does in the runner, with three exceptions (seen on Node 26.13).
  Two kinds of call are refused with `ERR_ACCESS_DENIED` even in audit mode,
  with nothing published: `process.binding`, and the calls on an open
  descriptor that Node's documentation says the permission system cannot tie
  to a path (`fchmod`, `fchown`, `futimes`, `fsync`, `fdatasync`). Each has
  a recording replacement (below), so a module that tries one is seen
  trying, whether or not it goes on. And the `node:path` module object is
  frozen, so a module that assigns to it fails to import under the watch,
  which fails the test.
- Recording replacements for what neither of those sees: the `fs` calls on a
  descriptor that is already open (`write`, `writev`, `ftruncate` and the
  five above, and their `Sync` forms; Node's documentation says such calls
  bypass the permission system), signals (`process.kill`, `process._kill`,
  `process._debugProcess`), `process.chdir` and `process.umask`,
  `process.binding` and `process._linkedBinding`, and
  `Atomics.waitAsync`, which the engine settles with no resource at all. A
  replacement keeps the original's name, length, prototype chain and every
  own property (symbols too, `util.promisify.custom` and Node's own); a
  property that is a function is replaced by a recording one. No timer
  function is replaced: a replaced `setTimeout` once lost its
  `util.promisify.custom` and changed what an import did (review finding
  pa-4). The process's listeners (so a signal handler, by any route), the
  environment, the global object's names and the built-in prototypes are
  compared when the process ends, and the handles open just after the
  import are counted.

The process reports when it ends, the last moment there is, and only if its
event loop was seen with nothing left to do before that. So an act an import
puts off is seen when it runs, and so is what a module's own listener does
when the process is about to end, or puts off once more from there (a
listener a module adds for the end itself is seen as a listener added). The
report goes on a pipe of its own
(descriptor 3), as one line carrying a secret the test makes for each run and
hands the watch on its standard input; the watch keeps it in its own module
scope. The test takes that line and nothing else, and only from a process
that ended by itself with status 0: a module that prints a report, ends the
process early (with any status, or a signal), or takes the watch's
`beforeExit` or `exit` listener away fails, saying how. A process still
running at the deadline (60 s) is killed, with its process group, by SIGKILL,
so one that ignores SIGTERM or has stopped itself fails as surely as one that
keeps busy; that it has ended is looked at, not taken on trust. A process
that has ended by then while something it started, in a group of its own,
still holds its pipes fails too, a moment after the deadline: the test does
not wait for whatever holds them. And nothing an import starts in the
watch's own group outlives the check: the group is ended on every way out,
and an import that left a process running there fails, also when the watch
itself ended cleanly.
The test file imports the scenarios itself only once the report has cleared
them (`stopUnlessCleared`); otherwise it stops at once with the report, and
imports nothing.

The test proves each of these with a made-up module of its own, never among
the scenarios: every timer route named above, the reviewer's two (a
`scheduler.wait` taken from the scheduler's prototype, and
`setTimeout[util.promisify.custom]`), and that `util.promisify(setTimeout)`
still resolves under the watch; an act put off behind two file reads; a
signal handler added through `process.on` and through
`EventEmitter.prototype.on`; a DNS lookup (callback and promise) and a
resolver's query (both kinds); `fs.watch` and `fs.watchFile`; a socket, a
server, a UDP socket and a child made with `new`; a worker; a write that
fails inside a try/catch; a signal; a write to standard error by descriptor;
`Atomics.waitAsync`; that a replaced `fs.write` still promisifies to its two
fields; that a printed report with `process.exit(7)`, with
`process.exit(0)`, or with the `beforeExit` or the `exit` listeners removed,
a forged line on the report's pipe, `process.exitCode`, and a signal to
itself each fail; that what a module's own `beforeExit` or `exit` listener
does, or puts off, at the end of the process is seen; that a call to
`process.binding` is recorded; that a busy process, one that ignores SIGTERM
and one that stops itself each fail within two seconds of a one-second
deadline, have ended, and leave nothing of their group behind, as does one
that ended after starting a process of its own group that holds the pipes;
that a process left running in the watch's own group is ended and fails the
import; and, by
running the same arrangement in a process of its own, that a failed import
stops the file before it imports a busy scenario, and, from the test file's
own text, that its top clears the scenarios before it imports any. The other
routes are seen the same way and not each proved.

What is not seen: a callback the engine alone puts off (a
`FinalizationRegistry`, say) that has not run by the time the event loop is
empty; what a worker thread does once it is made (the permission system does
not pass to it; making it is seen); acts the list above does not name, such
as `process.setuid` or setting `process.title`; and writing through the
standard streams, which may or may not show as a resource and is not what
the watch is for. Nor is the watch a defence against a module written to
defeat the watch itself: one that reaches into the watch's process to forge
its secret, unsubscribes its channels or patches the engine under it. Such a
module could do the same damage from its `run()`, which nothing watches.

## Parity

Every row of the inventory must be shown by a scenario in both players, or
carry a recorded exemption in `parity.mjs` (`EXEMPTIONS`) for the player
where it is not. `tests/e2e-parity.test.mjs` (part of `npm run check`)
fails when the inventory is not the plan's (every row of the table in
`.agents/plans/embedded-player.md`, and nothing else: a row whole, or, for
the rows `SPLITS` in `parity.mjs` records as split, every one of the parts
it approves and no other part), when the list of row ids is not the one the
test itself spells out (so a row or a part cannot come or go without a
change to the test as well), when
a row is shown in one player and neither shown nor exempted in the other,
when a row has nothing at all, and when an exemption names a row or player
that does not exist or excuses a row that a scenario already shows (it is
then stale). It prints every exemption, every test-only route
and which scenarios show each row, every time it runs, so that none of
them is taken for more than it is.

An exemption is not coverage. It records why no scenario can show that row
in this venue without a product change (`reason`), and what holds it
instead: tests of other kinds (`heldBy`: a file and the names of its tests;
the parity test checks that the file exists and that each name occurs in it
as a whole word; it cannot tell a test from a comment) and/or the step
the owner takes by hand (`ownerStep`). Today: row 10 (step-down relaunch)
and row 11 (the unrequested-play notice, which only a step-down shows), both
in both players. `playerfallback`'s D3 fallback notice is a different
notice on the same delivery: it is named as evidence in row 11's external
exemption, and claims no row.

A scenario that runs in both players shows its rows in both, except where
`parity.mjs` records it as failing there on a product defect
(`KNOWN_FAILURES`: the scenario, the player, the exact text the failure
carries, and the defect with its evidence; `intermittent` for a race that
does not fail every run, such as `pagefail`'s). Such a scenario shows nothing in
that player; a row only it claims there is printed as NOT SHOWN, RED, and
not counted. Today: `embeddedownconfig` in the external player (rows 13b
and 14; 14 is shown there by other scenarios, 13b by none) and `pagefail`
in both (it claims no row).

The parity test reads declarations only: it cannot see that a scenario
fails in one player, or claims a row it never reaches. That is the parity
VERDICT's job, from two full runs of the same commit, one of each player.
Every run writes `tests/e2e/results/<player>.json` (each scenario's
pass/fail/skip, its error, the rows it reported with `shown()` and the
test-only routes it took; plus the commit read before the build, whether the
worktree was dirty then or changed after any scenario, whether the run was
full, and whether it built the app it ran). `npm run e2e:verdict`
(`parity-verdict.mjs`) must be run with that same commit checked out and
clean: it reads the scenarios, the known failures and the exemptions from
the checkout, so it refuses runs of any other commit, and a dirty checkout.
It also refuses a run made with `--skip-build`: such a run ran whatever
binary was built last, of any code, so its commit says nothing about the
app. `built: true` in a results file means that the run ran the build and
started, for every scenario, the executable Cargo's report of that build
named for the app (recorded as `appBinary`), the same file each time: its
device, inode, size and modification time are noted as soon as the build
has ended and checked before every session starts. It does not prove that
nothing replaced the file in the moment between Cargo writing it and that
note, or between a check and the app's start, nor that a change which kept
all four (the same size and modification time, written in place) did not
happen. A results file is read only as the runner writes one: exactly its
names at the top, each of its kind (and no two that the runner cannot write
together), and every scenario's result whole: a skip and nothing else for a
scenario that does not run in that player, otherwise a pass or a failure
with its error (none for a pass, text for a failure) and its two lists,
each entry once, a route only if it is a registered one, and for a pass a
proof the runner itself would have passed. Anything else is reported in its
own words and counts for nothing. The runner applies the same rules to the
file it has just written and fails the run if it breaks one, so the two
cannot drift apart unseen. What the rules cannot tell is a file made by hand
that is exactly like one of the runner's. It reads the two and fails when a scenario failed other than as its
recorded known failure, when a known failure passed (take it out) or failed
with other text, when a scenario did not run, when the runs are of
different or uncommitted states or not full, and when a row is shown in a
player by no scenario that passed there and reported the row. It prints, for each row
and player, the scenarios that did, and marks rows shown only through
test-only routes. What no check can see: a scenario that reaches
`shown()` after an assertion was weakened or deleted; that is review's.

The opt-in live scenarios (`live/`) declare their rows too, and are loaded
through the same loader, but count for neither player: they are part of
neither run.

## The embedded player

`--embedded` runs the suite with the embedded player instead of the external
one (`.agents/plans/embedded-player.md`). Two things change:

- Every seeded config gets `"player": "embedded"`. A scenario plays the way
  it always did, through mpv's IPC socket; the mpv on the other end is the
  library inside Vela's own process.
- The venue is a private headless Wayland compositor (`weston
  --backend=headless --fake-seat --renderer=gl --idle-time=0`) instead of
  Xvfb, because that player needs Wayland. Weston has no colour management
  and no fractional scaling, so nothing about HDR or scaled displays is
  exercised here. `--fake-seat` gives it a seat: without one weston never
  activates the app's window, so the page never has focus (`:focus` never
  matches) although WebDriver still types into it. `--idle-time=0`: input
  sent through WebDriver does not count as activity, and an idle weston
  never draws a window opened afterwards, so screenshots late in a long run
  hung.

It additionally needs `weston` and a built libmpv set: by default the one
`scripts/build-libmpv.sh --container` leaves in `.libmpv/out/libmpv`, or
wherever `VELA_LIBMPV_DIR` points.

A full `--embedded` run (no scenario named) first runs
`npm run test:libmpv`: Rust tests, ignored in an ordinary `cargo test`,
that load that set without a window and check what the player relies on
mpv for when it changes HDR targets (each ask answered with its own
request; a refused value answered as refused; the shutdown event; a
change of a target reported, whoever made it, and never with an older
value once Vela's latest ask for it is answered; a target set back by
another client in the same pass as Vela's ask reported and put back). No
scenario can: weston never makes a play change mpv's target. If they fail,
or cannot load the set, the run fails before any scenario. A run of named
scenarios skips them and says so; `npm run test:libmpv` runs them alone.

So there are two venues: an ordinary run (the external player, on a private
Xvfb display) and an `--embedded` run (the embedded player, on a private
weston). A scenario can belong to one player with `onlyPlayer: 'embedded'`
or `onlyPlayer: 'external'` (and `onlyPlayerWhy`). In a run of the other
player it is reported as `SKIP` and not counted; named explicitly, it fails
instead.
`embeddedplay` is the scenario that proves a play really is embedded;
`playerfallback` is about what happens where it cannot be;
`embeddedownconfig` (both players, despite its name) plays with a user's
mpv.conf that tries to override Vela's own options (it fails in the
external player on a recorded product defect; see Parity);
`mpvargs` plays with Advanced arguments and own configuration off;
`embeddedlatestart` stages an embedded play that does not start in time
and checks that the same play carries on in the external player while the
late one stays out of it.

### One scenario, both players

Most scenarios run in both players and prove the same thing in each. Where
what a user does or sees differs, `helpers.mjs` has the step for each
player's user (`runPlayer` says which player the run is in):

- `stopPlayAsUser(driver, mpv, socketPath, what, { key })` stops a play as
  its user does: mpv's `quit` in the external player; in the embedded one
  the view's Close control, or a key of the view (`q`, Escape). Then it
  waits until the player stops answering and the page is the user's again.
  `playAndQuit` stops its play this way.
- `waitForPageUncovered(driver, what)`: the embedded player's view covers
  the page until a moment after its play has ended. A click made before
  then fails ("element not interactable" or "element click intercepted").
  Wait for this before using the page after a play that was not stopped
  through `stopPlayAsUser` (one that ended by itself, for instance).
- `clickUnderPlay(driver, elementId, what)` is a TEST-ONLY route: it clicks
  a control of the page while a play runs, which only the external player's
  user can do. In the embedded player it checks that the view is up and
  clicks through the DOM, underneath the view. It proves what the backend
  and the page do with that state in the embedded player, never that a user
  could take the step; every use says so where it is made.

- `processEnded(pid)` and `scenarioAppPids(configRoot)`: whether a process
  has ended (gone, or a zombie), and the pids of this scenario's own Vela
  (found by its `XDG_CONFIG_HOME`). An external play's mpv is a process of
  its own; an embedded play's `pid` (mpv's property) is Vela's.

Rows 12a and 12b have scenarios of their own, in both players:
`replaceplay` (a new play stops the running one; in the embedded player the
second Play is pressed through `clickUnderPlay`) and `exitplay` (Vela's exit
stops the play; started in both players by the page invoking `exit_vela`,
the command of the recovery screens' Exit Vela button, because neither
venue can press a window's close button and WebDriver's "close window"
does not exit Tauri). `markerbutton` (external only) presses the skip
button mpv draws, through mpv's own input commands over IPC, on a real
video output (`--vo=x11` on Xvfb).

A scenario branches on the player only where the behaviour itself differs
(the user's stop; the page cannot be used while the view covers it; what
"fullscreen" is), and says why at the branch.

`MpvIpc.connect` reports a socket file that refuses every connection as a
play that has already ended (mpv leaves its socket file behind): the usual
reason is a stream that would not open, which ends a play at once, in
either player without a message on the page.

### The player view

The player view's controls have four scenarios of their own, all
embedded-only. Each drives the page for real (WebDriver clicks, and keys and
pointer moves through WebDriver's Actions, which arrive as trusted events)
and reads the outcome from mpv over its IPC socket:

- `embeddedcontrols`: the `player-state` events arrive; pause, the seek bar,
  the seek keys in both directions, mute, volume, fullscreen on and off
  (checked by the page's real height and mpv's `osd-dimensions`, not by
  Tauri's answer) and stop, through the controls and through the keys.
- `embeddedskip`: a button policy and an intro that ends past the end of
  the clip. The play is paused through the page, the page's Skip Intro
  button is pressed, and the marker must clear through the script while the
  play stays up. The script draws no button of its own there.
- `embeddedcrop`: Vela's own `vela-crop.lua` crops a letterboxed clip by
  itself in automatic mode, on a fresh and on a resumed play, and only on
  the Crop control in manual mode; `C` takes the crop away. With cropping
  Off the view offers no Crop control and nothing crops, `C` included. It is
  the embedded counterpart of `autocrop`, which covers the stock script of
  the external player in the same three modes and is external-only. Row 9
  is split by mode (9a Off, 9b Manual, 9c Auto), and each scenario reports
  each mode shown at the end of that mode's legs, so a mode that is not
  exercised, or whose leg does not get to its end, is not shown in that
  player. `autocrop`'s Manual leg presses Shift+C through mpv's input
  commands over IPC (`mpvInput`, declared in its `testOnly`), so row 9b is
  marked in the external player as shown only through a test-only route.
- `embeddedfullscreenend`: fullscreen asked for through the view's control,
  then each way a play can end: a playlist's clean end with the next play
  started by the backend keeps it across the gap; an end in an error (the
  mock server refuses the stream), mpv told to quit over its socket, a clean
  end with no successor, Close, and a playlist's clean end whose next play
  the backend cannot start (the mock resolves no stream) each leave it
  within 2.5 s of the view
  going, far below the page's 50 s backstop. It prints the lifecycle events
  the page received for each, in order.

The controls hide after three seconds of playing untouched, and a hidden
control takes no click: `showPlayerBar` in `helpers.mjs` moves the pointer
over the view first. `cropscript` (ordinary venue, system mpv, no app) holds
the crop script's own contract, as `markerpayload` does for the marker
script. The debug app runs the scripts copied into
`<target directory>/debug/mpv-scripts/` (`src-tauri/target/debug/` unless
moved, as for the binary above) at build time, so after a change to a
script the scenarios that drive the app need a run without `--skip-build`.
