# ec-1: a window closed while a play is starting leaves that play a pointer to a surface that is gone

**Severity**: HIGH — a use of freed memory in Vela's own process, which can crash it. It needs the window to be closed in the short time between a play being asked for and its video surface being created.
**Status**: In progress
**Branch**: —
**Commit**: `fffc3bd`

## Evidence

Reviewer, verbatim: "host.rs:262-268 retains borrowed WindowHandles, consumed by session.rs:179-189 and surface.rs:208-211. The exit sweep in src-tauri/src/lib.rs:409 handles only ExitRequested/Exit. Pinned tauri-runtime-wry 2.11.4 delivers ExitRequested after destroying the last window. Closing after playback.rs:365 acquires the slot, but before the worker consumes the parent pointer, therefore destroys the parent before the sweep can acquire that lock."

## Predicted observable failure

Reviewer, verbatim: "The worker can pass a freed wl_surface pointer to ObjectId::from_ptr, which calls wl_proxy_get_class on it: a use-after-free capable of crashing Vela."

Reviewer's suggested direction, verbatim: "In embedded mode, intercept CloseRequested and prevent native destruction until all starting and running workers have released their window references. Coordinate completion asynchronously without blocking the window thread on the player-slot lock."

## What

To put its video surface under Vela's window, a play needs the window's own Wayland surface. `Host::prepare` asked the window's thread for the raw pointer and carried it away (in `WindowHandles`, declared safe to send between threads); the play's own thread turned it into an object in `VideoSurface::create`, after the caller had taken the player-slot lock, stopped any earlier player and started a thread. Nothing kept the window alive in between. GTK destroys the window's surface when the window is closed, and Tauri's exit sweep, which waits for a running play, runs only after that. A window closed in that gap left the starting play with a pointer to freed memory.

## Approach

The pointer no longer leaves the window's thread. `Host::prepare` now makes the play's surface there: the closure it runs on the window's thread fetches the window's surface and calls `VideoSurface::create` with it in the same breath, and what comes back is the finished video surface, which is Vela's own object. On that thread the window cannot be closed in the middle. After `create` nothing refers to the window's surface again; if the window goes away later the compositor simply unmaps the child. Two things make this hold rather than merely be true today: `WindowFacts`, the only thing that holds the pointer, contains a raw pointer and so cannot be sent to another thread, and a compile-time check next to it fails the build if anyone declares that it can; and `WindowHandles` is gone. Because the window's thread must not wait for the compositor, `create` no longer does (it used to wait one exchange to learn the surface's scale); `VideoSurface::settle` does that on the play's thread, first thing in `set_up`. This builds on `ec-9`: with the connection joined once, making a surface needs no exchange with the compositor at all.

## Files changed

- `src-tauri/src/player/embedded/host.rs` — `WindowFacts` (not sendable, with the compile-time check), `prepare` makes the surface on the window's thread, `Prepared` carries the surface; `whole`.
- `src-tauri/src/player/embedded/surface.rs` — `WlDisplay` (the one pointer that may travel), `Guest::display`, `VideoSurface::create` without the wait, `VideoSurface::settle`.
- `src-tauri/src/player/embedded/session.rs` — `WindowHandles` removed; `Session::start` and `set_up` are given the surface.
- `tests/embedded-player.test.mjs`, `tests/player-view.test.mjs` — one new wiring test; pins follow the moved text.

## Guard proof

Run 2026-11-13 at `fffc3bd` in a disposable worktree; `node --test tests/embedded-player.test.mjs` and `cargo +stable test --locked --lib player::`.

| Injection | Result |
| --- | --- |
| What holds the window's surface pointer is declared safe to send | does not COMPILE (`error[E0283]`, the check next to `WindowFacts`); Node FAIL `the window's own surface is used on the window's thread, at once, and never carried away` |
| The original defect written to get past the compiler: the pointer leaves the window's thread as a number and is used on the caller's thread | compiles, Rust tests pass; Node FAIL (the same test: the surface must be made inside the window-thread call) |
| Making the surface waits for the compositor, on the window's thread | Node FAIL |
| The play's thread never learns the surface's scale | Node FAIL |
| The window's surface is referred to a third time | Node FAIL |
| A new type is declared safe to send in the window's module | Node FAIL |
| The compile-time check is pointed at something else | Node FAIL |
| Unchanged | PASS |

Also run: six E2E scenarios in the embedded player under the headless compositor (`embeddedplay`, `playback`, `resume`, `markers`, `markerpayload`, `playverbs`): PASS. On `continuum`'s desktop (KDE, 1.7 scale, HDR): the picture is 2176x1280 for the 1280x753 page as before, so the scale is still learnt; HDR target 1137; no dropped frames.

## Coder dispute (if any)

## Known gaps

- Closing the window during a start cannot be staged in a test: the gap was a few milliseconds wide and the E2E harness has no way to close a native window at a chosen instant. The guard is that the pointer cannot be in another thread's hands at all (the compiler) and that it is used where it is fetched (source text).
- The connection pointer (`WlDisplay`) still travels and is used from the play's thread. It is valid until the process exits. A play still running when the process exits is the subject of `ec-2`.
- A surface is now made for a play before the caller knows whether the play will start (for example Vela may be shutting down). It is then dropped, which destroys it; nothing is shown in between, since a surface without a buffer is not on screen.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "Native window close can invalidate a starting player's Wayland pointer". Reviewer's severity: HIGH.
