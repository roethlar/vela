# lm-2: licence texts are outside the libmpv set's integrity check

**Severity**: MEDIUM — a set missing every licence text passes the check
that is documented as covering every file; shipping the texts is a
licensing requirement.
**Status**: Open
**Branch**: —
**Commit**: (filled in after commit)

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

(completed with the fix)

## Files changed

(completed with the fix)

## Guard proof

(completed with the fix)

## Coder dispute (if any)

None.

## Known gaps

(completed with the fix)

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-libmpv-set.md`.
Verification pass: pending.
