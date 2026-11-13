# Embedded player, Slice 5: the design the slice follows

Written 2026-11-13, before any code, for Slice 5 of
`.agents/plans/embedded-player.md` (player view controls and cropping).
The plan says WHAT the slice delivers (its sections "Frontend: the player
view", "Autocrop in the embedded player" and "Slice 5"); this file settles
HOW, where the plan leaves it open. The plan's log says what landed. If the
code and this file disagree after the slice lands, the code and the log
are right and this file is history.

How it was arrived at: a read-only map of the code, a draft, then two
probers (one in the app under the headless Wayland venue, one against the
mpv library Vela ships) and three critics (backend, page, scripts and
tests). Their sources and logs lived in a session scratch directory and
are NOT kept; what they measured is restated here as P1 to P12, and the
slice's own tests hold what matters.

Everything here is the agent's decision under the owner's working rule
(settle implementation details, record them). The four that change what a
user of the hidden player would notice are repeated in the plan's "For the
owner" list: S8's status, S8's HDR refusal, Escape (S4), and the gap
between plays in fullscreen (S5).

## Measured (2026-11-13)

P1. Keys can be tested for real in the headless venue: WebDriver's Element
    Send Keys and its Actions endpoint both deliver trusted `keydown`
    events to a capture-phase `window` listener (Space, arrows, letters,
    Escape), and Actions delivers pointer moves. Typing `C` arrives with
    `shiftKey` false; typing shifted `c` arrives as `C` with `shiftKey`
    true followed by a `keydown` whose key is `Unidentified`.
P2. In the embedded player mpv has no key input at all
    (`input-default-bindings=no`, no key forwarding): a Space keypress
    sent over IPC reaches only a script's forced binding and never pauses.
    Every key has to be mapped by the page.
P3. A capture-phase `window` listener that calls `preventDefault` and
    `stopPropagation` stops the page's own `window` handler, a focused
    button's Space click and a select's type-ahead (WebKitGTK 2.54).
    Preventing `mousedown` keeps focus off a button and its click still
    fires, but on a range input it kills the drag and on a select it keeps
    the popup from opening. After a mouse drag a range keeps focus and
    arrows then step it natively; a focused closed select changes value on
    arrows and on type-ahead letters (`f` picked "French").
P4. Fullscreen: the window plugin's `set_fullscreen` is refused by the
    capability file as it stands; with it allowed, headless weston honours
    the request within 250 ms, the page grows to the output's size and
    mpv's `osd-dimensions` follow (the view's ResizeObserver path). Tauri's
    `is_fullscreen` reports the last REQUEST, not what the compositor did
    (by reading tao 0.35.3 and tauri-runtime-wry 2.11.4), and its
    `inner_size` is not the page size.
P5. mpv's IPC socket takes further clients while Vela's trackers hold
    theirs; each can observe properties and send commands. Connect, one
    command and its reply take about a millisecond. A new client also
    receives events, so a reply is not "the next line". `time-pos` changes
    once per frame (about 24 a second). `set sid 5` on a clip with no
    subtitles answers "success". `track-list` entries of the test clips
    have no `title` or `lang`. Before the socket file exists a connect
    fails with ENOENT; mpv leaves the file behind when it ends, and a
    connect then fails with ECONNREFUSED. A volume set while mpv waits for
    its file is kept into the play.
P6. A skip whose target lies past the end of the file ends the play when
    the player is playing (the launch has `keep-open=no`, `idle=once`),
    whether it is a raw seek or the script's `activate-skip`. Held at the
    clamp (paused first, or `--keep-open=yes` from the user's options,
    which the embedded launch honours): a raw seek leaves
    `user-data/vela-markers/active` set, `activate-skip` clears it. Ending
    the play there is what the external player does too (skip the credits
    at the end, go on to the next episode); it is not treated as a defect.
P7. `vela-markers.lua`: only button arming waits for an OSD size
    (`draw_button` is the only reader of it); autoskip never did. With the
    drawing skipped under an option, arming, `active`, `activate-skip`,
    the forced Space binding and autoskip all work, with and without an
    OSD size; with the option at its default a call trace of the changed
    script equals the original's.
