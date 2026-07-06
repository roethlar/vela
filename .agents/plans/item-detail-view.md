# Plan: Item detail / "more info" view (in plan-review)

## Status
**Owner go given 2026-07-06** ("continue" on the handoff's `next = Plex item
detail view (awaiting go)`). Per repo convention the **codex plan-review loop runs
before implementation**; it is in progress (see Review log). No code written yet —
implementation slice 1 starts once the loop reaches an `accepted` verdict.
Original framing: owner asked whether Plex exposes more metadata than we surface
(yes — substantially) and to plan a detail view like Infuse / the Plex clients.

## Goal
A **detail / info surface** with richer info than the card: summary, rating(s),
content rating, genres, cast + crew, studio, air/release date, runtime, and
technical media specs (resolution, codec, HDR, audio, subtitles) — with **Play**
(and the existing context actions) from that surface. Graceful degradation where a
backend has less.

## Owner UX ruling (2026-07-06, binding)
Navigation is spec'd — no per-slice choice:
- **Continue Watching carousel: click = play** (immediate, unchanged).
- **Any library view: click → drill in, never instant-play.**
  - **Movie**: click → **info page** (a full-screen page with Back, not an overlay);
    play from the poster / a Play button on that page.
  - **Show**: click → **seasons** → a season → its **episodes**.
  - **Episode**: **one shared info page** for the season's episodes, with the
    displayed info updating as the selected episode changes (episode list + a detail
    panel bound to the selection — not a separate page per episode).
- **No half-built state (binding):** this ships complete, not incrementally
  user-visible. The new click routing + info surfaces must work across the backends
  the owner uses before the navigation flips. See "Sequencing" — build behind the
  current nav and flip last, rather than landing a Plex-only info page into the live
  flow while local/JF movies click into a stub.

## Today: no detail surface at all
- The whole UI is one file; clicking a movie/episode **plays immediately**, a
  show/season **drills down** (`+page.svelte:554-568`). The only overlays are the
  context menu and the queue drawer.
- The context menu (`+page.svelte:1189-1211`) has Play / Play next / Add to queue /
  Mark watched / Remove-from-Continue / Play-from-source. **No "Info".**
- `ItemDto` (`source/mod.rs:43-92`) is the ceiling the frontend can show today:
  title, year, summary, duration, poster/backdrop, watched/position, episode
  positioning. **No rating, content rating, genre, cast, crew, studio, tagline,
  air date, or stream specs.**
- No per-item metadata command exists (`commands.rs`; grep `detail|cast|genre` →
  nothing).

## What each backend can give a detail view
**Plex (richest).** Plex's XML *carries* most of this, but — corrected after
plan-review r1 (idv-1) — **none of it is parsed today; there is no
"already-parsed, just-dropped" rich field to remap.** Two facts to build on:
- The section-listing / hub parse is `video_from_attrs`
  (`plex_library.rs:1203-1255`): a manual **attribute allowlist** with a
  `_ => {}` that drops every other attribute, and it sets `media: vec![]`. So
  `contentRating`/`rating`/`audienceRating`/`studio`/`tagline`/`originallyAvailableAt`
  are **not** captured, and **no `Media`/`Part` is populated from a listing** either.
  `PlexVideo` has serde `rename`s for `Media`, but nothing deserializes a rich
  `PlexVideo` via serde — the live parse is this attribute reader.
- The per-item `/library/metadata/{rk}` call we *do* make
  (`get_part_url_for_rating_key`, `plex_library.rs:846-960`) is a **playback-only**
  bespoke stream parse: it walks `Media`/`Part` to pick the best playable part into
  a local `candidates` vec. It builds **no reusable rich struct** and reads no
  cast/crew/genre.

Therefore slice 1 is genuinely new parsing, in two parts:
- *Scalar attributes* present on the item element — `contentRating`, `rating`,
  `audienceRating`, `studio`, `tagline`, `originallyAvailableAt` — added to the
  detail parser (attribute reads).
- *Child collections* that need **descending into child elements** (the current
  parsers never descend into a Video's children): `Genre[]`, cast `Role[]` (name,
  character, headshot `thumb`), `Director[]`, `Writer[]`, `Country[]`, the
  `Media`/`Part` codec+resolution block for display, and per-`Stream` audio
  channels/codec + subtitle languages. This is a new dedicated detail parser over
  the `/library/metadata/{rk}` response, **not** attribute-widening of
  `video_from_attrs`. Budget slice 1 as the heaviest slice accordingly.

**Jellyfin/Emby.** Same endpoint, just widen the `Fields=` query
(`source/jellyfin.rs:780` etc. currently request only `Overview,ProviderIds`):
gains `Genres`, `People` (cast/crew + character + `PrimaryImageTag`),
`CommunityRating`, `CriticRating`, `OfficialRating`, `Studios`, `Taglines`,
`PremiereDate`, `MediaStreams` (audio/subtitle detail).

**Local / SMB / SSH (weakest).** `CachedMeta` ceiling is title/year/summary/poster
(`source/metadata.rs:21-28`); `.nfo` parses only `<title>/<year>/<plot>`
(`:216-229`) and ignores `<genre>/<director>/<actor>/<runtime>/<rating>/<studio>`;
iTunes gives summary+poster+year. **No runtime, cast, or genre today.** A detail
view here shows mostly title/year/summary/poster unless we widen the `.nfo` parse
(cheap win for sidecar-rich libraries — most metadata is already in the file).

## Season / episode
- Plex episode (via `/children`) already carries summary, duration, index,
  parentIndex, grandparentTitle, still (`thumb`→backdrop); a detail call adds air
  date, episode-level directors/writers/guest cast, stream specs.
- TVmaze (`metadata.rs:419-454`) gives episode name/summary/still; **no air date**
  (hardcoded `year: None`), no runtime. Seasons skip online lookup.

## Design (recommended)
1. **New `DetailDto`** (superset of `ItemDto` for one item) rather than bloating
   the hot listing `ItemDto`: keeps the grid path lean; the detail view fetches on
   demand.
2. **New trait method `MediaSource::item_detail(rating_key) -> DetailDto`** + a
   `get_item_detail` command (`lib.rs` handler). Give the trait method a graceful
   default (like `mark_played`/`remove_from_continue` in `source/mod.rs:170-182`) so
   backends opt in. Per backend:
   - Plex → `/library/metadata/{rk}` (already used for playback), parse the full
     child set (cast/crew/genre/media streams) — the new detail parser from
     "What each backend / Plex", not a remap of already-parsed fields.
   - JF/Emby → `/Users/{u}/Items/{id}` with a wide `Fields=` set.
   - Local → return `CachedMeta` + a widened `.nfo` parse; degrade gracefully.
   **Local-family off-loading (idv-3, binding):** the local/SMB/SSH `item_detail`
   reads `.nfo`/`CachedMeta` off disk and native SMB may hit the network. Those
   reads MUST run on the blocking pool (`run_blocking`/`spawn_blocking`), never on an
   async worker or under a shared lock — the repo's lock-across-blocking invariant,
   the same pattern `LocalSource` and the slice-7 `resolve_stream` fix (`e7c5231`)
   already follow. Opening a detail page on a slow/wedged mount must not stall the app.

   **Detail-source policy for merged cards (idv-2, binding):** a merged All-view
   card's `rating_key` is **not** a safe detail key. `rank_backings`
   (`commands.rs:2505-2545`) rewrites `rating_key`/`source_id` to the kind-ranked
   *play* face — and `kind_rank` (`commands.rs:2475`) ranks `local`=0 above
   `plex`=2/`jellyfin`=3, so the play face is usually the **local** copy while the
   card's rich display fields came from the Plex/JF backing. Calling
   `get_item_detail(play rating_key)` would open the sparse local page for a card
   that showed rich server metadata. Fix: the info view resolves a **detail key**
   to the richest-metadata backing — reuse the existing `backing: Vec<BackingRef>`
   (already on the merged `ItemDto`) and pick the first `plex|jellyfin|emby` backing,
   exactly mirroring how `watch_key` (`rank_backings`, `commands.rs:2532`) routes
   watched-state to a server backing when the play face is local-family. Play still
   uses the play `rating_key`; only *detail* re-targets. Non-merged cards use their
   own key unchanged.
3. **Frontend info surfaces** — new Svelte component(s), not more of `+page.svelte`:
   - **Movie info page**: poster/backdrop, title, meta row (year · runtime · content
     rating · rating), genres, summary, cast strip (headshots), crew, studio, a
     "Media" section (resolution/codec/HDR/audio/subs), and a **Play button on the
     poster**. Reached by clicking a movie card.
   - **Show → seasons → episodes** drill: clicking a show lists seasons; a season
     lists episodes (extends the existing `get_children` drill, `+page.svelte:554`).
   - **Episode info page (shared)**: the season's episode list plus a detail panel
     bound to the selected episode — selecting an episode updates title/still/
     summary/air-date/runtime/stream-specs in place; Play acts on the selection.
     **Two correctness requirements (idv-4, binding):** (a) `get_children` is
     **paged** (`+page.svelte:517`, `start`/`size` = `PAGE`); the episode list must
     load *all* pages (or load-on-demand as the user scrolls) — rendering only the
     first window silently drops episodes past `PAGE` in long seasons. (b) The
     per-selection `get_item_detail` fetch is async and racy: a fast episode switch
     can resolve an older fetch last and paint stale detail. Guard it with a
     **selection generation token** in the mould of the existing
     `homeGen`/`sourceGen`/`loadGen` guards (`+page.svelte:361-365`) — bump on each
     selection, and drop any detail response whose generation is stale.
   Nav wiring per the binding UX ruling (CW carousel click still plays).

## Sequencing — "no half-built state" (binding)
Because the click routing flips globally (movie click stops playing, starts opening
info), the info surface must be real on every backend the owner browses before that
flip is user-visible. Reconcile with the repo's commit-per-slice + reviewloop
discipline by **building behind the current nav and flipping last**:
- Slices 1-4 add backends + the component but keep the **old click behavior live**
  (component reachable only behind a dev flag / not yet wired to card click).
- The **final slice flips the navigation** once movie-info, show/season/episode
  drill, and the shared episode page all work across Plex + JF/Emby + local (with
  local degrading to a clean sparse page, not a broken/empty one).
- Each intermediate slice is still an independently committed, reviewed, guard-proven
  unit — it just isn't the user-visible entry point until the flip.

## Proposed slices (each its own commit + reviewloop codex; nav stays old until the flip)
1. **`DetailDto` + `item_detail` trait method (graceful default) + `get_item_detail`
   command, Plex.** Write the **new dedicated detail parser** over the
   `/library/metadata/{rk}` response — scalar attributes (contentRating/rating/
   audienceRating/studio/tagline/originallyAvailableAt) **and** child collections
   (cast `Role[]`/`Director[]`/`Writer[]`/`Genre[]`/`Country[]`/`Media`/`Part`/
   `Stream`), descending into child elements. This is not a remap of already-parsed
   fields (idv-1) — it is the heaviest slice. Backend unit tests over fixture XML.
2. **Jellyfin/Emby `item_detail`** (widen `Fields=`, parse people/genres/ratings/
   streams). Keeps the flip from being Plex-only.
3. **Local `item_detail`** — widen the `.nfo` parse (genre/cast/runtime/rating);
   return what exists; a clean sparse page when only filename data is present.
   The reads run **off-runtime on the blocking pool** (idv-3), never under a lock.
4. **Info-surface components** (movie info page; shared episode info page) wired to
   `get_item_detail`, behind the current nav (dev-flag reachable), across all three
   backends. Resolve the **detail key to the richest backing** for merged cards
   (idv-2) and apply the **episode-list paging + per-selection generation guard**
   (idv-4). E2E where practical.
5. **Flip the navigation** (the binding UX ruling): movie click → info page; show →
   seasons → episodes; episode → shared info page; CW carousel unchanged. This is
   the slice that makes it user-visible — only lands when 1-4 are complete.

## Proportionality / "is this worth it?"
- This is the single biggest step toward "richer client" — exactly the surface that
  makes Vela feel like Plex/Infuse. Owner asked for it and spec'd the flow, so it's
  wanted; it's also the most scope, and the "no half-built" rule means it lands as
  **one complete feature**, not a trickle. Budget it as a multi-slice project, not a
  quick win.
- **Local tier will look sparse** (title/year/summary/poster) unless the NAS has
  `.nfo` sidecars; widening the `.nfo` parse (slice 3) is the cheap lever. A sparse
  page is acceptable ("not broken"); an empty/erroring one is not.
- Hold the line at a **view**: resist creeping into Kodi-style metadata editing,
  scraper config, or artwork pickers (explicit non-goals below).

## Non-goals
- No metadata editing, no scraper/agent configuration, no artwork picker.
- No change to the delegated-mpv playback model.
- No new API keys (local stays keyless iTunes/TVmaze + `.nfo`).

## Verification
- Backend: unit tests on each backend's detail parse (fixture responses),
  guard-proven.
- Frontend: the detail route/page via the E2E harness (mock-JF seed already exists;
  a Plex fixture may need a mock or an env-gated live check — owner-cred-gated like
  other Plex live tests). Cover the idv-4 guards where practical: a long season
  (episodes past one `PAGE`) still lists every episode, and rapid episode selection
  never paints stale detail.

## Open decisions for owner
- Click flow — **RESOLVED** (binding UX ruling above): CW carousel plays; movie →
  info page (Play on poster); show → seasons → episodes; episode → shared info page.
- Backend order — **RESOLVED**: all backends before the nav flip (no half-built
  state). Slices land internally; the flip is last.
- How much to widen the local `.nfo` parse (cast/crew adds real value only if the
  owner's libraries carry it) — a scope dial, not a blocker.
- Info page presentation — **RESOLVED 2026-07-06 (owner-confirmed): full-screen
  route** (a dedicated info page with Back), NOT a floating overlay/popup. A movie's
  info page is just another drill-in level (like show→seasons→episodes), so Back
  behaves identically everywhere and the nav model stays consistent.

## Review log
Plan-review loop (playbook `reviewloop`, reviewer `codex` 0.142.5, read-only),
base=head `410fa4e`.

**r1 — 2026-07-06 — verdict `reopened`, 4 findings, all ADMITTED (coder
independently verified each against live code before accepting; none were style/
taste).**
- **idv-1 (HIGH) — Plex "already-parsed, just-dropped" premise is false.** Confirmed:
  `video_from_attrs` (`plex_library.rs:1203-1255`) is an attribute allowlist with
  `_ => {}` and `media: vec![]`; the per-item call `get_part_url_for_rating_key`
  (`plex_library.rs:846-960`) is a playback-only stream parse building no rich
  struct. Fixed: rewrote "What each backend / Plex" and slice 1 — all rich fields are
  genuinely new parsing (scalar attrs + child-element descent); slice 1 is the
  heaviest slice.
- **idv-2 (HIGH) — a single `get_item_detail(rating_key)` opens the wrong (sparse
  local) detail for merged cards.** Confirmed: `rank_backings`
  (`commands.rs:2505-2545`) rewrites `rating_key` to the kind-ranked play face, and
  `kind_rank` (`commands.rs:2475`) ranks local(0) above plex(2)/jf(3). Fixed: added a
  binding **detail-source policy** — resolve the detail key to the richest
  (`plex|jellyfin|emby`) backing via the existing `backing` list, mirroring
  `watch_key`; play still uses the play key.
- **idv-3 (MEDIUM) — local-family detail omitted the lock-across-blocking
  invariant.** Fixed: slice 3 + Design require the `.nfo`/`CachedMeta`/SMB reads to
  run on the blocking pool, off async workers and locks (same as `LocalSource` /
  `resolve_stream` `e7c5231`).
- **idv-4 (MEDIUM) — shared episode page under-specified for the paged/reactive
  frontend.** Confirmed `get_children` is paged (`+page.svelte:517`) and the guard
  pattern exists (`homeGen`/`sourceGen`/`loadGen`, `+page.svelte:361-365`). Fixed:
  the episode page must page the full list and guard the per-selection detail fetch
  with a selection generation token.

r2 dispatched after these revisions (verdict pending).
