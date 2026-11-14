# pk-1: checking an AppImage runs it

**Severity**: HIGH — `check-package.sh` asks an AppImage to unpack itself, which runs the program inside the file. Whoever checks an AppImage they did not build runs whatever it contains, with their own rights, even when only asking whether the set is absent. On the path `scripts/build.sh` takes, the AppImage is the builder's own.
**Status**: In progress
**Branch**: —
**Commit**: `53f2ecd`, `3d1593e`

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:185-187 copies the supplied AppImage, makes it executable, and runs it. A harmless shell script named untrusted.AppImage wrote an external marker and created squashfs-root/usr/bin/vela; --without-libmpv returned RESULT PASS."

## Predicted observable failure

Reviewer, verbatim: "Checking a package executes arbitrary package-controlled commands with the builder's privileges, even in absence-checking mode. Removing APPIMAGE_EXTRACT_AND_RUN does not prevent this."

Reviewer's suggested direction, verbatim: "Extract the AppImage filesystem using a trusted external parser without invoking its runtime. Reject unsupported formats before executing anything from them."

## What

`check-package.sh` unpacked an AppImage by making a copy of it executable and running it with `--appimage-extract`. Whatever the file held ran, with the rights of whoever checked it, in both modes of the check.

Reproduced before anything was changed: a shell script named `evil.AppImage` that writes a marker file outside the temporary directory and creates `squashfs-root/usr/bin/vela` got `RESULT PASS`, exit 0, from `check-package.sh --without-libmpv`, and the marker appeared.

## Approach

Two commits: the fix (`53f2ecd`), and what the attack on it found (`3d1593e`).

- **The check never runs an AppImage** (`53f2ecd`). A new function, `squashfs_offset`, reads the file's ELF header with `od` (32- or 64-bit, either byte order), takes the end of the section header table for where the program ends (the rule the AppImage runtime itself uses), refuses numbers that do not fit the file, and requires the squashfs magic `hsqs` at that place. A file that fails any of this is refused before anything else is done with it ("this is not an AppImage this check can read: no squashfs follows an ELF program in it"). Otherwise the image is read by `unsquashfs -no-progress -no-xattrs -o OFFSET -d DIR FILE` from the builder's own file; nothing is copied and nothing is made executable.
- **Where `unsquashfs` is missing**, that package cannot be checked: its line says so, the run exits 2, and the other packages named are still judged. The preflight gained `--appimage`, which makes it require `unsquashfs`; `scripts/build.sh` passes it only when an AppImage is among the bundles, so such a build stops before it builds anything.
- **After the attack** (`3d1593e`): an AppImage whose name begins with a dash reached `unsquashfs` as an option; it is now handed over as `./NAME`. And the test that a package cannot write outside the temporary directory held only the pacman unpacking: it now holds a deb's two unpackings too (the outer `ar` and the `data.tar` inside it), each case with an outside directory of its own, so a failure shows which unpacking let the package out.

Written by one helper agent, attacked by another, repaired by a third; then landed, read and injected by the coordinating agent.

## Files changed

- `packaging/libmpv/check-package.sh` — `squashfs_offset`; the AppImage branch of the unpacking; `--appimage`; the `./` prefix.
- `scripts/build.sh` — passes `--appimage` to the preflight when an AppImage is to be built.
- `tests/libmpv-packaging.test.mjs` — the AppImage tests (with a stand-in `unsquashfs` that checks `hsqs` is at the offset it is given and refuses a last argument beginning with a dash); the escape test for pacman and deb; the preflight and `build.sh` fixtures.
- `docs/development.md`.

## Guard proof

Run 2026-11-14 by the coordinating agent in a disposable worktree at `1348782`, one injection at a time, restored from the commit each time; `node --test tests/libmpv-packaging.test.mjs` (187 tests; unchanged: 186 pass, 1 skipped).

| Injection | Result |
| --- | --- |
| The image is made executable and run to unpack itself again (the original defect) | FAIL, 9 tests, among them `the package check reads an AppImage with unsquashfs and never runs it` and `one without the set passes; unsquashfs reads it from where its program ends` |
| A file with no squashfs behind an ELF program is not refused first | FAIL, 5 tests, among them `a file named like an AppImage that is a script: never run, refused before anything else` and `an ELF program with no squashfs after it is refused, and so is a cut-off header` |
| A name beginning with a dash reaches `unsquashfs` as it is | FAIL `one named relative to where the check runs, with a name that begins with "-": read all the same` |

The helpers ran, each alone and each failing its named tests: the self-extracting branch put back (the script AppImage gets `RESULT PASS`); offset 0 instead of a refusal; `-o 0`; a missing `unsquashfs` not counted as "could not be checked" (status 1, not 2); the preflight ignoring `--appimage`; `--appimage` accepted without `--preflight`; `build.sh` never passing `--appimage`; and `bsdtar -P` on each of the three `bsdtar` lines in turn (outer deb `ar`: exactly the 2 ar-member cases fail; deb `data.tar`: exactly the 5 deb cases; pacman/rpm: exactly the 5 pacman cases).

By hand, on real files: the reviewer's script AppImage gets `RESULT FAIL`, exit 1, in both modes, and writes no marker. Refused before `unsquashfs` is called: a real ELF with nothing appended, a 32-bit big-endian header with no sections, section offsets of 2^63-1 and 2^64-64, a bare `hsqs` with no ELF. A real third-party AppImage is accepted by the header reading (its computed offset holds `hsqs`).

## Coder dispute (if any)

## Known gaps

- **No real squashfs has been read by `unsquashfs` here.** squashfs-tools is not installed on this machine, so the one test that builds and reads a real image (`reads a real squashfs behind an ELF program`) was skipped in every run, and is very likely skipped on CI too. The flags, and that `unsquashfs` keeps the package's modes for the access check, are by reading. The stand-in holds the offset and the arguments only. That run, on a machine with squashfs-tools 4.5.1 or later, is still owed.
- An `unsquashfs` older than 4.5.1 (before CVE-2021-40153 and CVE-2021-41072 were fixed) could write outside the temporary directory when given a hostile image; the check does not look at the version (distributions backport fixes, so a version number would mislead).
- Only little-endian squashfs (`hsqs`) is accepted.
- Without `bsdtar` on the machine, the tests of this finding are skipped, not failed (CI's runner does not have it; see `.agents/state.md`, Blockers).

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `2f0d191..4b8293e`, 2026-11-14 01:05 UTC
(`.agents/review/codereview-2026-11-14-libmpv-packaging.md`). Reviewer's
title: "Extract AppImages without executing the package". Reviewer's severity: HIGH.
