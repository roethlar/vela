# pk-6: a temporary directory with a space in its name fails a good package

**Severity**: MEDIUM — with `TMPDIR` set to a path containing a space, the isolation step reads the wrong part of each line of `/proc/self/maps` and reports the set's own libraries as foreign. A correct package fails, and with it an opt-in build.
**Status**: In progress
**Branch**: —
**Commit**: `25fb666`, `a60f802` (and `3e008bf`)

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:154 honors TMPDIR and passes the extracted path to check-libmpv.py at line 270. That helper parses /proc/self/maps using line.split()[-1] at line 338. With TMPDIR containing spaces, the unchanged real Arch package failed isolation because the path prefix was discarded."

## Predicted observable failure

Reviewer, verbatim: "A valid package is rejected, and an opt-in build fails, solely because the builder's temporary directory contains spaces."

Reviewer's suggested direction, verbatim: "Parse the fixed fields of /proc/self/maps while preserving the complete pathname remainder. Exercise the package checker with a temporary directory containing spaces."

## What

After loading the library, the check reads `/proc/self/maps` to see that every library came from the set. It took the last word of each line for the path. With a temporary directory whose name has a space in it, the path lost its front, and the set's own libraries were reported as coming from elsewhere: a good package failed.

Reproduced before anything was changed: with a stand-in for that file, a set under `/tmp/vela libmpv  set with spaces …` failed the isolation check; and by hand, the real opt-in Arch package with `TMPDIR` set to a directory with two spaces in its name got `FAIL isolation` for all 15 libraries.

## Approach

Two commits: the fix (`25fb666`), and what the attack on it found (`a60f802`). A third commit, `3e008bf`, is a defect the attack found that is older than the fix; it is described at the end and verified with this finding.

- **The path is the whole sixth field** (`25fb666`). `check-libmpv.py` splits each line into at most six fields and takes the last, spaces and all. A line with fewer fields (a mapping of no file) is passed over.
- **A temporary directory with a line break in its name is refused** (`a60f802`). The kernel writes a line break in that file as the four characters `\012`, which cannot be told from a name that really holds those four characters, so such a directory could never pass. The check now says so before it unpacks anything (status 2, "set TMPDIR to another directory"). `--without-libmpv` loads nothing and is not affected. The same commit gives the tests' stand-in for that file the mappings of no file that the real one has, so the "passed over" rule is held by a test (before, taking it out left the suite green and broke the real run).
- **A relative temporary directory that begins with a dash no longer breaks the check** (`3e008bf`; found by the attack, older than the fix, and not under a package's control). The name was taken for an option by a tool the check calls; the check failed and left its temporary directory behind. The name is made absolute right after it is made.

Written by one helper agent, attacked by another, repaired by a third; then landed, read and injected by the coordinating agent.

## Files changed

- `packaging/libmpv/check-libmpv.py` — the reading of `/proc/self/maps`.
- `packaging/libmpv/check-package.sh` — the refusal of a line break; the absolute temporary directory.
- `tests/libmpv-packaging.test.mjs` — `a set in a directory whose name has spaces passes`; a `TMPDIR` with two spaces through `check-package.sh`; the line-break case; `a TMPDIR named relative to where the check runs, beginning with "-"`.

## Guard proof

Run 2026-11-14 by the coordinating agent in a disposable worktree at `1348782`, one injection at a time, restored from the commit each time; `node --test tests/libmpv-packaging.test.mjs` (187 tests; unchanged: 186 pass, 1 skipped).

| Injection | Result |
| --- | --- |
| The first commit taken out (reverse-applied): the last word of a line is the path again | FAIL `a set in a directory whose name has spaces passes` |
| A temporary directory with a line break in its name is not refused | FAIL `a temporary directory whose name has a line break: refused before anything is unpacked` |
| The temporary directory's name is not made absolute | FAIL `a TMPDIR named relative to where the check runs, beginning with "-"` |

The helpers ran, each alone and each failing its named tests: the old split put back (`FAIL isolation` with a cut-off path); the "passed over" rule removed (7 subtests, `IndexError`, once the stand-in had such lines); the line-break refusal disabled, and made unconditional (fails at the `--without-libmpv` half); the absolute-name line removed (`sed: invalid option -- d`).

By hand, the real package and set passed, with a real load and nothing left behind, under a `TMPDIR` with two spaces, a tab, quotes, non-ASCII letters, a trailing space, the words ` (deleted)`, a leading dash in an absolute path, and the four literal characters `\012`. With a real line break: status 2 and the message.

## Coder dispute (if any)

## Known gaps

- Without `bsdtar` on the machine the `check-package.sh` halves of this finding's tests are skipped (CI's runner does not have it; see `.agents/state.md`, Blockers). The Python-side test needs only Linux and `python3`.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `2f0d191..4b8293e`, 2026-11-14 01:05 UTC
(`.agents/review/codereview-2026-11-14-libmpv-packaging.md`). Reviewer's
title: "Handle spaces in the temporary library path". Reviewer's severity: MEDIUM.
