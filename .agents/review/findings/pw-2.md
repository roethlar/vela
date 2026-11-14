# pw-2: a play in the embedded player is ranked for the previous mpv window's screen

**Severity**: MEDIUM — the screen for Prefer Compatible is taken from the last external play's record whatever player the coming play will use. A play that will run in the embedded player is drawn in Vela's window, on Vela's screen, and gets a copy chosen for the screen mpv's window was on. Before this work such a play was ranked for Vela's screen. The embedded player is hidden, so only someone who has switched it on by hand can meet this.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:4893 accepts every previous external-player record without considering the upcoming player, which is resolved later at :5389. In a run initialized for embedding, switch the player setting from external playback on DP-1 back to embedded playback with Vela on HDMI-1. Ranking retains DP-1, while src-tauri/src/player/launch.rs:913 correctly omits external placement options. A temporary Rust probe confirmed this mismatch."

## Predicted observable failure

Reviewer, verbatim: "The embedded play renders on Vela's screen but Prefer Compatible selects resolution/HDR for the previous external window's screen. This regresses manual embedded launches, which previously used Vela's display."

Reviewer's suggested direction, verbatim: "Use Vela's display for an upcoming embedded play and inherit the external display only for an upcoming external play. Reconsider the target if fallback changes the effective player."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 1 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "Choose the compatibility display using the upcoming player". Reviewer's severity: MEDIUM.
