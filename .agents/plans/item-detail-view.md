# Plan: Item detail / "more info" view (DRAFT — awaiting owner decision)

## Status
**DRAFT / proposed 2026-07-06.** Owner asked whether Plex exposes more metadata
than we surface (yes — substantially) and, if so, to plan a detail view like
Infuse / the Plex clients. Not approved for implementation. No code written.

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
**Plex (richest).** We already fetch most of it and discard it:
- *Already in the section-listing response we pull* (parse-widening, no new call):
  `contentRating`, `rating`, `audienceRating`, `studio`, `tagline`,
  `originallyAvailableAt`, `Genre[]`, and the already-parsed `Media`/`Part`
  codec+resolution block (`plex_library.rs:104-175`; dropped in
  `source/plex.rs:77-128`).
- *Needs the per-item `/library/metadata/{rk}` call* (we already hit it for
  playback, `plex_library.rs:840-954`): **cast `Role[]`** (name, character,
  headshot `thumb`), `Director[]`, `Writer[]`, `Country[]`, and per-`Stream` audio
  channels/codec + subtitle languages.

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
   `get_item_detail` command (`lib.rs` handler). Per backend:
   - Plex → `/library/metadata/{rk}` (already used for playback), parse the full
     child set (cast/crew/genre/media streams).
   - JF/Emby → `/Users/{u}/Items/{id}` with a wide `Fields=` set.
   - Local → return `CachedMeta` + a widened `.nfo` parse; degrade gracefully.
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
1. **`DetailDto` + `item_detail` trait method + `get_item_detail` command, Plex.**
   Parse the full `/library/metadata/{rk}` set (cast/crew/genre/media streams) + map
   the already-parsed-but-dropped `Part`/`Media` fields. Backend unit tests.
2. **Jellyfin/Emby `item_detail`** (widen `Fields=`, parse people/genres/ratings/
   streams). Keeps the flip from being Plex-only.
3. **Local `item_detail`** — widen the `.nfo` parse (genre/cast/runtime/rating);
   return what exists; a clean sparse page when only filename data is present.
4. **Info-surface components** (movie info page; shared episode info page) wired to
   `get_item_detail`, behind the current nav (dev-flag reachable), across all three
   backends. E2E where practical.
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
- Frontend: the detail overlay via the E2E harness (mock-JF seed already exists;
  a Plex fixture may need a mock or an env-gated live check — owner-cred-gated like
  other Plex live tests).

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
