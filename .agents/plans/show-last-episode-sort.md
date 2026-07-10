# Plan: TV "Date Last Episode Added" sort (owner ask, 2026-07-10)

## Status
**DRAFT 2026-07-10 — plan review pending; implementation follows under
the owner's standing "continue with anything else you can do" go
(2026-07-10), with the plan-review loop as the quality gate.** Owner
report (sorting playtest, 2026-07-10, "add that to the queue, but don't
code"): sorting works, but the date-added sort on SHOW libraries uses
the series' own addedAt, so a show whose newest episode just arrived
doesn't surface.

## Diagnosis (code-confirmed 2026-07-10)
The owner's "it seems" reading is correct by construction: Vela's sort
keys are passed to Plex verbatim (`plex.rs items` →
`get_section_content_with_type_alpha_sorted(sort_ref)`), and
`addedAt:desc` on a show section sorts by the SERIES `addedAt` (when
the show was first added), not by its newest episode. Jellyfin maps
`addedAt` → `DateCreated` (`jellyfin.rs map_sort:594-611`) — same
series-level semantics. No existing key exposes leaf-added recency.

Server-side support exists on both backends for exactly this:
- **Plex:** show sections accept `sort=episode.addedAt:desc` — the key
  behind Plex Web's own "Last Episode Date Added" sort
  (**ASSUMPTION, verify at implementation** against the owner's live
  server: browse a show library with the new sort in the app and
  confirm a recently-updated show surfaces first; an unknown key
  degrades to Plex's default order, non-fatal).
- **Jellyfin:** `SortBy=DateLastContentAdded` (series-level "newest
  content added"). Emby (same client code path): **ASSUMPTION** it
  accepts the same name — Vela is Plex-first and JF/Emby listing polish
  is the same deferred class as `item_detail`; a server that ignores
  the SortBy returns its default order, non-fatal.

## Design
One new Vela sort key, SHOW-scoped end to end: `episodeAddedAt:desc`,
label **"Last episode added"** (placed after "Recently added" in the
dropdown).

1. **Backend whitelist:** `commands.rs ALLOWED_SORTS` +=
   `"episodeAddedAt:desc"`. The merged All-view whitelist
   (`get_type_listing`, `commands.rs:1029-1038`) is NOT extended: the
   DTO carries no per-item "newest episode" field to merge-sort on, so
   the key stays per-source, exactly like `rating:desc` (the existing
   comment already documents that class). The frontend never offers it
   there (below).
2. **Plex mapping:** translate the Vela key to Plex's `episode.addedAt`
   at the `plex.rs items()` boundary (small pure fn beside the call:
   `episodeAddedAt:desc` → `episode.addedAt:desc`, everything else
   passes through) — Vela keys happen to be Plex-native today; this is
   the first divergence, kept in one visible spot.
3. **Jellyfin mapping:** `map_sort` gains
   `"episodeAddedAt" => "DateLastContentAdded"`.
4. **Frontend:** the `SORTS` entry carries `showOnly: true`; the sort
   `<select>` filters it out unless the active SECTION is a show
   section (`active?.sectionType === "show"`); the merged type view
   keeps its existing `TYPE_SORTS` filter (key not added). `select()`
   gains the symmetric guard `selectType()` already has: entering a
   non-show section with a show-only sort selected resets to
   `titleSort:asc` — today `select()` never resets `sort`, so the
   show-only key would otherwise leak into a movie-section request.

## Non-goals
- No merged All-view support (no DTO field; per-source only, like
  rating).
- No episode-level data fetching or client-side recomputation — the
  server owns the semantics.
- No new sort for movie/video sections.
- No persistence of the selected sort across sessions (unchanged
  behavior).

## Verification
- Unit tests (guard-proven red→green): the Plex key translation
  (new pure fn: the one divergent key + passthrough for every other
  ALLOWED_SORTS entry) and `map_sort`'s new arm (JF `SortBy` name).
- `npm run check` + `npm run build`; from `src-tauri/`: `cargo check
  --locked`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked`.
- Full E2E suite on the VM for regression. NO new scenario: the mock
  serves movies only — asserting the new ordering would need
  series/episode support in the mock, and the ordering semantics live
  server-side anyway. Recorded automation gap (same class as the
  item-detail flows): the owner playtest is the behavioral check.
- Owner playtest (0.1.44, real Plex): open a show library → sort
  "Last episode added" → a show whose newest episode arrived recently
  surfaces at the top (the exact case from the report); movie sections
  don't offer the option; switching from a show section sorted this way
  to a movie section lands on Title (A–Z), not an error.

## Review log
(plan-review pending)
