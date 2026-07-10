# Plan: Continue Watching watched-state curation (defect + one-op curation) — COMPLETE

## Status
**COMPLETE — owner playtest VERIFIED 2026-07-10 ("installed and tested.
carousel fix verified.") on 0.1.42** (fix `02504be`). Open remainder: the
contested r6 finding awaits owner adjudication (see Review log / Accepted
edges; also queued in `.agents/state.md ## Next`). Plan retained as the
design record.

Original status: IMPLEMENTED 2026-07-10 — owner go given 2026-07-10
("go" after the drafted plan was summarized). Plan-review loop (codex, 6
rounds) closed same day: every admitted finding fixed, one final finding
contested on materiality and routed to the owner. Guard: new
`watchcurate` E2E scenario proven red→green on the owner's Linux VM (red
at the exact defect assertion against the pre-fix binary; full suite
11/11 green with the fix). Drafted on the owner's `plan` operator after
the 2026-07-10 defect report, folding in the queued-last 2026-07-08
one-op-curation ask (same surface, same ops).

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

One thin backend change. In `set_watched` (`commands.rs:2043`), replace
the `played=true`-only `recents::unrecord` with curation in BOTH
directions — **curate-first with rollback** (r2–r4 resolution):

- Curation (`recents::hide_with_undo`) runs BEFORE the awaited
  `mark_played`. The race the earlier drafts wrestled with — a play of
  the same item recorded during the server round-trip (up to ~15s client
  timeout, doubled by Plex's rediscover+retry) being dropped by the
  delayed curation, losing the sub-threshold resume position only Vela's
  stamp holds — has no window at all once curation is synchronous with
  the command's start. (r2's open-entry liveness guard was the wrong
  cure: r3 showed crash-stale "open" entries would skip curation forever,
  persistently reproducing the masking bug. Removed.)
- On a FAILED `mark_played`, the undo token restores the exact
  pre-curation state (the dropped entry at its position; exactly the
  tombstone keys the hide added, pre-existing ones untouched), so a
  failed server edit leaves no lasting local trace. Newer play activity
  wins: if the item was re-recorded between curation and the failed
  restore, the fresh entry and its cleared tombstones are left alone.
- **Edit serialization (r5 finding 2):** `set_watched` holds a dedicated
  `watch_edit_lock` (the `play_lock` pattern) across curate + server call
  + rollback, so overlapping edits cannot interleave — without it, two
  in-flight edits on one item share tombstones (`hide` is idempotent) and
  a first-edit rollback could strip the tombstone a second, successful
  edit relies on and restore stale recents over it.
- **Transient render heals (r5 finding 3):** the transient curated state
  CAN reach the screen — an unrelated `playback-ended` refresh during a
  slow failing edit reads the temporary tombstone. The frontend's
  `setWatched` error path therefore re-fetches after the backend rollback
  (alongside the error banner), repainting the restored truth instead of
  leaving the stale render until some later refresh.

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
- Frontend: the optimistic mutation and success-path `refreshWatchState`
  refetch stand (the recents entry gone + tombstone written means the
  refetch drops the card); the ONLY frontend change is the error-path
  re-fetch above (r5 finding 3).

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
- Marking the CURRENTLY-PLAYING item watched/unwatched (deliberate,
  mid-session) curates its live entry: at quit, `finish` finds no entry
  and the session's final position isn't stamped locally. Semantic, not a
  race: the user explicitly reset/completed the item mid-play; the server
  keeps any above-threshold position from the Stopped check-in, and the
  next play re-records normally.
- A play launched DURING an edit's server round-trip starts from the
  curated state — position 0 (r5 finding 1). Intent-aligned, not damage:
  the user expressed "reset"/"completed" moments earlier, so the raced
  play starting over matches the request even if the server edit later
  fails; on that failure the rollback restores the old stamp for FUTURE
  plays but cannot retroactively move the one already launched, and a
  sub-threshold final position from a play overlapping a FAILED edit can
  be lost (`finish` no-ops once the entry was dropped). Double-rare
  (failing server + racing play on one item), self-heals on the next
  play.
- **Residual interleaving class (r6, CONTESTED — owner to adjudicate):**
  an edit QUEUED behind a slow edit on the serialization lock has a
  pre-curation wait window; a play of the queued edit's item inside that
  window is curated away when the edit finally runs (same damage class
  as the bullet above: temporary Continue Watching absence, a
  sub-threshold stamp lost, self-heals on the next play). Reaching it
  takes a slow/failing edit on one item PLUS an edit AND a play on
  another item, all interleaved within the first edit's round-trip.
  Coder judgment: this residual class is inherent to an actionable UI
  over async server edits — each guard so far has produced its own new
  interleaving (r2→r6) — and the next narrowing (persisted per-entry
  timestamps + compare-and-swap curation) exceeds the cost-benefit line
  for a local media client. Routed to the owner with the review trail;
  ordering the CAS hardening as a follow-up plan reverses this
  disposition.
