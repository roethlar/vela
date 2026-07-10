# br-2: mock JF search route ignores the client's query contract

**Severity**: MEDIUM — a search-query regression (narrowed IncludeItemTypes,
dropped Recursive) passes against the mock while real Jellyfin/Emby returns
different results; the eh-12 fail-closed class, unguarded on the new branch.
**Status**: Verified
**Branch**: n/a (no-branches adaptation)
**Commit**: (filled at commit)

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
Red/green on the Linux VM: search scenario green as-is; a temporary client
narrowing (`IncludeItemTypes=Series` in jellyfin.rs search) must turn the
scenario RED (no results / violation), then restore green.

## Reviewer comments
(appended after the per-finding verdict)
