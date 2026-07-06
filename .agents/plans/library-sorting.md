# Plan: Library view sorting (DRAFT — awaiting owner decision)

## Status
**DRAFT / proposed 2026-07-06.** Owner asked for a minimum sort set across all
library views. Not approved for implementation yet. No code written.

## Goal
Sort options available in **all** library views: **date added, date last played,
title, release date, folder** (owner's minimum set). Today sorting is partial and
inconsistent across views/backends.

## What already exists (do not rebuild)
- Frontend already has sort state + a six-option dropdown: `titleSort:asc`,
  `year:desc`, `addedAt:desc`, `originallyAvailableAt:desc`, `rating:desc`,
  `lastViewedAt:desc` (`src/routes/+page.svelte:87,92-99`). The dropdown renders
  only at a section/merged **top crumb** (`+page.svelte:1140-1146`).
- Backend whitelist `ALLOWED_SORTS` already contains all six tokens
  (`commands.rs:28-35`, `validate_sort` `:3624-3630`).
- **Plex per-source honors all six today** — `items()` passes the token straight
  to the Plex server (`source/plex.rs:226-270` → `plex_library.rs:807-808`). So on
  a Plex library section, "recently added" / "recently played" already work.
- **Jellyfin/Emby per-source honors all six server-side** via `map_sort`
  (`source/jellyfin.rs:592-610`).

## The real gaps
1. **Merged "All" view is locked to title/year.** `get_type_listing` rejects any
   other sort (`commands.rs:2345-2349`); `merge_sort_page` only orders by year or
   title (`:2701-2711`). Root cause: the fields needed to sort the union
   (`addedAt`, a global last-played) aren't carried on `ItemDto`.
2. **Local family (local/SMB/SSH) silently ignores four of six.** `sort_and_page`
   matches only `year`, everything else falls through to title
   (`source/local.rs:651-669`). So the dropdown's addedAt/originallyAvailableAt/
   rating/lastViewedAt are no-ops on a local library.
3. **`folder` sort/browse does not exist anywhere** — not in `ALLOWED_SORTS`, not
   in any `map_sort`/`sort_and_page`. **It IS meaningful across backends** (owner
   uses folder view in Plex regularly), not local-only. Every backend exposes the
   underlying path: local — `rating_key` is the path; Plex — `PlexPart.file` is
   already parsed (`plex_library.rs:177-186`) and dropped in `to_item`; JF/Emby —
   the item `Path` / `MediaSources[].Path` (request via `Fields=Path`). Plex also
   has a **native By-Folder browse** endpoint (`/library/sections/{key}/folder`).
   New token; cross-backend. **DECIDED 2026-07-06 (owner): a flat "sort by folder"**
   — group the listing by its containing directory and sort, reusing the existing
   sort pipeline. **NOT** a hierarchical folder-browse mode (no new navigation
   surface, no Plex `/folder` browse). This keeps the slice small: carry a
   folder/path per item, sort by it.
4. **`ItemDto` lacks `addedAt`.** Plex parses `addedAt` but drops it in `to_item`
   (`source/plex.rs`); JF doesn't request `DateCreated`; the local VFS exposes no
   modified-time (`source/vfs.rs:9-46` has `file_len`, no `modified`).
5. **`last_watched_at_ms` only populated by Plex.** JF sets it `None`
   (`source/jellyfin.rs:699-701`); local has only `recents.rs` (capped at 20).
6. Drill-down children (show→season→episode) are unsorted by design
   (`children()` takes no sort) — natural season/episode order. Probably correct;
   confirm with owner whether episode sorting is wanted (out of scope by default).

## Mapping owner's set → work
| Owner key | Status | Work |
|---|---|---|
| **title** | ✅ everywhere | none |
| **release date** | ✅ at year granularity (= `year:desc`, works all backends) | none for year granularity; full-date precision is a separate, larger change (new DTO date field) — **recommend year granularity, skip full-date** |
| **date last played** | ✅ Plex; ⚠️ JF null; local ≤20 | populate JF `last_watched_at_ms` (parse `DatePlayed`); local: sort by `recents` where present (partial, document the limit) |
| **date added** | 🔧 nowhere on DTO | add `added_at_ms` to `ItemDto`; populate Plex (already parsed), JF (`Fields=DateCreated`), local (`Vfs::modified()` mtime); extend `sort_and_page` |
| **folder** | 🔧 new, **cross-backend** | new token; derive from the file path each backend exposes (local `rating_key`; Plex `Part.file` — parsed, dropped; JF `Path`); or use Plex's native `/folder` browse. Owner uses this in Plex — confirm sort-vs-browse |

## Proposed slices (each its own commit + reviewloop codex + guard proof)
1. **Local family: honor all existing sort tokens.** Extend `sort_and_page`
   (`local.rs:651-669`) to handle `originallyAvailableAt`→year, `rating` (n/a →
   title fallback, documented), `lastViewedAt` (via recents lookup), and prep for
   `addedAt`/`folder`. Unit-testable (pure sort over a Vec<ItemDto>). This alone
   makes the dropdown honest on local libraries.
2. **`added_at_ms` on `ItemDto` + populate per backend.** Add the field; Plex
   maps its already-parsed `added_at`; JF adds `DateCreated` to the `Fields=` query
   and parses it; local adds `Vfs::modified()` (mtime; SMB/SSH stat — network, so
   only read during the walk, cached in the listing). Local `addedAt` sort arm.
3. **`folder` flat sort (cross-backend).** New `folder` token in `ALLOWED_SORTS` +
   frontend `SORTS`, offered on **all** backends (owner uses it in Plex). Carry a
   per-item folder/path: local from `rating_key`; Plex from `PlexPart.file` (already
   parsed — map it into the DTO); JF/Emby by adding `Path` to `Fields=`. Sort arm:
   group by parent dir, then title. Server sources sort the fetched page by folder.
   No folder-browse navigation (owner chose flat sort, 2026-07-06).
4. **Populate JF `last_watched_at_ms`** (parse the ISO-8601 `DatePlayed`) so
   last-played sort is real on Jellyfin/Emby, not just Plex.
5. **Relax the merged "All" view.** Once `added_at_ms` and last-played exist on the
   DTO for all backends, allow addedAt/lastViewedAt in `get_type_listing`
   (`commands.rs:2345-2349`) and extend `merge_sort_page` (`:2701-2711`). Document
   that a backend missing a value sorts last. Folder in the merged view is possible
   once every backend carries a folder/path (slice 3), but mixing folder trees
   across sources is messy — default to offering folder only in per-source views;
   revisit merged-folder if the owner wants it.

## Proportionality / "is this worth it?"
- **Cheapest high-value chunk = slices 1-2** (local honors sorts + date-added
  everywhere). That closes the "dropdown lies on local" bug and gives the two most
  useful non-title sorts. Slices 3-5 are incremental.
- **Recommend deferring full-date release precision** — `year:desc` already covers
  "release date" for practically all browsing; a full `originallyAvailableAt` date
  field is plumbing across four backends for marginal gain.
- Owner is Plex-first: slices 1-2 + 5 (merged) deliver the felt improvement; 3-4
  are polish for the local/JF paths.

## Verification
- Rust unit tests on the pure sort (`sort_and_page` variants) — guard-proven
  red/green per the repo pattern.
- Backend server-side sorts (Plex/JF) are already exercised; the DTO-field
  population gets a parse unit test per backend.
- E2E optional (the merged-view sort could reuse the mock-JF + local seed harness).

## Open decisions for owner
- Folder sort — **RESOLVED 2026-07-06**: flat "sort by folder" (group by containing
  dir), not a folder-browse mode.
- Full-date release sorting vs. year granularity (recommend: year).
- Episode-level sorting inside a season (default: leave natural order).
- Whether folder is offered in the merged "All" view or per-source only
  (recommend: per-source only initially).
