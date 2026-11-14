# pa-2: the external player's cropping is tested in automatic mode only, and the row says off, manual and automatic

**Severity**: MEDIUM — row 9 is "Autocrop off / manual / auto". In the external player `autocrop` is the only scenario that stands for it, and it sets automatic mode and nothing else. A manual toggle that does nothing, or an Off that still crops, leaves the row shown and the verdict passing; the embedded player's scenario does exercise manual cropping, so the parity claimed is one-sided.
**Status**: In progress
**Branch**: —
**Commit**: `9bcdfbc`

## Evidence

Reviewer, verbatim: "tests/e2e/scenarios/autocrop.mjs:117 claims row 9, whose inventory includes off/manual/auto. Its configuration at :131 always selects auto, and its two legs at :148 and :159 test only fresh and resumed automatic cropping. It is the sole external-player coverage for row 9."

## Predicted observable failure

Reviewer, verbatim: "A broken external manual-crop binding, or an Off setting that still enables cropping, can leave the scenarios and verdict green. Manual cropping is exercised in embeddedcrop, so the declared parity conceals a one-sided proof."

Reviewer's suggested direction, verbatim: "Add external manual and Off legs that verify both the absence of automatic cropping and the manual toggle's effect. Credit the complete row only after all its modes have been exercised."

## What

The defect is as the reviewer describes. Row 9 is "Autocrop off / manual /
auto". In the external player `autocrop` was the only scenario standing for
it, and it ran automatic mode only (a fresh play and a resumed one). The
embedded player's `embeddedcrop` ran automatic and manual cropping, and not
Off.

What each mode does, read in the code (`autocrop_args` and `crop_args` in
`src-tauri/src/player/launch.rs`, the two scripts, the player view):

| Mode | External player | Embedded player |
| --- | --- | --- |
| Off | nothing is loaded | `vela-crop.lua` is not loaded; the view has no Crop control and `C` does nothing |
| Manual | the stock `autocrop.lua` with its own trigger off; the user presses Shift+C in mpv's window | `vela-crop.lua` with automatic cropping off; the view's Crop control or `C` toggles |
| Auto | the stock script with its own trigger off, plus Vela's shim, which triggers it after a settle delay | `vela-crop.lua` with automatic cropping on |

## Approach

`tests/e2e/scenarios/autocrop.mjs` (external only) gains two legs:

- Off: nothing is cropped through 8 s of playback, checked all the way
  through (longer than any automatic detection takes: the shim's 1 s or
  5 s delay, or the stock trigger's 4 s, plus 1 s of detection); the shim
  is not loaded; no key of mpv's window is bound to the crop toggle.
- Manual: mpv binds exactly Shift+C to the stock script's toggle (read from
  `input-bindings`); the shim is not loaded; nothing crops for 6 s; Shift+C
  crops the letterbox bars away; Shift+C again takes the crop away.

The key is sent through mpv's input commands over IPC (`mpvInput`), because
no venue here can type into mpv's window. That is a test-only route, and
the scenario declares it in `testOnly` for row 9 in the external player,
saying what it proves (the binding, the crop and the uncrop) and what it
does not (that a key pressed in mpv's window reaches mpv's input layer).

`tests/e2e/scenarios/embeddedcrop.mjs` gains an Off leg: the view offers no
Crop control, nothing crops and the crop script publishes no state for 3 s,
nor for 5 s after the user's `C` key.

Both scenarios now wait for the movie card and open its menu in the same
step; the grid can be re-rendering just after a play ends (seen once).

After the fix, mode by player: external Auto, Off and Manual are legs 1-2,
3 and 4 of `autocrop`; embedded Auto, Manual and Off are legs 1-2, 3 and 4
of `embeddedcrop`.

Made by a helper agent (Claude Opus 5.5), attacked by a second (which found
nothing to repair here), confirmed by a third on the merged content, and
read and landed as one commit by the coordinating agent.

## Files changed

- `tests/e2e/scenarios/autocrop.mjs`
- `tests/e2e/scenarios/embeddedcrop.mjs`
- `tests/e2e/README.md`

## Guard proof

The scenarios are the guard, and what proves them is a run with the mode
wrong. Each was restored afterwards.

External player, `autocrop`:

- The Off leg run with the mode `auto`: "off: nothing is cropped in 8000 ms
  of playback — broke during the hold: video-crop became "320x140+0+20"".
  The confirming helper got the same by making Off launch as Auto in
  `launch.rs`.
- The Off leg run with the mode `manual`: "off: no key of mpv's window
  crops".
- The first Shift+C not sent; the second not sent; the binding absent
  (Manual run with the mode `off`), with and without the binding check;
  Manual run as `auto`: each failed on its own sentence.
- The attacking helper's product change, Manual launched with the stock
  script's own trigger left on: "manual: nothing is cropped in 6000 ms
  without the key".

Embedded player, `embeddedcrop`:

- The Off leg run with the mode `auto`, and with `manual`: "the crop script
  published the state off".
- With the state clause off: "the view shows a Crop control". With that
  off too: the crop itself was seen, by itself in `auto`, and after the `C`
  key in `manual`.

On the committed content: 5 of 5 passing runs in each player by the
author, 5 of 5 by the attacking helper, and both full suites on the merged
content.

No check that plain `node --test` can run fails when this fix is taken
out. That part is being added as a follow-up commit before the fix goes to
the reviewer, whose sandbox cannot run the end-to-end suite.

## Coder dispute (if any)

## Known gaps

- Manual cropping in the external player is shown only through a test-only
  route. The parity test therefore prints row 9 in the external player as
  "ONLY VIA TEST-ONLY ROUTES", although Auto and Off use no such route: the
  marking is per scenario and row. The follow-up splits row 9 by mode so
  that the marking falls on Manual alone.
- The embedded Manual leg is the older one and was not broken again.
- Manual and Off in the external player are not checked on a resumed play.
- A play's Stopped check-in once arrived more than 15 s after `quit` in the
  external player (the Manual leg, once in eleven runs, with mpv still
  alive 3 s after `quit`). Not seen in the forty runs since. It may be a
  defect of the product or of mpv; it is not this finding's.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "Row 9 credits external crop modes that are never exercised". Reviewer's severity: MEDIUM.
