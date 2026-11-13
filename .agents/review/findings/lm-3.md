# lm-3: the checksum test cannot tell rejection from failed extraction

**Severity**: MEDIUM — the regression test for the pin check stayed green
under a mutation that let an unverified archive reach the compiler.
**Status**: Open
**Branch**: —
**Commit**: (filled in after commit)

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
