# pk-5: the licence copies of an Arch package may be links out of the package, or unreadable, and pass

**Severity**: MEDIUM — the licence texts under `usr/share/licenses/vela/libmpv` are compared by content only. Links to files outside the package pass (and dangle once installed), and so do copies no ordinary user can read. The licence texts are a condition of shipping the set.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:293 only compares licence contents with cmp, which follows symlinks. Replacing all Arch licence copies with absolute symlinks to the external reference directory still returned RESULT PASS. A licence copy with mode 0600 also passed."

## Predicted observable failure

Reviewer, verbatim: "An approved Arch package can have dangling licence links on the destination machine, or licence texts ordinary users cannot read, under usr/share/licenses/vela/libmpv."

Reviewer's suggested direction, verbatim: "Before comparing contents, verify that each licence copy and its ancestors remain inside the package and are accessible to ordinary users. Reject external symlinks."

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
title: "Validate containment and accessibility of Arch licence copies". Reviewer's severity: MEDIUM.
