# Plan: Actionable failure when npm is missing at build time

## Status

**DRAFT 2026-11-12. NOT APPROVED. No implementation may start.**

Owner ruling, given 2026-11-12:

- **D1 (install policy): RULED A.** Asked whether `scripts/build.sh` should
  only report a missing Node/npm (Option A) or additionally offer to install
  `npm` through pacman on Arch-family hosts (Option B), the owner answered
  verbatim: "A". `build.sh` stays a reporter; the documented policy "Rust
  and Node are not installed for you" stands. Slices 1-2 are the whole plan.
  Slice 3 is NOT to be implemented; it is kept below only as the record of
  the rejected option.

No ruling remains open. The plan as a whole still needs the owner's explicit
go before any implementation.

## Problem

Observed 2026-11-12 on the CachyOS host `continuum` (`.agents/machines.md`),
at `be82ca6`, with pacman `nodejs` 26.13.1 installed and pacman `npm` absent
(Arch packages them separately):

```
$ bash scripts/build.sh
/bin/sh: line 1: npm: command not found
node:internal/errors:999
  const err = new Error(message);
              ^
Error: Command failed: npm --version
    at genericNodeError (node:internal/errors:999:15)
    ... (about 20 more lines: stack, status, signal, output, pid)
Node.js v26.13.1
```

Cause, read from source:

- `scripts/build.sh:180-186` runs `ensure_linux_prereqs` and a `cargo`
  presence check. Neither looks for `node` or `npm`. Every listed pacman
  package was installed, so both passed silently.
- `scripts/build.sh:190` then runs `node scripts/check-js-toolchain.mjs`.
- `scripts/check-js-toolchain.mjs:50` calls
  `execSync("npm --version", ...)` unguarded. A missing `npm` makes
  `execSync` throw, and the uncaught exception prints the dump above.
- The same file reports an out-of-range toolchain with `throw new Error(...)`
  (lines 56-58), which prints the one useful line buried in a stack trace.

The same script is called by `scripts/build.ps1:282`,
`.github/workflows/ci.yml:59`, and `.github/workflows/release.yml` (lines 35,
90, 418). All of them only depend on its exit status and, on success, nothing
parses its stdout line. (Verified by grep on 2026-11-12: no test or workflow
matches on `expected Node` or `toolchain:`.)

Existing documented policy, `docs/development.md` (Linux prerequisites
paragraph): "Rust and Node are not installed for you." `build.sh` already
follows this for Rust: a missing `cargo` gets one line naming rustup and
exit 1.

## Goal

A build host that lacks `npm` (or `node`), or has one outside the
`package.json` `engines` ranges, gets a short actionable message and a
non-zero exit. No stack trace. No change to what counts as an acceptable
toolchain, to the success output, or to any exit status that is already
non-zero.

## Non-goals

- No change to `package.json` `engines`, `.node-version`, the workflows, or
  `scripts/build.ps1`.
- No version bump: build tooling only; the shipped app does not change
  (precedent: the 2026-10-03 build-health pass changed `build.sh` and stayed
  at 1.0.61).
- No apt or dnf Node/npm installation under any ruling. Those distros'
  packaged Node is below `engines.node` (Ubuntu 26.04 ships Node 22), so
  installing it would fail the range check anyway.

## Slice 1 — `check-js-toolchain.mjs` fails in one line

File: `scripts/check-js-toolchain.mjs`.

1. Add a helper near the top:

   ```js
   function fail(message) {
     console.error(`toolchain: ${message}`);
     process.exit(1);
   }
   ```

2. Replace the unguarded `execSync` (line 50) with a guarded call. Any
   failure to run `npm --version` (not found, non-zero exit, spawn error on
   any OS) is the same condition. Pass `stdio: ["ignore", "pipe", "ignore"]`
   so the shell's own `npm: command not found` line is not printed as well:

   ```js
   let npmVersion;
   try {
     npmVersion = execSync("npm --version", {
       encoding: "utf8",
       stdio: ["ignore", "pipe", "ignore"],
     }).trim();
   } catch {
     fail(
       `npm was not found or could not run; install npm ${npmRange} ` +
         `(package.json engines) and put it on PATH`,
     );
   }
   ```

3. Replace the range-mismatch `throw new Error(...)` (lines 56-58) with
   `fail(...)` carrying the SAME text
   (`expected Node ${nodeRange}/npm ${npmRange}; got ${process.version}/npm ${npmVersion}`).

4. Leave the three other `throw`s (missing engines, unparseable version,
   unsupported comparator) as throws: they are repo-defect assertions, not
   host conditions.

5. Success path unchanged: prints `toolchain: Node <v>/npm <v>`, exit 0.

Tests: new file `tests/js-toolchain-check.test.mjs`, added to the
`node --test` list in the `package.json` `check` script (the list is
explicit; a file not named there does not run under `npm run check`).
Normalize nothing; assert on stderr text only.

- T1 "missing npm is one actionable line": spawn
  `process.execPath scripts/check-js-toolchain.mjs` with `env.PATH` set to a
  fresh empty temp directory (on win32 also pass through `SystemRoot` and
  `ComSpec`). Assert exit status 1; stderr matches
  `/^toolchain: npm was not found or could not run; install npm \S+/`;
  stderr does NOT match `/\n\s+at /` and does NOT contain `node:internal`.
- T2 "out-of-range npm is one line": temp directory containing a fake `npm`
  that prints `1.0.0` (`#!/bin/sh` script; skip on win32 with the existing
  `posixBashUnavailable`-style reason), `env.PATH` = that directory only.
  Assert exit status 1; stderr matches
  `/^toolchain: expected Node .*; got v\S+\/npm 1\.0\.0\n$/`; no `\n\s+at `.
