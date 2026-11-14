# pa-4: a timer started through Node's timer modules is not seen by the watch on importing a scenario

**Severity**: MEDIUM — the parity test says that importing a scenario does nothing, and that timers are among what it watches. It watches the global timer functions only. A scenario that imports `setTimeout` from `node:timers`, or awaits a delay from `node:timers/promises`, at import passes; a callback can then run later, in the runner or during another scenario.
**Status**: In progress
**Branch**: —
**Commit**: `c294a09`, `be3f1f4` (repair)

## Evidence

Reviewer, verbatim: "tests/e2e-parity.test.mjs:77 patches global timer functions only. A temporary scenario importing setTimeout from node:timers and scheduling an unreferenced timer passed all 13 tests; an awaited node:timers/promises delay also passed. The resource-count check at :94 does not expose the unreferenced timer."

## Predicted observable failure

Reviewer, verbatim: "Importing a scenario can schedule callbacks that later affect the runner or another scenario while the guard reports no import side effects. This contradicts the advertised timer coverage."

Reviewer's suggested direction, verbatim: "Instrument the timer-module APIs as well as globals, or track timer creation through an appropriate runtime hook. Add negative tests for both promise-based timers and unreferenced timers."

## What

The defect is as the reviewer describes, and the helpers who attacked the
first fix found it went further.

- The reviewer's: the watch replaced only the global `setTimeout`,
  `setInterval`, `setImmediate` and `queueMicrotask`. A timer started at
  import through `node:timers` or `node:timers/promises` went unseen, and
  an unreferenced timer holds no handle that the count of handles would
  show.
- Found in the attack: the watch was taken down as soon as the import
  returned. `smoke.mjs`, given an `Atomics.waitAsync` whose settling wrote
  a file, still passed "importing a scenario has no side effects", and the
  file was written. `Atomics.waitAsync` itself was not watched.
- Found in the attack: the README said signal handlers were seen, but one
  added through `EventEmitter.prototype.on` called on the process was not.
  DNS queries, `fs.watchFile` without a handle, and sockets, servers and
  children made with `new` were not watched; some were seen only when
  Node's own code happened to call the watched `process.nextTick`.

## Approach

The scenarios are imported in a process of their own, a new file
`tests/e2e/import-watch.mjs`, which the parity test runs as `node
import-watch.mjs <dir> <prefix> ...`. It imports only Node's own modules
statically, puts the watch in place, and only then imports the loader and
the scenarios. The watch never comes down. The process reports, as one line
of JSON, only on `beforeExit`, when it has nothing left to do, so an act an
import puts off runs under the watch. A process that never gets there is
stopped by the test's timeout and fails with "the import never let its
process end". A child process was chosen over waiting a few turns of the
event loop because an empty event loop is a state and a number of turns is
a guess.

Newly watched: `setTimeout`, `setInterval` and `setImmediate` of
`node:timers` and of `node:timers/promises`, its `scheduler.wait` and
`scheduler.yield`, `AbortSignal.timeout`, `process.nextTick`,
`Atomics.waitAsync`; `connect` of `net.Socket`, `listen` of `net.Server`,
`bind`, `connect` and `send` of `dgram.Socket`, `spawn` of `ChildProcess`;
every DNS query of `node:dns`, its `promises` and both `Resolver`s;
`fs.watch`, `fs.watchFile` and `fs.promises.watch`. The process's listeners
are compared event by event between before the import and the end, so a
signal handler is seen whatever route added it.

The real-import test also requires that the child loaded exactly the
scenarios the test then uses, so an empty import cannot pass.

Made by a helper agent, attacked by a second, repaired by a third,
confirmed by a fourth, and read and landed as one commit by the
coordinating agent, which changed one thing: the probes that open a socket,
ask DNS or watch a file now take a refusal as well as a success. The
reviewer's sandbox answers EPERM to a listen on this machine's own address;
the probes as written crashed there before reporting (seen: `listen EPERM`
as an unhandled error), so the whole test would have failed there for a
reason that is not the watch's. With the change the parity test passes
inside `codex sandbox` in workspace-write mode.

