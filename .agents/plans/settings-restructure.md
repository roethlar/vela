# Plan: Settings restructured; a restart prompt for the video player

Status: APPROVED by the owner, 2026-11-17 ("yes"), with S1 amended as
below at the owner's question "why can we not just always start vela with
the assumption that internal is the preferred? then neither setting would
need a restart." S1 landed (`.agents/state.md` names the commit); S2 next.

## Slice S1, as amended and approved: every start is ready for Vela's window

Every start is embedded window mode (transparent window; on Linux,
WebKit's GPU renderer on) whichever player is chosen, unless something
rules it out (a start-up marker, no Wayland, the renderer turned off in
the user's environment), and then it says why. So neither choice needs a
restart and there is no prompt; the restart sentence goes from Settings,
`docs/usage.md` and the release notes. The original S1 below (a
restart-or-cancel dialog) is superseded.

## Why

The owner, 2026-11-17, after choosing Vela's window on the NVIDIA machine
and finding videos still in mpv's window: "add a prompt when that setting
is changed to restart or cancel the change. the settings window needs to
be restructured overall. the setting is buried under a ton of irrelevant
in-line help text that should be a popup or removed. there are 4 main
settings tabs and a ton of stuff is crammed into the player one. fix
that."

Today (`src/lib/Settings.svelte`): four tabs (Connected, Servers, Player,
Appearance). Player holds eleven groups in this order: duplicate playback
source, display resolution/HDR override, Continue Playing, Video player,
Player window, then "mpv player" (use own mpv config, mpv path, advanced
mpv options with examples, black-bar cropping, playback quality, skip
intros, credits, commercials), with about 500 words of grey help text
under them. Choosing Vela's window takes effect only at the next start,
and the only sign is one grey sentence.

## Slice S1: the restart prompt

- Choosing an option that needs a restart (Vela's window, while this run
  did not start in its mode) opens a dialog: "Vela's window takes effect
  after Vela restarts." with **Restart now** (saves the choice and
  restarts Vela) and **Cancel** (puts the choice back; nothing saved).
- Choosing mpv's own window needs no restart (it applies to the next
  video) and opens no dialog.
- Restart: a new command restarts the process through Tauri's
  `AppHandle::restart`, after Vela's normal exit path (a running play is
  stopped and reported first, as at quit).
- The setting's grey note about restarting goes.
- Tests: the dialog's two paths (a Svelte/frontend test), the command's
  wiring (a guard that it reaches the exit path), an end-to-end scenario
  choosing Vela's window, cancelling (nothing saved) and restarting (the
  next start is in Vela's window).

## Slice S2: the tabs

Proposed (the owner may change it):

| Tab | Holds |
|---|---|
| Connected | as today |
| Servers | as today, plus "When the same title exists on more than one server" |
| **Playback** (new) | Video player (first), Continue Playing, Skip intros / credits / commercials, Playback quality |
| **Picture** (new) | Resolution and HDR overrides, Player window, Black-bar cropping |
| **mpv** (was Player) | Use my own mpv config, mpv path, Advanced mpv options with their examples |
| Appearance | as today |

## Slice S3: the help text

Every grey paragraph is either removed (where the label and the choices
say it already) or moved behind an "ⓘ" next to its label that opens a
short popover; at most one short line stays under a control, and only
where a choice would otherwise surprise (for example "applies to the next
video"). Notes that report state (why Vela's window cannot be used here)
stay visible.

## Order and checks

S1, S2, S3, each its own commit with records; `npm run check`, the
frontend build, both Linux end-to-end suites in weston and the Windows
suite (Settings scenarios name the tabs and find controls by their
labels, so those scenarios move with it), and a codex hunt over each.

## The owner's part

Approve the plan, and the tab layout in S2 (or give your own).
