# pa-6: a run can say it built the app it ran when it built somewhere else and ran an older binary

**Severity**: MEDIUM — the results file records `built` whenever `--skip-build` was not given. The build honours `CARGO_TARGET_DIR` from the environment (and a Cargo configuration's target directory), while the runner always starts `src-tauri/target/debug/vela`. With the build redirected, an older binary at that path is run, recorded as built from the checked-out commit, and accepted by the verdict.
**Status**: Open
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "tests/e2e/run.mjs:405 records built: !skipBuild. The build inherits CARGO_TARGET_DIR through :59, while :22 and :251 always select an existing src-tauri/target/debug/vela. Cargo metadata confirmed that CARGO_TARGET_DIR=/tmp/vela-review-target redirects the output directory."

## Predicted observable failure

Reviewer, verbatim: "With a custom Cargo target directory and an older binary remaining at the hard-coded path, a full run builds elsewhere but executes the older app. Its results nevertheless claim built: true and the current commit, allowing the verdict to accept evidence from another product build."

Reviewer's suggested direction, verbatim: "Force the build to the directory used by the runner, or resolve and execute the actual build artifact. Record built only after establishing that this is the artifact the scenarios ran."

## What

## Approach

## Files changed

## Guard proof

## Coder dispute (if any)

## Known gaps

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "The build flag does not establish that the executed binary was built". Reviewer's severity: MEDIUM.
