# hd-3: the drawing thread waits for mpv each time it sets the HDR target

**Severity**: MEDIUM — the picture freezes for a quarter to half a second, and frames are dropped, each time the display changes during a play. (First recorded as "about a second"; measured since: 200 to 243 ms with the shipped library, about 399 ms in the reviewer's rig.)
**Status**: Verified
**Branch**: —
**Commit**: `0e7adca`

## Evidence

Reviewer, verbatim: "libmpv.rs:423 calls synchronous `mpv_set_property_string` through `follow_colour` in session.rs:356-361, on the thread responsible for rendering. `/usr/include/mpv/render.h` explicitly warns against this dependency. A headless software-renderer test of this setter with a pending frame blocked for approximately 399 ms."

## Predicted observable failure

Reviewer, verbatim: "A brightness change, monitor move or HDR toggle can stall rendering while mpv waits for that same thread to render, causing dropped frames and visible playback freezes until mpv's timeout releases the wait."

Reviewer's suggested direction, verbatim: "Use asynchronous property updates or a separate control thread while continuing to service rendering. Gate presentation until the updates complete so the frame and surface description still change together."

## What

A play has one thread that owns its video surface and its mpv, and mpv draws on that thread. When the display changed (HDR switched, the brightness moved, another monitor), that thread told mpv its new target with `mpv_set_property_string`, three times. That call waits for mpv's core. mpv's rule for the thread it draws on (render.h) forbids such a call: the core may at that moment be waiting for this very thread to draw, and then each side waits out one of mpv's own time limits.

Measured with the library Vela ships (mpv 0.42), a frame pending: one change of target stalled the thread for 200 to 243 ms, mpv dropped three or four frames and logged that it was "not being called". The reviewer measured about 399 ms for one setter in their own rig. So the freeze is a quarter to half a second per change, not the "about a second" the severity line first said (corrected above).

## Approach

The design, with what was measured before any code was written, is `.agents/review/hd-3-hd-2.design.md`. In short:

- **Ask without waiting.** `Targetable::ask` sends `mpv_set_property_async` and returns; mpv's answer arrives later as an event carrying the ask's number. `mpv_set_property_string` is gone from the function table. `Player::next_event` returns an enum (`None`, `Shutdown`, `Answer`, `Other`) and never waits; the one waiting read, `wait_for_event`, is used only at the start of a play, before mpv has anything to draw with.
- **A change of mpv's targets is a transition** (`hdr.rs`). The surface is tagged or untagged first, as before (that is pending until a frame is presented); then the three asks go out, each numbered with its transition and its option. The transition finishes when all three answers have been read (`hdr::answered`). A refused ask, or a surface that cannot be tagged, takes both sides to standard range through a second transition. A newer change replaces one in flight; answers to the old one are ignored. What is "in effect" is the goal from the moment a change begins.
- **Presentation is held while mpv's side is in flight** (`Tagging::frame`, the one place a frame's fate is decided). Every frame mpv hands over is still drawn and reported to mpv, but not presented. Reporting matters: without it mpv waits 200 ms after every frame and the freeze would be as long as before (measured). Not presenting matters: between mpv applying the first option and answering the last, it draws with some of the three (measured: a frame about 35% darker), and presenting would also commit the surface's new description early. The decision is taken after this turn's answers have been read and before anything is drawn.
- **One limit, one second, for the whole unbroken stretch** of changes; a replacing change does not restart it. Past it something is wrong: black is presented, once, and one line is logged. A single change is answered within about a frame (84 ms at most at 24 frames a second); a brightness slider dragged for ten seconds, one change per frame, held for up to 667 ms.
- **mpv's queue for Vela full** (a fault: a thousand unread events). An HDR change that cannot be sent counts as refused. A way back to standard range that cannot be sent waits, held, and is asked again once the queue has been read empty (`hdr::drained`): mpv takes no ask until then, and asks made before it filled still land. This departs from the design, which finished such a change on the spot; an attacking agent showed that left mpv drawing for HDR under an untagged surface.
- **The start of a play** collects mpv's answers in the same loop that waits for the compositor, for at most a second; a change still in flight then carries on under the hold.
- **Around it:** `set_wakeup` is called before the renderer exists. The embedded launch now sets `force-window=no` after the user's options, so that no option opens mpv's video output before the file is handed over (the hand-over, `loadfile`, is still a waiting call; see Known gaps).
- **A test against the real library** (`npm run test:libmpv`, five tests, ignored in an ordinary `cargo test` because CI has no mpv library): the layout and numbers of mpv's events, the address-of-a-pointer rule for a string value, a refused value, the option names and values `hdr.rs` really sends, the full queue, shutdown. `tests/e2e/run.mjs` runs them first in every full embedded run.