P8. FFmpeg's `bbox` compares raw luma samples with `min_val` and does not
    scale it by bit depth: `min_val=24` finds the bars on 8-bit video and
    takes them for picture on 10-bit; `24 << (depth - 8)` is right for
    both. Its metadata is per frame (nothing accumulates), arrives as
    `vf-metadata/<label>` with keys `lavfi.bbox.x1 x2 y1 y2 w h`, is an
    empty map right after insertion, and PQ tags change nothing.
P9. With real VA-API decoding (the embedded player's case) an inserted
    `bbox` fails silently: mpv disables the filter, the `vf` property
    still lists it, and only the missing metadata shows it. With copy-back
    or software decoding it works. So hardware decoding has to be off for
    the measurement, and failure is detected by "no metadata".
P10. A live change of `video-crop` can HALT playback in the shipped
    library, on SDR video: the position stops while mpv says it is not
    idle; pause and frame-step do not release it, a seek, a decoder change
    or a filter change does. Seen every time with copy-back decoding and
    nothing following the crop, and in 2 of 9 tries in the shape the
    embedded player has. Setting the crop, THEN removing the filter and
    restoring hardware decoding, and sending `seek 0 relative+exact` when
    hardware decoding was not switched, played on in 24 runs of 24.
    Removing a crop never halted. This is a stall of playback, not the
    unkillable wedge recorded in `.agents/decisions.md` (2026-06-28) for
    the gpu-next/Vulkan/HDR stack; that one was not tried in-process.
P11. mpv derives a script's name from its file name with every
    non-alphanumeric character turned into `_` (`vela-crop.lua` is
    `vela_crop`); `--script-opts-append` replaces an earlier value for the
    same key, so what Vela appends after the user's options wins.
P12. `read_line` with a read timeout keeps the bytes it already read in
    the buffer; clearing the buffer before each read, as the existing
    readers do, then loses the start of a line at every timeout.

## S1. `player_command`

- Tauri command `player_command { sessionId, name, args }`, `async`. Its
  body is one call, on the blocking pool, of a plain function that takes
  the player slot, the session id, the name and the arguments, so that
  Rust tests can drive it without a Tauri harness (there is none) and a
  Node pin can hold the command's body, as for the Close wiring today.
  The parameters are plain JSON values: every refusal comes from Vela's
  own parser with a fixed text, never from the framework's deserialiser.
- One enum with exactly the plan's ten names, parsed by one function with
  an exhaustive match and no catch-all. Arity is exact. Shapes:
  `toggle-pause []`, `seek-relative [seconds]` (finite, |s| <= 600),
  `seek-absolute [seconds]` (finite, 0 <= s <= 10,000,000),
  `set-volume [0..=100]` (finite number), `toggle-mute []`,
  `set-audio-track [id]` and `set-subtitle-track [id | "no"]` (a JSON
  integer from 1 to i32::MAX), `stop []`, `skip-active-marker []`,
  `toggle-autocrop []`.
- Each of the nine that reach mpv maps to one fixed JSON IPC command built
  in Rust: `cycle pause`, `seek <s> relative`, `seek <s> absolute`,
  `set volume`, `cycle mute`, `set aid`, `set sid`,
  `script-binding vela_markers/activate-skip`,
  `script-binding vela_crop/toggle`.
- In ONE hold of the player-slot lock the function decides identity and
  either stops the play (S3) or copies the play's socket path; the lock is
  released before the socket is touched. The embedded `Session` keeps the
  path; an external player has none, so the page can never drive mpv's
  own window. `close_embedded_play` keeps exactly one call site.
- Transport: mpv's IPC socket, one short connection per command, with a
  `request_id`; lines are read until the object carrying that id and no
  `event` key arrives (P5). Read and write limits of two seconds (a Unix
  socket connect either succeeds or fails at once).
- At most ONE command in flight per play (a gate kept with the path). A
  command arriving while one is in flight is answered at once as busy.
  After a command that got no answer, further commands are refused at
  once for five seconds, so a player that has stopped answering costs one
  waiting thread per five seconds, not one per key press. `stop` is never
  refused by the gate: it does not use the socket.
- Fixed texts for the page: not a player command; the player is not
  running (no such play, ENOENT, ECONNREFUSED, a closed connection); the
  player is busy; the player did not answer; the player refused. Nothing
  from mpv's reply, no path, no URL.
- Builds for other systems compile the command and answer "not running".
- The in-process handle is not used for commands: it belongs to the
  drawing thread (review finding hd-3).

## S2. `player-state`

- One thread per embedded play, spawned in `playback::play` after the
  end-of-file watcher and only when the player that STARTED is the
  embedded one (a play that fell back has no view). Its own IPC
  connection; it observes `time-pos`, `duration`, `pause`, `volume`,
  `mute`, `track-list`, `user-data/vela-markers/active`,
  `user-data/vela-crop/state`. It ends on end of stream, on the play's
  stop flag, or when it could not connect after the retries the other
  per-play threads allow.
- Reading: a read limit of 250 ms and a buffer that is cleared only after
  a complete line has been parsed (P12), capped at 1 MiB.
- It emits `player-state` through a closure built in `commands.rs`, at
  most 4 times a second (a change is sent at once when 250 ms have passed
  since the last event, otherwise at that boundary) and at least once a
  second.
- Payload, built field by field from named values:
  `{ sessionId, position, duration, paused, volume, muted, audioTracks,
  subtitleTracks, skip, crop }`. `duration` falls back to the item's own
  when mpv reports none. Tracks are `{ id, title, lang, selected }` for
  audio and subtitle entries only, text cut to 200 characters. `skip` is
  one of `intro`, `credits`, `commercial`, else null (the script publishes
  an empty string when nothing is active). `crop` is one of `off`,
  `measuring`, `on`, else null (null: no crop script, no crop control).
  `user-data` can be written by any script or IPC client, so both go
  through those closed sets and no other text from mpv reaches the page.
- The page ignores a `player-state` for another session, and unlistens
  correctly when the view unmounts before `listen()` resolves.

## S3. `stop` replaces `player_close`

- `stop` does what Close does today, `close_embedded_play` in the same
  single hold of the slot lock that checks identity (review finding ec-5),
  no socket, so it works at every moment of a play.
- `player_close` is removed (Rust, the handler list, the view). The guard
  that pins the Close wiring is rewritten for the new function and ec-5's
  injections are run against it again: stopping in two steps, stopping
  whatever play is there, stopping an external player. `ec-5.md` stays as
  history.
- After the view has sent `stop` it sends nothing more.

## S4. Keys

- One `keydown` listener on `window`, capture phase, while the view is
  mounted. For a key it acts on, `preventDefault` and `stopPropagation`.
- Map: Space: `skip-active-marker` while the last state shows a marker,
  else `toggle-pause`; Left / Right: `seek-relative` -5 / +5; Up / Down:
  +60 / -60; `m`: `toggle-mute`; `f`: fullscreen (S5); `q`: `stop`;
  Escape: leave fullscreen when the window is fullscreen, else `stop`;
  `C`, shifted or not as it arrives (P1): `toggle-autocrop`, only when the
  state says a crop script is loaded. Keys with Ctrl, Alt or Meta, keys
  named `Unidentified` and bare modifiers are not acted on.
- Auto-repeat acts only for the seek keys (mpv drops it for the others
  too); a held Escape cannot leave fullscreen and then stop.
- Focus (P3): pointer use never leaves focus on a control. `mousedown` is
  prevented on buttons only; a range input and a select hand focus back
  to the view's root on `change` (and the select on `blur`). With the
  keyboard: Space and Enter belong to a focused button; Space, Enter and
  Tab to a focused select; arrows to the focused volume slider. The page
  keeps the arrows on the seek bar (the same -5 / +5 / +60 / -60), and
  `f`, `m`, `q`, `C` and Escape everywhere.
- Before the first state has arrived only `stop` and fullscreen act; the
  other keys are swallowed without a notice.
- Space follows what the user sees: the HTML button and the key read the
  same last state. Within a quarter second of a marker's edge a press can
  pause instead of skip, or do nothing; accepted.
- The map is a pure function in one self-contained TypeScript module
  (no enum, no cross-file import other than `import type`: Node's type
  stripping and `svelte-check` agree on nothing else).

## S5. Fullscreen

- Command `player_fullscreen { sessionId, on }` (async, body on the
  blocking pool): `on: true` only for the running embedded play; `on:
  false` always. The identity check is made under the slot lock and the
  window is asked after the lock is released. It returns nothing.
- The real state comes from GDK: a `window-state-event` handler connected
  once at setup emits `window-fullscreen { on }` (P4: Tauri's own answer
  is only the last request). The page holds "fullscreen because of the
  player" outside the keyed view; it is true only while Vela asked AND
  the real state says fullscreen, and is cleared when the real state turns
  off (the user left with the compositor's shortcut).
- Leaving on events, never on a timer: `f`, Escape, the button; the
  user's stop; the page's own end-of-run path (no successor); a
  `playback-ended` with no view mounted and no continuation pending. A
  long backstop (the start limit plus 30 seconds) only against a lost
  event. A window the user made fullscreen themselves is left alone.
- Between two plays of a run the library page shows fullscreen for the
  gap. Recorded for the owner's eye.
- The picture follows a fullscreen change through the view's
  ResizeObserver and `player_set_rect`; nothing else repositions it.

## S6. The skip button and `vela-markers.lua`

- New script option `vela-markers-draw` (yes/no, default yes). The
  embedded launch passes `no` right after the payload option; the
  external launch is unchanged.
- The `draw=yes` path stays textually what it is; a gate chooses between
  `draw_button` and a quiet arm that sets the state and the forced Space
  binding and draws nothing, binds no mouse and does not wait for an OSD
  size. `button-bounds` stays empty when nothing is drawn.
- The "Skipped ..." toast stays on mpv's OSD, which is drawn in-process.
  (A reading of the plan's "suppresses only its OSD drawing": the button
  is what clicks cannot reach.)
- The view shows a button with the script's own wording ("Skip Intro",
  "Skip Credits", "Skip Commercials") while `state.skip` is set. It sends
  `skip-active-marker` and nothing else. It is not auto-hidden.
- In the embedded player the script's Space binding is reachable only by
  an IPC `keypress` (P2); the page's Space is the real path.

## S7. The notice line

- mpv's OSD keeps showing what Vela sends with `show-text` (the quality
  step-down line) and the script toasts. The view's line shows what the
  page has to say (a command that was refused or not answered), in an
  element with `role="status"` that is always in the DOM. Not auto-hidden.

