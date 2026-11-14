# hd-3 and hd-2: the design the two fixes follow

Written 2026-11-13, before any code, for review findings `hd-3` and `hd-2`
(`.agents/review/findings/`), both raised by
`.agents/review/codereview-2026-11-13-embedded-hdr.md`. The findings' own
records say what landed; this file is why it is built the way it is. If
the code and this file disagree after the fixes land, the code and the
records are right and this file is history.

How it was arrived at: a first draft was checked by two probes against the
library Vela ships (mpv 0.42.0, client API 2.6) and by three critics (one
on event ordering, one on mpv's API, one on the tests). The draft was
wrong in three ways that mattered, and this version corrects them:

- it skipped the swap report for frames it held back, which leaves mpv
  waiting 200 ms and more per frame, so the freeze `hd-3` describes would
  have stayed the same size;
- it tagged the surface when mpv's answers were in, which fails when the
  compositor has been asked for a newer description in the meantime, and
  reverses the guarantee that mpv is never asked for HDR on a surface that
  cannot be tagged;
- it gave up a fight over the targets by untagging the surface and leaving
  mpv with Vela's HDR values, which is the one outcome that must not happen.

The probes' sources and logs lived in a session scratch directory and are
NOT kept. What they measured is restated below as F1 to F9, and the parts
the fixes rely on are held by a test against the real library. Line
references into mpv are to the 0.42.0 source.

## Facts the design stands on (measured with the shipped library unless marked)

F1. `mpv_set_property_async` returns in microseconds on the render thread, frame pending or not.
    For `MPV_FORMAT_STRING` its `data` is the ADDRESS of a `char*` (passing the `char*` itself is
    undefined). Name and value are copied before it returns.
F2. Its reply (`MPV_EVENT_SET_PROPERTY_REPLY` = 4, `error`, `reply_userdata`) is queued after mpv's core
    has applied the option AND the video thread has acknowledged it. The video thread acknowledges only
    between frames, and after each frame it waits up to 200 ms for `mpv_render_context_report_swap`.
    So: draw and report every frame mpv hands over, skip only `eglSwapBuffers` -> replies within about
    one frame interval, no dropped frames, no log line. Draw but do not report, or do not draw ->
    211-242 ms per round, 3-4 dropped frames, and mpv logs "mpv_render_report_swap() not being called."
    or "mpv_render_context_render() not being called or stuck.".
F3. A render between the core applying an option and its reply can already use that option, and can see
    only some of the three (measured: a frame with `target-trc=pq` alone, about 35% darker). After the
    last reply, the next render uses all three, including a redraw of the current frame.
F4. Asks are applied and answered in the order asked; the last asked wins.
F5. A refused value still returns 0 from the async call and is answered with a negative `error` (-9).
    When the handle's event queue is full the call itself returns a negative code and NO reply ever comes.
F6. Property observation (`mpv_observe_property`, `MPV_FORMAT_STRING`; change event =
    `MPV_EVENT_PROPERTY_CHANGE` = 22 with `mpv_event_property { name, format, data }`, `data` being
    `char**` for a string, `format` 0 when the property is unavailable): an initial event fires per
    observed property; Vela's own asynchronous set produces a change event only when the value actually
    changed, and it is delivered AFTER that set's reply (queued events are returned before any property
    change is generated; a value that has changed again since it was captured is withheld). Value
    strings seen: `auto`, `pq`, `bt.2020`, `bt.1886`, `bt.709`, and the peak as a whole number (`356`).
F7. A profile mpv applies by itself when a file loads (`[extension.mkv]`, `[protocol.https]`) is
    reported 0.2 ms after `loadfile`, well before the first frame. A change made in the middle of a play
    (a `profile-cond` profile, a command on the IPC socket) can be drawn into one or two frames before
    its event can reach Vela; nothing in mpv's API prevents that.
F8. The surface's image description is pending state: `set_image_description` / `unset` take effect at
    the next `wl_surface.commit`, and on the video surface the only commit is `eglSwapBuffers`
    (`surface.rs` contains no `.commit()`). Destroying a description that was set but not yet committed
    is harmless.