- T3 "in-range toolchain still passes": fake `npm` printing `99.0.0`
  (satisfies `>=12` for any future floor below 99). Assert exit status 0 and
  stdout matches `/^toolchain: Node v\S+\/npm 99\.0\.0\n$/`. This runs on the
  suite's own Node, which must already be in range for `npm run check` to be
  meaningful; if it is not, T3 fails loudly, which is correct.

Red-proofs (each separately; restore from the committed state with
`git checkout -- <file>`, never from a file backup):

- R1: remove the `try/catch` (restore the bare `execSync`) → T1 must fail on
  the stack-trace assertion.
- R2: change the mismatch `fail(...)` back to `throw new Error(...)` → T2
  must fail on the stack-trace assertion.
- R3: make `fail` exit 0 → T1 and T2 must fail on exit status.
- R4: make the success path `process.exit(1)` → T3 must fail.

Commit: `build: report a missing or out-of-range npm in one line`.

## Slice 2 — `build.sh` names a missing Node/npm before the toolchain check

File: `scripts/build.sh`. Without `node` on PATH, line 190 fails with
bash's bare `node: command not found`; Slice 1 cannot help because Node
never starts.

1. Immediately before the `node scripts/check-js-toolchain.mjs` line, and
   outside the `if [ "$os" = linux ]` block (all three OS branches reach
   line 190), add:

   ```bash
   for tool in node npm; do
     command -v "$tool" >/dev/null 2>&1 || {
       echo "$tool is not installed (not on PATH); install a Node and npm inside the package.json engines ranges — see docs/development.md" >&2
       exit 1
     }
   done
   ```

2. Do not reorder `ensure_linux_prereqs`: it already runs first.

Tests, in `tests/build-script.test.mjs`, reusing `linuxPrereqFixture` with
`fakeLinuxPackageTools` given no `missingPattern` (extend the fixture to take
options rather than duplicating it). The fixture prepends `fakeBin` to the
real PATH, so the real `node`/`npm` would be found; for these tests run with
`PATH` = `fakeBin` plus a second temp directory holding symlinks to only the
coreutils the script needs before line 190 (`bash`, `dirname`, `grep`,
`uname` is already faked). Determine the exact list by running the test and
reading the `command not found` output; record the final list in a comment.

- T4 "no npm on PATH": fixture without the fake `npm`. Assert non-zero exit,
  stderr matches `/^npm is not installed \(not on PATH\)/m`, call log empty.
- T5 "no node on PATH": fixture without the fake `node`. Assert non-zero
  exit, stderr matches `/^node is not installed \(not on PATH\)/m`.
- The two existing prerequisite tests must stay green unmodified in their
  assertions.

Red-proofs:

- R5: delete the new loop → T4 and T5 must both fail for the missing-message
  reason (not for a fixture error).
- R6: reduce the loop to `for tool in node` → T4 alone must fail.
- R7 (wiring): move the loop below `npm install`/build → T4 must fail because
  the call log is no longer empty or the message is absent.

Docs: `docs/development.md`, Linux prerequisites paragraph — after "Rust and
Node are not installed for you." add one sentence that a missing `node` or
`npm` is reported before anything builds. No other doc changes.

Commit: `build: name a missing node or npm before the toolchain check`.

## Slice 3 — REJECTED (D1 ruled A, 2026-11-12): offer `npm` through pacman

Do not implement. Record of what Option B would have been:

- In `ensure_linux_prereqs`, pacman branch only: when
  `command -v npm` fails, append `npm` to `missing` (pacman's `npm` depends
  on `nodejs`, so one package covers both). Test by PATH presence, NOT by
  `pacman -Q npm`: a host whose npm comes from nvm/fnm/mise has a working
  `npm` and no pacman package, and must not be prompted.
- Slice 2's loop stays as the backstop for apt/dnf/macOS/Windows and for a
  declined prompt.
- Change the `docs/development.md` sentence to say Node/npm are offered on
  Arch-family hosts only.
- Tests: pacman-only fixture (hide the fake `apt-get`/`dnf`), no `npm` on
  PATH → stderr names `npm` under "Missing build prerequisites", and with
  `--install-deps` the call log's first line is
  `sudo pacman -S --needed npm`. A second test with an `npm` on PATH that
  pacman reports as not installed → no prompt. Red-proof each separately.
- Known cost: pacman's `nodejs` tracks the newest Node. When it reaches 27
  it leaves `engines.node` (`<27`) and the offered install produces a
  toolchain the Slice 1 check then rejects; the list would need to move to a
  `nodejs-lts-*` package at that point.

## Verification before claiming completion

From the repo root, on a host inside the `engines` ranges:

- `node scripts/check-js-toolchain.mjs`
- `npm ci`
- `npm run check`
- `node --test tests/`
- `git diff --check`

Rust commands are not required: nothing under `src-tauri/` changes. Say so
in the completion report rather than implying they ran.

Manual acceptance on an Arch-family host (the original repro): with `npm`
temporarily off PATH (`env PATH=<dir without npm> bash scripts/build.sh`, do
not uninstall the package), the output is the single Slice 2 line and exit 1.

## Paperwork on landing

- `.agents/machines.md`, CachyOS section: replace the "Unfixed" sentence
  about `build.sh` with the landed commit ids.
- `.agents/state.md` "Now": one bullet; remove this plan from the open list
  when the owner confirms.
- This file's Status line: record the owner's D1 wording and the go verbatim.
