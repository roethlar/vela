# Codereview 2026-11-17 — overlay mode (the embedded player under X11)

**Withdrawn, 2026-11-17: the code this hunt reviewed is removed.** Overlay
mode existed only to give X11 the embedded player, and X11 is not supported
in any way (`.agents/repo-guidance.md`); building it was the working agent's
error. ov-3 and ov-5 were not repaired, and the repairs of ov-1, ov-2 and
ov-6 went with the code. ov-4 (a test) stands.

Outcome: a codereview codex hunt over `39b400f..dea0793` (overlay mode,
`671cba9`; `embeddedlatestart`'s notice, `ea93dff`; `MpvIpc.close`,
`dea0793`): **findings**, five candidates, all admitted (one HIGH, four
MEDIUM). One more, ov-6, found by the working agent while verifying ov-1.

## Dispatch record

- Why: the range lands product code the working agent wrote (overlay mode:
  `overlay.rs`, the start-up decision, the session's overlay play) and the
  harness code of two E2E repairs.
- Reviewer: codex / host default / host default / standard, called plainly
  as in `codereview-2026-11-16-platforms.md`, in a disposable worktree at
  the head SHA (`~/dev/vela-agent-keep/wt/rv-final-fix15`); schema-valid,
  `capability_ok` true, both pins echoed. Brief: the scratchpad's
  `review-final/add-fixes15.py` slice `fix15`.

## Findings

| ID   | Severity | Finding (one line)                                                      | Status | Commit |
|------|----------|--------------------------------------------------------------------------|--------|--------|
| ov-1 | HIGH     | An overlay start waits for GTK's thread while holding the player slot, which GTK's thread can be waiting for | `[x]` | `4b91540` |
| ov-2 | MEDIUM   | Exit waits on GTK's thread for an overlay play whose teardown needs GTK's thread | `[x]` | `d41a926` (since removed) |
| ov-3 | MEDIUM   | A profile can change `video-timing-offset` after the overlay play starts, and frames are then shown early | `[-]` | withdrawn: code removed |
| ov-4 | MEDIUM   | `embeddedlatestart` leaves a log request unhandled when its player goes away | `[x]` | `5c4e188` |
| ov-5 | MEDIUM   | The overlay guard passes with the renderer never freed, or freed without its context current | `[-]` | withdrawn: code removed |
| ov-6 | MEDIUM   | A play replacing an overlay play can be refused because the old one is still drawing | `[x]` | `9303cee` |

### ov-1

Reviewer, verbatim, evidence: "src-tauri/src/player/embedded/session.rs:709
calls Target::begin, which synchronously dispatches to GTK at
overlay.rs:433. This runs beneath playback.rs:888–894 while current_child
is locked. A pending player_set_rect command takes that same lock on GTK at
commands.rs:6560, preventing GTK from servicing begin. A headless
rendezvous probe through playback::play reproduced the timeout." Predicted
failure: "A rectangle update overlapping an overlay start freezes the
entire UI until the ten-second dispatch timeout, then triggers external
fallback and disables embedded starts for the remainder of the run."

Verified by the working agent: the embedded start runs `Session::start`
under `player_slot` (`playback.rs:888`); an overlay play's set-up waits for
GTK's thread to make its renderer (`Target::begin`, `on_window_thread`,
10 s). `player_set_rect` is a synchronous command, run on GTK's thread, and
takes `current_child` (`commands.rs:6560`); so do `claim_player_fullscreen`
(through a synchronous command), `place_running_play` (posted to GTK's
thread) and the exit handler (`lib.rs:524`). `Host::prepare`'s own comment
states the rule this broke: everything that asks the window is done before
the caller locks the player slot. Admitted.

