# Plan: carry the mpv window's size and position from one play to the next

## Status

**DRAFT 2026-11-12. NOT APPROVED. No implementation may start.**

Owner rulings already given (2026-11-12), verbatim:

- **Request.** "when I do play the next episode, the mpv window resets to
  its default position and size. can we maintain the last set geometry for
  each subsequent play in a session? if yes, can we carry that over to new
  sessions as well (with an option)?" Observed on a Windows install.
- **D1 (which plays inherit): every play in the session.** Asked whether
  manual plays should inherit the previous window state or only automatic
  successors, the owner answered: "all in current session". This reverses
  the 2026-07-18 rule in `.agents/plans/playback-window-state-continuity.md`
  ("Manual plays retain their configured defaults; window size and position
  remain untouched").

Open, not yet ruled (each is asked in chat, one at a time):

- **D2 (across app restarts): RULED IN PART 2026-11-12.** Offered an
  on/off setting that would remember size and position only, the owner
  answered: "full screen is usually the goal for me, so how about options?
  remember last, full screen, default? if last is full screen, then restore
  full screen next time." So the setting is a three-way choice, not a
  checkbox, and "remember last" includes fullscreen (the agent's reading:
  maximized and screen too, since they are the same kind of state).
  **D2(b) RULED 2026-11-12:** asked whether the setting governs the first
  play of a run or every play, the owner answered: "yes, first run only,
  follow the setting on a new Vela launch session". So the setting decides
  only how the first player window of a Vela run opens; later plays in
  that run follow the previous window per D1, in all three modes.
  **D2(a) RULED 2026-11-12:** asked which of the three a fresh install
  starts with, the owner answered: "remember last". An absent
  `mpv_window_mode` therefore means "remember".
- **D3 (window position on Windows).** Whether to add the Windows-only code
  that position needs. Slice 4 depends on it; Slices 1-3 do not.

The plan needs the owner's explicit go after those rulings.

## What exists today

- Every play starts a new mpv process, by default with `--no-config`, so
  nothing about the window survives unless Vela passes it.
- `src-tauri/src/playback.rs`: `PlaybackWindowState { fullscreen,
  maximized }` (both `Option<bool>`), filled by `WindowStateObservation`
  from IPC `property-change` events (`spawn_position_reader` observes
  `time-pos`, `fullscreen`, `window-maximized`, `display-names`,
  `display-width`, `display-height`). `window_state_args` turns a state
  into `--window-maximized=yes|no` and `--fullscreen=yes|no`, applied in
  `play` after the user's Advanced arguments, then `screen_name_arg`, then
  `lifecycle_args`, then the IPC option.
- `src-tauri/src/commands.rs`: `AppState.playback_window_session` holds one
  `PlaybackWindowSession { session_id, observation }`, published after a
  successful launch. `inherited_window_state` and `inherited_screen_name`
  return a value only when `replace_session` names that exact session, i.e.
  only for an automatic successor. A manual play passes `None` and inherits
  nothing.

## Feasibility evidence

Probe on `continuum`, 2026-11-12: mpv 0.41.0 on a private Xvfb display
(X11, `--vo=x11`, no window manager), 1280x720 test source unless noted,
properties read over IPC.

| Launch | `osd-width` x `osd-height` | Notes |
|---|---|---|
| no size option | 1280 x 720 | `current-window-scale` 1.0 |
| `--geometry=800x450+120+90` | 800 x 450 | exact |
| `--geometry=800x450`, 640x480 source | 800 x 450 | exact even when the aspect differs |
| `--autofit=800x450`, 640x480 source | 600 x 450 | fits inside the box; not exact |
| runtime `window-scale` 0.5 | 640 x 360 | a runtime resize is reported |
| runtime fullscreen | 1920 x 1080 | reports the display, not the window |

Conclusions used below:

- `osd-width`/`osd-height` report the current window size, including after
  a runtime resize, and `--geometry=WxH` reproduces it exactly. Use
  `--geometry`, not `--autofit` or `window-scale`.
- While fullscreen the reported size is the display's. Size must be
  recorded only while the window is neither fullscreen nor maximized.
- mpv 0.41 lists no property for the window's position (`mpv
  --list-properties`: nothing position-like; `window-id` exists).

NOT probed, and each is an assumption until its acceptance step:

- A real window manager, and hi-dpi scaling. If `osd-width` is in physical
  pixels while `--geometry` is interpreted in scaled units (or the reverse),
  the window would grow or shrink on every play.
- Wayland honouring `--geometry=WxH` for size. Wayland clients cannot set or
  read their own position at all; that part is certain.
- Windows and macOS behaviour.

## Goal

1. Within one run of Vela, every play (manual or automatic) opens its mpv
   window as the previous play left it: fullscreen, maximized, screen, and
   the windowed size.
2. A three-way setting (D2) decides how the first player window of a run
   opens: as the last run left it (fullscreen included), always fullscreen,
   or mpv's default.
3. If D3 is ruled in: on Windows the windowed position is carried the same
   way.

Unchanged: one mpv process per play; the order of Vela's protected options;
Advanced arguments and own-config mode stay in effect for a play that has
nothing to inherit (the first play of a run with nothing remembered); the
failed-launch rule (a launch that fails publishes no state); session
validation for automatic successors.

## Non-goals

- Position on Wayland (impossible) and on macOS and X11 (not requested, not
  verified). On those platforms position is simply never recorded.
- Any change to what an automatic successor is, or to `replace_session`
  validation: a stale continuation is still rejected before launch.
- Remembering per-title or per-server geometry.

## Slice 1 — every play in a run inherits the last observed window state

`src-tauri/src/commands.rs`:

- `inherited_window_state` and `inherited_screen_name` stop requiring
  `replace_session` to match: they return the snapshot of whatever
  `playback_window_session` currently holds, for any play. Remove the
  `replace_session` parameter from both. `replace_session` keeps its other
  job (expected-session validation) untouched.
- `playback_window_session` is only replaced after a successful launch
  (existing behaviour), so the record of the last successful play persists
  after its mpv exits and through failed launches. Confirm that by reading
  `after_successful_play` and the failure paths; do not add a clear.
- Keep sampling AFTER stream resolution (existing comment in
  `play_by_key`), so a just-ended process has time to deliver final events.
- Update the doc comments on `PlaybackWindowState`,
  `PlaybackWindowSession` and the sampling block: they currently say
  "exact automatic successor" and "manual plays deliberately start from
  configured defaults".

Existing tests that pin the old rule must change (approval of this plan is
the owner's approval to change them): find them with
`grep -n "inherited_window_state\|inherited_screen_name" src-tauri/src/commands.rs`
and in `tests/e2e/scenarios/` (grep `window-maximized`, `fullscreen`,
`screen-name`). For each, rewrite to the new rule rather than deleting.

New Rust unit tests (names indicative):

- `a_manual_play_inherits_the_last_observed_window_state`: a published
  session with fullscreen `Some(true)`; `inherited_window_state(Some(&s))`
  returns it with no session argument.
- `nothing_published_inherits_nothing`: `None` → default state, no screen.
- `a_failed_launch_keeps_the_previous_record` (wiring): drive the same
  helper the launch path uses (`after_successful_play` with an `Err`) and
  assert `playback_window_session` still holds the earlier session.

Red-proofs, separately: restore the `replace_session` gate (first test
fails); make the failure path overwrite the record (third fails).

E2E (`continuum` is a working venue; mpv there runs with `--vo=null`, so
only argv can be asserted, not a drawn window): extend the scenario that
already inspects mpv's argv for `--fullscreen`/`--window-maximized` (find
it by grep) so that a MANUAL second play carries the first play's flags.
If no scenario inspects argv, add the assertion to `playverbs`. Red-proof
it by restoring the gate.

Commit: `feat: every play inherits the previous player window state`.

## Slice 2 — carry the windowed size

`src-tauri/src/playback.rs`:

- Observe `osd-width` and `osd-height` (next free observation ids) in
  `spawn_position_reader`. Heed the existing warning there: consumers match
  on the property NAME, never on any numeric `data`.
- Extend `PlaybackWindowState` with
  `windowed_size: Option<(u32, u32)>` and keep the per-launch isolation of
  `WindowStateObservation`.
- Recording rule, as a pure, clock-injected state machine (unit-testable
  without mpv):
  - A size sample (both dimensions known, both > 0) becomes the CANDIDATE.
  - A candidate is CONFIRMED into `windowed_size` when a later event or the
    final snapshot arrives at least 1000 ms after it AND `fullscreen` and
    `maximized` are each not `Some(true)`.
  - When `fullscreen` or `maximized` changes to `Some(true)`, DISCARD the
    unconfirmed candidate. (On X11 the size change can be reported before
    the maximized flag; without this the maximized size would be stored as
    the windowed size.)
  - While either flag is `Some(true)`, ignore size samples.
  - At snapshot time an unconfirmed candidate is accepted if neither flag
    is `Some(true)` (the user resized and quit within a second).
- `window_state_args`: when `windowed_size` is `Some((w, h))`, emit
  `--geometry={w}x{h}` FIRST, before `--window-maximized` and
  `--fullscreen`, so a fullscreen play still knows its windowed size when
  the user leaves fullscreen.
- A size smaller than 160x90 or larger than 16384 in either dimension is
  not recorded (garbage guard).

Tests (Rust unit): one per rule above, plus `window_state_args` ordering
and omission when unknown. Red-proof each rule separately: drop the
discard-on-flag; drop the 1000 ms confirmation; record while fullscreen;
emit geometry after `--fullscreen`.

E2E: argv assertion that a second play carries `--geometry=WxH` equal to
what the first reported. With `--vo=null` mpv reports no `osd-width`, so
this needs either a scenario that injects the property through the mock IPC
path, if one exists, or it is NOT E2E-testable here; say which in the log.
Do not write a scenario that depends on a drawn window (see
`.agents/machines.md`, "This venue never runs mpv with a real video
output").

Acceptance on real desktops (owner present; nothing automated proves these):

- `continuum` (KDE Wayland, check the display's scale factor first): play,
  resize the mpv window, close; play something else manually. The second
  window opens at the first one's size. Repeat three times WITHOUT
  resizing: the size must not drift. If it drifts by the display scale
  factor, divide (or multiply) by mpv's `display-hidpi-scale` property
  when recording, re-test, and record the finding.
- The same on the affected Windows install.
- Fullscreen a play, quit from fullscreen, play again: opens fullscreen;
  leave fullscreen: the window returns to the earlier windowed size.

Commit: `feat: keep the player window size between plays`.

## Slice 3 — how the player window opens: a three-way setting (D2)

The setting, "Player window opens" (final wording at implementation), has
three values:

- **Remember last** — the first play of a run opens as the last play of
  the previous run left its window: fullscreen, maximized, screen, windowed
  size (and position where Slice 4 provides it). Quit from fullscreen →
  next run opens fullscreen.
- **Full screen** — the first play of a run opens fullscreen.
- **Default** — the first play of a run gets nothing from Vela: mpv's own
  default, or whatever the user's Advanced arguments or own config say
  (today's behaviour).

Per D2(b): in all three modes, later plays in the same run follow the
previous play's window (D1). The setting is consulted only when nothing
has been published in this run.

- `src-tauri/src/config.rs`: two new optional fields on `AppConfig`,
  following the existing `Option` + validation pattern (unknown or
  malformed values invalidate the document, as for `continue_playing`):
  `mpv_window_mode: Option<"remember" | "fullscreen" | "default">` (absent
  = "remember", per D2(a)) and `mpv_window: Option<{ fullscreen:
  Option<bool>, maximized: Option<bool>, screen: Option<String>, width:
  Option<u32>, height: Option<u32>, x: Option<i32>, y: Option<i32> }>`. It
  is one bounded value, so it belongs in `config.json` by the 2026-07-14
  storage criterion.
- Written through the existing `config::update` path when a play ends
  (the `on_end` closure already performs config I/O there), only in
  "remember" mode and only when the final snapshot differs from the stored
  value. Never on a timer.
- Read once per launch as the fallback when nothing has been published in
  this run. An in-run observation always wins over the stored value.
- "fullscreen" mode: when nothing has been published in this run, the
  inherited state is `fullscreen: Some(true)` and nothing else.
- A stored screen name that no longer exists must not break the launch:
  check how `screen_name_arg` already treats an unknown name and keep that
  behaviour; add a test.
- Leaving "remember" stops reads and writes and removes the stored value
  on the next config write.
- Old-config compatibility: both fields absent must load exactly as today
  (round-trip test), and a config written with them must still load in the
  previous release — check how unknown fields are treated by the previous
  release's `AppConfig` (strict or tolerant) and record it; if strict, say
  so plainly in the release note.
- `src/lib/Settings.svelte`, player tab: one three-way control (select or
  radio group) with the commands `get_/set_mpv_window_mode` beside the
  existing `get_/set_continue_playing` pair.

Tests: config round-trip with and without the fields; invalid shapes and
unknown mode strings rejected; each mode's first-play arguments (remember
with a stored fullscreen → `--fullscreen=yes`; fullscreen mode with nothing
stored → `--fullscreen=yes`; default → no window arguments); the
in-run-wins-over-stored rule; a source-text guard that the
Settings control exists only while the launch path reads the setting (the
pattern of `tests/transcoding-ui.test.mjs`). Red-proof each.

Commit: `feat: optionally remember the player window across restarts`.

## Slice 4 — ONLY IF D3 IS RULED IN: window position on Windows

mpv reports no position, so Vela reads it from Windows.

- First, a probe on a real Windows desktop session (it cannot run over SSH:
  there is no desktop there): start mpv with `--geometry=800x450+300+200`,
  read `window-id` over IPC, call `GetWindowRect` and
  `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` on that handle, and
  record which of them (or the client origin from `ClientToScreen`) equals
  the `+300+200` that was passed. That decides which rectangle is recorded.
  Record the result in this plan before writing the code.
- `src-tauri/src/playback.rs`, `#[cfg(windows)]` only: observe `window-id`;
  sample the chosen rectangle's top-left at most once every 2 s while
  handling IPC events, and once more when the reader sees the connection
  close is too late (the window is gone), so the last periodic sample
  stands. Same windowed-only rule as size.
- Add the needed `windows` crate feature (`Win32_UI_WindowsAndMessaging`,
  plus `Win32_Graphics_Dwm` if the probe picks the DWM rectangle) to the
  existing `cfg(target_os = "windows")` dependency block. No new crate.
- `PlaybackWindowState.windowed_position: Option<(i32, i32)>`;
  `window_state_args` emits `--geometry={w}x{h}+{x}+{y}` when both are
  known (negative coordinates are legal on multi-monitor desktops; format
  them in the form mpv accepts and confirm it in the probe).
- Off-screen guard: before using a position, check with
  `MonitorFromPoint(..., MONITOR_DEFAULTTONULL)` that the window's
  top-left and centre fall on a current monitor; if not, drop the position
  and keep the size.
- Interaction with the existing `--screen-name`: it is asserted after the
  window-state options. In the probe, confirm an absolute `+X+Y` on a
  second monitor is honoured with `--screen-name` also present; if they
  conflict, omit `--screen-name` when a position is being passed, and
  record that.

Tests: pure functions for the sampling throttle, the windowed-only rule and
argument formatting (run everywhere); the Win32 calls behind a small trait
so the logic is tested with a fake on all platforms. `cargo +stable test`
and clippy must be run on `netwatch-01`, since Linux CI does not compile
`cfg(windows)` code. Acceptance on the affected install: move and resize
the window, play again, three times without touching it — no drift.

Commit: `feat: keep the player window position between plays on Windows`.

## Version, notes, docs

One `scripts/bump.sh` in the first code commit; `RELEASE_NOTES.md` bullets
per landed slice; `docs/usage.md` player section: what is carried, that
position is Windows-only, and the setting if Slice 3 lands.

## Verification before claiming completion

The full set in `.agents/repo-guidance.md` (Verification). On `continuum`
that excludes the MSRV check and `cargo audit` (GitHub CI covers them) and
requires `cargo +1.100.0 clippy` as well as `+stable` until the local
stable is updated (`.agents/machines.md`). E2E from a disposable worktree.
Slice 4 additionally needs the Windows build and tests on `netwatch-01`.

## Paperwork on landing

- `.agents/decisions.md`: a dated entry recording D1-D3 and that D1
  supersedes the manual-play rule of the 2026-07-18 window-state plan.
- `.agents/plans/playback-window-state-continuity.md`: a one-line banner
  pointing here for the superseded rule.
- `.agents/state.md`, and this file's Status and implementation log.