- Rollback micro-losses on a FAILED server edit: tombstone keys the FIFO
  cap (200) evicted during the hide are not resurrected; and an explicit
  "Remove from Continue Watching" issued on the same item DURING the
  failing edit's round-trip is undone by the rollback (the remove's own
  hide adds no new tombstones — they are already present from the edit —
  so the restore strips them and re-inserts the entry). Two conflicting
  user actions on one item inside one failed server call, surfaced by the
  edit's error banner; re-removing recovers. Accepted.
- Related gap observed while reviewing, NOT in this plan's scope: queue
  plays and auto-advance never enter Vela's recents at all, so a
  queue-interrupted session doesn't surface in Continue Watching. Queued
  in `.agents/state.md ## Next` for a separate owner decision.

## Slice (single commit + version bump; reviewloop codex after landing)
1. `commands.rs set_watched`: curate-first via `hide_with_undo` (both
   directions), rollback via `restore_hidden` on a failed `mark_played`,
   the whole body serialized on a new `AppState.watch_edit_lock`; doc
   comment updated to the new semantic. `+page.svelte setWatched`: error
   path re-fetches after the backend rollback.
2. `recents.rs`: new `hide_with_undo`/`restore_hidden` (undo-token
   curation) and `untombstone(cfg, key)` (exact-key tombstone removal),
   all unit-tested (round-trip incl. entry position and
   only-added-tombstones; newer-play-wins restore; exact-key clear);
   `commands.rs play_by_key`: call `untombstone` best-effort after a
   successful mpv spawn (covers queue plays and auto-advance).
3. Mock fidelity (plan-review r1, finding 2): `mockjf.mjs` PlayedItems
   POST/DELETE also reset `positionTicks` to 0, matching real servers
   (**VERIFIED 2026-07-10** against jellyfin/jellyfin master:
   `BaseItem.MarkUnplayed` → `ResetPlayedState` unconditionally zeroes
   `PlaybackPositionTicks`; the `PlayedItems` POST endpoint calls
   `MarkPlayed(user, datePlayed, resetPosition: true)` —
   `PlaystateController.UpdatePlayedStatus`. Plex scrobble/unscrobble
   clearing the view offset is covered live by the owner playtest).
   Without this, "full reset" would show green while a real resume point
   survives.
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
     the Resume refetch, then hold a bounded NEGATIVE watch (~3s) that
     the hero card never appears (plan-review r2, finding 2: the mock
     records the request before responding and the frontend applies hubs
     after the response resolves, so a single immediate check right after
     the request lands could pass in flight while broken tombstone
     filtering renders the card a moment later). This is the first live
     guard that a tombstone suppresses the SERVER hub feed
     (curation.mjs only ever proves recents-feed suppression — its mock
     Resume is hardcoded empty and its restart leg reinserts only
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

**r2 — 2026-07-10 — verdict `reopened`, 2 findings, both ADMITTED.** Base
`4365fb3`, head `61f666e`, `guard_confirmed:false`. (First r2 dispatch
died mid-run on reviewer-side model capacity — `turn.failed`, no verdict;
fail-closed, re-dispatched once per the playbook. The reviewer also
correctly noted uncommitted implementation files in the shared worktree
and isolated its evidence to the pinned SHA.)
1. Edit-vs-play race: `set_watched` awaits `mark_played` before curating,
   so a play starting inside that window records a fresh open entry that
   the delayed hide would drop and tombstone — erasing an active session
   from Continue Watching until the next play. Fixed: curation goes
   through `hide_unless_playing` (open entry ⇒ skip; live playback wins);
   remaining slivers documented as accepted edges.
2. Leg 3's negative assertion could pass in flight (request recorded
   before the response; hubs applied after it resolves; the card is
   already absent from leg 2). Fixed: bounded ~3s negative watch that the
   card NEVER appears, replacing the single immediate check. (The
   implementation had this hardening from coder self-review before the r2
   verdict arrived; the plan text now matches it.)

**r3 — 2026-07-10 — verdict `reopened`, 3 findings: two exposed the r2
amendment as an overshoot (removed), one text contradiction (fixed).**
Base `4365fb3`, head `c71fd2e`, `guard_confirmed:false`.
1. The open-entry guard misses a play that starts AND ends inside the
   `mark_played` window (`finish` stamps `ended_at_ms > 0`, so the
   delayed curation still drops it). Disposition: the full-session-
   inside-one-round-trip case is not reachable by a human and its damage
   self-heals on the next play — folded into the documented accepted
   edge rather than guarded.
