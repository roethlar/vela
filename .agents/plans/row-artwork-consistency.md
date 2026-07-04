# Plan: Uniform artwork shape within a row

Status: DRAFT — not approved for implementation. Covers the `ISSUES.md` entry
(Open - Owner-Reported 2026-07-04): "Rows mix poster and content-frame
artwork" — 2:3 posters next to 16:9 episode stills at different heights in
the same Home row. The poster-vs-content-view question was resolved by the
owner on 2026-07-04 as the split policy below (design-language decision,
`.agents/decisions.md`; reference
`reference_screens/infuse-home-reference.png`).

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
- Mixing only happens in hub rows: they interleave movies/shows (2:3) with
  episodes (16:9). Browse grids are naturally uniform.
- Local items never appear in resume hubs: the local source has no watch
  state (`played: None`, `local.rs:252`; `ProgressTarget::None`,
  `local.rs:508-524`), so Continue Watching / On Deck entries are always
  server-backed.

## Design (split policy — decided 2026-07-04, Infuse reference)

Resume rows show scenes; catalog rows and grids show posters; every row is
internally uniform.

1. Row policy:
   - Resume rows (Continue Watching, On Deck): every card 16:9 — episodes
     keep their scene stills (current behavior), movies/shows use backdrop
     art. Progress bar and the title + S·E/episode-name caption stay.
   - Catalog rows (Recently Added Movies/TV, similar hubs) and library
     grids: every card 2:3 — episodic entries render series artwork, not
     episode stills (the reference shows exactly this: a series poster for
     an episodic entry in Recently Added).
   - Season/episode drill-down lists and the queue drawer keep today's
     behavior (already uniform in context).
   - The movie-backdrop gap for local files does not bite: local items
     cannot appear in resume rows (no watch state), and catalog rows never
     use backdrops.
2. Backend — two new optional `ItemDto` fields
   (`src-tauri/src/source/mod.rs`):
   - `series_poster` (`seriesPoster`): Plex `grandparentThumb` (parser arms
     at `plex_library.rs:1134-1151`), Jellyfin/Emby `SeriesId` +
     `SeriesPrimaryImageTag` (`jellyfin.rs:414-429`; URL via the existing
     `poster_url` shape, `jellyfin.rs:196-206`), local via the show-level
     TVmaze lookup in `metadata.rs` where available.
   - `backdrop`: Plex `art` attribute (same parser and photo-transcode
     mechanism as `poster`, landscape dimensions), Jellyfin/Emby
     `BackdropImageTags[0]`. Not populated for local items.
   - Token exposure is unchanged in kind: these are the same
     poster-transcode URL forms already accepted as local-only exposure.
3. Frontend (`src/routes/+page.svelte`): the shape choice at `:691` becomes
   row-policy-driven instead of mediaType-driven. Resume rows render
   landscape for all items (`backdrop ?? poster` for movies/shows, stills
   for episodes); catalog rows/grids render portrait for all items
   (`seriesPoster ?? poster` for episodes). The `.noart` fallback inherits
   the row's box shape either way.

Non-goals: no true position frames (Plex BIF / Jellyfin trickplay preview
images — server-generated, a separate feature if ever), no per-row "smart"
shape voting, no change to image failure handling (`failedPosters`), no
nav/sidebar work (that's `.agents/plans/library-all-view-rework.md`).

## Verification

- Full CI set: `npm run check`, `npm run build`; from `src-tauri/`:
  `cargo check --locked`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked`.
- Rust unit tests: Plex XML parsing picks up `grandparentThumb` and `art`;
  Jellyfin series-primary and backdrop URL assembly; ItemDto serialization
  field names.
- Owner playtest against the reference screenshot: Continue Watching/On Deck
  render equal-height 16:9 cards including an in-progress movie (backdrop);
  Recently Added rows render equal-height 2:3 posters including episodic
  entries (series art); a local-source catalog row degrades to uniform 2:3
  `.noart` boxes without layout breakage.

## Open points to settle at approval

1. Resume-row card size: adopt larger, reference-like hero cards in this
   change, or keep current row heights and treat sizing as a later
   design-language pass (proposed: sizing later, shape now).
2. Whether local-source series art ships in v1 or `series_poster` stays
   server-backend-only at first.
3. Whether "Recently Added TV" should list shows/seasons instead of episodes
   (the reference shows series-level entries) — product call, separate from
   this plan's mechanics.
