# Plan: Playlists and Continue Playing

Status: **APPROVED 2026-07-15 — implementation authorized by the owner.** Scope
was approved by the owner 2026-07-14; the owner gave the implementation go on
2026-07-15. Product decisions that outlive this plan are in
`.agents/decisions.md` (2026-07-14).

Origin: the in-app play queue does not survive an app restart (owner,
2026-07-14). The design discussion that followed concluded that **the queue
should not exist at all.** The owner's reasoning, recorded because it is the
justification for deleting shipped code:

> Ephemeral queues are a music idiom, not a video one. The only preset video
> sequence worth having is a show binge, and there the sequence IS the show's
> own episode order — which Continue Playing already walks. Anything larger (a
> movie series, or a meta-series like "all Star Trek shows in order") is a real
> named playlist. Infuse has no Up Next queue for exactly this reason: its verbs
> are play, or add to a named playlist. That is the model people expect.

So: two mechanisms, each with one job. **Continue Playing** walks a show.
**Named playlists** hold a curated sequence. Neither is an ephemeral scratch
list, and the queue is deleted.

Investigation also surfaced a second, independent defect the queue was hiding:
**the Continue Watching carousel does not reflect anything played through the
dispatcher**, because `play_by_key` records no recent. That bug survives the
queue's deletion and is S2 below.

---

## Owner rulings (settled; do not reopen without the owner)

1. **There is no play queue.** "Add to queue", "Play Next", the queue chip and
   the queue drawer are **deleted**. Playback context is a single item, or a
   named playlist.
2. **Named playlists are durable objects** in a new Playlists sidebar entry.
   Vela's own playlists may mix items from DIFFERENT servers in one list — the
   thing no single server's playlist API can represent, and the reason this is a
   Vela-native feature.
3. **Playing a playlist never mutates it.** A cursor walks the list; the list
   does not change. Clicking its 4th item plays from there and drops nothing.
4. **Server playlists appear read-only**, alongside Vela's own.
5. **Playlists are stored in their own JSON file**, not in `config.json`, and
   not in a database. Nothing else moves out of `config.json`.