F9. `wp_color_manager` requests the compositor did not offer end the connection, which is the whole
    app's. Nothing in this work adds a protocol request; `surface.rs` should not need to change.

## The invariant

Every frame that is PRESENTED (swapped) was drawn while mpv's three applied targets agreed with the
surface state that swap commits:
tagged for peak P <=> mpv draws for pq / bt.2020 / P; untagged <=> mpv draws for auto / auto / auto,
or for the user's own values where Vela is not asserting HDR.
It holds from the moment Vela has seen a change. It cannot hold for the frame or two mpv may draw
between a change made by something else and the event that reports it (F7); the tests must not claim it
for those.

## hd-3: ask without waiting; hold presentation while mpv's side is in flight

### libmpv.rs
- New functions in the table: `mpv_set_property_async`. Remove `mpv_set_property_string` from the table
  (nothing may use it once this lands; it is the waiting call the finding is about).
- `Targetable` (hdr.rs) becomes: `fn ask(&self, request: u64, option: &str, value: &str) -> bool` -
  "ask mpv, without waiting, to set this option; its answer arrives later as an event carrying
  `request`. `false`: the ask could not be sent and no answer will come."
- `Player::next_event()` returns an enum, never blocks, and is matched WITHOUT a wildcard arm wherever
  it is used: `None`, `Shutdown`, `Answer { request: u64, taken: bool }`, `Other`. (hd-2 adds a variant.)
- One waiting variant for the start of a play only (see set_up): e.g. `wait_for_event(limit)`. It may be
  called only before the render context exists.
- Existing code, same rule (render.h:71-96): `player.set_wakeup` moves above `Renderer::create`. The
  blocking `mpv_command(loadfile)` in `Live::load` stays, with a comment saying why it cannot stall at
  that one point (no video output exists before the first file is loaded, so the core cannot be waiting
  for this thread) - verify that claim in the mpv source and in Vela's launch options (no
  `force-window`) before writing it; if it does not hold, say so in your report instead of writing it.

### hdr.rs: a change of mpv's targets is a transition
All decisions live here, against the two traits, so that they are tested without a compositor or an mpv.
`session.rs` only passes things on.

- `Tagging` gains the pending transition: its number, which of its three asks are still unanswered,
  whether any was refused or could not be sent, and its goal (HDR with a target, or standard range, and
  whether standard range is the consequence of a refusal). Each ask carries its own `request` value
  (transition number and option index combined), so an answer says which option of which transition it
  answers. hd-2 needs the per-option part; build it now.
- `in_effect` is the GOAL from the moment a transition begins (as today: `description_ready` records it).
  A finish only confirms it, or reverts it to standard range after a refusal. With "recorded at the
  finish" three sequences go wrong (HDR switched off while the transition to HDR is pending; the
  brightness moved back while a transition to a new peak is pending; the same output described again
  while its transition is pending); each needs a unit test with a transition pending.
- `Step::ShowHdr(t)`: `surface.tag()` FIRST, at the moment the description is ready, exactly as today
  (pending state; the hold keeps it uncommitted). If the surface cannot be tagged, mpv is never asked for
  HDR, not even for a moment (the existing guard `a_surface_that_cannot_be_tagged_never_gives_mpv_an_hdr_target`
  keeps its meaning). Then begin a transition to HDR: three asks, `target-trc=pq`, `target-prim=bt.2020`,
  `target-peak=<nits>`.
- `Step::ShowSdr`: `surface.untag()` (pending), begin a transition to standard range: three `auto` asks.
- A refused HDR ask, or one that could not be sent: when that transition's last answer is in, in effect
  becomes standard range, the surface is untagged (pending) and a transition to standard range begins;
  when THAT finishes the change is `HdrRefused`. A surface that cannot be tagged goes the same way.
- A transition none of whose asks could be sent finishes on the spot (as refused). The standard-range
  transition that follows must not start another from inside that path if it cannot send either: it
  finishes on the spot too, and says so once on stderr.
