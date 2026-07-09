# Plan: clickable people — actor/director/writer browse (DRAFT)

## Status
**DRAFT 2026-07-09.** Owner asked whether Plex supports clicking actor/director
names on info pages to find other titles ("does plex make it easy to click
things like actor / director names on the info pages and find other things by
that name?") and, on the affirmative answer, said "draft it". This plan awaits
the repo-convention codex plan-review loop and then an explicit owner go
before any code. Plex-first, consistent with the item-detail track's
2026-07-08 amendment.

## Goal
On the info surfaces (movie info page; shared episode page), a person's name —
cast members, directors, writers — is clickable when the backend can identify
the person, and opens a browse grid of everything else in that server's
libraries with that person. Sources that cannot identify people keep plain
text (the sparse-page bar: never broken, never erroring).

## What Plex provides (evidence + labeled assumptions)
- The `/library/metadata/{rk}` response Vela already parses for the detail
  surface carries people as child elements: `<Role tag="Name" role="Character"
  thumb="..." id="123" />`, `<Director tag="Name" id="456" />`, `<Writer ...
  id="789" />`. The numeric `id` is a server-local tag id. Vela's serde structs
  (`PlexRole`, `PlexTag` in `plex_library.rs`) currently capture only
  `tag`/`role`/`thumb` and DROP `id`.
- Library sections accept those ids as filters:
  `/library/sections/{key}/all?actor={id}` (also `director={id}`,
  `writer={id}`), returning every matching item of the section's native type.
  Standard `X-Plex-Container-Start/Size` paging applies.
- **ASSUMPTION (verify at implementation):** the `id` attribute is present on
  Role/Director/Writer in the owner's server responses, and the section filter
  accepts it. Verify with a fixture captured from a live response plus an
  env-gated live check (owner-cred-gated, like other Plex live tests). New
  Plex agents also emit a `tagKey` GUID; this plan uses only the numeric `id`.
- Tag ids are per-server. A person query therefore goes to the SAME source
  that served the detail (`DetailDto.source_id`); merged All-view cards
  already route detail through `detail_key`, so the person's source is the
  detail's source. No cross-source person merging (a different server has
  different ids for the same person) — explicit non-goal below.

## Design

### Backend (slice 1)
1. **Parse widening:** add `id: Option<u64>` to `PlexRole` and `PlexTag`
   (serde attr capture; `PlexTag` is shared by Genre/Country, which simply
   ignore it).
2. **DTO shape:** people become identifiable references:
   - `CastMember` gains `person_key: Option<String>` (namespaced
     `"<source_id>:<tag_id>"`, same convention as every other key).
   - `DetailDto.directors`/`writers` change from `Vec<String>` to
     `Vec<PersonRef { name, person_key: Option<String> }>`. DetailDto is
     fetched on demand and never persisted (no recents/cache/config
     round-trip), so the shape change is compatibility-safe; the frontend
     `Detail` mirror and both detail components' credit rendering update in
     the same slice (still plain text — clicks land in slice 2).
3. **Query path:** new `MediaSource` trait method with a graceful `Err`
   default (the `item_detail` pattern):
   `person_items(person_key, kind: actor|director|writer) -> Vec<ItemDto>`.
   Plex impl: enumerate the source's video sections (movie + show), fetch
   `/library/sections/{key}/all?{kind}={id}` with full
   Container-Start/Size pagination per section, map through the existing
   `to_item`, concatenate. Sections are per-type so results are naturally
   mixed movies + shows; sort merged results **year desc, title asc
   tiebreak** (person pages read newest-first; open dial below). No cap —
   person filters return modest sets; pagination is per-section and bounded
   by library size.
4. **Command:** `get_person_items(person_key, kind)` routed by key namespace
   (the `get_children` pattern) + `lib.rs` registration.

### Frontend (slice 2)
5. **Person results view:** mirror the SEARCH pattern, not the section
   pattern: a one-shot load into `items` with `hasMore=false`, `mode =
   "browse"`, a single crumb (`With <Name>` for actors; `Directed by <Name>`
   / `Written by <Name>` for crew), and a `personView` state (key, kind,
   name) parallel to `searchTerm` so `goCrumb(0)` re-runs the query the way
   a search root re-runs the search. Results are ordinary `ItemDto`s, so
   clicking them routes through the existing `open()` (movie → info page,
   show → seasons drill); Back walks the trail as everywhere else. The
   detail crumb bar gets this for free (the underlying trail is the person
   crumb).
6. **Click targets:** in `ItemDetail`, each cast card becomes a button when
   `person_key` is present (whole card), and Directed by / Written by render
   as per-name links instead of a joined string. In `SeasonDetail`, the
   panel's per-episode Directed by / Written by get the same treatment. No
   key → the exact current plain-text rendering (non-Plex sparse pages are
   automatically inert). Entering a person view closes the detail
   (`closeDetail()` then run the person query — same shape as the heading
   show link navigation).

## Slices (each its own commit + reviewloop codex + version bump)
1. **Backend + DTO:** id capture, PersonRef/CastMember shape, `person_items`
   Plex impl + command, frontend type mirror + rendering compile-through
   (no user-visible change). Unit tests, guard-proven red/green: id capture
   from fixture XML; filter-URL construction (kind → param, id, paging);
   namespacing (present id → `"src:id"`, absent id → None, never a dangling
   prefix).
2. **Frontend clicks + person view:** the browse view, crumb/reload wiring,
   clickable credits on both components. No JS runner (recorded gap) — owner
   playtest is the behavioral check.

## Non-goals
- No person profile page (bio, headshot hero) — the result is a plain browse
  grid with a labeled crumb.
- No Discover/streaming results — server library content only.
- No cross-source person merging (ids are server-local by design).
- No JF/Emby implementation in this pass; their clean equivalent
  (`/Items?PersonIds=`) is a recorded follow-up that slots into
  `person_items` with no frontend change, alongside the deferred
  `item_detail` backends.
- No genre/collection/studio chip filtering — separate ask if wanted.

## Verification
- Slice 1: `cargo test` guards above (fixture-driven, guard-proven), full CI
  command set on the Windows host baseline.
- Slice 2: svelte-check/build; owner playtest — click a cast member on a
  movie page (grid of their titles, newest first, movie/show mix), a
  director on an episode page, a result routes per the nav flip, Back
  returns to the info page's trail; a non-Plex sparse page shows plain-text
  names.
- Env-gated live check for the filter endpoint against the owner's server
  (assumption above), run once during slice 1.

## Open decisions for owner (defaults proposed, none blocking the draft)
- **Result order:** proposed year desc (newest first). Alternative: title A–Z
  to match library default.
- **Cast list length:** Plex returns the full cast (can be dozens); the cast
  strip already scrolls horizontally — proposed: keep all, no "top billed"
  truncation.
- **Episode-level crew links** (SeasonDetail): proposed yes, same mechanism.

## Review log
(plan-review loop pending)
