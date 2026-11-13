# ec-6: an embedded start that fails after the checks passed fails the play instead of falling back

**Severity**: MEDIUM — the user gets an error instead of a film, once; decision D3 says a play is never failed over the embedded player.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/egl.rs:166-173 checks available formats without creating the required OpenGL 3.3 context. If context or renderer creation subsequently fails, host.rs:375-381 only marks future starts unavailable, and playback.rs:376 propagates the error. No external launch is attempted for that play."

## Predicted observable failure

Reviewer, verbatim: "On a machine that passes the preliminary probe but cannot create the renderer, the first Play fails outright; only a later Play falls back. This contradicts D3 and the new development documentation's fallback promise."

Reviewer's suggested direction, verbatim: "After confirming complete cleanup of the failed embedded start, rebuild the launch for the external player and complete the original play with its fallback notice."

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
title: "An embedded setup failure does not fall back for the current play". Reviewer's severity: MEDIUM.
