# lm-2: licence texts are outside the libmpv set's integrity check

**Severity**: MEDIUM — a set missing every licence text passes the check
that is documented as covering every file; shipping the texts is a
licensing requirement.
**Status**: In progress (fix landed; verification pending)
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
Verification pass: pending.
