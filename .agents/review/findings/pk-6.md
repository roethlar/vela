# pk-6: a temporary directory with a space in its name fails a good package

**Severity**: MEDIUM — with `TMPDIR` set to a path containing a space, the isolation step reads the wrong part of each line of `/proc/self/maps` and reports the set's own libraries as foreign. A correct package fails, and with it an opt-in build.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:154 honors TMPDIR and passes the extracted path to check-libmpv.py at line 270. That helper parses /proc/self/maps using line.split()[-1] at line 338. With TMPDIR containing spaces, the unchanged real Arch package failed isolation because the path prefix was discarded."

## Predicted observable failure

Reviewer, verbatim: "A valid package is rejected, and an opt-in build fails, solely because the builder's temporary directory contains spaces."

Reviewer's suggested direction, verbatim: "Parse the fixed fields of /proc/self/maps while preserving the complete pathname remainder. Exercise the package checker with a temporary directory containing spaces."

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
title: "Handle spaces in the temporary library path". Reviewer's severity: MEDIUM.
