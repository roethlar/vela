# pk-1: checking an AppImage runs it

**Severity**: HIGH — `check-package.sh` asks an AppImage to unpack itself, which runs the program inside the file. Whoever checks an AppImage they did not build runs whatever it contains, with their own rights, even when only asking whether the set is absent. On the path `scripts/build.sh` takes, the AppImage is the builder's own.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:185-187 copies the supplied AppImage, makes it executable, and runs it. A harmless shell script named untrusted.AppImage wrote an external marker and created squashfs-root/usr/bin/vela; --without-libmpv returned RESULT PASS."

## Predicted observable failure

Reviewer, verbatim: "Checking a package executes arbitrary package-controlled commands with the builder's privileges, even in absence-checking mode. Removing APPIMAGE_EXTRACT_AND_RUN does not prevent this."

Reviewer's suggested direction, verbatim: "Extract the AppImage filesystem using a trusted external parser without invoking its runtime. Reject unsupported formats before executing anything from them."

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
title: "Extract AppImages without executing the package". Reviewer's severity: HIGH.
