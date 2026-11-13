# lm-2: licence texts are outside the libmpv set's integrity check

**Severity**: MEDIUM — a set missing every licence text passes the check
that is documented as covering every file; shipping the texts is a
licensing requirement.
**Status**: Reopened (round 1); repair in progress
**Branch**: —
**Commit**: `7ca7e75`

## Evidence

`scripts/build-libmpv.sh` (at `b19d7d1`, line 327) writes `file` lines to
the `MANIFEST` for `*.so*` only. `packaging/libmpv/check-libmpv.py` (line
62) rejects only unlisted libraries. The reviewer copied a built set
without its `licenses/` directory: `RESULT PASS`. `docs/development.md`
(lines 124 and 157 at that commit) says the check compares every file.

## Predicted observable failure

A cached or damaged set with no licence texts, or without the mpv build
patch that ships beside them, passes the `libmpv` CI job and is uploaded
as the artifact that packaging will consume.

## What

The manifest and the check cover the libraries only, so the part of the
set that the licence decision (`.agents/decisions.md` 2026-11-12,
"Licensing is settled", rule 3: "with their licence texts") depends on is
unguarded, and the documentation overstates the check.

## Approach

The manifest now lists every file of the set (libraries, licence texts,
the mpv build patch), and the check compares the directory with it in both
directions: every listed file present and unmodified, no file that is not
listed, and at least one licence text listed for every `component` line.
The last rule matters on its own: a manifest that never listed a
component's licence would otherwise agree with a directory that lacks it.
A manifest entry that points outside the set is refused.

The two documentation sentences the reviewer cited are now true as
written, so they are unchanged.

## Files changed

- `scripts/build-libmpv.sh` — `collect()` writes a `file` line for every
  file under the set's directory (`find . -type f ! -name MANIFEST`)
- `packaging/libmpv/check-libmpv.py` — `check_manifest()` rewritten as
  above
- `tests/dependency-policy.test.mjs` — a static guard on the manifest's
  coverage, and "the libmpv integrity check covers every file of the set,
  licence texts included", which drives `check_manifest()` over an intact
  set and ten broken ones

## Guard proof

`tests/dependency-policy.test.mjs` › "the libmpv integrity check covers
every file of the set, licence texts included" and › "the shipped libmpv
set is an LGPL build with its licence texts". Run 2026-11-13 from the
committed state, each restored with `git checkout`, each FAILING its test:
only libraries walked for unlisted files (the pre-fix behaviour); the
licence-per-component rule removed; size and checksum not compared;
missing files not reported; entries outside the set accepted; the build
script's manifest listing libraries only.

On a real set: the set built before this change (manifest listing
libraries only) now FAILS with "no licence text listed for component" for
each component; after `scripts/build-libmpv.sh --container collect` the
manifest lists 29 files and the check passes.

## Coder dispute (if any)

None.

## Known gaps

The check proves a licence text is present and unmodified since the build.
It does not judge whether a text is the right one for its component; the
build script names each file explicitly.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-libmpv-set.md`.
Verification pass, round 1, recorded 2026-11-13 05:45 UTC (this record is
committed):

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly (gpt-6-astra @ max per the
  host's codex config). Thread `01a1d369-f24b-7650-a747-e835ce3789d3`, in
  the disposable worktree at the head SHA; ends in `turn.completed`; 16
  tool calls; finished 05:43 UTC.
- Reviewed head `7ca7e75745655af38eafbb02b36f16eed74cce48`, base
  `29026c24b65dd233bf6cf0b431f8443a57b8b701` (both equal the pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  1. "packaging/libmpv/check-libmpv.py:65 — The mpv build patch counts as
     its required licence. Removing LICENSE.LGPL and Copyright plus their
     MANIFEST entries from the real set still produces RESULT PASS. Require
     an actual licence entry distinct from the patch, and guard this case."
  2. "packaging/libmpv/check-libmpv.py:72 — islink checks only the final
     path component. Replacing licenses/ with a symlink to an external
     licence directory, without changing MANIFEST, still produces RESULT
     PASS. Reject symlinked parents or validate resolved containment, and
     add a regression case."

Orchestrator: schema-valid, pins match. Both comments are correct and
reproduce; the finding is reopened and the repair follows as its own
commit.
