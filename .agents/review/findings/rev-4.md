# rev-4: All-view type listing hides total source failure as an empty grid

**Severity**: LOW — a blank library with no error message when every source is offline/unreadable; misleading but recoverable.
**Status**: In progress
**Branch**: `fix/rev-4-surface-total-failure` (stacked on rev-3)
**Commit**: `a1239b2722afe6f418bed8c72459713e6ec6cd81`

## Evidence
`src-tauri/src/commands.rs` `get_type_listing`: `let Ok(sections) = ... else
{ continue }` and `if let Ok(items) = ...` discard every error; the command
returns `Ok(vec![])` when all sources fail. Contrast `aggregate()`
(`commands.rs`), which surfaces the last error when everything failed and
nothing was produced.

## Predicted observable failure
With all servers offline (or all mounts unreadable), opening Movies/TV
Shows/Videos in the All view renders an empty grid with no error banner,
indistinguishable from an empty library.

## What
The merged listing adopted aggregate()'s skip-failing-sources stance but not
its all-failed error path.

## Approach
Adopted aggregate()'s all-failed stance in both halves of the merged
listing. `fetch_all_merged` now returns `Result`: per-section item errors
are tolerated while any section succeeds (partial view stays useful), but
when every section failed on the final pass the last error is returned
instead of an empty list. `get_type_listing` likewise tracks sections()
errors: if no section contributed AND something failed, the command errors
(a genuinely empty library — no sections of the type, no failures — still
returns empty). The frontend already renders command errors via its
existing error banner; no UI change needed.

## Files changed
- `src-tauri/src/commands.rs` — `fetch_all_merged` → `Result` with
  any_ok/last_err tracking; `get_type_listing` sections-error tracking;
  `FailingSource` test double and the guard test; two fetch tests updated
  to unwrap.

## Guard proof
- `commands::merge_tests::merged_fetch_surfaces_total_failure_but_tolerates_partial`
  — all sections failing → Err containing the source error; one healthy
  source among failures → Ok with its items. Reverting to error-swallowing
  makes it FAIL (verified); restoring makes it PASS (verified).

## Coder dispute (if any)
None.

## Known gaps
Stacked on rev-3's branch; same function as rev-1.

## Reviewer comments
(pending)
