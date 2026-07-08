# Plan: Library view sorting (DRAFT — awaiting owner decision)

## Status
**LANDED 2026-07-06.** All slices landed (`c368270`, `9a47d43`, `21552c9`) +
`reviewloop codex` converged r1-r3 (r1 3 findings, r2 1 finding, r3 accepted;
fixups `c904c66`, `19b2735`; trail `.agents/review/index.md` loop `sort-1`).
Folder dropped; JF/local last-played population deferred. Effective set delivered:
date added, date last played, title, release date — per-source AND merged views.
REMAINING: owner playtest.

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
3. **`folder` sort — DROPPED 2026-07-06 (owner).** The owner uses Plex's folder
   view only for *podcasts* (audio, where flexget can't reliably inject metadata,
   so folder/file names are the fallback). Vela is video-only, and video doesn't
   need it. Additionally, Plex has no server-side path sort (its By-Folder is a
   browse mode), so a correct folder sort on Plex would require fetching whole
   sections client-side — not worth it for a need that doesn't apply to video.
   Folder sort is out of scope. Owner's effective minimum set is now: date added,
   date last played, title, release date.
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
| **folder** | ❌ DROPPED (owner 2026-07-06) | podcast/audio need only; video-only Vela doesn't need it; Plex has no server-side path sort |

## Slices
1. **LANDED (`c368270`) — local family honors release-date + last-played sorts.**
   `sort_and_page` handled only year; now `originallyAvailableAt`→year and
   `lastViewedAt`→`last_watched_at_ms`, each with a case-insensitive title tiebreak;
   unsupported tokens fall back to title deterministically. Unit-tested, guard-proven.
2. **LANDED (`9a47d43`) — date-added sort.** Added `added_at_ms` to `ItemDto` +
   `Vfs::modified_ms` (file mtime; default None, real impl for `StdFs`; SMB left None
   — deferred). Populated from the mtime during the local walk and from Plex `addedAt`.
   Local `addedAt` sort arm. JF `added_at_ms` stays None (server sort already works;
   DateCreated-in-Fields is a follow-up). Unit-tested, guard-proven.
3. **NEXT — relax the merged "All" view.** Allow addedAt / lastViewedAt /
   originallyAvailableAt in `get_type_listing` (`commands.rs:2345-2349`) + extend
   `merge_sort_page` (`:2701-2711`), and widen the frontend `TYPE_SORTS`
   (`+page.svelte:76`). A backend missing a value sorts last (document it). This is
   what makes the sorts work in *all* views, not just per-source.
4. **DROPPED — `folder` sort** (owner 2026-07-06: podcast/audio need, not video).
5. **DEFERRED (low value for a Plex-first owner) — populate JF `last_watched_at_ms`
   + `added_at_ms`** (parse the ISO-8601 dates, add to `Fields=`). Only matters for
   the merged view ranking JF items correctly; JF per-source sorts server-side today.
   Local last-played is likewise unpopulated (recents aren't merged into library
   items) — a follow-up if the owner wants local last-played sorting.

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
- Folder sort — **DROPPED 2026-07-06** (podcast/audio need, not video).
- Full-date release sorting vs. year granularity — using **year** (covers it).
- Episode-level sorting inside a season — left natural order.
