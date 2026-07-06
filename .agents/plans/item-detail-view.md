# Plan: Item detail / "more info" view (DRAFT — awaiting owner decision)

## Status
**DRAFT / proposed 2026-07-06.** Owner asked whether Plex exposes more metadata
than we surface (yes — substantially) and, if so, to plan a detail view like
Infuse / the Plex clients. Not approved for implementation. No code written.

## Goal
Clicking an item (movie, series, season, episode) opens a **detail overlay** with
richer info than the card: summary, rating(s), content rating, genres, cast +
crew, studio, air/release date, runtime, and technical media specs (resolution,
codec, HDR, audio, subtitles) — with **Play** (and the existing context actions)
from the detail surface. Graceful degradation where a backend has less.

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
3. **Frontend detail overlay** — a new Svelte component (not more of
   `+page.svelte`): poster/backdrop, title, meta row (year · runtime · content
   rating · rating), genres, summary, cast strip (headshots), crew, studio, a
   "Media" section (resolution/codec/HDR/audio/subs), and Play + existing actions.
   Reached from a card click (movies/episodes → detail instead of instant play; add
   a Play button in the detail) and/or a new context-menu "Info" entry. **Owner
   decision:** does a movie click open detail-then-play, or keep instant-play and
   put detail behind "Info"? (Infuse opens detail; instant-play is faster.)

## Proposed slices (each its own commit + reviewloop codex)
1. **`DetailDto` + `item_detail` trait method + `get_item_detail` command,
   Plex-only** (the richest, highest-ROI backend). Parse the full item-metadata
   response into the DTO. Backend unit tests on the XML/JSON parse.
2. **Frontend detail overlay component** wired to `get_item_detail`, Plex data.
   The felt feature lands here.
3. **Jellyfin/Emby `item_detail`** (widen `Fields=`, parse people/genres/ratings/
   streams).
4. **Local `item_detail`** — widen the `.nfo` parse (genre/cast/runtime/rating)
   and return what exists; degrade cleanly when only filename data is present.
5. (Optional) **Cast/crew headshots + genre chips polish**, episode-level detail.

## Proportionality / "is this worth it?"
- This is the single biggest step toward "richer client" — exactly the surface
  that makes Vela feel like Plex/Infuse. Owner asked for it, so it's wanted, but
  it's also the most scope. **Recommend Plex-first (slices 1-2)** as a self-
  contained deliverable: it's the owner's primary backend, the data is already
  fetched, and it proves the pattern before investing in JF/local parity.
- **Local tier will look sparse** (title/year/summary/poster) unless the NAS has
  `.nfo` sidecars; set that expectation. Widening the `.nfo` parse (slice 4) is the
  cheap lever for sidecar-rich libraries.
- Keep it one clean overlay; resist creeping into full Kodi-style metadata editing,
  scraper config, or a library-management surface (explicit non-goal below).

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
- Movie/episode click → detail-then-play, or instant-play + "Info" entry?
- Backend order: Plex-only first (recommended) vs. all backends together?
- How much to widen the local `.nfo` parse (cast/crew adds real value only if the
  owner's libraries carry it).
