# hd-3: the drawing thread waits for mpv each time it sets the HDR target

**Severity**: MEDIUM — the picture freezes for about a second when the display changes during a play (three settings, each waiting out one of mpv's timeouts).
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "libmpv.rs:423 calls synchronous `mpv_set_property_string` through `follow_colour` in session.rs:356-361, on the thread responsible for rendering. `/usr/include/mpv/render.h` explicitly warns against this dependency. A headless software-renderer test of this setter with a pending frame blocked for approximately 399 ms."

## Predicted observable failure

Reviewer, verbatim: "A brightness change, monitor move or HDR toggle can stall rendering while mpv waits for that same thread to render, causing dropped frames and visible playback freezes until mpv's timeout releases the wait."

Reviewer's suggested direction, verbatim: "Use asynchronous property updates or a separate control thread while continuing to service rendering. Gate presentation until the updates complete so the frame and surface description still change together."

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
title: "Avoid synchronous property writes on the render thread". Reviewer's severity: MEDIUM.
