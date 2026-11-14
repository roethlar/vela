# pa-4: a timer started through Node's timer modules is not seen by the watch on importing a scenario

**Severity**: MEDIUM — the parity test says that importing a scenario does nothing, and that timers are among what it watches. It watches the global timer functions only. A scenario that imports `setTimeout` from `node:timers`, or awaits a delay from `node:timers/promises`, at import passes; a callback can then run later, in the runner or during another scenario.
**Status**: In progress
**Branch**: —
**Commit**: `c294a09`

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
