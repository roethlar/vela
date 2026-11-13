# ec-9: every embedded play adds a registry the compositor keeps for the life of Vela

**Severity**: LOW — memory in the compositor grows by a small amount with every embedded play and is only given back when Vela exits.
**Status**: In progress
**Branch**: —
**Commit**: `2f65c44`

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/surface.rs:118 requests a new registry each time join runs, including every VideoSurface::create. The guest connection deliberately never disconnects. The installed Wayland protocol specifies that get_registry server resources survive destruction of the client proxy and are released only when the connection disconnects."

## Predicted observable failure

Reviewer, verbatim: "Repeated plays accumulate compositor-side registry resources for the lifetime of Vela, despite dropping each VideoSurface."

Reviewer's suggested direction, verbatim: "Reuse a registry and global bindings owned for the lifetime of the shared display, while keeping per-play surfaces and event handling separately owned."

## What

The video surface is created on the Wayland connection GTK holds, which Vela joins as a guest. Joining meant: make a guest connection object, ask the compositor for its registry (the list of globals), bind what is needed. That was done once to check the machine and again in `VideoSurface::create` for every play. The protocol has no request that gives a registry back, so the compositor keeps each one until the connection closes, and this connection is the window's and closes when Vela exits. Each play left one more.

## Approach

`surface::Guest` is Vela's place on the connection: the guest connection and the globals bound on it. `Host::check` joins once (`Guest::join`), where it used to probe and throw the result away, and keeps the guest with the loaded library in the answer it already caches for the run. A play is handed that guest; `VideoSurface::create` gives the play an event queue of its own and makes the surface from the shared globals, asking the registry nothing. Since the connection no longer goes away with the play, nothing can be left for a closing connection to sweep up: `VideoSurface`'s `Drop` destroys every object the play made, by name, and the test holds the list of them against the struct's fields. The queue the globals were bound on is kept and emptied at each play, so the compositor's later announcements (a monitor plugged in) do not pile up on it.

## Files changed

- `src-tauri/src/player/embedded/surface.rs` — `Guest`, `Globals` (split out of the per-play `State`), `Guest::join`, `Guest::drain`; `VideoSurface::create` takes the guest; `missing_protocol` and `release_globals` are gone.
- `src-tauri/src/player/embedded/host.rs` — `Ready { lib, guest }` is what the one-time check keeps; `prepare` hands the guest on.
- `src-tauri/src/player/embedded/session.rs` — `Start::guest`.
- `tests/embedded-player.test.mjs`, `tests/player-view.test.mjs` — one new wiring test; three pins follow the moved text.

## Guard proof

Measured, 2026-11-13, from the Wayland protocol log (`WAYLAND_DEBUG=client`) of the `embeddedplay` E2E scenario, which plays twice in one run (two `get_subsurface` requests in each log): before (`85b38ff`) five `wl_display.get_registry` requests; after (`2f65c44`) three, which are GTK's, Mesa's and Vela's one.

Injections, run at `2f65c44` in a disposable worktree; `node --test tests/embedded-player.test.mjs` (and `cargo +stable test --locked --lib player::` unchanged, 96 pass):

| Injection | Node |
| --- | --- |
| The original defect: a play asks for a registry of its own | FAIL `the window's Wayland connection is joined once for the whole run, never per play` |
| The availability check, and with it the join, is repeated for every play | FAIL |
| The check's answer is not kept | FAIL (and `an embedded play is offered only in a run that started in embedded window mode`) |
| A play leaves its subsurface on the shared connection | FAIL |
| A play leaves its colour feedback object | FAIL |
| A play leaves its viewport | FAIL |
| A play leaves its output query | FAIL |
| A play holds a new Wayland object that the list does not name | FAIL |
| The connection is joined twice | FAIL |
| Unchanged | PASS |

Also run: `embeddedplay`, `playback`, `resume` and `markers` under the headless compositor in the embedded player: PASS.

## Coder dispute (if any)

## Known gaps

- The guard is source text plus one measurement by hand. Nothing in CI counts requests: CI has no compositor, and in a run with one GTK cannot be told which registry is whose without the protocol log.
- The globals bound at the join are kept for the life of the process and never given back; that is one set, not one per play.
- Two plays that overlap for a moment (one replacing the other) now share one connection. Each has its own event queue; an event for one that the other's thread happens to read off the socket is delivered to its owner's queue, and is acted on at the owner's next wake-up, at most a tenth of a second later.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "Every embedded play leaks another server-side Wayland registry". Reviewer's severity: LOW.

### Verification round 1, recorded 2026-11-13 09:34 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d42e-9486-7652-9486-8e135400e6b4`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 09:13 UTC, finished 09:25 UTC.
- Reviewed head `2f65c44220b29192c34823e35d1540f7c0578993`, base
  `7aca86ba8364fe397a952813d4cccc74533ad268` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/player/embedded/surface.rs:402 — Concurrent dispatch on the shared backend can strand queue.roundtrip(): another play or Guest::drain can consume its sync.done between the completion check and prepare_read. A deterministic headless probe using the pinned library reproduced the wait until an unrelated event arrived; separate backends completed immediately. This can stall surface creation or HDR setup. Coordinate roundtrip completion with dispatch and waiting.
  - src-tauri/src/player/embedded/surface.rs:714 — Dropping a play with get_information replies outstanding now leaks its Rust event queue. The retained backend later enqueues the information object's events into the abandoned queue; QueueEvent → QueueProxyData → QueueHandle forms a strong-reference cycle. A headless probe with WpImageDescriptionInfoV1 confirmed retention after done and backend destruction; the former per-play backend released it. Retire outstanding information objects and drain their queues before discarding the play.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. Both comments are correct, and both come from the same choice in
the fix: every play shared one connection object, and with it one
libwayland event queue. Then one thread can dispatch what another is
waiting for, and a play that ends leaves its part of the shared object
behind. The finding only asked for one registry per process; it did not
need the plays to share a queue. The repair keeps the registry and the
globals bound once, and gives each play a connection object and queue of
its own again, on which it uses those same globals.
