# pw-1: the copy is chosen for one screen and the window can open on another

**Severity**: MEDIUM — with the Prefer Compatible source policy, the screen a copy is ranked for is read before the stream is resolved, and the screen the window opens on is read again after it. If the previous mpv window changes screen, or a late report of its screen arrives, in between, the play opens on the new screen with a copy chosen for the old one: the wrong resolution or HDR choice for that play.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/commands.rs:4912 samples the display for ranking at :5189, before stream resolution. Window placement samples it again at :5446. If the previous mpv moves screens or delivers a late display event during resolution, these snapshots disagree. A temporary Rust probe reproduced selection of 2160p for DP-1 followed by placement on 1080p HDMI-1."

## Predicted observable failure

Reviewer, verbatim: "A manual replacement opens on the newly observed screen with a copy ranked for the old screen, producing an inappropriate resolution or HDR choice and potentially unnecessary bandwidth use."

Reviewer's suggested direction, verbatim: "Compare the ranking target with the post-resolution display snapshot and reselect/re-resolve when it changes. Keep window inheritance sampling after resolution."

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
title: "Revalidate the compatibility display after stream resolution". Reviewer's severity: MEDIUM.
