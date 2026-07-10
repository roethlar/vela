# Plan: Continue Watching watched-state curation (defect + one-op curation)

## Status
**DRAFT 2026-07-10 — awaiting plan review and owner go before any code.**
Drafted on the owner's `plan` operator after the 2026-07-10 defect report,
folding in the queued-last 2026-07-08 one-op-curation ask (same surface,
same ops — `.agents/state.md ## Next`).

## Owner reports (verbatim intent)
1. 2026-07-10: right-click mark watched/unwatched from the carousel "does
   not mark the video appropriately until I also remove it from continue
   watching"; while a video is in Continue Watching the watched status
   can't be changed *from anywhere* until it's removed.
2. 2026-07-08: "if I mark a video in the carousel as unwatched, it stays in
   the carousel. if I remove it from continue watching, the watched status
   remains. so I have to do two ops to get what I want."

## Diagnosis (code-confirmed 2026-07-10)
The server write works; the *display* is masked by Vela's own Continue
Watching feeds. Key evidence: "remove from continue watching" makes the
correct status appear, yet `remove_from_continue` performs **no** scrobble
(`src-tauri/src/commands.rs:2118` — tombstone + recents drop + best-effort
hub removal only). So the earlier scrobble/unscrobble had already landed
server-side; only Vela's carousel kept rendering stale state.

The hero cover-flow merges THREE feeds (`src/routes/+page.svelte:285`,
`heroItems`): Vela's local recents (`recents::list`, a snapshot frozen at
playback time — `src-tauri/src/recents.rs:139`), the server continue hub
(`home.continue`/`resume`), and Vela's synthetic On Deck hub
(`vela.ondeck`, built from `/library/onDeck` —
`src-tauri/src/source/plex.rs:342`). The local copy deliberately wins the
dedup ("it carries the freshest position", `+page.svelte:291`). Tombstones
(`hidden_from_continue`) suppress items across ALL feeds, but today only
the explicit "Remove from Continue Watching" writes one.

Per direction:
- **Mark unwatched** (code-certain): `set_watched`
  (`src-tauri/src/commands.rs:2043`) drops the recents entry only on
  `played=true` — deliberately never on unwatched. The stale local
  snapshot (frozen `played`/`view_offset_ms`) survives the refetch and
  keeps rendering an in-progress card, no matter where the state was
  changed from. The optimistic frontend mutation (`+page.svelte:831`) is
  clobbered by the same refetch.
- **Mark watched** (mechanism plausible, not reproduced — labeled
  assumption): the recents entry IS dropped, but no tombstone is written,
  so any server-side persistence — `home.continue` cache/lag or the
  separately fetched On Deck feed still listing the item on the immediate
  refetch — resurfaces a hub copy in the carousel. The fix below does not
  depend on which server feed persists: the tombstone suppresses all of
  them deterministically, the same guarantee `remove_from_continue`
  already relies on ("the tombstone above already guarantees the UX",
  `commands.rs:2126`). Owner playtest is the proof the symptom is gone.

## Design: watched-state changes curate Continue Watching in the same op

One thin backend change. In `set_watched` (`commands.rs:2043`), after a
successful `mark_played`, replace the `played=true`-only
`recents::unrecord` with an **unconditional `recents::hide`** (both
directions), discarding the returned server key:

- `hide()` (`recents.rs:112`) already does exactly what's needed: drop the
  recents entry (matching either identity of a merged card) and tombstone
  the full identity set, FIFO-capped at 200.
- **Tombstone lifecycle (plan-review r1, finding 1 — binding):** today only
  `recents::record` clears tombstones (`recents.rs:44`), and only the
  frontend's direct-play path invokes `record_recent`
  (`+page.svelte:737`); queue plays and auto-advance go through
  `play_by_key` (`commands.rs:2158`, the single chokepoint for ALL
  playback triggers) without touching recents. With watched-state changes
  now writing tombstones, a queue play of a tombstoned item would leave it
  suppressed while genuinely in progress. Fix in the same slice: new
  `recents::untombstone(cfg, key)` (exact-key removal), called
  best-effort in `play_by_key` after the mpv spawn succeeds — mirroring
  the frontend's "a FAILED play must not clear a tombstone" discipline
  (`+page.svelte:735`). Direct plays then clear twice (record_recent +
  untombstone) — idempotent, harmless.
