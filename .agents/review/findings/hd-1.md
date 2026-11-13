# hd-1: an old description of the output can replace a newer one

**Severity**: MEDIUM — a change of the display during a play (HDR switched off, the brightness moved) is then missed until the next change, and the picture stays drawn for the display as it was. It needs a compositor that answers an earlier question after acknowledging a later one; KWin does not.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "surface.rs:202-208 replaces the shared `arriving` accumulator, while surface.rs:222-251 ignores which information object sent each event. Destroying the previous description at surface.rs:583-586 does not destroy its information child. A socket-pair test using the real bindings confirmed that old information arriving after the replacement's Ready consumed the accumulator: the result remained reference/max 265/465, and the subsequent 203/203 reply was discarded."

## Predicted observable failure

Reviewer, verbatim: "An output change is lost until another notification. HDR can remain selected after it is disabled, or playback can retain the previous brightness and tone-mapping target."

Reviewer's suggested direction, verbatim: "Give each information object a query generation and its own accumulator. Publish only the current generation's completed description; ignore obsolete replies."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `74f44b6..e8db400`, 2026-11-13 08:38 UTC
(`.agents/review/codereview-2026-11-13-embedded-hdr.md`). Reviewer's
title: "Associate information replies with their output query". Reviewer's severity: MEDIUM.
