# pk-3: a plain Arch build can put an older package, with the set in it, into dist/

**Severity**: MEDIUM — after an opt-in build and a version change, `scripts/build.sh` without the flag copies every package it finds in `packaging/arch/` to `dist/`, the older one with the whole set included. A build that was not asked for the set then hands one out.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "scripts/build.sh:338 restricts makepkg --packagelist collection to opt-in builds; lines 355-364 collect every matching older package for default builds. Using the build fixture with an earlier real opt-in package in packaging/arch, a build without the flag exited 0 and copied that package into dist/."

## Predicted observable failure

Reviewer, verbatim: "After an opt-in build and a version change, a default build distributes an older package containing the complete library set despite the flag being absent."

Reviewer's suggested direction, verbatim: "Collect only the current makepkg outputs for default Arch builds too. Add a regression case with an earlier opt-in package beside the current output."

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
title: "Keep earlier opt-in Arch packages out of default-build output". Reviewer's severity: MEDIUM.
