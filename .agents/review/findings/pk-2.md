# pk-2: the libraries of a set that has failed its checks are loaded anyway

**Severity**: HIGH — `check-libmpv.py` goes on to load `libmpv.so.2` after the set has failed its manifest check, so a package whose library is a link to somewhere else, or has been altered, gets its code run (a library's start-up code runs on load) before the check says FAIL.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:270 invokes check-libmpv.py, whose lines 207-217 continue to ctypes.CDLL after manifest validation fails. Replacing the packaged libmpv.so.2 with a symlink to the external reference set produced manifest failures, followed by a successful load and an isolation failure naming the external library."

## Predicted observable failure

Reviewer, verbatim: "The checker loads libraries outside the unpacked package and can execute modified library constructors before reporting failure. Its eventual rejection does not undo that execution."

Reviewer's suggested direction, verbatim: "Complete source, checksum, and containment validation before loading any library. Abort the load phase immediately when those checks fail."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `2f0d191..4b8293e`, 2026-11-14 01:05 UTC
(`.agents/review/codereview-2026-11-14-libmpv-packaging.md`). Reviewer's
title: "Reject invalid sets before loading their libraries". Reviewer's severity: HIGH.