- **No** server-side `removeFromContinueWatching` call is added:
  scrobble/unscrobble is the server-side op; the tombstone guards the
  local UX. The explicit remove action keeps sole ownership of the server
  hub-removal call.
- No frontend change: the optimistic mutation and `refreshWatchState`
  refetch stand; with the recents entry gone and the tombstone written,
  the refetch drops the card from the carousel.

Resulting one-op semantics (resolves owner report 2 by design — no new
menu entries, no wording change):
- **Mark watched** = watched everywhere + leaves Continue Watching.
- **Mark unwatched** = full reset everywhere + leaves Continue Watching.
- **Remove from Continue Watching** (unchanged) = the dismiss-only op:
  leaves the carousel, watched state and progress untouched.

Accepted edges (called out, not blocking):
- Mark-watched/unwatched on an item never in the carousel writes a
  harmless tombstone (bounded FIFO 200, cleared on play).
- Tombstones are per-key: marking a SHOW watched doesn't tombstone its
  episodes' keys, so an On Deck episode card could outlive a show-level
  scrobble for one refetch window. Same identity limit the explicit
  remove action has today; out of scope.
- Playing a tombstoned item OUTSIDE Vela (another device/Plex Web) revives
  it server-side but Vela keeps suppressing it until a Vela play or FIFO
  eviction. Pre-existing class: the explicit remove action has had exactly
  this semantic since it shipped; this plan extends it to watched-state
  edits and documents it (leg 3 of the E2E asserts the suppression as
  intended behavior). If the owner wants server activity to revive
  tombstones, that is a separate follow-up.
- `untombstone` is exact-key: a multi-server merged card's sibling
  watch-key tombstone isn't cleared by a queue play of the play key. Rare
  (requires a merged title marked watched/unwatched, then queue-played);
  self-heals on any direct play (`record` clears the full identity set).
- Related gap observed while reviewing, NOT in this plan's scope: queue
  plays and auto-advance never enter Vela's recents at all, so a
  queue-interrupted session doesn't surface in Continue Watching. Queued
  in `.agents/state.md ## Next` for a separate owner decision.

## Slice (single commit + version bump; reviewloop codex after landing)
1. `commands.rs set_watched`: unconditional `hide`, doc comment updated to
   the new semantic.
2. `recents.rs`: new `untombstone(cfg, key)` (exact-key tombstone
   removal), unit-tested; `commands.rs play_by_key`: call it best-effort
   after a successful mpv spawn (covers queue plays and auto-advance).
3. Mock fidelity (plan-review r1, finding 2): `mockjf.mjs` PlayedItems
   POST/DELETE also reset `positionTicks` to 0, matching real servers
   (**ASSUMPTION, verify at implementation** against Jellyfin's
   MarkUnplayed/MarkPlayed semantics; Plex scrobble/unscrobble likewise
   clears the view offset). Without this, "full reset" would show green
   while a real resume point survives.
4. Mock hub feed (plan-review r1, finding 3): `mockjf.mjs` gains an
   OPT-IN `serveResume: true` — `/Users/{u}/Items/Resume` returns movies
   with `positionTicks > 0 && !played` (faithful server behavior).
   Default stays the hardcoded empty list so existing scenarios
   (`curation.mjs` and its EMPTY_HOME assertions) keep their current
   guards unchanged.