## Files changed

- `tests/e2e/import-watch.mjs` (new)
- `tests/e2e-parity.test.mjs`
- `tests/e2e/README.md`

## Guard proof

In `tests/e2e-parity.test.mjs`, each probe a made-up scenario module
written under a temporary directory, never among the scenarios:

- "the import watch sees a timer started by any route, and what an import
  puts off": 19 timer and put-off routes, `Atomics.waitAsync`, and a file
  written behind two chained reads.
- "the import watch sees a signal handler, a DNS query, a file watcher, and
  a socket or child made with new": 12 probes.
- "an import that keeps its process busy for good fails".
- "importing a scenario has no side effects" (the real import), now also
  requiring no error and the exact list of loaded scenarios.

Red-proofs, each restored afterwards:

- First fix: each timer watch removed in turn (`node:timers`,
  `timers/promises`, `scheduler.wait`, `scheduler.yield`, `AbortSignal`,
  `nextTick`, the globals, `syncBuiltinESMExports`): its probe failed.
- Repair: the watch made to stop recording once the import returns:
  `deferredwrite: [] should include fs.writeFileSync()`. `Atomics.waitAsync`
  unwatched: its probe failed. The attacker's change to `smoke.mjs`
  (`Atomics.waitAsync`, then a write): the real-import test failed. The
  child made to load nothing: the real-import test failed on the loaded
  names. The timeout branch removed: the busy test failed. The listener
  comparison removed: `prototypelistener` failed. Each prototype watch and
  each DNS watch removed in turn: its probe failed.
- The coordinating agent, after its own change to the probes: the watch on
  `net.Server.listen`, on `dgram.Socket.bind`, on `dns.promises`, and on
  `fs.watch` removed in turn: `servernew`, `udpnew`, `dnspromises` and
  `fswatch` each failed with "should include ...", and all passed once
  restored.

`npm run check` was green at this commit (417 tests).

## Coder dispute (if any)

## Known gaps

- Only the listed routes are watched. A native addon, or a module of Node's
  that is not named, is not seen; the README says so. TLS and HTTP/2
  sockets normally go through the two watched prototype methods, but no
  probe shows it.
- Of the watched names, one route of each new kind is proved (the README
  lists which); `dgram` `connect` and `send`, `fs.promises.watch` and the
  DNS queries other than `lookup` and `resolve4` are watched the same way
  and not each proved.
- Handles are counted just after the import returns, not at the end, where
  the event loop is empty by definition.
