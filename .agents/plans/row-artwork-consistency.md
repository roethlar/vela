# Plan: Uniform artwork shape within a row

Status: DRAFT — not approved for implementation. Covers the `ISSUES.md` entry
(Open - Owner-Reported 2026-07-04): "Rows mix poster and content-frame
artwork" — 2:3 posters next to 16:9 episode stills at different heights in
the same Home row.

## Facts (confirmed by code reading, 2026-07-04)

- One component renders every card: the `poster` snippet in
  `src/routes/+page.svelte:672-737`, used by Home hub rows (`:804`) and the
  library grid (`:835`). Shape is chosen per item at a single choke point:
  `class:landscape={item.mediaType === "episode" || item.mediaType ===
  "video"}` (`:691`) → `.art` is 2:3 (`:1157`) or 16:9 (`:1204`); hub-row
  widths 118px vs 190px (`:1275-1287`) produce the height mismatch.
- Exactly one image field travels end-to-end: `ItemDto.poster`
  (`src-tauri/src/source/mod.rs:37`). Plex fills it from `thumb` only —
  `grandparentThumb`/`parentThumb`/`art` are received in XML and discarded
  (`src-tauri/src/plex_library.rs:1134-1151`); Jellyfin/Emby capture only
  `ImageTags.Primary` (`src-tauri/src/source/jellyfin.rs:438-442`); local
  episodes get a TVmaze episode still (`src-tauri/src/source/metadata.rs:
  393-427`). So an episode card cannot show its series poster today, and a
  movie card has no 16:9 art.
- Mixing only happens in hub rows (Continue Watching, On Deck, Recently
  Added TV): they interleave movies/shows (2:3) with episodes (16:9). Browse
  grids are naturally uniform (a season's episode list is all-16:9).

## Design (proposed: poster-uniform hub rows)

Hub rows render every card 2:3; episodes use their series poster. Browse
grids, season episode lists, and the queue drawer keep today's behavior.
Poster-uniform is proposed over landscape-uniform because portrait artwork
exists for every media type on every backend, while 16:9 art for movies is
not captured anywhere and is weakest for local files; the episode caption
(S·E line, `grandparentTitle`) already identifies the episode.

1. Backend — carry series artwork for episodes as a new optional field
   `ItemDto.series_poster` (`seriesPoster`, `src-tauri/src/source/mod.rs`):
   - Plex: parse `grandparentThumb` into `PlexVideo` (parser arms at
     `plex_library.rs:1134-1151`) and transcode it exactly like `poster`
     (`source/plex.rs:80-83`).
   - Jellyfin/Emby: capture `SeriesId` + `SeriesPrimaryImageTag` on
     `BaseItem` (`jellyfin.rs:414-429`) and build the series-primary URL with
     the existing `poster_url` shape (`jellyfin.rs:196-206`).
   - Local: reuse the show-level TVmaze lookup in `metadata.rs` to cache one
     show poster per series; if unavailable, leave `series_poster` empty.
   - Token exposure is unchanged in kind: these are the same
     poster-transcode URL forms already accepted as local-only exposure.
2. Frontend (`src/routes/+page.svelte`):
   - In hub-row rendering only: drop the `.landscape` class and render
     `item.seriesPoster ?? item.poster` for episode/video items; all hub
     cards use the 118px 2:3 geometry. The `.noart` fallback then inherits a
     uniform 2:3 box in rows too.
   - Browse grid (`:833-837`), episode lists, and the queue drawer thumb
     (`:897-901`) are untouched.

Non-goals: no backdrop/16:9 art capture for movies, no per-row "smart"
shape voting, no changes to image failure handling (`failedPosters`), no
detail-page artwork work.

## Verification

- Full CI set: `npm run check`, `npm run build`; from `src-tauri/`:
  `cargo check --locked`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked`.
- Rust unit tests: Plex XML parsing picks up `grandparentThumb`;
  Jellyfin series-primary URL assembly; ItemDto serialization field name.
- Owner playtest: Home with the known mixed row (movie + episode in Continue
  Watching) shows equal-height 2:3 cards; episode cards show series art with
  the S·E caption; a local-source episode row degrades to `.noart` (or show
  art if the lookup lands) without layout breakage.

## Open points to settle at approval

1. Confirm poster-uniform over landscape-uniform (landscape would need
   movie/show backdrop capture on all backends — larger, weaker for local).
2. Whether local-source series art ships in v1 or `series_poster` stays
   backend-server-only at first (local rows would show 2:3 `.noart` boxes).
3. Whether "Recently Added TV" should list shows/seasons instead of episodes
   (sidesteps stills entirely for that row) — product call, separate from
   this plan's mechanics.