6. **The carousel is unchanged** — Continue Watching only. Playlists never
   appear in it (owner: "it never updates to reflect the items in a named
   playlist"). What DOES change is that plays finally register in it (S2).
7. **Continue Playing is a three-mode setting** (Infuse's model), consulted when
   a playlist ends or a single item finishes: `off` stops; `on` keeps walking
   down Continue Watching; `only-tv` plays the next episode of the series, rolls
   into the next season, and stops when the show runs out. **Default:
   `only-tv`.** "Next episode" means strictly the next in order, watched or not,
   so a deliberate rewatch keeps rolling.
8. **The play verbs are Play / Resume / Play from Beginning / Add to Playlist →.**
   An item with no resume position offers **Play**. An in-progress item offers
   **Resume** AND **Play from Beginning**, as two explicit choices, everywhere
   playback can be started (context menu, detail page, Continue Watching card).
9. **There is NO resume prompt, and no countdown.** It was only ever wanted for
   an in-progress item reached by AUTO-ADVANCE — and mpv owns the screen by then,
   so there is nowhere to draw it. **Auto-advance onto an in-progress item
   resumes silently**, which is what the code already does. This is a direct
   consequence of ruling 10; if embedded video were ever adopted, revisit it.
10. **Video stays external.** Embedding mpv was considered and rejected — see
    `.agents/decisions.md` (2026-07-14). Nothing here depends on it.

---

## Current code (verified 2026-07-14 at `9426f75`; re-verify before editing)

**The queue, all of which is being deleted.**
- `AppState.queue: Arc<Mutex<Vec<commands::QueueItem>>>` and
  `AppState.queue_index` (`src-tauri/src/lib.rs:61`) — in-memory only; nothing
  writes them to disk.
- Commands `queue_play_next`, `queue_append`, `queue_list`, `queue_clear`,
  `queue_remove`, `queue_play_at` (`src-tauri/src/commands.rs:2396-2477`).
- Frontend: the queue chip, the drawer, `queueStatus`, `queueAttempt`,
  `refreshQueue`, the 3s poll timer (`src/routes/+page.svelte:1414-1580`,
  `1640-1649`, `1919-1927`).
- **`play_item` (`commands.rs:2380`) replaces the whole queue with one item.**
  With the queue gone it becomes a plain play.
- **Per-surface status slice 2 (`67358fd`) gave the queue drawer and chip their
  own status line. It goes with the queue.** So does step 2 of the 0.1.48
  playtest ask in `.agents/state.md`.
- **The E2E `queue` and `surfaces` scenarios exercise the drawer and the chip's
  red mark.** `surfaces` must keep red-proving the surfaces that REMAIN (the
  edit line, the detail page) — it is the guard for per-surface status. Only its
  queue assertions go.

**Keep the dispatcher, repoint it.** Playing *through* a playlist still needs
mpv-finished → play-next-item. Reuse the `queue_advance` notify machinery
(`playback.rs:707`, `lib.rs:181-213`), but drive it from
`(playing_playlist_id, index)` instead of an ephemeral vec: on advance, load the
playlist from the store and take `index + 1`. This is what makes ruling 3 free —
there is no scratch copy to keep in sync, and a mid-playback edit to the playlist
naturally affects what plays next.

**Cross-source dispatch already works.** Item keys are namespaced
`<source_id>:<raw>` (`namespace_key`, `src-tauri/src/source/mod.rs:368`) and
`Registry::route` (`source/mod.rs:414`) dispatches per item. **A list mixing Plex
and Jellyfin items already plays correctly today.** Mixed-source playlists need
no new dispatch machinery.

**The real bug: dispatcher plays record nothing.** `play_by_key`
(`commands.rs:2237`) resolves and spawns mpv but never calls `record_recent`; the
comment at `commands.rs:2365` says so outright. Only the frontend direct-play path
records (`src/routes/+page.svelte:1484`). **This is why the carousel does not
reflect sequence playback** — Vela's half of the hero merge stays empty and only
the server's hub half moves. Recorded as a defect in `.agents/state.md` since
2026-07-10; S2 closes it, and it applies to playlist playback exactly as it did to
the queue.

**The carousel merge stays as it is.** `heroItems`
(`src/routes/+page.svelte:472`) = Vela's recents ∪ every hub whose `hubPolicy` is
`"hero"`; `hubPolicy` (`:445-447`) folds continue / resume / **ondeck** into the
hero. No change needed (ruling 6).

**Watch state is already solved; do not rebuild it.**
- `MAX_RECENTS = 20` (`src-tauri/src/recents.rs:14`);
  `DEFAULT_WATCHED_THRESHOLD = 95` percent (`recents.rs:17`), overridable via
  `AppConfig.watched_threshold_percent` (`config.rs:22`).
- `finish()` (`recents.rs:61`) drops an entry that ended past the threshold —
  *"watched to the end: no longer continue watching"*. **Stopping in the end
  credits therefore already counts as watched and keeps no resume position**, so
  it correctly offers Play, not Resume (ruling 8).
- `resume_stamp_ms` (`recents.rs:91`) is Vela's fallback position; the server's
  `resolve_stream().resume_ms` wins when non-zero (`commands.rs:2278`). **This is
  also the signal that decides Play vs Resume + Play from Beginning.**

**Dead-source handling has a precedent.** `filter_live_recents`
(`commands.rs:2175`) keeps entries from removed sources in the config untouched and
filters them at *read* time. Playlists follow this shape but **mark** rather than
filter — a curated entry is never silently dropped.

**Tokens are already on disk.** `AppConfig.recents` stores full item snapshots
including token-bearing poster URLs, under owner-only permissions, per the
2026-05-23 decision. Playlist entries may do the same. The only real cost is
*staleness* (a rotated token yields a broken thumbnail) — handle with poster
fallback, not with a new storage rule.

**Episode walking needs no new server API.** The trait already has
`children(container_key, offset, limit)` (`source/mod.rs:309`), and `ItemDto`
carries `parent_key` (season, `:101`), `grandparent_key` (show, `:106`) and episode
positioning (`:181`). `item_detail` (`:353`) and `person_items` (`:361`) are the
precedent for a trait method with a default implementation.

**There is no database and no second store.** `config.json` (`config.rs:199`) is
the only persistent file; no `rusqlite`/`sled`/`redb` dependency exists. The
`metadata_cache.json` and listing cache described in `ISSUES.md` **no longer
exist** — they died with the local-source removal (2026-07-08). `ISSUES.md` and the
stale "listing-cache" comment at `source/mod.rs:63` are drift; fix separately, not
in this plan.

---

## The model

**Playback context is a single item, or a playlist.** Nothing else.

- Play an item → it plays. When it ends, **Continue Playing** decides what
  happens next.
- Play a playlist → a cursor starts at its head (or at the item you clicked) and
  walks it. When it runs out, **Continue Playing** decides what happens next.
- Playing a playlist never changes it.
- Vela playlists are editable (reorder, remove, rename, delete, add). Server
  playlists are read-only: play only.

### Continue Playing (config `continue_playing`, default `only-tv`)

Consulted when a playlist ends, or when a single item finishes.

- `off` — stop.
- `on` — play the head of Continue Watching, then keep walking down it.
- `only-tv` — if what just finished was an episode, play the next episode in
  order (rolling into the next season, skipping season 0 unless already in it);
  otherwise stop. Stop when the show runs out.

**This is the binge mechanism.** It is what replaces the queue for the only video
sequence the owner actually wants preset: the show's own order.

Two hard requirements:

- **`on` must walk the same Continue Watching list the carousel renders**, not a
  fresh server query. A second source of truth will diverge from the first — this
  repo has already paid for that lesson (the shared error banner, r17–r24). It
  also makes `on` respect Continue Watching tombstones for free: a removed item is
  not on the strip, so auto-play cannot reach it.
- **`on` needs a no-repeat guard.** An item the server does not mark watched can
  resurface at the head of Continue Watching and replay forever. A continuous
  auto-play run must never play the same key twice.

---

## Slices

Each slice lands and commits on its own, with its guard red-proven **after** the
fix (inject the regression, demand the test fail for the right reason, restore
from a committed state — see `.agents/state.md`, "RED-PROOF EVERY GUARD").

### S1 — Delete the queue; rework the play verbs

Removal plus the new context menu. No playlists yet.

- Remove `AppState.queue`, `queue_index`, and the six `queue_*` commands.
- Remove the queue chip, drawer, poll timer, and `queueStatus` from
  `+page.svelte`.
- `play_item` becomes a plain play (no queue to replace).
- **Keep** the `queue_advance` notify machinery and the dispatcher loop — S3
  repoints them at a playlist cursor. Renaming them (`advance_*`) is in scope.
- Context menu becomes **Play** (no resume position) or **Resume** +
  **Play from Beginning** (in progress). "Add to Playlist →" arrives in S3.
  Same verbs on the detail page and on a Continue Watching card. The resume
  position already resolves via `resolve_stream().resume_ms` /
  `resume_stamp_ms` — that is the signal for which verbs to show.
- Rewrite the E2E `queue` scenario; **`surfaces` must keep its remaining
  red-proofs** (the edit line, the detail page).
- Update `.agents/state.md`: step 2 of the 0.1.48 playtest ask is moot.
- Guards: suite green with the queue gone; `surfaces` still red-proves the edit
  line and the detail page; E2E — an in-progress item offers both verbs and
  "Play from Beginning" really starts at 0.

### S2 — Every play records a recent

Closes the defect recorded in `.agents/state.md` (2026-07-10). **Independent of
playlists — this is the bug that made the carousel wrong, and it can land before
S3.**

- `play_by_key` records a recent for **every** play, including dispatcher-driven
  ones. It currently has only key/title/duration; pass the item snapshot through
  so `record_recent`'s `ItemDto` can be built.
- Preserve the existing rule that a **failed** play records nothing and clears no
  tombstone (`commands.rs:2366`).
- Guards: unit test; E2E — play via the dispatcher, item appears in Continue
  Watching with its position.

### S3 — Vela playlists: store, sidebar, editor

The largest slice; the editor is most of it.

- New module `src-tauri/src/playlists.rs` owning `playlists.json` beside
  `config.json`. **Reuse `config.rs`'s proven write discipline**: atomic save,
  owner-only Unix permissions, cross-process lock, fail-closed parse. Extract the
  shared helpers from `config.rs` rather than reimplementing them.
- Types: `Playlist { id, name, items: Vec<PlaylistEntry>, created_ms,
  updated_ms }`; `PlaylistEntry` carries what recents already persist (namespaced
  key, title, subtitle, duration, poster).
- Commands: `playlist_list`, `playlist_get`, `playlist_create`,
  `playlist_rename`, `playlist_delete`, `playlist_add_items`,
  `playlist_remove_item`, `playlist_reorder`.
- Playback: `playlist_play(id, start_index)` sets `(playing_playlist_id, index)`;
  the dispatcher advances by re-reading the playlist and taking `index + 1`.
  **Playing never writes to the playlist.** An in-progress item reached by
  auto-advance resumes silently (ruling 9).
- Sidebar "Playlists" entry; a playlist detail view with reorder, remove, rename,
  delete. Context menu gains **"Add to Playlist →"**.
- **Dead entries are kept and marked unavailable**, not dropped: an entry whose
  source is removed or offline renders as unavailable and is skipped on playback.
  Follow `filter_live_recents` (`commands.rs:2175`) for the live-source check, but
  mark instead of filtering.
- The playlists view and the playlist detail each own their status line
  (per-surface status, 2026-07-14).
- Guards: unit tests (CRUD; reorder; dead-entry marking; fail-closed parse;
  **playing a playlist leaves it byte-identical**). E2E — create, edit, play a
  playlist; a mid-playlist edit changes what plays next; restart and the playlist
  is still there.

### S4 — Server playlists (read-only)

- Trait: `async fn playlists(&self) -> Result<Vec<PlaylistDto>, String>` and
  `async fn playlist_items(&self, key: &str) -> Result<Vec<ItemDto>, String>`,
  both with default implementations returning empty (precedent: `item_detail`,
  `person_items`).
- Implement for Plex, Jellyfin, Emby. Keys are namespaced like every other key, so
  a server playlist's items route correctly for free.
- Sidebar groups them under their server. **Play only — no editing, no writing
  back.** An offline server's group renders unavailable and does not break the
  view.
- **The E2E mock servers need playlist endpoints** before any of this can be
  guarded. That is new mock work; budget for it.
- Guards: unit tests per source parser; E2E — a server playlist lists, plays, and
  offers no edit affordance.

### S5 — Continue Playing

- `AppConfig.continue_playing: Option<String>`, three-state, default `only-tv`.
  Follow `mpv_autocrop` (`config.rs:43`) as the precedent for a validated
  three-state string that fails closed to its default on an unknown value.
- Settings UI.
- **Where the decision lives:** the Continue Watching list is computed in the
  frontend, so the backend cannot see it. When a playlist ends (or a single item
  finishes) the backend emits an event; the **frontend** applies the Continue
  Playing rule against the list it already renders and invokes the next play. One
  source of truth.
- New backend command `next_episode(item_key) -> Option<ItemDto>`: resolve the
  episode's `grandparent_key` (show) → `children()` for seasons → `children()` for
  episodes → the next in order, rolling into the next season, skipping season 0
  unless already in it, `None` at the end of the show.
- No-repeat guard for `on`.
- Guards: unit tests for `next_episode` (mid-season; season rollover; end of show;
  specials skipped; specials honoured when starting in them). E2E for each of the
  three modes, including `on` not replaying the same item twice.

---

## Non-goals

- **A play queue, in any form.** Deleted (ruling 1). Do not reintroduce "Add to
  queue" or "Play Next" without an explicit owner decision.
- **A resume prompt or countdown.** Deleted (ruling 9) — it is unbuildable while
  mpv owns the screen, and the explicit Resume / Play from Beginning verbs make it
  unnecessary.
- **Embedding video in the webview.** Stays external. See `.agents/decisions.md`
  (2026-07-14) — Wayland cannot embed a foreign process's surface, and the route
  that would work risks the HDR passthrough that is the whole reason mpv is
  external. If revisited it is a *spike*, not a plan, and its first question is
  "does HDR survive?".
- **Writing to server playlists.** Read-only (ruling 4).
- **SQLite, or any database.** JSON (ruling 5).
- **Moving anything else out of `config.json`.** Recents (capped at 20),
  tombstones (capped at 200) and the sort map are small, bounded and
  machine-written; splitting them buys nothing (ruling 5).
- **Playlists in the carousel.** Ruling 6.
- Rebuilding watch-state or the watched threshold — already correct.

---

## Risks

- **S1 deletes shipped, recently-landed work.** Per-surface-status slice 2
  (`67358fd`, 2026-07-14) gave the queue its own status line; it goes with the
  queue. The `surfaces` E2E scenario must keep red-proving the two surfaces that
  remain — do not let the queue's removal silently gut the guard.
- **S3's editor is the bulk of the work**, and S4 spans three server APIs plus new
  mock endpoints. If the plan has to be cut, cut from these two.
- **The `on` mode's replay loop** is the sharpest correctness hazard. Guard it.
- **Two sources of truth for "what plays next"** is the failure mode this design is
  built to avoid. If a later change moves the Continue Watching merge into the
  backend, it must move *wholly*, not partially.
- **Playlist playback position is not persisted.** Quit mid-playlist and the cursor
  is gone; the episode you stopped on is in Continue Watching, and you can restart
  the playlist from that item. Accepted edge, not a defect. Revisit only on an
  owner report.

## Implementation log

### S1 — complete and externally accepted 2026-07-15

Commit `ec5d613` deletes the queue model, six commands, queue UI/status/polling,
and queue E2E scenario; turns `play_item` into full-`ItemDto` single-item
playback with an explicit beginning flag; and exposes Play or Resume + Play from
Beginning on the context menu, item/season detail, and Continue Watching. The
mpv EOF notify remains as neutral plumbing for S3.

The coder separately red-proved the backend start-mode selector, queue absence,
each visible playback surface, the frontend intent-to-IPC mapping, and retained
detail/edit error ownership. Restored local gates passed and Linux real-app E2E
passed 18/18. Two independent Grok 0.2.101 / `grok-4.5` sessions reviewed exact
base `7f8a2c2` and head `ec5d613`, independently produced the forced-beginning
red and restored green, and accepted with no comments. The fail-closed harness
trail and exact evidence are in `.agents/review/findings/pl-s1.md`.

### S2 — complete and externally accepted 2026-07-15

Commit `c6bc5c1` makes the shared backend `play_by_key` path the sole owner of
play-start recording. It records the complete item only after mpv and tracker
setup succeed, leaves failed launches and tombstones untouched, shapes an
explicit beginning to zero, and gates the matching end callback until the
start-record attempt completes. The frontend `record_recent` writer and Tauri
command are deleted.

The coder separately red-proved beginning shaping, success-only side effects,
start/end ordering, backend ownership, failed-launch no-write behavior, and
both successful/failed tombstone legs. Restored Rust tests pass 101/101 and the
focused Linux real-app `playback surfaces` run passes 2/2; the full local gates
and Linux E2E 18/18 had already passed on the committed slice. Two independent
Grok 0.2.101 / `grok-4.5` sessions reviewed exact base `4e4eec0` and head
`c6bc5c1`, independently red/green-proved different backend guards, and
accepted with no comments. Exact evidence:
`.agents/review/findings/pl-s2.md`.

The plan's literal dispatcher-driven recent E2E moves to S3 because S1 left
only a notification drain, not a sequence caller; S2 proves the same shared
path without resurrecting queue or test-only dispatch state. Before S3 enables
auto-advance, add a per-play session identity so a replaced tracker's delayed
finish cannot re-front an old item or stamp a newer same-key session.

Next implementation slice: S3, Vela-native playlist persistence, editing,
mixed-source playback, and cursor-driven auto-advance.

## Verification

The repo's standard set (`.agents/repo-guidance.md`): `npm run check`,
`npm run build`, and from `src-tauri/`: `cargo check --locked`,
`cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`.
E2E (`npm run e2e`) is Linux-only — see `.agents/machines.md` for the VM.

## Open questions

- None blocking. Every design decision above is an owner ruling.