- A new step while a transition is pending begins a new transition; answers to the old one are ignored
  (F4 makes the last asked win on mpv's side too).
- `hdr::answered(request, taken, ...) -> Change`: the only way a transition finishes. `Change` is what
  it is today (`Nothing`, `Hdr(peak)`, `Sdr`, `HdrRefused`), now returned at the finish; a finished
  change "needs a frame", which makes the loop draw and present one even when mpv has no new frame
  (a paused picture).
- `Tagging::changing()`: a transition is pending. `Tagging::settled()` additionally requires none pending.

### Frames: the hold
- One function in hdr.rs decides what happens to a frame, from: does mpv have a new frame, was a redraw
  asked for (resize, a finished change), is a transition pending, the time. Its answers:
  nothing / draw and report but do not present / draw, present, report / the fault case below.
  `Live::run` calls it once per iteration and does what it says; it contains no decision of its own.
- While a transition is pending every frame mpv hands over is DRAWN and REPORTED
  (`renderer.draw`, `renderer.frame_shown`) and NOT presented (`window.swap` skipped). Reporting is what
  keeps mpv's video thread from waiting (F2); not presenting is what keeps a half-changed picture (F3)
  and the surface's pending state (F8) off the screen.
- The decision is taken after the mpv events of this iteration have been read and before the draw, and
  no event is read between a draw and the decision to present it. So a frame is presented only if, when
  its draw began, every answer had been seen.
- One limit for the whole unbroken time a transition is pending; a superseding transition does not
  restart it. Measure before choosing the number: reply latency with a low frame rate clip (2 fps) and
  while paused, with draw+report; pick
  the limit with headroom over the slowest ordinary case (not below 500 ms). At expiry: say so once on
  stderr and present BLACK (`egl::Window::show_black`; black is the same picture under either
  description), keep drawing and reporting mpv's frames, and resume presenting when the transition
  finishes. Never present mpv's frame while a transition is pending.

### set_up (start of a play)
- No render context exists yet, so waiting for mpv is allowed there and only there. The loop that today
  makes up to `COLOUR_EXCHANGES_AT_START` compositor round trips also collects mpv's answers (a short
  waiting read while `tagging.changing()`), until `tagging.settled()` or one overall limit of about a
  second. A `Shutdown` seen there is an error return, not something to swallow.
- A transition still pending when set_up's limit runs out simply continues in the run loop: the hold is
  already in force, black is already showing.
- The same `follow_colour` is used at the start and during the play, as today.

## hd-2: observe the three options; put them back while HDR is in effect

- `mpv_observe_property` for `target-trc`, `target-prim`, `target-peak` as strings, once, right after
  `mpv_initialize`. `next_event()` gains `TargetChanged { option, value: Option<String> }`; the strings
  are copied out before the next call into mpv.
- `hdr::target_changed(option, value, ...) -> Change`. A reported value that differs from what Vela is
  asserting (trc and prim compared as text, the peak as a number) is SOMEONE ELSE'S exactly when:
  HDR is in effect, and either no transition is pending, or the pending transition's ask for THAT option
  has already been answered. In every other case it is ignored: before that option's answer it is either
  Vela's own earlier value or about to be overwritten by the pending ask (F4, F6); in standard range the
  user may set what they like, as with options in their configuration today.
  Why this is exact: a reply is queued when its ask is applied, queued events are returned before any
  property change is generated, and a captured value is withheld once the property has changed again
  (client.c:949-958, 1815-1816). This reasoning must be held by a test against the real library (below),
  not assumed.
- Someone else's change with no transition pending: begin a transition to the SAME HDR target at once
  (three asks); presentation is held until it is answered; its finish forces a frame, so a paused picture
  drawn for the foreign target is replaced. It does not touch the surface at all (it may not call `tag`:
  a newer description may be outstanding, and `tag` would fail though the surface is correctly tagged).
- Someone else's change while a transition is pending (the option already answered): remember it; when
  the transition finishes, begin the same transition once more instead of declaring it finished.
- A fight is bounded by RATE, not by a count per play: more than 5 restorations within 10 seconds ->
  give up through an ordinary transition to standard range (untag, three `auto` asks), and latch
  standard range for the rest of the play: no later description from the compositor brings HDR back, and
  later changes by others are theirs. One line on stderr. Time is passed in, so tests choose it.
- Nothing is logged that came from the user's configuration except the option's name.
- Known limits, to be stated in the finding's record, not worked around: the frame or two of F7 for a
  change in mid-play; standard range in effect is never defended.