5. E2E guard, new scenario `tests/e2e/scenarios/watchcurate.mjs` (mock JF,
   one streamable movie, `serveResume: true`; assertions gated on the
   mock actually receiving the post-action refetch — the
   `markwatched.mjs` eh-15 pattern — then asserting the hero card
   selector, not EMPTY_HOME, since the Resume hub may be non-empty):
   - Leg 1 (unwatched — RED today): play-and-quit partway (recents
     stamped, Resume hub non-empty → hero card present) → **Mark
     unwatched** from the hero menu → assert `DELETE PlayedItems`
     arrived, the refetch landed, the hero card is GONE, the recents
     entry is gone, and a tombstone is written. Fails today: recents
     survive, no tombstone.
   - Replay leg: play again — assert the tombstone clears (existing
     `record` behavior) AND playback starts from ~0 (`playAndQuit`'s
     first time-pos sample < 2s), proving the full reset end-to-end
     (guards against a stale resume point from either the server mock or
     Vela's own stamp).
   - Leg 2 (watched — RED today on the tombstone): **Mark watched** from
     the hero menu → assert `POST PlayedItems`, refetch landed, hero card
     gone, recents gone, tombstone written.
   - Leg 3 (hub-copy suppression — the mechanism the watched direction
     relies on; plan-review r1, finding 3): with leg 2's tombstone still
     in place, mutate the mock's userData directly
     (`positionTicks > 0, played: false` — simulating a stale/cached or
     externally-revived server hub copy), force a home refresh, gate on
     the Resume refetch, assert the hero card does NOT appear. This is
     the first live guard that a tombstone suppresses the SERVER hub
     feed (curation.mjs only ever proves recents-feed suppression — its
     mock Resume is hardcoded empty and its restart leg reinserts only
     `cfg.recents`).
6. Rust unit tests: `untombstone` add/remove round-trip (guard-proven).
   No unit test for `set_watched` itself: the command layer needs `State`
   and the changed behavior is `hide()`'s, already unit-guarded in
   `recents.rs`. The E2E scenario is the behavioral guard, guard-proven
   red→green per repo rule (run the scenario against the pre-fix binary →
   legs 1/2 fail; apply fix, rebuild → suite green).

## Non-goals
- No change to the explicit "Remove from Continue Watching" action or its
  server call.
- No menu rewording or combined menu entries.
- No refresh/merge redesign of the hero feeds (local-wins dedup stays).
- No JF/Emby-specific work: `set_watched` is source-agnostic at the
  command layer; per-source `mark_played` impls are untouched.

## Verification
- Full CI set (backend touched): `npm run check`, `npm run build`; from
  `src-tauri/`: `cargo check --locked`, `cargo clippy --all-targets
  --locked -- -D warnings`, `cargo test --locked`.
- E2E on the owner's Linux VM (standing venue, `git push vm main`): new
  scenario red→green guard-proof + full suite green.
- Owner playtest on real Plex (also confirms the labeled watched-direction
  assumption is moot post-fix): with a movie mid-progress in the carousel —
  (a) Mark unwatched from the carousel → leaves the carousel in one op,
  library card shows clean unwatched; (b) replay partway → it returns;
  (c) Mark watched from the carousel → leaves the carousel, library card
  shows ✓; (d) Remove from Continue Watching on another in-progress item →
  leaves the carousel with progress intact (Plex Web still resumes it).
- On acceptance, record the semantic change ("watched-state changes curate
  Continue Watching; remove = dismiss-only") in `.agents/decisions.md`.

## Open decisions for owner (defaults proposed, none blocking)
- **Mark-unwatched semantic:** proposed full reset — it leaves Continue
  Watching (matches report 2 verbatim). Alternative: keep it in the
  carousel at 0:00 — rejected as recreating the two-op dance.
- **Tombstone breadth:** proposed unconditional (harmless, bounded,
  self-clearing). Alternative: tombstone only when a recents entry
  existed — more logic, no user-visible benefit.

## Review log
Plan-review loop (playbook `reviewloop`, reviewer `codex exec --json
--sandbox read-only` 0.144.1, mac host).

**r1 — 2026-07-10 — verdict `reopened`, 3 findings, all ADMITTED (verified
against live code).** Base `4365fb3`, head `d810c02`,
`guard_confirmed:false` (read-only design review).
1. Tombstone lifecycle incomplete: only `recents::record` clears
   tombstones and only direct frontend Play records; `queue_play_at` →
   `play_by_key` never touches recents, so a queue play of a tombstoned
   item leaves it suppressed while genuinely in progress. Fixed: added
   `untombstone` in `play_by_key` (post-spawn) to the design; the
   out-of-Vela-play revival case is documented as an accepted pre-existing
   edge (same class as explicit remove), and the broader queue-plays-
   never-in-recents gap is queued separately.
2. Mock `DELETE PlayedItems` didn't clear `positionTicks`, so the
   "full reset" claim could pass while a real resume point survived
   (replay would resume ~6s). Fixed: mock POST/DELETE reset position
   (labeled assumption re real-server parity) + a replay-starts-at-0
   assertion.
3. The plan's claim that `curation.mjs` already guards tombstone
   application "across both feeds" was false — the mock Resume feed is
   hardcoded empty, so hub-feed suppression had NO guard anywhere. Fixed:
   opt-in `serveResume` mock fidelity + leg 3 asserting a live hub copy
   stays suppressed while tombstoned; the false claim removed.