How it was made, for whoever reviews it: written by one agent from the design, then attacked by three others from different sides (event ordering; guards that do not bite; mpv's API and the wiring). They raised fifteen findings, all closed in two rounds of repair; one of the first repairs was itself found wrong by the confirming agent and redone. The coordinator then read the product code, ran the full check, and injected its own regressions (the first seven rows below).

## Files changed

- `src-tauri/src/player/embedded/hdr.rs` — `Transition`, `Goal`, `Frame`, `HOLD_LIMIT`, `answered`, `drained`, `Tagging::{begin, finish, refused, frame, changing, waiting_for_room}`; `carry_out` now begins a change instead of making it; `Change::needs_a_frame` is gone (a finish owes a frame). The test rig is new: a stand-in mpv that applies each ask when the test chooses, a queue that can fill, and a search over every order of a script of events. Five tests against the real library.
- `src-tauri/src/player/embedded/libmpv.rs` — `mpv_set_property_async` in place of `mpv_set_property_string`; `Event`, `classify`, `next_event`, `wait_for_event`.
- `src-tauri/src/player/embedded/session.rs` — `follow_colour`, `say`, `set_up`'s colour loop, `Live::run`'s event loop and its five ways of dealing with a frame, `Live::load`'s comment.
- `src-tauri/src/player/launch.rs` — `embedded_required_args` gains `force-window=no`; the recorded launches follow. `src-tauri/src/playback.rs` — one assertion follows.
- `tests/embedded-player.test.mjs`, `tests/mpv-lifecycle.test.mjs` — the wiring pins, rewritten for the new shape and extended.
- `tests/e2e/run.mjs`, `tests/e2e/README.md`, `package.json` — the real-library tests and where they run.

## Guard proof

Run 2026-11-13 by the coordinator in a disposable worktree at the fix's head; `cargo +stable test --locked --lib embedded` (or `launch`), `node --test tests/embedded-player.test.mjs tests/mpv-lifecycle.test.mjs`, and `npm run test:libmpv` against the shipped library.

| Injection | Rust (stand-ins) | Node | Real library |
| --- | --- | --- | --- |
| Unchanged | PASS (103) | PASS | PASS (5) |
| A frame is presented while mpv's targets are changing (`Tagging::frame` gives `Show` for `Hold`) | FAIL, 11 tests, among them `every_presented_frame_agrees_with_its_surface_in_every_order`, `a_frame_drawn_while_mpv_has_applied_some_of_the_three_is_never_presented` | | |
| A held frame is not reported to mpv (the 200 ms wait returns) | | FAIL `the running play never waits for mpv and does with each frame what hdr decides` | |
| Standard range without untagging the surface | FAIL `hdr_switched_off_while_mpv_is_still_changing_to_it_is_switched_off` and two more | | |
| The run loop drops mpv's answers | | FAIL, two tests | |
| `force-window` left to the user | FAIL, the three recorded launches | FAIL `the embedded player's required options follow everything the user controls` | |
| The `char *` itself is handed to mpv instead of its address | PASS (a stand-in cannot see it) | | FAIL: the test process dies |

The agent that wrote the fix ran 37 injections, each failing its named test; an independent agent re-ran the ones belonging to the fifteen findings. Among them, each alone: a change finishes on its first answer; answers to a replaced change are counted; every change has the same number; an answer is counted twice or for the wrong option; a refusal is not remembered; mpv is asked for HDR before the surface is tagged, or although it could not be; a refusal leaves the surface tagged; "in effect" is recorded at the finish; a finished change owes no frame; a frame owed by one change is presented while the next is in flight; the hold never runs out, restarts with a replacing change, shows mpv's frame instead of black, or shows black only the first time; the limit set to 10 ms and to 600 ms; a play counts as settled while mpv is changing; the way back that cannot be sent is finished on the spot, or retried before the queue is read empty (fails against the real library too); the waiting setter is put back, or a waiting call is added to `ask`; `next_event` waits; the run loop uses the waiting read, gains a catch-all arm, decides the frame before reading events, or draws after presenting; `set_up` swallows a shutdown or does not collect answers; `set_wakeup` is called after the renderer exists; the answer event's number or its error is misread (fails against the real library); an option name or value `hdr.rs` sends is misspelt (fails against the real library only); `run.mjs` no longer reaches the real-library tests.

Also run by the coordinator at the fix's head: the full check (JS, Rust 1.89, stable and 1.100 clippy, 494 Rust tests); the whole embedded E2E suite, which now begins with the real-library tests; and a play of an HDR clip on `continuum`'s desktop (KDE, HDR on). Results are in the plan's log.

## Coder dispute (if any)

## Known gaps

- **Handing mpv its file is still a waiting call** on the play's thread (`mpv_command`, `loadfile`), after the renderer exists. It cannot stall while mpv has no video output, and Vela's launch now keeps every OPTION from opening one early. A conditional profile in the user's own mpv.conf (measured: it turned `force-window` back on after start-up) or a command on mpv's IPC socket still can. Then the call may wait for this thread for up to one of mpv's limits (about 200 ms); not measured in Vela. Making the hand-over asynchronous would change how a refused file is reported at the start of a play, and was left alone.
- **A long drag of the brightness slider.** Changes that replace each other are held as one stretch. The longest measured was 667 ms; while it lasts fewer frames are presented, and a stretch over a second shows black. How many descriptions KWin sends during a real drag is not known. This is in the owner's checks for Slice 4.
- **On a real HDR desktop only the change at the start of a play was seen.** A change in mid-play needs the display to change. The headless test venue has no colour management, so no end-to-end scenario reaches a transition; the order of the run loop is held by source text and by the stand-ins, and mpv's side by the real-library tests.
- **A resize during a hold** shows the old picture at the new size until the change finishes (at most the limit).
- **The full queue** was exercised against the real library without a renderer, never in a running play.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `74f44b6..e8db400`, 2026-11-13 08:38 UTC
(`.agents/review/codereview-2026-11-13-embedded-hdr.md`). Reviewer's
title: "Avoid synchronous property writes on the render thread". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 00:06 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d758-167a-7331-9a7d-0304001387ef`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 23:57 UTC, finished 00:03 UTC.
- Reviewed head `0e7adca54df16297a4de6953930b8d43ace197cd`, base
  `dc864f34e5b91163cb77ce60208af2d95589c00b` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. Comments, verbatim:
  - src-tauri/src/player/embedded/hdr.rs:1653 — Independently confirmed guard failures for synchronous calls, premature presentation, omitted frame reports and omitted force-window protection; all passed after restoration.
  - src-tauri/src/player/embedded/session.rs:544 — Accept the documented synchronous loadfile gap for hd-3: conditional profiles or IPC can still cause a startup stall, but recurring HDR updates no longer wait for mpv.
  - tests/e2e/run.mjs:250 — Verification passed: 103 embedded, 35 launch, 20 Node and all five tests against the supplied shipped library. Embedded runtime and E2E were inspected but not run because no display, compositor or GPU is available.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
