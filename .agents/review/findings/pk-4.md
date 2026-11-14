# pk-4: a package whose directories above the set cannot be entered still passes

**Severity**: MEDIUM — the check looks at the permissions of `usr/lib/Vela` and below, not at `usr` and `usr/lib`. A package that installs `usr/lib` with mode 0700 passes with 'every user may read every file of the set', and on the machine it is installed on nobody but root can load the library.
**Status**: In progress
**Branch**: —
**Commit**: `2faadcf`

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:234-235 checks usr/lib/Vela and its libmpv subtree, omitting usr and usr/lib. Repacking the real set into a deb with usr/lib mode 0700 still returned RESULT PASS and 'every user may read every file of the set'."

## Predicted observable failure

Reviewer, verbatim: "The checker approves packages whose directory permissions prevent ordinary users from reaching or loading the libraries. Its load succeeds because extraction makes the checking account the owner."

Reviewer's suggested direction, verbatim: "Validate traversal permissions on every packaged directory between the installation root and the set, as well as permissions within the set."

## What

The check looked at the permissions of the set and of the directory it is in, and not at `usr` and `usr/lib` above them. A package that closed one of those passed, and once it was installed nobody but root could have loaded the library.

Reproduced before anything was changed: a package with `usr/lib` at mode 0700 got no `FAIL access` line.

## Approach

One commit, `2faadcf`. A new helper, `not_enterable ROOT DIR`, walks from `usr` down and names the first directory that other users may not enter. The check fails with "access: DIR cannot be entered by every user (other users have '…'), and the set is below it". What is asked of those two directories is entry only (0711 is enough), as for any system directory; the existing rule for `usr/lib/Vela` and for the set itself (read and enter) is unchanged.

Written by one helper agent and attacked by another, which found the finding closed at its root; then landed, read and injected by the coordinating agent.

## Files changed

- `packaging/libmpv/check-package.sh` — `not_enterable`; its call in the access check.
- `tests/libmpv-packaging.test.mjs` — `usr/lib closed`, `usr closed`, `usr/lib readable but not to be entered`, both closed (only the first is named), and 0711 on both passing.
- `docs/development.md`.

## Guard proof

Run 2026-11-14 by the coordinating agent in a disposable worktree at `1348782`, restored from the commit; `node --test tests/libmpv-packaging.test.mjs` (187 tests; unchanged: 186 pass, 1 skipped).

| Injection | Result |
| --- | --- |
| The directories above the set are not looked at (the original defect) | FAIL `usr/lib closed`, `usr closed`, `usr/lib readable but not to be entered`, `usr and usr/lib both closed: the first is named` |

The helpers ran, each alone and each failing its named tests: no ancestor check (4 subtests); every closed directory reported instead of the first (the "both closed" case); read-and-enter asked instead of enter (the 0711 case); read asked instead of enter (the 0744 and 0711 cases).

By hand, on repacked copies of the real opt-in Arch package, with `--set` and a real load where the package passed: `usr/lib` 0700 fails and names `usr/lib`; `usr/lib` 1755 and 4711 pass; `usr/lib` 2700 fails; `usr` 0754 fails and names `usr`; `usr` 0701 passes.

## Coder dispute (if any)

## Known gaps

- The package's own root entry (`./` in a deb) is not judged; the walk starts at `usr`. By reading: neither dpkg nor pacman applies a package's mode to `/`.
- Without `bsdtar` on the machine this finding's tests are skipped (CI's runner does not have it; see `.agents/state.md`, Blockers).

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `2f0d191..4b8293e`, 2026-11-14 01:05 UTC
(`.agents/review/codereview-2026-11-14-libmpv-packaging.md`). Reviewer's
title: "Check permissions on every ancestor of the packaged set". Reviewer's severity: MEDIUM.
