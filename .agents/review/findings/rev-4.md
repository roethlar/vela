# rev-4: All-view type listing hides total source failure as an empty grid

**Severity**: LOW — a blank library with no error message when every source is offline/unreadable; misleading but recoverable.
**Status**: Open
**Branch**: `fix/rev-4-surface-total-failure`
**Commit**: (pending)

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
(pending)

## Files changed
(pending)

## Guard proof
(pending)

## Coder dispute (if any)
None.

## Known gaps
Stacked on rev-3's branch; same function as rev-1.

## Reviewer comments
(pending)