- The probes really act, on this machine only: a lookup of `localhost`, a
  resolver pointed at 127.0.0.1 and cancelled at once, a connect to
  127.0.0.1:9 destroyed at once, a listen and a bind on port 0 closed at
  once, a spawn of a file that does not exist.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "The import watch misses timers from Node's timer modules". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 10:21 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d988-cd2a-7972-9a62-de08aa31933b`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 10:09 UTC, finished 10:19 UTC.
- Reviewed head `c294a09eaf95e068d863971074d9c8147984e529`, base
  `ddbb7433d915b3ad2299103911a7992e32738528` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/import-watch.mjs:143 — Only the scheduler instance is patched. A temporary scenario calling Object.getPrototypeOf(scheduler).wait.call(scheduler, 500, { ref: false }).then(writeFile) returned a completely clean report. The same injection passed all 19 parity tests; importing it in a process kept alive demonstrated the deferred file write. The predicted timer failure remains, contradicting README lines 72–84.
  - tests/e2e/import-watch.mjs:42 — Wrappers discard function properties and change import behavior. A temporary scenario using setTimeout[promisify.custom]?.(300, undefined, { ref: false }).then(writeFile) reported no effects because wrapping erased promisify.custom. The normal import scheduled the timer and wrote the file. Preserve and instrument these callable properties instead of silently disabling the operation being checked.
  - tests/e2e-parity.test.mjs:49 — Scenario stdout is trusted as the report, and nonzero exit status is ignored. I injected a scenario that wrote a file, printed a fabricated clean report with the expected names, and called process.exit(7): all 19 tests passed. Removing the beforeExit listener and printing that report also bypassed the helper. Require a trustworthy completion protocol and successful child termination.
  - tests/e2e-parity.test.mjs:44 — spawnSync's default SIGTERM does not enforce a deadline when the child ignores SIGTERM or stops itself with SIGSTOP. Both probes remained blocked beyond a 200 ms timeout and required an external SIGKILL after 1.8 seconds. The advertised bounded failure can hang indefinitely.
  - tests/e2e-parity.test.mjs:58 — Scenarios are imported again in the test process even when the watched import times out. Injecting a referenced setInterval into smoke.mjs, with the child deadline shortened to 200 ms, failed the import assertion but kept the suite running until externally killed after eight seconds. Avoid importing a rejected scenario into the parent. Guard mutations separately failed for missing timer, promise-timer, and deferred-write recording; after restoration, all 19 tests passed.

Orchestrator: a valid verdict, reopened on five comments, and all five
are right. The first fix replaced a list of functions by recording ones.
That is a list of routes, and there is always another route; and a
replaced function is not the same function.

### Repair (round 2), `be3f1f4`

The watch no longer keeps a list of functions. It looks where Node itself
sees an act (`tests/e2e/import-watch.mjs`, run by the new
`tests/e2e/import-check.mjs` as `node --permission-audit
--allow-fs-read=* import-watch.mjs <dir> <prefix> ...`):

1. Every async resource Node creates, from before the loader is imported
   until the report, is recorded by its type (`node:async_hooks`). Only
   `PROMISE` is allowed: importing the 57 real scenarios makes about 4,000
   promises and nothing else. A timer is a `Timeout` whatever route started
   it, an unreferenced one too.
2. Node's permission system runs in audit mode with reading allowed, and
   the watch listens to its diagnostics channels: a write by path, a child
   process, a worker, the network and DNS, an addon, WASI are each
   published by Node itself, an attempt inside a try/catch included. Audit
   mode was chosen over the enforcing mode because enforcing changes what
   an import does (a refused act throws), which is the fault of comment 2
   in another form. The cost: a mistaken act really happens, once, inside
   the test.
3. The few acts neither net sees keep a recording replacement
   (`tests/e2e/import-watch-record.mjs`): calls on a descriptor already
   open, `process.kill` and its kin, `chdir`, `umask`,
   `Atomics.waitAsync`. A replacement keeps the original's name, length,
   prototype chain and every own property, symbols included, and replaces a
   function-valued property by a recording one. No timer function is
   replaced. The process's listeners, the environment, the global object's
   names and the built-in prototypes are compared at the end, as before.

The report: the test makes a secret for each run and hands it over on
standard input; the watch reads it before importing anything of the suite
and keeps it in its own module scope. The report is one line on a pipe of
its own (descriptor 3), carrying the secret. The test takes it only if it
is the one line there, carries the secret, and the process then ended by
itself with status 0.

The deadline: the watch runs in a process group of its own, and at the
deadline the group is killed with SIGKILL. A process that had ended by
then while something it started in a group of its own still held its pipes
is failed a moment later.

The test file: `stopUnlessCleared(imported)` stands at its top, before any
`loadScenarios`, and throws with the watch's report unless it is clean.

Where each comment is held, all in `tests/e2e-parity.test.mjs`:

| Comment | Held by |
| --- | --- |
| 1, the scheduler's prototype | "the import watch sees the reviewer's timer routes, and changes nothing they do", module `prototypewait` |
| 2, properties lost | the same test: `promisifycustom`, `promisifyworks` (it must still resolve), `replacedkeepsproperties`; and "a recording replacement keeps everything the original function has" |
| 3, a forged report and a bad end | "a module that forges the report, ends the process, or takes the report away fails": printed report with `process.exit(7)` and with `process.exit(0)`, `beforeExit` listeners removed, a line without the secret and one with a made-up secret on descriptor 3, `process.exitCode`, a signal to itself |
| 4, the deadline | "an import that keeps its process busy for good, or stops it, fails at the deadline": busy, ignoring SIGTERM, stopped; each within two seconds of a one-second deadline |
| 5, the parent's import | "the scenarios are imported here only on a clean report of the watched import", "a watched import that fails stops the file before it imports any scenario", and "this file imports the scenarios only after the watched import has cleared them" |

How it was made: a helper agent (Claude Opus 5.5) wrote the repair on a
local branch in two commits, the second adding one probe after its own
red-proof had shown the first probe of a forged report to pass for the
wrong reason. The helper that was to attack the repair did not run: the
model's safety filter stopped it at its brief, which asked it to get acts
past the watch. Nothing was done to get around that. The coordinating
agent read the three source files, and added two things before landing it
as one commit: the bound for a process that holds the pipes after the
watch's process has ended (with its test), and the test of the test file's
own top. So this repair reaches the reviewer with its author's proofs and
the coordinating agent's, and without a helper's attack.

Earlier probes: all are kept. Their expected texts changed to what the net
that now sees the act reports (for instance `timers.setTimeout()` became
`resource Timeout`, `fs.writeFileSync()` became `fs FileSystemWrite ...`,
`net.Server.listen()` became `net Net`). No probe was dropped and no
assertion loosened; the real-import test gained two (no permission check,
no resource other than promises).

Red-proofs. The helper's, each restored from the commit afterwards:
`Timeout` allowed (the timer probes failed); an old-style wrapper around
`setTimeout` (`promisifycustom` failed); a wrapper whose promisified form
resolves wrongly (`promisifyworks` failed); symbol keys skipped in the
replacement (`replacedkeepsproperties` and the unit test failed); the
report taken from standard output (`exitzero` was believed, and the test
failed); the status ignored (`exitseven`); the secret ignored
(`forgedsecret`); SIGTERM in place of SIGKILL (`ignoresterm` took 60 s, and
`stopsitself` hung until killed from outside); `stopUnlessCleared` made to
return (two tests failed); `importRefusal` made to clear everything (six
tests failed); nothing allowed, and the error stream not made first (the
real import failed). The coordinating agent's, the file put back and
checked byte for byte after each: the bound after the deadline taken out
(the pipes test failed); `stopUnlessCleared` moved below the first
`loadScenarios`, and taken out (the test of the file's top failed each
time).

Checks: `npm run check` green before the commit (431 tests). The whole of
`tests/e2e-parity.test.mjs` passes inside `codex sandbox` in
workspace-write mode, where a listen on this machine's own address answers
EPERM (26 of 26 with `--test-isolation=none`).

Not covered:

- A callback only the engine puts off (a `FinalizationRegistry`) that has
  not run when the event loop is empty; what a worker does once made;
  process acts the list does not name (`process.setuid`, `process.title`).
- Under audit mode `fchmod`, `fchown`, `futimes`, `fsync`, `fdatasync` and
  `process.binding` throw under the watch and not in the runner: there the
  watch does change what an import does. No scenario uses them.
- The watch is no defence against a module written to defeat the watch
  itself (one that forges the secret from inside the process, takes the
  watch's listeners off the channels, or patches the engine). The README
  says so: such a module could do the same damage from its `run()`, which
  nothing watches.
- Only Node 26.13.1 was run; 26.12.1, the floor of the supported range, is
  covered by its documentation.
- The probes really act on this machine: temporary files, a lookup of
  `localhost`, sockets on 127.0.0.1 closed at once, a spawn of a file that
  does not exist, a worker, and one process that lives eight seconds.
