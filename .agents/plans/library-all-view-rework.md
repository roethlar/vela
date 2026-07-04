# Plan: Library nav + "All" view rework (consolidated, deduped, cross-source)

Status: DRAFT — not approved for implementation. Covers the `ISSUES.md`
owner-direction entry (2026-07-04): the "All" view must become a consolidated
listing by content type — one entry per title backed by every source that
carries it, playback defaulting to the best source with a per-title override
— with persistent metadata caching for SMB/local as the performance
prerequisite. Related: `.agents/plans/smb-source-labeling.md` (gives mounts
real source identity; the ranking phase depends on it landing first). Design
north star: the 2026-07-04 design-language decision (`.agents/decisions.md`)
with `reference_screens/infuse-home-reference.png`.

## Facts (confirmed by code reading, 2026-07-04)

- The All view is a plain concatenation: `get_sections`/`get_hubs`/`search`
  fan out over `selected(None)` and `aggregate()` appends per-source results
  in registry order — no interleaving, merging, or dedup
  (`src-tauri/src/commands.rs:1758-1814, 2041-2077`). The nav renders one
  flat button per (library × source) with a `· sourceName` tag
  (`src/routes/+page.svelte:596-605`).
- A uniform grouping key already exists: `SectionDto.section_type` ∈
  `movie|show|video` across all backends (`commands.rs:2079-2085`,
  `plex.rs:115-142`, `jellyfin.rs:667-691`, `local.rs:296-320`), and
  `ItemDto.media_type` is equally uniform.
- No cross-source identity exists: no Plex `guid`, no Jellyfin/Emby
  `ProviderIds` are parsed; items carry only `source_id` + title/year.
- Local/SMB browsing is slow because nothing is persisted: every
  `sections/items/children/search` call re-walks the filesystem
  (`local.rs:215-224, 327-506`) with per-entry `canonicalize`
  (`local.rs:43-52`), and for SMB every call is network round-trips over a
  FUSE mount (`lib.rs:411-424`, `smb.rs:304-345`). The only cache today is
  `metadata_cache.json` (online lookup results keyed by path,
  `metadata.rs:43-122`) — whole-file, non-atomic writes.
- The gold-standard persistence pattern to copy is `config.rs:278-319`
  (temp file + `sync_all` + atomic rename, 0600) with the `update()` locking
  discipline (`config.rs:224-250`); no SQLite anywhere, JSON only.

## Phases (each independently landable, in order)

### Phase A — persistent listing cache for local-family sources

Goal: SMB/local browse serves instantly from a snapshot, refreshed in the
background.

- New module (e.g. `source/listing_cache.rs`): per configured root, persist
  the walked structure — sections, items per section, children per container,
  plus per-directory mtimes and a schema version. Storage: JSON file(s) in
  the config dir using the `save_config` atomic pattern; owner-only perms.
- Read path: `LocalSource::sections/items/children` serve the snapshot when
  present, then trigger a background re-walk (`spawn_blocking`, no shared
  locks across FS/network work) that diffs against dir mtimes and rewrites
  the snapshot; a change signal reuses the watch-state refresh event
  mechanism (`.agents/plans/watch-state-refresh.md`) or a sibling
  `listings-updated` event so the UI picks up changes.
- `detect_kind()` results and canonicalized root paths are cached per
  rebuild; per-entry symlink-escape checks stay (narrow-roots decision,
  2026-05-23, must not weaken).
- Search: v1 searches the snapshot when present (bounded, fast), falling
  back to the live walk when absent.
- Adjacent hardening in this phase (separate commit): make
  `metadata_cache.json` writes atomic via the same temp+rename pattern —
  closes the open P1 "bound and decouple metadata cache writes" remainder.

### Phase B — consolidated Library nav (reference-shaped)

Goal: navigation stops listing (library × source) and takes the reference
shape: Home, a consolidated Library split by content type, and
per-connection Files entries.

- New aggregated listing command (e.g. `get_type_listing(content_type,
  page)`): fan out `items()` across every section of that `section_type`
  over all sources, tag items with their source, and return a merged,
  title-sorted page.
- Frontend: replace the source-chip row + flat section tabs with the
  reference structure — `Home`; `Library` with content-type entries
  (Movies / TV Shows / Videos as present in the union); `Files` with one
  entry per connection (each server, each named mount from
  `smb-source-labeling.md`) preserving today's per-source sections for
  direct browsing.
- No dedup yet — duplicates appear once per source until Phase C, tagged so
  the change is honest.
- Open points: paging strategy (merged infinite scroll needs either
  per-source cursors merged server-side — proposed — or fetch-N-per-source
  and client merge); whether Phase B renders a true left sidebar or
  restyles the existing top nav first (structure identical either way).

### Phase C — cross-source identity and dedup

Goal: one card per title, backed by every source that carries it.

- Capture free provider ids: Plex `guid`s (imdb/tmdb/tvdb) in the XML
  parser; Jellyfin/Emby `ProviderIds`. Local items keep parsed title+year.
- Canonical identity: provider id match when both entries have one;
  otherwise normalized title (case/punctuation-folded) + year. Movies merge
  at item level; shows merge at show level (episodes browse within the
  chosen backing source — per-episode merging is out of scope).
- DTO: merged entries carry `backing: [{source_id, key}]` plus display
  fields chosen from the richest backing (server metadata preferred over
  local parse). Watch state on a merged card comes from a server-backed
  entry (local has none).
- Dedup lives in Rust at the Phase B aggregation layer, unit-tested.
- Known risk (accepted for v1, escape hatch = Phase D override): distinct
  cuts sharing title+year can false-merge; provider ids prevent this
  wherever a server exposes them.

### Phase D — playback source ranking + per-title override

Goal: play the best source by default; let the owner pick per title.

- Default rank order is a single policy constant (proposal: local folder >
  SMB/SSH mount > Plex > Jellyfin/Emby; owner tunes wording/order at
  approval). Requires per-mount source identity from
  `smb-source-labeling.md`.
- Play action on a merged card resolves: explicit per-title override if
  recorded, else rank order. Override UI: the merged item exposes its
  backing list ("Play from…"); the choice persists in config keyed by
  canonical identity (config save path already atomic + locked).
- Existing `play_by_key` routing works unchanged once the backing key is
  chosen.

## Verification

- Per phase: full CI set (`npm run check`, `npm run build`; from
  `src-tauri/`: `cargo check --locked`, `cargo clippy --all-targets --locked
  -- -D warnings`, `cargo test --locked`).
- Unit tests per phase: snapshot round-trip + mtime invalidation (A); merged
  paging determinism (B); identity normalization + provider-id precedence +
  false-merge guards (C); rank resolution incl. override (D).
- Owner playtests: SMB browse is instant on second open (A); All nav shows
  the three type entries and a merged grid (B); a title present on Plex +
  SMB renders once with both backings visible (C); default play picks the
  ranked source and an override sticks across restarts (D).

## Open points to settle at approval

1. Phase B paging strategy (server-side merged cursors vs client merge).
2. Default rank order wording in Phase D (performant vs reliable trade).
3. Whether Home hubs also merge in v1 (proposed: no — hubs stay per-source
   concatenation; only the type listings dedup).
4. Snapshot refresh cadence (on section open only — proposed — vs periodic).
