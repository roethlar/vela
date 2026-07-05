# eh-2: Unknown scenario names in a mixed filter silently pass

**Severity**: MEDIUM — a filtered E2E run can report green while a requested scenario never ran (false-green class)
**Status**: Open
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: (filled in after commit)

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
(pending)
