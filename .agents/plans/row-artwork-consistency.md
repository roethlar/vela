# Plan: Uniform artwork shape within a row

Status: APPROVED 2026-07-04 (owner), with each plan's "proposed" defaults adopted. Covers the `ISSUES.md` entry
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
   - Continue Watching: a single hero carousel replaces the card row — one
     large centered 16:9 card showing the most recently watched item (scene
     still for episodes, backdrop for movies), with the progress bar and
     title + S·E/episode-name caption. Prev/next arrows float overlaid on
     the hero image's left/right edges — not separate side controls — and
     swap the hero through the hub's other items in recency order (arrows
     are the baseline, proposed hover/focus-revealed to keep the artwork
     clean; swipe/trackpad is a nicety). The hero requests larger transcode
     dimensions than grid artwork.
   - On Deck: same 16:9 artwork rules (stills for episodes, backdrops for
     movies); whether it stays a landscape row under the hero or folds into
     the hero rotation is an open point below.
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
   row-policy-driven instead of mediaType-driven, plus a new hero-carousel
   block for the Continue Watching hub (hero-index state, prev/next
   handlers, landscape art selection `backdrop ?? poster` for movies vs the
   episode still). Catalog rows/grids render portrait for all items
   (`seriesPoster ?? poster` for episodes). The `.noart` fallback inherits
   the box shape of its context.

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
- Owner playtest against the reference screenshot: Continue Watching renders
  as the hero carousel with the last-watched item centered, prev/next
  cycling through the other recents (movie hero shows backdrop, episode
  hero shows its still, progress bar correct on each); On Deck and any
  16:9 row render equal heights; Recently Added rows render equal-height
  2:3 posters including episodic entries (series art); a local-source
  catalog row degrades to uniform 2:3 `.noart` boxes without layout
  breakage.

## Amendment 2026-07-04 (owner direction; supersedes the hero shape above)

Owner playtest found the hover-revealed arrows invisible in practice and the
server-fed hub unable to reflect a short play. Per the cover-flow decision in
`.agents/decisions.md` (2026-07-04): the hero becomes a cover-flow capped at
~30% of window height — older items fanned behind-left, newer behind-right
(foobar2000 reference), side cards clickable, arrows always visible — fed by
recents ∪ server continue hubs, newest first, deduped, rendered as ONE
consolidated hero. Vela records recents itself: the frontend snapshots the
item at play time (`record_recent`), the playback end notifier stamps the
final mpv position (`EndNotify` now carries it) and drops entries past the
watched threshold (`watched_threshold_percent`, default 95%). Known gap,
accepted: backend auto-advance plays (queue) are not snapshotted in v1 —
they typically run long enough to enter the server hub anyway.

## Open points to settle at approval

1. On Deck presentation: keep it as a 16:9 landscape row under the hero
   (proposed — its "next up" set is distinct from in-progress recents) or
   fold its items into the hero rotation.
2. Whether local-source series art ships in v1 or `series_poster` stays
   server-backend-only at first. (Implemented 2026-07-04 as
   server-backend-only; local episodic entries fall back to their episode
   still / `.noart` in portrait boxes. Local series art remains a follow-up.)
   (2026-07-09: the local series-art follow-up is DEAD — local sources
   removed, decision `.agents/decisions.md` 2026-07-08.)
3. Whether "Recently Added TV" should list shows/seasons instead of episodes
   (the reference shows series-level entries) — product call, separate from
   this plan's mechanics.
