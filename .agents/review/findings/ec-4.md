# ec-4: an include inside a profile the user selects is applied after Vela's protected options

**Severity**: MEDIUM — the user's file can then set `idle`, after which mpv never ends at the end of a file and the play's final bookkeeping and Continue Playing never run. It needs own-configuration mode, a profile containing an include, and that profile selected in the Advanced options.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/options.rs:101-108 separates direct include options but batches profile selection with Vela's protected options. With own mpv.conf defining [example] containing include=/path/override.conf, Advanced --profile=example, and override.conf containing idle=yes, a headless libmpv 0.42 probe of the same include sequence returned idle=yes while vo and input-ipc-server retained Vela's values."

## Predicted observable failure

Reviewer, verbatim: "The startup verification passes, but EOF leaves mpv running indefinitely. Final playback bookkeeping and continuation do not complete."

Reviewer's suggested direction, verbatim: "Finish applying user options and their nested profiles before applying Vela's protected tail in a separate step. Test the effective values through libmpv, including profile-contained includes."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "Includes inside user profiles override the protected option tail". Reviewer's severity: MEDIUM.
