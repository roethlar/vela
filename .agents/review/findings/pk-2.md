# pk-2: the libraries of a set that has failed its checks are loaded anyway

**Severity**: HIGH — `check-libmpv.py` goes on to load `libmpv.so.2` after the set has failed its manifest check, so a package whose library is a link to somewhere else, or has been altered, gets its code run (a library's start-up code runs on load) before the check says FAIL.
**Status**: Verified
**Branch**: —
**Commit**: `12fa2fa`, `9b12157` (and `1348782`)

## Evidence

Reviewer, verbatim: "packaging/libmpv/check-package.sh:270 invokes check-libmpv.py, whose lines 207-217 continue to ctypes.CDLL after manifest validation fails. Replacing the packaged libmpv.so.2 with a symlink to the external reference set produced manifest failures, followed by a successful load and an isolation failure naming the external library."

## Predicted observable failure

Reviewer, verbatim: "The checker loads libraries outside the unpacked package and can execute modified library constructors before reporting failure. Its eventual rejection does not undo that execution."

Reviewer's suggested direction, verbatim: "Complete source, checksum, and containment validation before loading any library. Abort the load phase immediately when those checks fail."

## What

The check loads the package's `libmpv.so.2` to ask it its version and to see where its libraries come from. Loading a library runs its start-up code. The load was made even when the set had already failed the checks before it, so an altered library, or one reached through a link out of the package, ran before the check said FAIL.

Reproduced before anything was changed: with a stand-in loader, damaged sets were handed to it; through `check-package.sh` the real `check-libmpv.py` printed `FAIL load: … file too short` after `FAIL manifest` (the load was tried); and the reviewer's case, a copy of the real Arch package with `libmpv.so.2` replaced by a link to the built set, printed `ok   load` and loaded the library from outside the package.

## Approach

Two commits: the fix (`12fa2fa`), and what the attack on it found (`9b12157`). A third commit, `1348782`, is a defect the attack found that is older than the fix; it is described at the end and verified with this finding.

- **Nothing of a set that failed is loaded** (`12fa2fa`). `check-libmpv.py` stops after a failed manifest check, before the load, and says so (`skip load: …`). `check-package.sh` does not start `check-libmpv.py` at all when an earlier check of its own failed (place, access, source, pins).
- **A package is checked for the set only against the set it was built from** (`9b12157`). The attack showed the gate was not enough: without `--set`, the check trusted the package's own MANIFEST, so a package whose MANIFEST had been rewritten to match its altered library passed everything before the load and was loaded (its constructor wrote a marker). Checking a package for the set now REQUIRES `--set DIR`; without it the check stops before it unpacks anything (status 2), and so does the preflight. The comparison with the built set therefore always comes before the load, and what is loaded is, byte for byte, what the builder built. `--without-libmpv` loads nothing and still works without `--set`.
- **The check reads no named pipe, nor anything through a link, of a package** (`1348782`; found by the attack, older than the fix). A package whose MANIFEST, or a licence text of its set, was a named pipe hung the check, the build's own run of it included. The package's MANIFEST is now read only when it is a regular file of the package and not a link; a set's licence text likewise.

Written by one helper agent, attacked by another, repaired by a third; then landed, read and injected by the coordinating agent.

## Files changed

- `packaging/libmpv/check-libmpv.py` — the stop before the load.
- `packaging/libmpv/check-package.sh` — the gate before `check-libmpv.py`; `--set` required; the regular-file rule for what is read.
- `tests/libmpv-packaging.test.mjs` — the stand-in loader records every load; the "not loaded" cases; `no set given`; the named-pipe cases, each with a 20-second limit.
- `docs/development.md`.

## Guard proof

Run 2026-11-14 by the coordinating agent in a disposable worktree at `1348782`, one injection at a time, restored from the commit each time; `node --test tests/libmpv-packaging.test.mjs` (187 tests; unchanged: 186 pass, 1 skipped).

| Injection | Result |
| --- | --- |
| `check-libmpv.py` goes on to the load after a failed manifest check (the original defect) | FAIL, 13 tests, among them `one byte of a library changed, and nothing else wrong: not loaded`, `libmpv.so.2 is a link to a library outside the set: not loaded` and `libmpv.so.2 a link to a library outside the package: not loaded` |
| A package is judged for the set without `--set` | FAIL `no set given: no package is judged, and nothing of one is loaded` |
| The named-pipe commit taken out (reverse-applied) | FAIL, 5 tests, among them `its MANIFEST a named pipe: never read, and the package fails` |

The helpers ran, each alone and each failing its named tests: the old flow put back (11 subtests; the stand-in's recorded loads are no longer empty); the gate in `check-package.sh` replaced by `if false` (the three "failed before" cases: `check-libmpv.py was started on a package that had failed`); the manifest's link rule removed (the link case alone: `ok load`, a library "from elsewhere"); the `--set` requirement removed in two ways; the MANIFEST read forced (the pipe case hangs until its 20 seconds are up, the link case passes through the link); the set-side test removed from the licence comparison (hangs).

By hand, on the real opt-in Arch package (`vela-1.0.63-1-x86_64.pkg.tar.zst`) at the fix's content: with `--set` and a real load, `RESULT PASS`, exit 0 (`isolation: 15 libraries mapped from the set`); without `--set`, status 2 and nothing unpacked; the attack's package (a library whose constructor writes a marker, its MANIFEST rewritten to match) leaves no marker either way.

## Coder dispute (if any)

## Known gaps

- The load itself is by design the one place where code from a package runs. With `--set` it is the builder's own code, compared byte for byte first.
- Without `bsdtar` on the machine the `check-package.sh` halves of this finding's tests are skipped (CI's runner does not have it; see `.agents/state.md`, Blockers). The Python-side tests (the stand-in loader) need only Linux and `python3`.
- File names from a package are printed as they are; a name holding terminal control characters reaches the builder's terminal. Not changed here.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `2f0d191..4b8293e`, 2026-11-14 01:05 UTC
(`.agents/review/codereview-2026-11-14-libmpv-packaging.md`). Reviewer's
title: "Reject invalid sets before loading their libraries". Reviewer's severity: HIGH.

### Verification round 1, recorded 2026-11-14 03:07 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d7e8-3fe8-70c2-9181-5e52301562f7`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 02:34 UTC, finished 02:44 UTC.
- Reviewed head `13487828af78d0e8a5a4c1ae181efe8dc3981da7`, base
  `8e6299e6403d7467705206c691ab7e50099dbf9c` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
