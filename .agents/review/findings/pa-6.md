# pa-6: a run can say it built the app it ran when it built somewhere else and ran an older binary

**Severity**: MEDIUM — the results file records `built` whenever `--skip-build` was not given. The build honours `CARGO_TARGET_DIR` from the environment (and a Cargo configuration's target directory), while the runner always starts `src-tauri/target/debug/vela`. With the build redirected, an older binary at that path is run, recorded as built from the checked-out commit, and accepted by the verdict.
**Status**: In progress
**Branch**: —
**Commit**: `f00cdc4`, `8cf3a05` (repair)

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

Orchestrator: a valid verdict, reopened on five comments, and all five
are right. The first fix worked out where the build would put the binary.
Every rule of Cargo's that it did not rebuild was a way to start an older
binary and still record `built: true`.

### Repair (round 2), `8cf3a05`

The runner no longer works the path out for a run that builds.

- **A run that builds.** The build is asked for Cargo's own report: `npm
  run tauri -- build --debug --no-bundle --
  --message-format=json-render-diagnostics`. The Tauri CLI (2.11.4) passes
  the argument on to its one `cargo build`, in `src-tauri/`, with the
  front-end build and the `custom-protocol` feature unchanged; a logging
  stand-in for `cargo` showed the call. Cargo prints a message for each
  executable it produced or found fresh, with its path as Cargo itself
  resolved it. The app is the executable of a binary target of the package
  whose manifest is `src-tauri/Cargo.toml`; its path is used as printed.
  No such message, more than one executable, a relative path, or no
  "build-finished" message stops the run, and so does a build that fails,
  also after Cargo has named the app.
- **`built: true`** in a results file now means: the run ran the build and
  started the executable Cargo reported for it. The file records the path
  (`appBinary`) and where it came from (`appBinaryFrom`).
- **A run with `--skip-build`** builds nothing, so nothing reports. It
  still takes the place from `cargo metadata`, asked in `src-tauri/` with
  the same environment: the app's own package only, its one binary, and
  the path put together with a separator and nothing that changes what a
  path with `..` means. It is recorded `built: false`, which the verdict
  refuses. It does not follow a configuration file's `build.target`; the
  README says so.
- **One place.** `locateAppBinary` in `tests/e2e/app-binary.mjs` does all
  of this, and the runner calls it once, with its own environment for the
  build and for any question to Cargo alike. The driver is made with the
  result and takes no path: `newSession()` starts that app and nothing
  else, and refuses a file that is no longer the one found (its device,
  inode, size and time of change are noted when it is found).

Where each comment is held, in `tests/e2e-harness.test.mjs` (the tests of
this finding carry "(pa-6)" in their names; there are fifteen):

| Comment | Held by |
| --- | --- |
| 1, a configuration file's `build.target` | a real `cargo build` of a made-up crate with that setting and an "OLD" binary planted where the first fix looked: the path returned must print "NEW" |
| 2, `..` through a symbolic link | the same with `CARGO_TARGET_DIR` set through a link: the path is Cargo's, unchanged |
| 3, a workspace with another binary | a two-package workspace: the app's executable is taken, the other's does not stop the run |
| 4, the wiring | the runner's text, by counting: every mention of `newSession` must be the one accepted call; one call of `locateAppBinary`; every use of the path one of the accepted forms; no `built` of the runner's own; the driver sends the path it was made with. And at run time the driver refuses a path given to `newSession` |
| 5, `CARGO_BUILD_TARGET_DIR` | real builds with it relative and absolute; the README no longer lists what moves the default place for a run that builds, because nothing has to be listed |

How it was made: a helper agent (Claude Opus 5.5) wrote the repair on a
local branch (three commits), a second attacked it and found five things,
and a third repaired them (five commits): a parameter that shadowed the
path and started another one; a session started through a computed name,
another driver module or a scenario's own call; a build that failed after
naming the app and was still taken; parts of the checks that could be
changed with every test green; and the reported file overwritten before
the session. The third helper's own red-proofs then found two of its new
guards empty (the inode and the size parts of the identity; a stand-in
build whose output did not arrive in the sandbox) and fixed both. The
coordinating agent read the three source files and landed the eight
commits as one.

Red-proofs, each restored from the commit afterwards: the build path made
to return the prediction again (the `build.target` test failed with "is
not the binary the build wrote"); a `--skip-build` run recorded as built;
the exit status of the build ignored; the manifest match taken out; each
accepted form of the runner's text broken in turn; a second `newSession`
with a path; each part of the file's identity taken out on its own.

Real runs, external player: a built run of `smoke` printed "e2e: running
.../src-tauri/target/debug/vela (from cargo's report of this run's
build)", passed, and its results file says `built: true` with that path;
a built run with `CARGO_TARGET_DIR` moved compiled the whole app there
and started that binary; `--skip-build smoke` passed and was recorded as
not built; `--skip-build` with a `src-tauri/.cargo/config.toml` naming an
empty directory stopped with "app binary missing" naming it.

Checks: `npm run check` green before the commit (472 tests). The fifteen
tests pass inside `codex sandbox` in workspace-write mode. Both full
end-to-end suites are being run at `8cf3a05`, since the runner changed; the
result goes into the plan's log.

Not covered:

- `--skip-build` does not follow a configuration file's `build.target`.
- The file's identity is its device, inode, size and time of change, not
  its contents: a replacement of the same size and time, one made between
  Cargo writing the file and the note, and one made between the check and
  the start are not seen. The README says so.
- That `tauri build` passes the message format on rests on what
  tauri-cli 2.11.4 was seen to do; a CLI that drops it stops the run ("the
  build printed no report from Cargo").
- The text test of the runner cannot catch a deliberate way round it that
  uses none of the words it counts.

### Verification round 2 (repair-delta redispatch), recorded 2026-11-14 12:50 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1da0f-dcf4-7d20-87e8-3509a297116a`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 12:37 UTC, finished 12:46 UTC.
- Reviewed head `8cf3a056fbf8577df8ed9062aa89c8289a34e98c`, base
  `9e005c449b9e377986a0eece38b543e7685a03e5` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - tests/e2e/app-binary.mjs:111 — Any JSON object with a string reason is swallowed as Cargo output. A non-Cargo diagnostic with reason: 'dependency-warning', emitted before a real Cargo build, disappeared from terminal output. Recognize Cargo message schemas and echo unmatched JSON; the existing output test only covers JSON without reason.
  - tests/e2e-harness.test.mjs:410 — The wiring guard still permits another executable. I added a helper importing Driver as Client, constructed it with an OLD binary, and shadowed driver inside runScenario's existing try block. Both accepted newSession calls then used that client, while all fifteen tests passed. A stubbed fetch confirmed the alternate application path. The guard checks spellings rather than the binding actually used.