## S8. `vela-crop.lua` (new, MIT, written by Vela from the steps below)

STATUS: the plan's "For the owner" item 3 (cropping is reimplemented for
the embedded player) is still an ASSUMPTION the owner has not ruled on,
and `.agents/decisions.md` (2026-07-05) still says Vela "does not
re-implement video-geometry processing". The slice ships S8 under that
assumption, hidden with the rest of the embedded player, and says so in
the plan's log and in `.agents/state.md`.

- Loaded only by the embedded launch, and only when the cropping setting
  is `manual` or `auto`. The embedded launch never names `autocrop.lua` or
  `vela-autocrop.lua`. The external launch stays byte for byte.
- Option `vela-crop-auto` (yes/no), DEFAULT NO: a lost option must never
  turn Manual into Automatic. The launcher passes `=no` for manual and
  `=yes` for auto, and passes no delay: the script owns its defaults (a
  settle delay of 5 seconds, a gather of about 1 second), and a user or a
  test changes the delay with `--script-opts-append=vela-crop-delay=N`.
- Binding `toggle` (`script-binding vela_crop/toggle`): a crop in effect
  is removed; otherwise measure and apply. A toggle during a measurement
  is ignored. A manual toggle cancels a pending automatic measurement.
- It will not measure: an image; a file with less left to play than the
  gather plus a second; video whose pixel format it cannot name; and HDR
  video (transfer `pq` or `hlg`, or Dolby Vision, or no video parameters
  yet). For HDR it says so with a toast. REASON: the recorded unkillable
  wedge was on HDR with a live `video-crop`, in-process it would freeze
  Vela, and it has not been tried; the owner can lift this after a trial
  with them present (it is in the plan's owner checks). This is a parity
  exemption for inventory row 9 that Slice 6 must carry.
- Measurement: if decoding is in hardware and not copy-back, remember
  `hwdec` and set it to `no` (P9); insert a labelled `lavfi` `bbox` with
  `min_val = 24 << (depth - 8)` (P8); observe its metadata; the gather
  window runs from the first box that arrives, with a 4 second cap; the
  result is the union of every box seen; no box at all means the filter
  failed and nothing is cropped.
- A result is refused when it is the whole frame (no bars) or smaller than
  half the frame in either dimension. Dimensions are rounded to even.
- Applying (P10): set `video-crop`, then remove the filter, then restore
  `hwdec`; when `hwdec` was not switched, follow with
  `no-osd seek 0 relative+exact`. Removing a crop needs none of this.
- Cleanup on end of file and on a new file: filter removed, timers
  stopped, `hwdec` restored.
- It publishes `user-data/vela-crop/state`: `off`, `measuring`, `on`.
- Licensing: the file is written from this list, not from the GPL script;
  its header says MIT and Vela-authored and does not say "based on". The
  slice records a mechanical comparison showing no line of `autocrop.lua`
  beyond bare mpv API calls appears in it. `PROVENANCE.md` describes it
  the way the markers entry is written (the hidden player is not named)
  and its GPL sentence covers all Vela-authored scripts.
- Plumbing: `PlaySpec.crop_script` from `resolve_resource`, an embedded
  branch at the launch gate with the same exists-check as the others,
  every `PlaySpec` literal, both recorded embedded launch lists, the
  external golden test given the new field with its expected list
  unchanged, and the Arch PKGBUILD line for the new file. The PKGBUILD's
  missing line for `vela-markers.lua` is NOT added here: it is a recorded
  blocker waiting for the owner (`.agents/state.md`).

## S9. The controls

- A bar overlaid at the bottom of the view (the video rectangle stays the
  whole view): play/pause, time, seek bar, mute, volume, audio tracks,
  subtitles (with "Off"), crop (only when `state.crop` is not null;
  disabled while `measuring`), fullscreen, close. The skip button above
  the bar at the right, the notice line above it at the left.
- Seek bar and volume are native range inputs with their own classes (not
  `.progress`); tracks are native selects. `color-scheme: dark` on the
  view, so they render dark over the picture in every theme.
- The seek bar seeks ONCE, on release (`change`). From the first `input`
  to the `change` it shows the drag position and state does not write to
  it; after release it holds the target until a state lands within a
  second of it or three seconds pass. It is disabled when the duration is
  unknown or zero.
- Time: `m:ss / m:ss`, both sides `h:mm:ss` from one hour, `--:--` for an
  unknown or zero duration, negative and non-finite positions clamped.
- Icons from `Icon.svelte`, extended in its own style, each used with a
  LITERAL name (the UI foundation test requires it); every button has an
  accessible name; mute carries `aria-pressed`; the seek bar has a label
  and a spoken value.
- The bar hides after 3 seconds without pointer movement or key press
  while playing, and the pointer with it; it shows on movement, on a key,
  while paused, and while a control has keyboard focus. Hidden means
  faded out AND `visibility: hidden` (switched at the end of the fade, at
  once on show), so a hidden control takes no click. The rule is a pure
  reducer that takes a clock.
- All pure logic lives in one module beside `playerView.ts` and is tested
  by plain Node. Motion uses `var(--ease)`. Names avoid the word the page
  may not contain; the state type does not go into `types.ts`.
- Not carried over from mpv's window, recorded for Slice 6: double-click
  for fullscreen, the wheel for volume, the 9/0 volume keys.

## S10. Unchanged, and said so in the log

- Volume, mute, track choice, pause and a MANUAL crop do not survive the
  automatic quality step-down (mpv is restarted); in auto mode the crop is
  measured again after the delay. The external player behaves the same.
- Server check-ins never report a pause, in either player.
- The seek bar and track lists of a server-transcoded play are whatever
  mpv reports; not tested without a real server.
- Settings still says "Loads mpv's bundled autocrop.lua" and "press
  Shift+C"; the first is not true of an embedded play and cannot be
  reworded before Slice 6 (the word ban).

## Tests

Each guard is red-proved: the regression injected with the fix in place,
the test seen to fail for that reason, restored from git.

- Rust, the command: every unknown name and every wrong arity, type or
  range refused (mutant: a passthrough arm); the exact bytes each of the
  nine sends, against a test listener; an event line before the reply is
  skipped; another session's id refused; an external player refused;
  `stop` stops with no socket touched; twenty commands against a listener
  that never answers make one connection; the five-second refusal.
- Rust, the state: the payload builder drops `external-filename` and
  unknown keys; a URL in `active`, in `crop`, or in any unknown key never
  appears in the serialised payload; a line that arrives in two pieces
  across a read timeout is parsed whole; the throttle with an injected
  clock.
- Rust, the launch: the embedded launch never names the stock scripts
  (mutant: restore them), names `vela-crop.lua` for manual and auto only,
  with `auto=no` for manual (mutant: `yes`) and `auto=yes` for auto;
  `vela-markers-draw=no` only in the embedded launch; the external golden
  list unchanged with the new field set.
- The allow-list as a type: one module exports the ten names `as const`
  and the single `sendPlayerCommand(sessionId, name, args)` that holds the
  one `invoke("player_command"`; `svelte-check` refuses any other name
  (mutant: a control sending `loadfile`). A Node guard requires the view
  to call `invoke` nowhere but through the named wrappers (matching
  `invoke<T>(` too). The ten names live in one JSON file that a Rust test
  (`include_str!`: every name parses, every variant maps back) and a Node
  test (the TypeScript list equals it, by importing the module) both read.
  No test reads Rust with a regular expression.
- Node, pure logic: the key map over every key, modifier, repeat, focus
  kind and state; time format; the visibility reducer; the seek bar's
  drag and pending target. And the wiring: the pins in
  `tests/player-view.test.mjs` rewritten (the mount markup, the invoke
  count, the `onMount` and cleanup shapes) and each re-proved. Every new
  test file is added to the `check` script, which lists files by name.
- Scripts, with real mpv in the ordinary venue (`markerpayload` and a
  sibling for the crop script): `draw=no` with a button policy arms under
  `vo=null` and `draw=yes` does not; `vela-crop.lua` on 8-bit and 10-bit
  letterboxed clips, manual and auto, no bars, an HDR-tagged clip refused
  (mutants: unscaled `min_val`; default `auto=yes` with the option
  dropped). The differential trace that shows `draw=yes` unchanged is run
  once at implementation and recorded.
- E2E under headless weston, embedded only:
  - `embeddedcontrols`: state arrives; pause, seek, mute, volume and stop
    through the controls and through real keys (P1); fullscreen on and off
    (page height and `osd-dimensions`, not Tauri's answer). Mutants:
    `toggle-pause` a no-op; the seek's sign flipped; a `stop` that does
    not close.
  - `embeddedskip`: a button policy and a marker that ends past the end
    of the clip; the play is PAUSED through the page first and `pause`
    read back; the HTML button is pressed; then `active` is empty, the
    button stays away for two seconds, and the play is still alive.
    Mutant: `skip-active-marker` mapped to a raw seek past the end.
  - `embeddedcrop`: letterboxed clip, delay shortened through the user's
    options; auto sets the expected `video-crop` on a fresh and on a
    resumed play; manual leaves it empty past the delay and crops on the
    control. Mutants: the binding spelled `vela-crop/toggle`; `auto=no` in
    auto mode.
  - `embeddedplay` moves the pointer (Actions) before it clicks Close; the
    external `autocrop` scenario is marked external-only and
    `embeddedcrop` is its counterpart for Slice 6's parity guard.
- By eye, for the owner (added to the plan's list): the bar over bright
  HDR video and in a light theme; a select's popup over the picture and
  in fullscreen; the picture's edges after entering and leaving
  fullscreen; cropping on SDR with repeated toggles; and, with the owner
  present and warned that a wedge would freeze Vela, the HDR crop trial.
