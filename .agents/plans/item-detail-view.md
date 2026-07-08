# Plan: Item detail / "more info" view (APPROVED — implementing)

## Status
**APPROVED for implementation 2026-07-06.** Owner go given ("continue" on the
handoff's `next = Plex item detail view (awaiting go)`), and the repo-convention
**codex plan-review loop CLOSED accepted at r3** (base=head `410fa4e`; three rounds,
six findings idv-1..6 all resolved — see Review log). Implementing slices 1-5, each
its own commit + `reviewloop codex` + version bump; nav stays old until the slice-5
flip ("no half-built state"). One non-blocking owner open-decision remains (merged-
show episode playback source, idv-5 — default set, needed only by slice 4/5).
Original framing: owner asked whether Plex exposes more metadata than we surface
(yes — substantially) and to plan a detail view like Infuse / the Plex clients.
**AMENDED 2026-07-08 (owner, Plex-first)** — see "Owner amendment 2026-07-08"
below; it supersedes the backend-coverage half of "no half-built state" and
re-orders the remaining slices.

## Owner amendment 2026-07-08 (Plex-first)
Owner directives (verbatim): "sources other than plex are deprioritized. get this
perfect with plex, then we'll worry about the others." And, settling the non-Plex
routing fork: "plex items only go to detail page from library views, not from
continue watching carousel. other sources should behave the same way."
Recorded in `.agents/decisions.md` (2026-07-08).

What changes:
- **JF/Emby `item_detail` (old slice 2) and local `item_detail` (old slice 3) are
  DEFERRED** — do not start without an explicit owner go. The trait's graceful-`Err`
  default already covers them at runtime.
- **The navigation flip is uniform across ALL sources** (refined ruling): library-view
  clicks route to the detail surface (movie → info page; show → seasons → episodes;
  episode → shared info page); the Continue Watching carousel keeps click-to-play
  everywhere. Non-Plex items open the SAME pages rendered **sparse from listing data**
  (`ItemDto`: title/year/summary/poster/duration). `get_item_detail` is still called;
  an `Err` (unimplemented backend) falls back silently to listing data — never an
  error state. When the deferred backends land later, the same pages simply get
  richer; no nav change. This also covers merged cards whose `detail_key` resolves to
  a backing with no `item_detail` yet (e.g. JF-only): detail errs → sparse fallback.
- **What "no half-built state" still means:** the flip lands only when the surface is
  complete and polished for Plex AND the non-Plex sparse page is clean (never
  broken/empty/erroring) — the same bar the plan already set for local.

Amended slice order (slice 1 LANDED 0.1.31, unchanged):
- **Slice 2 (was slice 4, now Plex-scoped + sparse fallback):** info-surface
  components (movie info page; shared episode info page; seasons/episodes drill)
  wired to `get_item_detail`, dev-flag reachable behind the current nav; backend
  `detail_key` + server-preferred detail rank in `rank_backings` (idv-2/6);
  merged-show drill through `detail_key` (idv-5); episode-list paging + per-selection
  generation guard (idv-4); sparse fallback from listing `ItemDto` when detail errs.
- **Slice 3 (was slice 5):** flip the navigation for all sources per the refined
  ruling above.
- **Then:** owner playtest → polish rounds ("get this perfect with plex").
- **Deferred:** JF/Emby `item_detail` (old slice 2); local `.nfo` widening (old
  slice 3). Resume only on owner go; they slot in with no nav change.

The amendment itself gets no separate plan-review round; its technical content
(sparse fallback, slice re-scope) is reviewed as part of the amended slice 2's
`reviewloop codex`.

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
- **No half-built state (binding — SUPERSEDED IN PART 2026-07-08):** this ships
  complete, not incrementally user-visible. ~~The new click routing + info surfaces
  must work across the backends the owner uses before the navigation flips.~~ Per the
  owner amendment above: the flip no longer waits for JF/Emby/local `item_detail`;
  it waits for a polished Plex surface plus a clean sparse page on other sources.
  Build behind the current nav and flip last still holds.

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
**Plex (richest).** How Plex XML is parsed today (corrected across plan-review
r1→r2; r1 mis-stated this and r2/idv-1 corrected it):
- The **listing** parse (`get_items`, `plex_library.rs:609-694`) is **serde**:
  quick_xml captures element order (pass 1), then `serde_xml_rs::from_str` into
  `ItemsContainer` (`:669`). So `PlexVideo` **is** deserialized via serde, and its
  `#[serde(rename="Media")] media` / `PlexMedia`'s `#[serde(rename="Part")] parts`
  (`plex_library.rs:138`,`:179`) **are** populated from listings — the codec/
  resolution block is genuinely already parsed. (`video_from_attrs`,
  `plex_library.rs:1203-1255`, is a *separate* attribute-only reader used on other
  paths — not the listing parser; ignore it for detail.)
- What is **not** parsed today (absent from the `PlexVideo`/`PlexMedia`/`PlexPart`
  serde structs): the scalar attrs `contentRating`, `rating`, `audienceRating`,
  `studio`, `tagline`, `originallyAvailableAt`; and the child collections `Genre[]`,
  cast `Role[]` (name/character/headshot `thumb`), `Director[]`, `Writer[]`,
  `Country[]`, and per-`Stream` audio-channel/codec + subtitle detail.
- The per-item `/library/metadata/{rk}` call we already make
  (`get_part_url_for_rating_key`, `plex_library.rs:846-960`) is a playback-only
  stream parse — it builds no reusable rich struct.

Slice-1 approach (simplest robust): Plex `item_detail` makes **one
`/library/metadata/{rk}` call** and parses the whole response with a **new serde
struct** (`PlexDetail` in a `DetailContainer`), the *same idiom* as
`ItemsContainer`. serde_xml_rs already maps repeated child elements to `Vec`
(`PlexVideo.media`, `PlexVideo.guids` prove it), so `Genre[]`/`Role[]`/`Director[]`/
`Writer[]`/`Country[]`/`Media`/`Part`/`Stream[]` are ordinary struct fields — **not
a hand-rolled streaming descent, and no need to touch the listing parser.** The
metadata endpoint returns the authoritative full record, so slice 1 doesn't depend
on which fields the *listing* happens to include. Still the heaviest slice (the
struct + mapping is large), but mechanically it's serde widening.

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

   **Detail-source policy for merged cards (idv-2 + idv-6, binding):** a merged
   All-view card's `rating_key` is **not** a safe detail key. `rank_backings`
   (`commands.rs:2505-2545`) rewrites `rating_key`/`source_id` to the kind-ranked
   *play* face — and `kind_rank` (`commands.rs:2475`) ranks `local`=0 above
   `plex`=2/`jellyfin`=3, so the play face is usually the **local** copy while the
   card's rich display fields came from a server backing. Calling
   `get_item_detail(play rating_key)` opens the sparse local page for a card that
   showed rich server metadata.
   - **Do NOT derive the detail target by scanning the post-rank `backing` list**
     (idv-6): `dedup_across_sources` puts the display face — the richest by
     summary/poster/year — at `backing[0]` (`commands.rs:2678`), but `rank_backings`
     then **re-sorts `backing` by play preference** and overwrites `source_id`
     (`:2520-2527`). So after ranking, neither `backing[0]` nor "the first
     `plex|jellyfin|emby` entry" reliably equals the display face (Plex sorts before
     Jellyfin by `kind_rank`, so a Jellyfin-faced card could detail from Plex).
   - **Fix:** add an explicit **`detail_key`** field to the merged `ItemDto`,
     computed **in `rank_backings`** (which holds all backings + their kinds) by a
     dedicated **detail rank that prefers metadata-rich servers** — `plex` <
     `jellyfin`/`emby` < `smb`/`ssh` < `local` (the *reverse* of `kind_rank`, because
     the detail page exists to show cast/genre/streams that only servers carry).
     Deterministic, independent of play order and of `watch_key`. When only
     local-family backings exist, `detail_key` is that local item (a clean sparse
     page — accepted). Play still uses `rating_key`; only *detail* uses `detail_key`.
     Non-merged cards leave `detail_key` unset and use their own key.
