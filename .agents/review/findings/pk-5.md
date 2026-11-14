# pk-5: the licence copies of an Arch package may be links out of the package, or unreadable, and pass

**Severity**: MEDIUM — the licence texts under `usr/share/licenses/vela/libmpv` are compared by content only. Links to files outside the package pass (and dangle once installed), and so do copies no ordinary user can read. The licence texts are a condition of shipping the set.
**Status**: In progress
**Branch**: —
**Commit**: `8876601` (and `547726c`)

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:293 only compares licence contents with cmp, which follows symlinks. Replacing all Arch licence copies with absolute symlinks to the external reference directory still returned RESULT PASS. A licence copy with mode 0600 also passed."

## Predicted observable failure

Reviewer, verbatim: "An approved Arch package can have dangling licence links on the destination machine, or licence texts ordinary users cannot read, under usr/share/licenses/vela/libmpv."

Reviewer's suggested direction, verbatim: "Before comparing contents, verify that each licence copy and its ancestors remain inside the package and are accessible to ordinary users. Reject external symlinks."

## What

An Arch package keeps a second copy of every licence text of the set under `usr/share/licenses/vela/`. The check compared those copies with the set's texts by content only. A copy that was a link out of the package passed (on the user's machine it leads wherever it points there, or nowhere), and so did a copy, or a directory of copies, that no ordinary user can read.

Reproduced before anything was changed, by seven new tests that all found no `FAIL licences` line: a link out by absolute path, one by relative path, a link to the set's own copy, a directory of copies that is a link out, a copy at mode 0600, a directory of copies at 0700, and a closed `usr/share/licenses`.

## Approach

One commit for the finding, `8876601`. A second commit, `547726c`, is a defect the attack found that is older than the fix; it is described at the end and verified with this finding.

- **Each licence copy has to be a file of the package itself, that every user can read** (`8876601`). A copy that is a link, or is reached through one (its real path is not its path inside the package), fails with "is a symbolic link, or is reached through one; a licence text must be a file of the package itself". The directories above a copy must be ones every user may enter; the first closed one is named, once per package. The comparison with the set's text is made only on a regular file. Then the copy itself must be readable by other users.
- **A program reached through a link out of the package is not the package's** (`547726c`; found by the attack, older than the fix). The check decides that a file is a package of Vela by finding `usr/bin/vela` in it, and tested only the last part of that path for being a link. A package that was nothing but a link `usr -> /usr` therefore counted as Vela on a machine that has Vela installed, and passed the check that the set is absent. The program's real path must now be its path inside the package.

Written by one helper agent, attacked by another, repaired by a third; then landed, read and injected by the coordinating agent.

## Files changed

- `packaging/libmpv/check-package.sh` — the licence loop; `not_enterable` stops quietly at a directory the package does not have (a missing copy is still reported as missing); `of_vela`.
- `tests/libmpv-packaging.test.mjs` — the seven `a licence copy: …` cases; `a package in which a directory above the program is a link to somewhere else` (pacman and rpm, both modes, linked to a directory the test makes).
- `docs/development.md`.

## Guard proof

Run 2026-11-14 by the coordinating agent in a disposable worktree at `1348782`, one injection at a time, restored from the commit each time; `node --test tests/libmpv-packaging.test.mjs` (187 tests; unchanged: 186 pass, 1 skipped).

| Injection | Result |
| --- | --- |
| A licence copy that is a link, or reached through one, is not refused | FAIL `a licence copy: a link out of the package, by absolute path`, `… by a relative path`, `a licence copy: a link to the set's own copy, inside the package`, `a licence copy: a directory of copies that is a link out of the package` |
| A copy other users cannot read passes | FAIL `a licence copy: a copy only its owner may read` |
| A closed directory above a copy passes | FAIL `a licence copy: a directory of copies that other users may not enter`, `a licence copy: the directory of all of them closed: said once` |
| The link-out-of-the-package commit taken out (reverse-applied) | FAIL `a package in which a directory above the program is a link to somewhere else` |

The helpers ran, each alone and each failing its named tests: no link check (the 4 link cases); the link test without the real-path comparison (the directory-link case); no directory check; the closed directory said for every copy instead of once; no read check; the walk not stopping at a missing directory (a spurious "cannot be entered" line); `of_vela` reduced to "is a file" (the new case passes a package that is only a link).

By hand, on repacked copies of the real opt-in Arch package: `usr/share` 0700 fails; `usr/share/licenses/vela` 0744 fails; a copy at 0640 fails, at 0604 passes; a copy hard-linked to the set's own file passes (it is a regular file of the package); a link to itself fails with the link message; a copy replaced by a directory fails as missing.

## Coder dispute (if any)

## Known gaps

- The plain "is a link" half of the link test has no test of its own: every link in the tests is also caught by the real-path comparison. By hand, a link to itself is caught by it and would otherwise be reported as missing rather than as a link.
- Without `bsdtar` on the machine this finding's tests are skipped (CI's runner does not have it; see `.agents/state.md`, Blockers).

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `2f0d191..4b8293e`, 2026-11-14 01:05 UTC
(`.agents/review/codereview-2026-11-14-libmpv-packaging.md`). Reviewer's
title: "Validate containment and accessibility of Arch licence copies". Reviewer's severity: MEDIUM.
