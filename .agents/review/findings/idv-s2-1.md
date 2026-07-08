# idv-s2-1: Episode Info inside an open season page re-lists seasons as episodes

**Severity**: LOW — a dev-flagged surface in this slice (nav not yet flipped),
but the defect would ship user-visible with the flip slice.
**Status**: Verified
**Branch**: (no-branches adaptation — landed on `main`)
**Commit**: `0ecd819` (fixup); found reviewing slice `7085fdf` (amended slice 2,
base `3acf581`)

## Evidence
`src/routes/+page.svelte` — `openInfo()` inferred an episode's season from the
last browse crumb whenever `mode === "browse"`. The detail view deliberately
layers over browse state without touching it, so with a season page open above
a *seasons* grid (Info on a season card from a show's seasons view), the crumb
still points at the **show**. `SeasonDetail` episode rows reuse the global
context menu, so Info on an episode row inside that open page passed the show
key as `seasonKey`.

## Predicted observable failure
Show → seasons grid → Info on a season card → shared episode page → right-click
an episode row → Info: the new page calls `get_children(show)` and renders the
show's *seasons* in the episode list with nothing selectable as the episode —
wrong list, empty detail panel.

## What
The crumb-based season inference trusted a crumb that does not describe the
list the episode actually came from.

## Approach
A season key is now accepted only from a source that provably owns the episode
(`seasonKeyFor` in `+page.svelte`): (1) an already-open shared season page
supplies its own `seasonKey` (Info on its rows remounts with that episode
selected); (2) a browse crumb is trusted only when the current grid's `items`
contain the episode (i.e. the grid *is* that season's child list); (3) anything
else — home rails, search results, hero items — degrades to single-episode
mode, never a wrong list.

## Files changed
- `src/routes/+page.svelte` — new `seasonKeyFor(ep)`; `openInfo()` episode
  branch routes through it.

## Guard proof
Frontend routing logic (no JS unit runner in repo — same standing gap as the
hero merge ordering and sspf-14; the E2E harness is Linux-only and this slice
was built on the Windows host, so no E2E leg either — recorded follow-up for
the Linux host). Verified by `npm run check` (0/0) + `npm run build` clean and
by the case analysis above; the backend `detail_key` half of the slice is
unit-tested and guard-proven red/green (`merge_tests::merged_detail_key_*`,
`detail_key_*`).

## Reviewer comments
- **r1** 2026-07-08 `codex` (codex-cli 0.142.5, npm install; read-only),
  reviewed `7085fdf` base `3acf581`, `guard_confirmed: false`, verdict
  **reopened**. Comment (LOW): "openInfo() infers an episode's season from the
  global browse crumb, but SeasonDetail episode rows reuse the same global
  context menu. If a user opens Info on a season card from the seasons grid,
  the underlying crumb is still the show; choosing Info on an episode inside
  that SeasonDetail passes the show key as seasonKey, so the new SeasonDetail
  calls get_children(show) and renders seasons/empty selection instead of that
  episode's season." Admitted — confirmed against the code before fixing.
- **r2** 2026-07-08 `codex` (codex-cli 0.142.5, read-only), reviewed `0ecd819`
  base `3acf581`, `guard_confirmed: false` (read-only sandbox; the coder's
  guard proofs stand), verdict **accepted**, 0 comments. Loop converged
  (r1 reopened → r2 accepted clean).