3. **Frontend info surfaces** — new Svelte component(s), not more of `+page.svelte`:
   - **Movie info page**: poster/backdrop, title, meta row (year · runtime · content
     rating · rating), genres, summary, cast strip (headshots), crew, studio, a
     "Media" section (resolution/codec/HDR/audio/subs), and a **Play button on the
     poster**. Reached by clicking a movie card.
   - **Show → seasons → episodes** drill: clicking a show lists seasons; a season
     lists episodes (extends the existing `get_children` drill, `+page.svelte:554`).
     **Merged-show drill (idv-5, binding):** `get_children` routes whatever key it is
     handed (`commands.rs:3262`), and a merged show's `rating_key` is the *play* face
     (local, per `rank_backings`), so drilling a merged local+server show would open
     the **local** seasons/episodes — whose items carry no `backing`, so the episode
     info page can't retarget to a rich server. Drill the show through **`detail_key`**
     (the server backing) so seasons/episodes come from the rich source. Its episodes
     then also play from that server; the top-level per-title override stays a
     merged-*movie* concern (episodes aren't individually merged today). If only
     local backings exist, the drill stays local (sparse but present — accepted). See
     the open decision below on episode playback-source for merged shows.
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

## Sequencing — "no half-built state" (binding — superseded in part by the 2026-07-08 owner amendment)
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

## Proposed slices (each its own commit + reviewloop codex; nav stays old until the flip) — ORIGINAL ORDER; superseded by the amended slice order in the 2026-07-08 owner amendment (slices 2-3 below DEFERRED)
1. **`DetailDto` + `item_detail` trait method (graceful default) + `get_item_detail`
   command, Plex.** Plex `item_detail` = one `/library/metadata/{rk}` call parsed
   into a **new `PlexDetail` serde struct** (`DetailContainer`), same idiom as the
   existing `ItemsContainer` — scalar attributes (contentRating/rating/audienceRating/
   studio/tagline/originallyAvailableAt) as struct attrs and child collections
   (`Role[]`/`Director[]`/`Writer[]`/`Genre[]`/`Country[]`/`Media`/`Part`/`Stream[]`)
   as `#[serde(rename=…, default)] Vec<…>`, exactly as `PlexVideo.media`/`.guids`
   already do (idv-1: serde descends into children — no hand-rolled parser, no touch
   to the listing path). Heaviest slice (large struct + `DetailDto` mapping). Backend
   unit tests over fixture XML.
2. **Jellyfin/Emby `item_detail`** (widen `Fields=`, parse people/genres/ratings/
   streams). Keeps the flip from being Plex-only.
3. **Local `item_detail`** — widen the `.nfo` parse (genre/cast/runtime/rating);
   return what exists; a clean sparse page when only filename data is present.
   The reads run **off-runtime on the blocking pool** (idv-3), never under a lock.
4. **Info-surface components** (movie info page; shared episode info page) wired to
   `get_item_detail`, behind the current nav (dev-flag reachable), across all three
   backends. Includes the backend `detail_key` field + the **server-preferred detail
   rank** in `rank_backings` (idv-2/idv-6), routing both `get_item_detail` and the
   merged-show `get_children` drill through `detail_key` (idv-5); plus the
   **episode-list paging + per-selection generation guard** (idv-4). E2E where
   practical.
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
- **Merged-show episode playback source (idv-5, surfaced by plan-review r2).** The
  plan's default drills a merged local+server show through `detail_key` (the server),
  so its episodes both *show* rich detail and *play* from the server — even if a local
  copy exists. Rationale: episodes aren't individually merged, so shown==played stays
  consistent and rich; the local-preferred play override remains a merged-*movie*
  feature. Alternative if the owner prefers local direct-play for such episodes: keep
  the drill on the play face and accept a sparse (local) episode page. Rare for a
  Plex-first library (needs the *same show* on Plex **and** a local folder); default
  stands unless the owner says otherwise. Non-blocking for slices 1-3.
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

**r2 — 2026-07-06 — verdict `reopened`, 3 findings, all ADMITTED (verified against
live code; one of them corrected a mistake r1 introduced — the loop working as
intended).**
- **idv-1 correction (HIGH) — r1's fix was itself factually wrong.** r1 (both codex
  and coder) keyed on `video_from_attrs` and concluded Media/Part aren't parsed from
  listings. False: the listing parser is `get_items` (`plex_library.rs:609-694`),
  which uses `serde_xml_rs::from_str` into `ItemsContainer` (`:669`) and **does**
  populate `PlexVideo.media`/`PlexMedia.parts` via serde `rename`s (`:138`,`:179`);
  `video_from_attrs` is a separate reader on other paths. Re-fixed: rewrote the Plex
  section + slice 1 to the real mechanism — one `/library/metadata/{rk}` call parsed
  by a **new `PlexDetail` serde struct** (serde already descends into child `Vec`s,
  as `PlexVideo.media`/`.guids` prove); no hand-rolled descent, no listing-parser
  change.
