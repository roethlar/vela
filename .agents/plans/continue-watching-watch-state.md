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
  the full identity set, FIFO-capped at 200. Replaying the item clears the
  tombstone (`record`, `recents.rs:44`) — existing, guarded behavior.
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

## Slice (single commit + version bump; reviewloop codex after landing)
1. `commands.rs set_watched`: unconditional `hide`, doc comment updated to
   the new semantic.
2. E2E guard, new scenario `tests/e2e/scenarios/watchcurate.mjs` (mock JF
   with one streamable movie, no hub content — the `curation.mjs` seed
   pattern, so the hero is fed by recents alone):
   - Leg 1 (unwatched direction — RED today): play-and-quit partway →
     movie in hero → context-menu **Mark unwatched** → assert the mock
     got `DELETE PlayedItems`, the hero empties, the config's recents
     entry is gone, and a tombstone is written. Fails on current code
     (recents survive; no tombstone).
   - Leg 2 (watched direction — RED today on the tombstone): replay
     (clears tombstone, hero returns) → **Mark watched** → assert `POST
     PlayedItems`, hero empties, recents gone, and a tombstone is
     written. The tombstone assertion is the red part today.
   - Feed-suppression machinery itself (tombstone application across both
     feeds, restart survival) is already guarded by `curation.mjs` — not
     re-proven here.
3. No new Rust unit test: the command layer needs `State` and the changed
   behavior is `hide()`'s, already unit-guarded in `recents.rs`; a
   passthrough-helper test would be vacuous. The E2E scenario is the
   guard, guard-proven red→green per repo rule (revert the `commands.rs`
   change, rebuild debug, scenario must fail; restore, suite green).

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
(plan-review pending)
