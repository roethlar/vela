# lm-3: the checksum test cannot tell rejection from failed extraction

**Severity**: MEDIUM — the regression test for the pin check stayed green
under a mutation that let an unverified archive reach the compiler.
**Status**: In progress (fix landed; verification pending)
**Branch**: —
**Commit**: `1e2b953` (propagation) and `29026c2` (the test)

## Evidence

`tests/dependency-policy.test.mjs` (at `b19d7d1`, line 362) hands the build
a text file under the pinned tarball's name. Text fails at `tar` whether or
not the checksum is enforced. The reviewer moved the statement that prints
the path in `fetch()` (`scripts/build-libmpv.sh:72`) above the checksum
guard: all 12 tests stayed green, and with a well-formed archive of the
wrong checksum the build went on to a stub compiler, because `unpack()`
(line 79) did not propagate `fetch`'s failure.

## Predicted observable failure

A change that weakens or reorders the pin check passes `npm run check`,
and a tampered source tarball is extracted and compiled.

## What

Two things: `set -e` did not reach into `$(...)`, so a failed `fetch` did
not stop its caller; and the test's fixture could not exercise that.

## Approach

Two parts.

1. `1e2b953`, landed before this verdict was read, after the first GitHub
   run of the job showed the same hole from the other side (a failed
   download did not stop the build): the script sets
   `shopt -s inherit_errexit`, so a `fetch` that fails inside `$(...)`
   stops `unpack` and the build at that point.
2. The test's fixture is now a well-formed `tar.gz` with the pinned name
   and the wrong contents, laid out so the `lua` step would extract and
   compile it, with a `gcc` first on `PATH` that leaves a mark if it runs.
   The test requires exit status 1, the mismatch message as the LAST line
   of stderr, no extracted tree and an unreached compiler.

## Files changed

- `scripts/build-libmpv.sh` — `shopt -s inherit_errexit` (`1e2b953`)
- `tests/dependency-policy.test.mjs` — "the libmpv build refuses a tarball
  that does not match its pin": well-formed archive, sentinel compiler,
  four assertions

## Guard proof

`tests/dependency-policy.test.mjs` › "the libmpv build refuses a tarball
that does not match its pin". Run 2026-11-13, each restored from the
committed file afterwards:

- Remove the final checksum line of `fetch()`: FAIL (no "checksum
  mismatch" message).
- The reviewer's mutation (print the path before the check) together with
  removing `inherit_errexit`, which is the state the reviewer tested: FAIL
  ("the archive must not be extracted").
- Restored: PASS.

The reviewer's mutation alone no longer breaks anything: `fetch` still
exits 1 at the mismatch and the build stops there, so the test passes for
the right reason.

## Coder dispute (if any)

None.

## Known gaps

None known.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-libmpv-set.md`.
Verification pass: pending.