Repaired: the overlay asks GTK's thread only through a queue of its own
(`JOBS`, `on_gtk`), which GTK's main loop runs, and which GTK's thread also
runs while it waits for the player slot (`lock_slot_on_window_thread`, used
by all three of Linux's takings of the slot on that thread). A unit test has
the slot's holder wait for work only its taker can do
(`the_slot_is_taken_once_its_holder_has_had_the_work_it_waits_for`); the
guard in `tests/embedded-player.test.mjs` holds the queue and the three
places.

### ov-2

Reviewer, verbatim, evidence: "src-tauri/src/player/embedded/session.rs:780
synchronously calls Target::end, which queues GTK work at overlay.rs:472.
When exit_vela/app.exit runs while an overlay play is drawing, lib.rs:528
drops its Session on GTK; Session::drop waits five seconds, followed by the
two-second leftover wait at lib.rs:552. The queued teardown cannot execute
during these waits." Predicted failure: "A healthy active overlay play is
treated as stuck during exit. Shutdown stalls for at least seven seconds,
with another two-second wait when RunEvent::Exit follows, placing exitplay
close to its ten-second deadline." Verified (`lib.rs:514-552`), and seen:
the first run of the embedded suite under X11 had `exitplay` say "Vela's
process ended 9025 ms after the exit was asked". Admitted.

Repaired: the exit handler, on GTK's thread, once it holds the slot and
before it drops any play, does what the plays have already asked of that
thread and frees what still draws (`end_drawing_before_exit`); each play's
teardown then finds its phase `Done` and asks nothing. `exitplay` under X11
afterwards: "Vela's process ended 41 ms after the exit was asked". The
guard holds the order in the exit handler.

### ov-3

Reviewer, verbatim, evidence: "src-tauri/src/player/embedded/session.rs:694
sets video-timing-offset only before initialization; OverlayLive::turn at
line 758 provides no runtime protection. A conditional mpv profile can
change it when a file loads. A display-free probe with the shipped mpv 0.42
confirmed the value changes from 0 to 0.5 despite load-scripts=no.
overlay.rs:301 continues rendering with BLOCK_FOR_TARGET_TIME disabled."
Predicted failure: "With such a profile in the supported personal mpv
configuration, video is presented before its target time and loses
audio/video synchronization." The macOS host, which also never blocks
while it draws, protects the option for the whole play
(`macos_timing.rs`). Admitted.

### ov-4

Reviewer, verbatim, evidence: "tests/e2e/scenarios/embeddedlatestart.mjs:69
starts requestLog('trace') but does not await it until after
getProp('pid'). If the late player disconnects before replying, the first
promise rejects while only the second is being awaited; the catch never
handles it." The working agent had met it in a run and repaired it in
`5c4e188`, after the hunt's range. Closed.

### ov-5

Reviewer, verbatim, evidence: "tests/embedded-player.test.mjs:1148–1161
checks calls to end_drawing but never checks its teardown behavior.
Replacing overlay.rs:330's drop(renderer) with mem::forget(renderer) still
passed all 35 tests. Removing the make_current call before destruction also
passed all 35." Admitted.

### ov-6

Found by the working agent while verifying ov-1: a play that replaces a
running play takes the old player out of the slot, asks it to stop without
waiting (`commands.rs:6081`), and starts at once. In overlay mode there is
one GL area and one renderer: the new play's `begin_drawing` refuses while
the old renderer exists ("another play is still drawing in Vela's
window"), so whenever the new play's begin reaches GTK's thread before the
old play's teardown, the new play falls back to mpv's own window and
Vela's window is off for the rest of the run. `replaceplay` passed under
X11 because the old play usually wins that race.

Repaired: `begin_drawing` ends whatever is still drawing (freeing its
renderer on GTK's thread with the context current, `end_drawing`) before it
begins; the replaced play's own teardown then finds its phase `Done` and
frees nothing. Only one embedded play is ever the current one, a replaced
one has been told to stop, and a play that will not end is kept out by
`session::unfinished_plays` before any start. The guard holds the call and
that nothing refuses.
