# pk-3: a plain Arch build can put an older package, with the set in it, into dist/

**Severity**: MEDIUM — after an opt-in build and a version change, `scripts/build.sh` without the flag copies every package it finds in `packaging/arch/` to `dist/`, the older one with the whole set included. A build that was not asked for the set then hands one out.
**Status**: Verified
**Branch**: —
**Commit**: `26910e5`, `07ebee4`

## Evidence

Reviewer, verbatim: "scripts/build.sh:338 restricts makepkg --packagelist collection to opt-in builds; lines 355-364 collect every matching older package for default builds. Using the build fixture with an earlier real opt-in package in packaging/arch, a build without the flag exited 0 and copied that package into dist/."

## Predicted observable failure

Reviewer, verbatim: "After an opt-in build and a version change, a default build distributes an older package containing the complete library set despite the flag being absent."

Reviewer's suggested direction, verbatim: "Collect only the current makepkg outputs for default Arch builds too. Add a regression case with an earlier opt-in package beside the current output."

## What

A build without `--with-libmpv` on Arch copied every package file it found beside the PKGBUILD into `dist/`. An earlier package left there, one built WITH the library set, was handed out again by a build that was meant to have none.

Reproduced before anything was changed, with the build's test fixture: a default Arch build with a `vela-0.9.0` package (with the set) beside the PKGBUILD ended with both `vela-0.9.0-1-x86_64.pkg.tar.zst` and `vela-1.0.0-1-x86_64.pkg.tar.zst` in `dist/`, and `makepkg` was never asked what it had made.

## Approach

Two commits: the fix (`26910e5`), and what the attack on it found (`07ebee4`).

- **Every Arch build takes exactly what `makepkg --packagelist` names** (`26910e5`), with or without the flag, and fails when a named file is not there. Before, only the opt-in build did. Builds that are not Arch are unchanged.
- **A build whose `makepkg` cannot name its package fails** (`07ebee4`). After the first commit such a build handed out nothing and still reported success; a release script that trusts the exit status would have gone on with an empty `dist/`. It now exits 1 with a message that says so, both when `makepkg --packagelist` fails and when it names nothing.

Written by one helper agent, attacked by another, repaired by a third; then landed, read and injected by the coordinating agent.

## Files changed

- `scripts/build.sh` — the Arch branch of collecting what was built.
- `tests/libmpv-packaging.test.mjs` — the `build.sh` fixture: an older package beside the new one; `PKGDEST` elsewhere; a named package missing; `makepkg` failing, and naming nothing.
- `docs/development.md`.

## Guard proof

Run 2026-11-14 by the coordinating agent in a disposable worktree at `1348782`, one injection at a time, restored from the commit each time; `node --test tests/libmpv-packaging.test.mjs` (187 tests; unchanged: 186 pass, 1 skipped).

| Injection | Result |
| --- | --- |
| Every package file beside the PKGBUILD is taken again (the original defect) | FAIL, 12 tests, among them `beside this build's package`, `a default Arch build hands out the package it made, and not an earlier one with the set` and `an older package beside the new one is neither checked nor copied` |
| A build whose `makepkg` names nothing reports success | FAIL `makepkg cannot say which package it made: nothing is handed out, and that is a failure` and `makepkg cannot say which package it made: nothing is checked, and that is a failure` |

The helpers ran, each alone and each failing its named tests: the old condition put back (6 tests; `dist/` holds `vela-0.9.0-1-x86_64.pkg.tar.zst`); `build.sh` as it was before the second commit (`nothing matched`, status 0); and a version that fails only when `makepkg` itself fails (the names-nothing case still fails, so each of the two cases bites on its own).

## Coder dispute (if any)

## Known gaps

- **Not run against a real `makepkg` build**: the tests drive `build.sh` with a stand-in. By hand, the real `makepkg --packagelist` in `packaging/arch` prints one absolute path, and neither the PKGBUILD nor this machine's `makepkg.conf` makes a debug package.
- The older `find` branch for Arch in `build.sh` can no longer be reached; it was left, to keep the commit to its finding.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `2f0d191..4b8293e`, 2026-11-14 01:05 UTC
(`.agents/review/codereview-2026-11-14-libmpv-packaging.md`). Reviewer's
title: "Keep earlier opt-in Arch packages out of default-build output". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 03:07 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d7f1-5275-7f42-bdaa-3e556aafe3ec`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 02:44 UTC, finished 02:51 UTC.
- Reviewed head `13487828af78d0e8a5a4c1ae181efe8dc3981da7`, base
  `8e6299e6403d7467705206c691ab7e50099dbf9c` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
