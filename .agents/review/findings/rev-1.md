# rev-1: Dedup can end infinite scroll before all titles are loaded

**Severity**: MEDIUM — silent content omission: later library titles become unreachable in the All view.
**Status**: In progress
**Branch**: `fix/rev-1-dedup-page-underflow`
**Commit**: `60f62590b97a714489928b27b3a41142fd5f3627`

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
`get_type_listing` now collects the contributing sections once and delegates
to a new `fetch_merged` helper that fetches per-section items at increasing
depth (doubling from `start+size`, capped at `MAX_MERGE_FETCH = 4096`) until
the deduped union can fill the requested window, no section returned a full
window (nothing more exists anywhere), or the cap is hit. A short page now
genuinely means end-of-library, which is exactly the signal the frontend's
`hasMore` logic reads. Root cause (window arithmetic ignoring collapse), not
symptom (frontend heuristics), is what changed.

## Files changed
- `src-tauri/src/commands.rs` — `get_type_listing` restructured;
  new `fetch_merged` + `MAX_MERGE_FETCH`; `FakeItems` test source and the
  guard test in `merge_tests`.

## Guard proof
- `commands::merge_tests::merged_fetch_deepens_past_dedup_collapse_to_fill_the_window`
  — a duplicate pair inside the initial window with unique titles beyond it;
  the deduped result must still fill the window. Reverting the deepening to a
  single pass makes it FAIL (verified); restoring makes it PASS (verified).

## Coder dispute (if any)
None — confirmed against the code.

## Known gaps
Touches the same function as rev-4; branches are stacked (merge order rev-1 →
rev-5), each reviewed against its own pinned base.

## Reviewer comments
- **Round 1** — codex (codex-cli 0.142.5), reviewed `562302ae` against base
  `5d2e2b9`, guard_confirmed=true, verdict **reopened**, 2026-07-04 (UTC).
  Comment: `src-tauri/src/commands.rs:1871` — the absolute `MAX_MERGE_FETCH`
  stop can still return a short page while a section returned a full window;
  once `start+size` is at or near 4096, duplicates before the window again
  end paging early, leaving later titles unreachable.
  Coder response: correct — the cap recreated the bug at depth. Fix-up:
  remove the cap; the loop already terminates via `!any_full` (every section
  returning less than asked means nothing more exists anywhere), so no
  absolute ceiling is needed. Termination-under-total-duplication covered by
  a new test.
- **Round 2** — codex (codex-cli 0.142.5), reviewed `2064526` against base
  `5d2e2b9`, guard_confirmed=true, verdict **reopened**, 2026-07-04 (UTC).
  Comment: stopping at `deduped.len() >= want` still leaves the window's
  *contents* unstable: a unique title hidden behind duplicates beyond one
  section's fetched prefix can surface on a later page's deeper fetch,
  re-sort ahead of the window, and be skipped by the start offset forever,
  while a later title duplicates across pages.
  Coder response: correct — any count-based early stop is wrong by
  construction (window stability, not window size, is the requirement; the
  same mechanism also bites when a source's collation diverges from the
  merged comparator). Fix-up: fetch exhaustively (deepen until `!any_full`,
  no early stop) and page from a per-(type, sort) in-memory snapshot held in
  `AppState`, rebuilt whenever a listing is entered (`start == 0`) and
  reused verbatim for continuation pages — pages window one immutable list,
  so skip/duplicate across pages is structurally impossible.
