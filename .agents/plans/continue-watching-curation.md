# Continue Watching curation: remove action, mark-watched sync, On Deck fold-in

Status: APPROVED 2026-07-04 via the owner-delegation decision in
`.agents/decisions.md`. Implements the three queued items in `ISSUES.md`
§ "Continue Watching curation (2026-07-04)".
Owner choices locked 2026-07-04: On Deck items fold into the flow (no row),
ordering is interleaved by recency; removal includes the Plex server side.

## Background (evidence, verified 2026-07-04 against the owner's server)

- The hero cover-flow merges Vela's recents with hubs whose `hubPolicy` is
  "hero" (`src/routes/+page.svelte`); `ondeck` hubs are routed to a 16:9 row.
- The owner's Plex server currently returns NO On Deck hub from `/hubs` (the
  endpoint Vela reads), and its `home.continue` hub carries only genuinely
  in-progress items (2 at probe time). `/library/onDeck` returned 3 — the
  same 2 plus an in-progress movie the hero never shows. The hub's presence
  is server-controlled; Vela must not depend on it.
- No removal path exists anywhere: `recents.rs` has no delete, no command
  exposes one, the hero context menu has no entry.
- `setWatched` updates the card in place: it never re-fetches hubs and never
  touches `cfg.recents`, so marked-watched items linger in the hero (both
  halves of the merge).

## Slice 1 — On Deck folds into the flow, interleaved by recency

Backend (`src-tauri/src/plex_library.rs`, `src-tauri/src/source/plex.rs`):
- New `PlexLibrary::get_on_deck()` fetching `/library/onDeck` (same XML walk
  as hubs; items only).
- `PlexSource::hubs()` appends a synthetic hub (`hub_identifier:
  "vela.ondeck"`, title "On Deck") with those items, filtered to playable
  video like the rest. Fetch failure degrades to no hub, matching the
  existing per-hub resilience stance.
- `ItemDto` gains `last_watched_at_ms: Option<u64>`: populated from Plex
  `lastViewedAt` (seconds → ms) on hub/onDeck items, and from the recents
  entry's `ended_at_ms` in `recents::list`. (Jellyfin/Emby resume items get
  `DatePlayed`/equivalent where cheap; otherwise None.)

Frontend (`src/routes/+page.svelte`):
- `hubPolicy`: any id containing "ondeck" → "hero" (the 16:9 On Deck row
  policy is dropped — dead code on servers without the hub, superseded
  otherwise).
- `heroItems`: dedupe by rating key as today (recents copy wins — it carries
  the freshest local position), then sort descending by
  `last_watched_at_ms`; items with no timestamp keep their relative order
  after all timestamped ones. This is the owner's "interleaved by recency":
  a next-up episode ranks by when its show was last watched.

Record in `.agents/decisions.md` on landing: supersedes the split-artwork
decision's "On Deck stays a 16:9 row" treatment (ISSUES.md already flags
this supersession as required).

Out of scope: Jellyfin `/Shows/NextUp` and Emby equivalents (parity
follow-up, recorded, not built here).

## Slice 2 — mark-watched curates the hero

- Backend `set_watched` also removes the key from `cfg.recents` (watched =
  not "continue watching", same semantic as `finish()` past threshold).
- Frontend `setWatched` triggers the same hub + recents re-fetch the
  `playback-ended` path uses, so the server hub copy leaves the flow without
  a restart. Mark-UNwatched only re-fetches (it must not resurrect a recents
  entry; the item re-enters the flow via the server hub if the server says
  so).

## Slice 3 — explicit "Remove from Continue Watching"

- Config gains `hidden_from_continue: Vec<String>` (rating keys; bounded by
  retain-on-use, see below). This tombstone is what makes removal stick:
  the hero merge drops any item whose key is tombstoned, so removal survives
  server hubs that still carry the item.
- New command `remove_from_continue(rating_key)`:
  1. drops the key from `cfg.recents`,
  2. adds the tombstone,
  3. for Plex-backed items, best-effort server-side removal via Plex's
     remove-from-continue-watching action (exact route verified against the
     live server during implementation; failure is non-fatal — the tombstone
     already guarantees the UX).
- Tombstone lifecycle: playing the item again (`recents::record`) clears its
  tombstone — watching something is the explicit opposite of "stop
  suggesting it". Tombstones for keys no longer present in any feed are
  pruned opportunistically at record time to keep the list small.
- Frontend: hero card context menu gains "Remove from Continue Watching" →
  invoke + the standard hub/recents re-fetch. No watched-state change.
- Jellyfin/Emby server-side removal: investigate during implementation; if
  not cheap, tombstone-only for them in this slice (recorded as follow-up).

## Ordering & commits

One slice per commit, in the order above (each is independently shippable;
slice 1 is the most visible owner ask). Each ships with unit tests
guard-proven per AGENTS.md (revert change → test fails → restore).

## Verification

- Full repo verification per `.agents/repo-map.json` (npm check/build, cargo
  check/clippy/test from `src-tauri/`).
- Unit: hero merge ordering (interleave + no-timestamp tail), tombstone
  suppression and clear-on-replay, set_watched recents drop, onDeck XML
  parse.
- Live against the owner's server: *Blood and Bone* (onDeck-only today)
  appears in the flow; mark-watched drops an item without restart; remove
  hides an item and survives an app restart and a hub refresh; replaying a
  removed item brings it back.

## Implementation notes (2026-07-04, all three slices landed)

- Commits: slice 1 `d2ea1a7`, slice 2 `cf5af95`, slice 3 `d259213`. Full
  suite green after each (78 Rust tests at the end; svelte-check and
  `npm run build` clean). All new tests guard-proven (change reverted →
  test fails → restored).
- Deviations / refinements:
  - Hero merge ordering lives in the frontend (`heroItems` in
    `+page.svelte`); the repo has no JS test runner, so ordering is NOT
    unit-tested — the Rust halves (onDeck XML parse incl. `lastViewedAt`,
    recents `ended_at_ms` stamping) are. Covered instead by svelte-check
    and the planned E2E harness (`.agents/plans/e2e-harness.md`).
  - Tombstone pruning: the plan's "prune keys absent from any feed at
    record time" is not implementable backend-side (hub feeds aren't
    available there); implemented as clear-on-replay + FIFO cap of 200
    (`MAX_HIDDEN` in `recents.rs`).
  - Removal is one command (`remove_from_continue`), not `remove_recent`:
    it drops the recents entry, tombstones, and best-effort calls
    `MediaSource::remove_from_continue` (Plex implements it; default
    no-op). Jellyfin/Emby server-side removal and their
    `last_watched_at_ms` timestamps remain recorded follow-ups.
  - A recents entry whose session is still open (`ended_at_ms == 0`)
    reports no `last_watched_at_ms` and sorts after stamped items until
    mpv exits — self-correcting, noted here for honesty.
- Live verification status: Plex `/actions/removeFromContinueWatching`
  existence confirmed against the owner's server (400 without `ratingKey`,
  404 with a bogus one; an unknown action returns 404 for both shapes).
  The real-item end-to-end checks in the Verification section above are
  pending: the automated-permissions layer correctly refused a live
  mutating PUT on a real hub item mid-implementation. First in-app use or
  the E2E harness closes this; failure is non-fatal (tombstone owns the
  UX).
