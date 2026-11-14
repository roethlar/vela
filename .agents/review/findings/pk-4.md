# pk-4: a package whose directories above the set cannot be entered still passes

**Severity**: MEDIUM — the check looks at the permissions of `usr/lib/Vela` and below, not at `usr` and `usr/lib`. A package that installs `usr/lib` with mode 0700 passes with 'every user may read every file of the set', and on the machine it is installed on nobody but root can load the library.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:234-235 checks usr/lib/Vela and its libmpv subtree, omitting usr and usr/lib. Repacking the real set into a deb with usr/lib mode 0700 still returned RESULT PASS and 'every user may read every file of the set'."

## Predicted observable failure

Reviewer, verbatim: "The checker approves packages whose directory permissions prevent ordinary users from reaching or loading the libraries. Its load succeeds because extraction makes the checking account the owner."

Reviewer's suggested direction, verbatim: "Validate traversal permissions on every packaged directory between the installation root and the set, as well as permissions within the set."

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
title: "Check permissions on every ancestor of the packaged set". Reviewer's severity: MEDIUM.
