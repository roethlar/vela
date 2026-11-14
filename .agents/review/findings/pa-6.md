# pa-6: a run can say it built the app it ran when it built somewhere else and ran an older binary

**Severity**: MEDIUM — the results file records `built` whenever `--skip-build` was not given. The build honours `CARGO_TARGET_DIR` from the environment (and a Cargo configuration's target directory), while the runner always starts `src-tauri/target/debug/vela`. With the build redirected, an older binary at that path is run, recorded as built from the checked-out commit, and accepted by the verdict.
**Status**: In progress
**Branch**: —
**Commit**: `f00cdc4`

## Evidence

Reviewer, verbatim: "tests/e2e/run.mjs:405 records built: !skipBuild. The build inherits CARGO_TARGET_DIR through :59, while :22 and :251 always select an existing src-tauri/target/debug/vela. Cargo metadata confirmed that CARGO_TARGET_DIR=/tmp/vela-review-target redirects the output directory."

## Predicted observable failure

Reviewer, verbatim: "With a custom Cargo target directory and an older binary remaining at the hard-coded path, a full run builds elsewhere but executes the older app. Its results nevertheless claim built: true and the current commit, allowing the verdict to accept evidence from another product build."

Reviewer's suggested direction, verbatim: "Force the build to the directory used by the runner, or resolve and execute the actual build artifact. Record built only after establishing that this is the artifact the scenarios ran."

## What

The defect is as the reviewer describes: `tests/e2e/run.mjs` fixed the app
binary at `src-tauri/target/debug/vela` and recorded `built: !skipBuild`,
while the build honours `CARGO_TARGET_DIR` and a Cargo configuration's
`target-dir`. This is in the rule the coordinating agent added just before
Slice 6 landed.

The helper who attacked the first fix found that it still had the defect in
another form. It asked `cargo metadata` from the repository root, but
`tauri build` runs Cargo in `src-tauri/`, and Cargo reads its configuration
files upwards from the directory it runs in and takes a relative
`CARGO_TARGET_DIR` from there. With a `src-tauri/.cargo/config.toml` naming
a target directory, the build went to one place and the runner started an
older binary from another: a `--skip-build` run of `smoke` with that file
pointing at an empty directory passed.

## Approach

The runner asks Cargo where it builds: `cargo metadata --format-version 1
--no-deps --manifest-path Cargo.toml`, run in `src-tauri/` with a copy of
the environment the build gets. It starts `<target_directory>/debug/<the
one binary target>`, with `--skip-build` too. It prints the path it runs,
records it in the results file as `appBinary`, stops with "cannot tell
where Cargo builds the app" when Cargo cannot say, and says "app binary
missing: <path>, where Cargo builds it" when the file is not there. It
refuses `CARGO_BUILD_TARGET`, which would put the binary under
`<target_directory>/<target>/debug/`.

The step from Cargo's answer to the path is a new module with nothing else
in it, `tests/e2e/app-binary.mjs` (`appBinaryFromMetadata`,
`cargoMetadataCwd`, `METADATA_ARGS`). Asked from the same directory with
the same environment, a relative `CARGO_TARGET_DIR` resolves as the build
resolves it, so nothing is rewritten.

`tests/e2e/README.md` and `docs/development.md` say where the binary is
taken from and what `built: true` in a results file means: the run ran the
build, then started the binary at the path Cargo gave. They also say what
it does not prove.

Made by a helper agent, attacked by a second, repaired by a third,
confirmed by a fourth, and read and landed as one commit by the
coordinating agent, which added one test. The confirming helper had found
that no test failed when the runner asked Cargo and then started a fixed
path anyway. The new test holds the runner's own text to it (below).

## Files changed

- `tests/e2e/app-binary.mjs` (new)
- `tests/e2e/run.mjs`
- `tests/e2e-harness.test.mjs`
- `tests/e2e/README.md`
- `docs/development.md`

## Guard proof

In `tests/e2e-harness.test.mjs`:

- "the app binary is taken from Cargo's target directory": the step from
  an answer to a path, and each refusal (no JSON, no or a relative target
  directory, no or two binary targets, `CARGO_BUILD_TARGET`).
- "Cargo is asked where it builds, and CARGO_TARGET_DIR moves the answer":
  the real Cargo, asked as the runner asks.
- "Cargo is asked from where the build runs it, so src-tauri's own Cargo
  configuration counts": a made-up project laid out as this one, under a
  temporary directory, whose `src-tauri/.cargo/config.toml` names a target
  directory; the answer must follow it, and a relative `CARGO_TARGET_DIR`
  must land under `src-tauri/`.
- "the runner asks Cargo from the directory the build runs it in": the
  runner's text.
- "the runner takes the app binary from Cargo's answer, and starts that
  one" (the coordinating agent's): in the runner's text, `appBinary` is
  declared once and given one value, the one made from Cargo's answer;
  every session is started with it; and the runner names no target
  directory of its own.

The two tests that ask Cargo skip only where `cargo` is not installed.

Red-proofs, each restored afterwards:

- The fixed path back; the `CARGO_BUILD_TARGET` check removed; the
  target-directory check removed: each failed the unit test.
- The question asked from the repository root again, in `app-binary.mjs`:
  the made-up project answered `.../src-tauri/target/debug/madeup` instead
  of `.../configured-target/debug/madeup`. In `run.mjs` only: the test of
  the runner's text failed.
- The coordinating agent, on its own test: Cargo asked and then a fixed
  path given to `appBinary`; a session started with a path of its own; the
  answer thrown away and a path put together from parts. Each failed "the
  runner takes the app binary from Cargo's answer, and starts that one".
- Real runs, external player, `--skip-build smoke`: with
  `src-tauri/.cargo/config.toml` naming an empty directory the run stopped
  with "app binary missing" naming that directory; the same with the
  runner asking from the repository root passed on the old binary (the
  defect); with the file removed the run passed and printed the usual path.

`npm run check` was green at this commit (422 tests).

## Coder dispute (if any)

## Known gaps

- A Cargo configuration file's `build.target` is not detected; only the
  environment's `CARGO_BUILD_TARGET` is. In that setup an older binary at
  `<target directory>/debug/` could still run. The README says so.
- Nothing shows that the binary was not replaced between the build and the
  scenarios.
- No run was made that builds into a moved target directory (it would
  compile everything again). That `tauri build` runs `cargo build` in
  `src-tauri/` with the runner's environment rests on a helper's log of a
  stand-in `cargo`, not on such a run.
- The test of the runner's wiring reads the runner's text. It fails on the
  plain ways of starting another binary; a deliberate way around it (a
  second variable, a path built from character codes) would pass.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `ab95c49..ac3e563`, 2026-11-14 07:27 UTC
(`.agents/review/codereview-2026-11-14-embedded-parity.md`). Reviewer's
title: "The build flag does not establish that the executed binary was built". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 10:29 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d992-1da2-7e31-83de-90f4c776ea0c`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 10:19 UTC, finished 10:28 UTC.
- Reviewed head `f00cdc472623d796e115b474461e81249f738922`, base
  `c294a09eaf95e068d863971074d9c8147984e529` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/app-binary.mjs:44 — Cargo configuration's build.target still reproduces pa-6. A real build wrote target/<triple>/debug/madeup while this helper selected target/debug/madeup; executing them printed NEW and OLD respectively. The runner would still record built: true. Documenting this limitation does not close the finding.
  - tests/e2e/app-binary.mjs:55 — path.join changes filesystem semantics by collapsing '..' across symlinks. With CARGO_TARGET_DIR=/tmp/probe/link/../target and link pointing to physical/nested, Cargo built physical/target/debug/madeup, but the helper selected /tmp/probe/target/debug/madeup. I reproduced execution of an OLD binary while the actual artifact printed NEW. Preserve Cargo's path semantics or consume the actual executable artifact.
  - tests/e2e/app-binary.mjs:49 — Metadata includes every workspace member, so counting all binary targets rejects unrelated helper executables. A two-package workspace successfully built the selected app from src-tauri, but this helper threw 'names 2 binary targets'. The new preflight therefore aborts valid workspace setups; restrict selection to the app package.
  - tests/e2e-harness.test.mjs:117 — The launch regex silently ignores nested calls. Replacing the first launch with newSession(path.resolve(path.join('/tmp', 'old-vela'))) left all five tests passing because the restart remained matched. Removing target-directory variables from the metadata environment also escaped the guards. The original fixed-path injections did fail, and restoration passed, but the wiring guarantee remains incomplete.
  - tests/e2e/README.md:18 — The stated default-path exceptions omit CARGO_BUILD_TARGET_DIR. Both relative and absolute values moved Cargo's output and the helper's selection in real probes without CARGO_TARGET_DIR or target-dir configuration. docs/development.md repeats this omission.
