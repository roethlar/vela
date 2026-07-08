# eh-2: Unknown scenario names in a mixed filter silently pass

**Severity**: MEDIUM — a filtered E2E run can report green while a requested scenario never ran (false-green class)
**Status**: Verified
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: `404f86a`

## Evidence
`tests/e2e/run.mjs:24,127-134` — `nameFilter` selects scenarios by name,
and the run only errors when *zero* scenarios match. Trigger:
`npm run e2e -- smoke typo` where `smoke` exists and `typo` does not.

## Predicted observable failure
The run executes only `smoke`, prints `1/1 passed`, and exits 0 — the
caller believes `typo` (e.g. a misspelled real scenario) passed when it
never ran.

## What
Partial filter matches are indistinguishable from full matches, so a typo
in one scenario name yields a silent false-green.

## Approach
`run.mjs` loads all scenario modules first, then rejects any filter name
that matches none of them — the run exits 1 listing the unknown names and
the available ones, before anything launches. Filtering applies only after
that validation.

## Files changed
- `tests/e2e/run.mjs` — scenario loading/filter split with unknown-name
  rejection

## Guard proof
No JS unit runner exists in this repo (recorded gap). Manual red/green
check instead: `npm run e2e -- --skip-build smoke typo` must exit nonzero
naming `typo` without launching anything; red = exits 0 running only smoke
before the fix.

Executed 2026-07-05: red — `smoke typo` → `1/1 passed`, exit 0. Green —
`e2e: unknown scenario(s): typo — available: smoke`, exit 1, nothing
launched; `smoke` alone still passes (1/1, exit 0).

## Coder dispute (if any)
None — admitted as filed.

## Known gaps
None.

## Reviewer comments
codex (codex-cli 0.142.5), manual-check mode, 2026-07-05 ~08:39 UTC.
Reviewed `404f86ae42b0564e470cfea1b0c3f17e227c335e` against base
`cfe6ee4d55b03393e130bf4b05eb7562b19e45ed`. Verdict: **accepted**,
`guard_confirmed: true`. Comments: all modules load before filter
validation, unknown names exit 1 before anything launches; the red/green
proof is discriminating; edge cases (empty filter, all-unknown, empty
scenario dir) check out statically.