## Tests

1. hdr.rs unit tests on a rig that replaces today's `Fake` / `FakeMpv` / `Play`:
   - the fake mpv APPLIES each ask at a point the test chooses between ask and answer, and answers later
     (real mpv applies before it answers, one option at a time; a fake that applies at answer time
     cannot show a hold that begins one draw too late);
   - it has a foreign setter that changes what it draws BEFORE the change event is delivered;
   - every presented frame is checked for the invariant against what the fake has applied at draw time
     and what the surface would commit;
   - a bounded every-order test: for small scripts of compositor steps, applies, answers, frames and (for
     hd-2) foreign sets, run every interleaving the real orderings allow and assert the invariant on every
     presented frame, that nothing is left pending at the end, and that the last presented frame shows
     the final state;
   - the sequences the critics constructed, each as a named test: a description asked for while an
     HDR transition is pending; a display change while a transition is pending (HDR switched off,
     the same output described again, the brightness moved back); a refusal on any of the three
     asks; asks that could not be sent; a change drawn between the first and the second option
     being applied. For each behaviour, a test and the mutation that must make it fail.
2. A test against the REAL library (`#[ignore]`, since CI has no mpv library): load the shipped set
   (`VELA_LIBMPV_DIR`, default `<repo>/.libmpv/out/libmpv`), no window, no render context
   (`vo=null`, `ao=null`, no configuration, idle): an ask is answered with its request and `taken`;
   a refused value (`target-peak=5`) is answered as refused; (hd-2) the three initial events, a change
   made through another route (`set` command) is reported with the expected text; and the ordering the
   hd-2 rule relies on: a long chain of transitions with changing peaks, drained the way the session
   drains, never delivers a differing value for an option after that option's answer of the newest
   transition. It must be RUN somewhere routinely: in every full `npm run e2e -- --embedded` run (that
   venue already requires the library); pin that wiring with a Node source-text test and document it in
   `tests/e2e/README.md`.
3. Node source-text wiring pins (`tests/embedded-player.test.mjs`): the guards that today pin the
   synchronous setter, `follow_colour`, the set_up loop and the run loop's draw/swap/report lines must be
   rewritten for the new shape and each re-proved. They must hold the WIRING: the run loop reads events
   without a wildcard and hands answers (and changes) to hdr; frames go through the one decision; a held
   frame is reported; nothing on the play's thread calls a waiting mpv function once the render context
   exists.
4. Headless weston has no colour management, so no end-to-end scenario reaches a
   transition. The embedded scenarios show that the rewritten loop still plays; a scripted
   run on a real HDR desktop shows the transitions themselves.

## Corrections made while the fixes were written

The fixes were attacked before they landed, and three things above did not
survive. The code and the findings' records are right; this section says
where the text above is now wrong.

- **hd-3, an ask that cannot be sent.** Above: such a change finishes on
  the spot. An attack showed that a way back to standard range finished
  like that can leave mpv drawing for HDR under an untagged surface, because
  asks made before mpv's queue filled still land. As built: an HDR change
  with an unsent ask counts as refused at once; a way to standard range with
  an unsent ask waits, held, and is asked again once the queue has been
  read empty (`hdr::drained`).
- **hd-3, the hand-over of the file.** Above: `loadfile` cannot stall
  because no video output exists before the first file. That holds only
  while `force-window` is off, and a user's option could turn it on. As
  built: the embedded launch turns it off after the user's options; a
  conditional profile or an IPC command can still turn it on, which is a
  recorded gap of `hd-3`.
- **hd-2, which reports count as someone else's.** Above: a report counts
  once mpv has answered Vela's ask for that option, on the strength of the
  order of mpv's events. An attack showed, against the shipped library,
  that mpv reports an observed option only when it differs from what it
  read at the end of its previous pass: a value set back, in the same pass
  as Vela's ask, to what Vela was last told is never reported (300 of 300
  tries). As built: each answer makes mpv observe that option anew under
  the ask's own number, mpv then reports it once more as it reads it after
  the answer, and a change finishes only when all three options have been
  answered and reported afresh. Nothing depends on the order of events.