2. **The strongest finding of the loop:** `ended_at_ms == 0` does not
   prove liveness — a kill/crash mid-playback leaves a permanently
   "open" entry, and the r2 guard would then skip curation for that item
   on every future watched-state edit: a persistent recurrence of the
   exact masking bug this plan exists to fix. ADMITTED; resolution:
   REMOVE the r2 guard (back to unconditional `hide`) and document the
   original race as an accepted edge — its worst case (rare, cosmetic,
   self-healing) is strictly smaller than the guard's (uncommon but
   persistent). Same loop shape as person-browse r2: the reviewer
   correcting the coder's previous-round fix.
3. The Slice section still said "unconditional `hide`" while the design
   section said guarded — a real internal contradiction (the coder had
   amended one section and not the other). Fixed; with the r2 guard
   removed both sections now genuinely agree on unconditional `hide`.

**r4 — 2026-07-10 — verdict `reopened`, 1 finding, ADMITTED (it refuted
the accepted-edge justification with correct numbers).** Base `4365fb3`,
head `f94ce8b`, `guard_confirmed:false`. The r3 resolution called the
edit-vs-play window "sub-second" and the damage "cosmetic"; in fact the
mark request runs on ~15s-timeout clients (`jellyfin.rs`,
`plex_library.rs`) with a Plex rediscover+retry doubling it, Play stays
enabled during the await, and a short raced play's sub-threshold resume
position — which ONLY Vela's stamp holds (the 2026-07-04 hero decision) —
is permanently lost when the delayed curation drops its entry. Resolution:
stop defending the edge; CLOSE the race with curate-first + rollback
(`hide_with_undo`/`restore_hidden`, newer-play-wins restore). The design,
slice, and accepted-edges sections were rewritten accordingly; the
remaining documented edges are the deliberate mid-play mark semantic and
two rollback micro-losses on failed server edits.

**r5 — 2026-07-10 — verdict `reopened`, 3 findings: two ADMITTED and
fixed, one dispositioned as intent-aligned.** Base `4365fb3`, head
`1de098b`, `guard_confirmed:false`.
1. A raced play during the edit's round-trip launches from position 0
   (the curated state) and a failed edit cannot retroactively fix it; a
   sub-threshold stamp can be lost. Disposition: intent-aligned — the
   user just asked for a reset/completion, so the raced play starting
   over matches the expressed intent; the failed-edit variant is
   double-rare and self-healing. Documented as an accepted edge, not
   guarded.
2. Undo tokens carry no generation, so overlapping edits could
   interleave: a first-edit rollback strips the tombstone a second,
   successful edit shares (`hide` is idempotent) and restores stale
   recents over it. ADMITTED and fixed: `set_watched` serializes on a
   dedicated `watch_edit_lock` (the `play_lock` pattern) across
   curate + call + rollback.
3. The "frontend never renders transient curation" claim was false — a
   `playback-ended` refresh during a slow failing edit consumes the
   temporary tombstone, and the error path never refetched. ADMITTED and
   fixed: `setWatched`'s catch now re-fetches after the backend rollback;
   the design text corrected.

**r6 — 2026-07-10 — verdict `reopened`, 1 finding, CONTESTED (recorded,
routed to owner — loop closed by coder judgment, not agreement).** Base
`4365fb3`, head `8eb0981`, `guard_confirmed:false`. Finding: the
`watch_edit_lock` itself creates a pre-curation wait window for a QUEUED
edit, inside which a play of that edit's item can record and untombstone,
only to be curated away when the queued edit finally runs — factually
correct, admitted as accurate. Contested on materiality, not accuracy:
the window needs a slow/failing edit + a second edit + a raced play on
the same item, all interleaved; the damage is the same bounded,
self-healing class as the r5-1 accepted edge; and the rounds r2→r6 form
an asymptote — every guard added produced a new interleaving of its own,
because SOME window is inherent to an actionable UI over async server
edits. The next narrowing (persisted per-entry timestamps +
compare-and-swap curation) was judged past the cost-benefit line for
this slice. Recorded in Accepted edges and routed to the owner for
adjudication. Loop tally: r1 3 findings (all fixed), r2 2 (fixed), r3 3
(two exposed the r2 fix as an overshoot — removed; one text fix), r4 1
(fixed by design change), r5 3 (two fixed, one dispositioned), r6 1
(contested). Core defect guard: E2E red→green proven on the target
platform; full suite 11/11.
