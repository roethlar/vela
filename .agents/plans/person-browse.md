# Plan: clickable people — actor/director/writer browse (COMPLETE — owner-verified)

## Status
**COMPLETE 2026-07-09 — implemented and owner-playtest verified ("works
well") on 0.1.39.** Slice 1 backend `35fcc67` (loop `pb-s1` clean r1),
slice 2 frontend `b290b31` (loop `pb-s2` clean r1). Plan retained as the
design record; JF/Emby person browse stays deferred on an explicit owner go.

Original status: **REVIEWED 2026-07-09, awaiting owner go before any code.** Drafted on the
owner's ask ("does plex make it easy to click things like actor / director
names on the info pages and find other things by that name?" → "draft it").
The codex plan-review loop CLOSED accepted at r3 (base `0338176`, head
`926162c`; r1 and r2 each surfaced a real state-machine defect, both fixed —
see Review log). Plex-first, consistent with the item-detail track's
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

   **State exclusivity + refresh (plan-review r1+r2, binding):** `personView`
   joins the mutually-exclusive browse-root family and follows `searchTerm`'s
   disciplines exactly, or watch-state refreshes corrupt the grid:
   - *Entering* the person view clears the other roots (`active = null`,
     `activeType = null`, `searchTerm = ""`), bumps `loadGen`/`homeGen`
     exactly as `runSearch` does, and sets the single crumb.
   - *Root switches clear it; child drills preserve it (r2).* `select`,
     `selectType`, `runSearch`, `selectSource`, and `goHome` clear
     `personView` the way they already reset `searchTerm`/`active`. But
     `open()`'s show drill PRESERVES it, exactly as it preserves
     `searchTerm` today (`+page.svelte:547-565` appends the crumb without
     touching the root state) — that surviving root state is what lets
     `goCrumb(0)` re-run the root query (`+page.svelte:612-619`). Clearing
     it on the drill would strand the person crumb with nothing to re-run.
   - *`refreshWatchState`* (fires on `playback-ended` and every
     watched-state edit, `+page.svelte:143-153`) gains a `personView`
     branch beside the `searchTerm` branch, gated to the ROOT level:
     when `personView` is set and the person root is the visible level
     (`crumbs.length === 1`), re-run the person query; a drilled level
     under the person root (`crumbs.length > 1`) refreshes through the
     existing `resetAndLoad()` path, which works there because the drilled
     crumb has a `ratingKey`. Without the branch, the root level would
     blank (`items` emptied; person crumb has no
     `ratingKey`/`active`/`activeType` to reload) or repaint a stale
     `activeType` listing (`+page.svelte:475-495`). The gate deliberately
     does NOT copy search's ungated behavior (an ungated re-run yanks a
     drilled view back to the root — a live search quirk this plan does
     not import or fix).
   - *`goCrumb` re-entry* routes to the person re-run when the target root
     is a person crumb (the existing search-root special case, extended;
     depends on the survival rule above).
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
  names; AND the r1 refresh case: mark a title watched from the person
  grid's context menu (and finish a playback started from it) — the grid
  must stay populated with the person's titles, not blank or swap to
  another listing.
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
Plan-review loop (playbook `reviewloop`, reviewer `codex` 0.143.0, read-only).

**r1 — 2026-07-09 — verdict `reopened`, 1 finding, ADMITTED (verified against
live code).** Base `0338176`, head `a3a9fe0`. Finding: the person view had no
refresh story — `refreshWatchState` (`+page.svelte:143-153`) re-runs only
`searchTerm` or falls to `resetAndLoad()`, which empties `items` and then, for
a rootless person crumb, either loads nothing (blank grid) or reloads a stale
`activeType` listing (`+page.svelte:475-495`); fires on every playback-end and
watched-state edit. Fixed: added the binding "State exclusivity + refresh"
block to the person-view design (personView joins the mutually-exclusive
browse-root family: cleared by every navigation entry, clears the others on
entry, gets its own `refreshWatchState` and `goCrumb` re-run branches) and a
matching playtest check in Verification.

**r2 — 2026-07-09 — verdict `reopened`, 1 finding, ADMITTED (verified against
live code; it corrected an overshoot the r1 fix introduced — the loop working
as intended).** Base `0338176`, head `05ef618`. Finding: the r1 block listed
`open`'s show drill among the entries that clear `personView`, which
contradicts the live search-root pattern the plan mirrors — `open()` appends
a show crumb WITHOUT clearing `searchTerm` (`+page.svelte:547-565`), and
`goCrumb(0)` re-runs the root query only because that state survives
(`:612-619`); clearing on the drill would strand the person crumb with
nothing to re-run, recreating the r1 blank/stale path. Fixed: root switches
clear `personView`, child drills preserve it; the `refreshWatchState` person
branch is gated to the root level (`crumbs.length === 1`), with drilled
levels refreshing through the existing `ratingKey` path (and explicitly NOT
importing search's ungated root-yank quirk).

**r3 — 2026-07-09 — verdict `accepted`, 0 comments** (reviewed_sha `926162c`,
base `0338176`; `guard_confirmed:false` — read-only pass over a design doc).
The r2 finding is confirmed closed; no further material defect. **Plan-review
loop CLOSED — the plan awaits the owner's implementation go.** Healthy
converging loop: r1 (1) → r2 (1, correcting the r1 fix) → r3 (clean).
