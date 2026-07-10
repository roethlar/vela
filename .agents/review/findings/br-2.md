# br-2: mock JF search route ignores the client's query contract

**Severity**: MEDIUM — a search-query regression (narrowed IncludeItemTypes,
dropped Recursive) passes against the mock while real Jellyfin/Emby returns
different results; the eh-12 fail-closed class, unguarded on the new branch.
**Status**: Verified
**Branch**: n/a (no-branches adaptation)
**Commit**: `36dec5d`

## Evidence
`tests/e2e/mockjf.mjs` search branch: matches on `searchTerm` only.
`src-tauri/src/source/jellyfin.rs::search()` always sends
`Recursive=true` + `IncludeItemTypes=Movie,Series,Episode,Video,MusicVideo`.

## Predicted observable failure
A client change sending `IncludeItemTypes=Series` (or dropping the param)
still gets the mock movie back → the search scenario passes while a real
server omits movies from results.

## What
The dls-s2 search branch was added without the listing branch's contract
discipline (eh-12).

## Approach
Faithful semantics + fail-closed shape: the search branch type-FILTERS
(movies returned only when `IncludeItemTypes` includes `Movie` — a real
server filters, it doesn't error), and a missing `IncludeItemTypes` or
missing `Recursive=true` (the client always sends both) records a
contractViolation and returns 400, mirroring the listing check. The search
scenario's existing final `contractViolations` assert then covers it.

## Files changed
- `tests/e2e/mockjf.mjs` — search-branch contract

## Guard proof
Run on the Linux VM 2026-07-09/10:
- RED: client narrowed to `IncludeItemTypes=Series` in jellyfin.rs search →
  search scenario FAILS ("timed out … waiting for search hit in the results
  grid") — the mock now filters like a real server.
- GREEN: hack reverted → search PASSES; full suite 10/10.

## Reviewer comments
codex-cli 0.144.0 (read-only), reviewed_sha `36dec5d`, base_sha `8c596d0`,
`guard_confirmed:true` (contract verified against jellyfin.rs search()),
verdict **accepted**, 0 comments — 2026-07-10 (UTC).