- **idv-6 (MEDIUM) — the idv-2 "first `plex|jellyfin|emby` backing" rule doesn't
  match the display face.** Confirmed: `dedup_across_sources` puts the richest face at
  `backing[0]` (`commands.rs:2678`) but `rank_backings` re-sorts `backing` by play
  kind and overwrites `source_id` (`:2520-2527`), so the post-rank order is play
  order (Plex before Jellyfin), not display/richness order. Fixed: replaced the
  scan-the-list rule with an explicit **`detail_key`** computed in `rank_backings`
  via a dedicated **server-preferred detail rank** (reverse of `kind_rank`),
  deterministic and independent of play order.
- **idv-5 (HIGH) — the merged-card fix covered `get_item_detail` but not the show
  drill.** Confirmed: `get_children` routes the key it's handed
  (`commands.rs:3262`), which for a merged show is the local play face → local
  children with no `backing`, so the episode page can't retarget to a rich server.
  Fixed: drill the merged show through `detail_key` too (idv-5); added an owner
  open-decision on the episode playback-source tradeoff (server-rich vs local
  direct-play) — default = server, non-blocking for slices 1-3.

**r3 — 2026-07-06 — verdict `accepted`, 0 comments** (reviewed_sha=base_sha
`0df45b7`; `guard_confirmed:false` — codex read-only on a design doc). idv-1..6 all
confirmed resolved; no new material defect. **Plan-review loop CLOSED — the plan is
APPROVED for implementation.** Healthy converging loop: r1 (4) → r2 (3, one
correcting an r1 error) → r3 (clean).
