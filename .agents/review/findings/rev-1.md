# rev-1: Dedup can end infinite scroll before all titles are loaded

**Severity**: MEDIUM — silent content omission: later library titles become unreachable in the All view.
**Status**: Open
**Branch**: `fix/rev-1-dedup-page-underflow`
**Commit**: (pending)

## Evidence
`src-tauri/src/commands.rs` `get_type_listing`: fetches `start+size` items per
section, dedups the union (`dedup_across_sources`), then windows
`.skip(start).take(size)`. `src/routes/+page.svelte` `loadMore`:
`hasMore = page.length >= PAGE`.

## Predicted observable failure
When duplicates collapse inside the fetched window (e.g. a title present on
two sources, or twice in one source's first 60), the returned page is shorter
than PAGE even though more unique titles exist beyond the fetch depth; the
frontend sets `hasMore = false` and scrolling stops — titles past that point
never appear.

## What
Stateless merged paging assumed dedup-free windows: fetch depth `start+size`
only yields `start+size` merged entries when nothing collapses. Any collapse
under-fills the window and the short page is indistinguishable from "end of
library".

## Approach
(pending)

## Files changed
(pending)

## Guard proof
(pending)

## Coder dispute (if any)
None — confirmed against the code.

## Known gaps
Touches the same function as rev-4; branches are stacked (merge order rev-1 →
rev-5), each reviewed against its own pinned base.

## Reviewer comments
(pending)
