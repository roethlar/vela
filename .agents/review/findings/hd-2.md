# hd-2: a profile mpv applies by itself when a file loads can undo the HDR target

**Severity**: MEDIUM — mpv then draws something other than what the surface is tagged as, which is wrong brightness and colour for the whole play. It needs own-configuration mode and a protocol or extension profile in the user's mpv.conf that sets one of the three target options.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "session.rs:284-292 sets and tags the HDR target before `loadfile` at line 315. mpv applies automatic protocol/extension profiles during file loading. A user profile such as `[protocol.https]` with `target-trc=bt.1886` therefore overrides PQ after tagging. A headless libmpv test using the equivalent `[protocol.memory]` profile reproduced this override; no profile-contained include was involved."

## Predicted observable failure

Reviewer, verbatim: "mpv renders SDR-encoded pixels onto a surface still tagged PQ, producing visibly incorrect brightness and colour. With no compositor change, Vela never restores agreement."

Reviewer's suggested direction, verbatim: "Apply and verify the protected targets after automatic file profiles execute but before presenting video, for example through a load hook. Keep subsequent target changes coordinated with the surface description."

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
title: "Apply protected HDR targets after automatic file profiles". Reviewer's severity: MEDIUM.
