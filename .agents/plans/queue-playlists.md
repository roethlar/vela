# Plan: Persistent Queue, Playlists, and Continue Playing

Status: **DRAFTED 2026-07-14 — awaiting owner go before any code.** Scope
approved by the owner 2026-07-14 ("everything we discussed": persistence, named
playlists with an editor, read-only server playlists, the melded carousel,
continue-playing modes, and the resume prompt). Every design ruling recorded in
`## Owner rulings` below was made by the owner in that session. Product
decisions that outlive this plan are in `.agents/decisions.md` (2026-07-14).

Origin: the in-app queue does not survive an app restart, which limits its
usefulness (owner, 2026-07-14). Investigation of that complaint surfaced a
second, larger defect — the Continue Watching carousel does not reflect the
queue at all — and the design that resolves both.

---

## Owner rulings (settled; do not reopen without the owner)

1. **Up Next is not a playlist.** It is an ephemeral *consumption* queue behind
   the queue icon. Items fall off as they are played and do not repeat unless
   the user added them more than once. It survives a restart; what survives is
   what has not been watched yet.
2. **Named playlists are durable objects** in a new Playlists sidebar entry.
   Playing one never consumes it. Vela's own playlists may mix items from
   different servers in one list.
3. **Skip-ahead consumes.** Clicking the 4th item in the queue drawer plays it
   and drops items 1–3 with it. Up Next only ever moves forward.
4. **Playlists are stored in their own JSON file**, not in `config.json`, and
   not in a database. Nothing else moves out of `config.json`.
5. **Server playlists appear, read-only**, alongside Vela's own.
6. **The carousel is one strip with two regions**: Up Next occupies the head,
   Continue Watching (recents ∪ server hubs) sits behind it. Up Next flows into
   Continue Watching as it drains. Named playlists never appear in the strip;
   their *items* do, once played into Up Next.
7. **Continue Playing is a three-mode setting** (Infuse's model): `off` stops
   when the queue drains; `on` keeps walking down the Continue Watching strip;
   `only-tv` plays the next episode of the series, rolls into the next season,
   and stops when the show runs out. **Default: `only-tv`.**
8. **"Only TV" means strictly the next episode in order**, watched or not — a
   deliberate rewatch keeps rolling.
9. **A resume prompt** ("Continue from 34:12 / Start from beginning", 5-second
   countdown defaulting to resume) fires on user-initiated plays of an item
   that has a resume position.
10. **Video stays external.** Embedding mpv in the webview was considered and
    rejected for now — see `.agents/decisions.md` (2026-07-14). It is not a
    prerequisite for anything in this plan.

---

## Current code (verified 2026-07-14 at `618de3b`; re-verify before editing)

**The queue is in-memory only.**
- `AppState.queue: Arc<Mutex<Vec<commands::QueueItem>>>` (`src-tauri/src/lib.rs:61`)
  and `AppState.queue_index` — a *cursor*, not a consumption model.
- Nothing in `src-tauri/src/config.rs` writes either to disk.
- `QueueItem` (`src-tauri/src/commands.rs:2148`): `rating_key`, `title`,
  `duration_ms`, `poster`, `subtitle`.

**Cross-source dispatch already works.** Item keys are namespaced
`<source_id>:<raw>` (`namespace_key`, `src-tauri/src/source/mod.rs:368`), and
`Registry::route` (`source/mod.rs:414`) splits the key and dispatches to the
owning source per item. **A queue mixing Plex and Jellyfin items already plays
correctly today.** Mixed-source playlists need no new dispatch machinery.

**Queue plays record nothing.** `play_by_key` (`commands.rs:2237`) resolves and
spawns mpv but never calls `record_recent`; the comment at `commands.rs:2365`
says so outright. Only the frontend direct-play path records
(`src/routes/+page.svelte:1484`). **This is why the carousel does not reflect
the queue** — Vela's half of the merge stays empty and only the server's hub
half moves. Already recorded as a defect in `.agents/state.md` (2026-07-10).

**`play_item` clears the queue.** `commands.rs:2380` replaces the whole queue
with the single item and sets the cursor to 0. Harmless today (the queue dies at
exit anyway); **destructive once the queue persists.** Ruling 3 + 6 change this.

**Auto-advance is cursor-based.** `lib.rs:181-213` waits on `queue_advance`,
takes `queue_index + 1`, and calls `play_by_key`. Consumption replaces this.

**The carousel merge is in the frontend.** `heroItems`
(`src/routes/+page.svelte:472`) = Vela's recents ∪ every hub whose `hubPolicy`
is `"hero"`; `hubPolicy` (`+page.svelte:445-447`) maps continue / resume /
**ondeck** to `"hero"`. So the server's next-up is already folded in — that is
the "server next semantics" the owner observed.

**Watch state is already solved; do not rebuild it.**
- `MAX_RECENTS = 20` (`src-tauri/src/recents.rs:14`);
  `DEFAULT_WATCHED_THRESHOLD = 95` percent (`recents.rs:17`), overridable via
  `AppConfig.watched_threshold_percent` (`config.rs:22`).
- `finish()` (`recents.rs:61`) drops an entry that ended past the threshold —
  *"watched to the end: no longer continue watching"*. **Stopping in the end
  credits therefore already counts as watched and keeps no resume position.**
- `resume_stamp_ms` (`recents.rs:91`) is Vela's fallback position; the server's
  `resolve_stream().resume_ms` wins when non-zero (`commands.rs:2278`).

**Dead-source handling has a precedent.** `filter_live_recents`
(`commands.rs:2175`) keeps entries from removed sources in the config untouched
and filters them at *read* time. Playlists follow this shape, but **mark**
rather than filter (ruling: a curated entry is never silently dropped).

**Tokens are already on disk.** `AppConfig.recents` stores full item snapshots
including token-bearing poster URLs, under owner-only permissions, per the
2026-05-23 decision. Playlist entries may do the same. The only real cost is
*staleness* (a rotated token yields a broken thumbnail) — handle with poster
fallback, not with a new storage rule.

**Episode walking needs no new server API.** The trait already has
`children(container_key, offset, limit)` (`source/mod.rs:309`), and `ItemDto`
carries `parent_key` (season, `source/mod.rs:101`), `grandparent_key` (show,
`:106`) and episode positioning (`:181`). `item_detail` (`:353`) and
`person_items` (`:361`) are the precedent for adding a trait method with a
default implementation.

**There is no database and no second store.** `config.json` (`config.rs:199`) is
the only persistent file; no `rusqlite`/`sled`/`redb` dependency exists. The
`metadata_cache.json` and listing cache described in `ISSUES.md` **no longer
exist** — they died with the local-source removal (2026-07-08). `ISSUES.md` and
the stale "listing-cache" comment at `source/mod.rs:63` are drift; fix
separately, not in this plan.

---

## The model

### Two kinds of list

| | Up Next | Named playlist |
|---|---|---|
| Where | queue icon / drawer, head of the carousel | Playlists sidebar entry |
| Consumed by playing | **yes** — items fall off | **no** — never |
| Ordered by | the user, and by what has been played | the user |
| Mixed sources | yes | yes (Vela-owned); server playlists are single-source |
| Editable | add / remove / clear / skip-ahead | full editor (Vela-owned); read-only (server) |

### Verbs

Acting on a playlist mirrors acting on an item, and **always copies**:
- **Play** → clear Up Next, copy the playlist's items in, start.
- **Play Next** → copy in after the currently playing item.
- **Add to queue** → copy onto the end.
- **Save Up Next as playlist** → the inverse; how most playlists get made.

Playing a playlist never mutates it. Only two things clear Up Next: playing a
playlist, and an explicit Clear.

### The carousel

One strip, two regions, with a **visible seam**:

```
[ Up Next: ep3 ][ ep4 ][ ep5 ] | [ Continue Watching: movie ][ show ][ ... ]
   ^ the plan (ordered by you)      ^ the shelf (ordered by history + server)
```

- **Click in the queue region** → skip-ahead: it plays, and everything above it
  is dropped (ruling 3).
- **Click in the shelf region** → it plays now; **Up Next is untouched** and
  resumes from its head afterwards. Nothing there was "passed over".
- **Dedup, queue wins.** If ep3 is queued *and* the server's on-deck offers it,
  it renders once — in the queue region.
- **"Remove from Continue Watching" is region-dependent.** On a shelf card it is
  today's tombstone. On a queued card it means "remove from the queue".
- Auto-advance is safe *because* the queue is visible at the head of the strip:
  nothing starts that the user could not see coming.

### Continue Playing (config `continue_playing`, default `only-tv`)

Consulted **only when Up Next drains.** While the queue has items, the queue
wins, always.

- `off` — stop.
- `on` — play the head of the shelf, then keep walking down it.
- `only-tv` — if the item that just finished was an episode, play the next
  episode in order (rolling into the next season); otherwise stop. Stop when the
  show runs out. Strictly the next episode, watched or not (ruling 8).

Two hard requirements:

- **`on` must walk the same melded list the carousel renders**, not a fresh
  server query. A second source of truth will diverge from the first — this repo
  has already paid for that lesson (the shared error banner, r17–r24). It also
  makes `on` respect Continue Watching tombstones for free: a removed item is
  not on the strip, so auto-play cannot reach it.
- **`on` needs a no-repeat guard.** An item the server does not mark watched can
  resurface at the head of the shelf and replay forever. A continuous auto-play
  run must never play the same key twice.

Specials: skip season 0 when walking, unless the episode that just played was
itself in season 0.

### Resume prompt

On a **user-initiated** play of an item with a resume position: an in-app
overlay offering "Continue from `<time>`" or "Start from beginning", with a
5-second countdown defaulting to resume.

- **Never on auto-advance.** mpv owns the screen by then; a countdown behind a
  fullscreen player is an invisible stall.
- Requires knowing the resume position *before* spawning mpv. Today it is
  resolved inside `play_by_key` (`commands.rs:2278`). Expose it to the frontend
  and let the caller pass an explicit start position.
- In a TV binge the next episode usually has no resume position, so the prompt
  will rarely fire mid-run. That is correct.

---

## Slices

Each slice lands and commits on its own, with its guard red-proven **after** the
fix (inject the regression, demand the test fail for the right reason, restore
from a committed state — see `.agents/state.md`, "RED-PROOF EVERY GUARD").

### S1 — Persist Up Next; consumption replaces the cursor

- New module `src-tauri/src/playlists.rs` owning `playlists.json` beside
  `config.json`. **Reuse `config.rs`'s proven write discipline**: atomic
  save, owner-only Unix permissions, cross-process lock, fail-closed parse.
  Extract the shared helpers from `config.rs` rather than reimplementing them.
- Types: `PlaylistEntry` (a `QueueItem` plus what recents already persist),
  `UpNext { items: Vec<PlaylistEntry> }`, `Playlist { id, name, items,
  created_ms, updated_ms }`. One file holds Up Next and all Vela playlists.
- Replace `AppState.queue_index` with consumption:
  - the head of `queue` is what plays next;
  - a play **pops** the item it played;
  - `queue_play_at(index)` drops `0..=index` (skip-ahead, ruling 3);
  - track the currently-playing item separately from the pending list so the
    drawer can show "Now playing" above what remains.
- Rewrite auto-advance (`lib.rs:181-213`) to pop the head instead of walking a
  cursor.
- **`play_item` no longer clears the queue** (`commands.rs:2380`): it plays now
  and leaves Up Next intact. If the played key is in Up Next, consume **one**
  copy (so it does not replay; multiple deliberate copies survive).
- Persist on every mutation.
- Guards: unit tests (round-trip; consumption; skip-ahead drops above;
  one-copy-consumed; fail-closed parse). E2E `queuepersist` — add items, restart
  the app, queue intact. Model it on the existing `sortpersist` restart scenario.

### S2 — Queue plays record recents

Closes the defect recorded in `.agents/state.md` (2026-07-10).

- `play_by_key` records a recent for **every** play, including queue plays and
  auto-advance. It currently has only key/title/duration; pass the
  `PlaylistEntry` snapshot through so `record_recent`'s `ItemDto` can be built.
- Preserve the existing rule that a **failed** play records nothing and clears
  no tombstone (`commands.rs:2366`).
- Guards: unit test; E2E — play from the queue, item appears in Continue
  Watching with its position.

### S3 — The melded carousel

- `heroItems` (`+page.svelte:472`) becomes: Up Next in order, then the existing
  recents ∪ hubs merge, deduped by rating key with **the queue winning**.
- Render the seam and mark queued cards.
- Region-dependent click and context-menu behaviour (see The carousel, above).
- Any queue failure raised from a carousel card reports on the **queue's**
  surface (the existing `queueStatus`), never the view banner — per the
  per-surface-status decision (2026-07-14).
- Guards: E2E — queued items head the strip; dedup; shelf click leaves the queue
  intact; queue-region click drops the items above it.

### S4 — Continue Playing

- `AppConfig.continue_playing: Option<String>`, three-state, default `only-tv`.
  Follow `mpv_autocrop` (`config.rs:43`) as the precedent for a validated
  three-state string that fails closed to its default on an unknown value.
- Settings UI.
- **Where the decision lives:** the melded list is computed in the frontend, so
  the backend cannot see it. On queue-drain the backend emits an event; the
  **frontend** applies the Continue Playing rule against the list it already
  renders and invokes the next play. This keeps one source of truth.
- New backend command `next_episode(item_key) -> Option<ItemDto>`: resolve the
  episode's `grandparent_key` (show) → `children()` for seasons → `children()`
  for episodes → the next in order, rolling into the next season, skipping
  season 0 unless already in it, `None` at the end of the show.
- No-repeat guard for `on`.
- Guards: unit tests for `next_episode` (mid-season; season rollover; end of
  show; specials skipped; specials honoured when starting in them). E2E for each
  of the three modes, including `on` not replaying the same item twice.

### S5 — Named playlists (Vela-owned)

The largest slice; the editor is most of it.

- Commands: `playlist_list`, `playlist_get`, `playlist_create`,
  `playlist_rename`, `playlist_delete`, `playlist_add_items`,
  `playlist_remove_item`, `playlist_reorder`, `playlist_save_from_up_next`.
- Sidebar "Playlists" entry; a playlist detail view with reorder, remove,
  rename, delete.
- Context menu gains "Add to playlist →".
- Playlist verbs (Play / Play Next / Add to queue), all copying into Up Next.
- **Dead entries are kept and marked unavailable**, not dropped: an entry whose
  source is removed or offline renders as unavailable and is skipped on
  playback. Follow `filter_live_recents` (`commands.rs:2175`) for the live-source
  check, but mark instead of filtering.
- The playlists view and the playlist detail each own their status line
  (per-surface status).
- Guards: unit tests (CRUD; reorder; dead-entry marking; save-from-up-next).
  E2E — create, edit, play a playlist; confirm playing it does not consume it.

### S6 — Server playlists (read-only)

- Trait: `async fn playlists(&self) -> Result<Vec<PlaylistDto>, String>` and
  `async fn playlist_items(&self, key: &str) -> Result<Vec<ItemDto>, String>`,
  both with default implementations returning empty (precedent: `item_detail`,
  `person_items`).
- Implement for Plex, Jellyfin, Emby. Keys are namespaced like every other key,
  so a server playlist's items route correctly for free.
- Sidebar groups them under their server. Play / Play Next / Add to queue only —
  **no editing, no writing back.**
- An offline server's group renders as unavailable and does not break the view.
- **The E2E mock servers need playlist endpoints** before any of this can be
  guarded. That is new mock work; budget for it.
- Guards: unit tests per source parser; E2E — a server playlist lists, plays, and
  cannot be edited.

### S7 — Resume prompt

- Expose the resolved resume position before playback (new command, or return it
  from a pre-play resolve) and let the caller pass an explicit start position to
  the play path.
- Overlay with a 5-second countdown, defaulting to resume.
- **User-initiated plays only** — never on auto-advance.
- Guards: E2E — an item with a resume position prompts; the countdown resumes;
  "Start from beginning" starts at 0; auto-advance never prompts.

---

## Non-goals

- **Embedding video in the webview.** Stays external. See
  `.agents/decisions.md` (2026-07-14) — Wayland cannot embed a foreign
  process's surface, and the route that would work risks the HDR passthrough
  that is the whole reason mpv is external. If it is ever revisited it is a
  *spike*, not a plan, and its first question is "does HDR survive?".
- **Writing to server playlists.** Read-only (ruling 5).
- **SQLite, or any database.** JSON (ruling 4).
- **Moving anything else out of `config.json`.** Recents (capped at 20),
  tombstones (capped at 200) and the sort map are small, bounded and
  machine-written; splitting them buys nothing (ruling 4).
- Rebuilding watch-state or the watched threshold — already correct.

---

## Risks

- **S5's editor is the bulk of the work**, and S6 spans three server APIs plus
  new mock endpoints. If the plan has to be cut, cut from these two.
- **S1 changes shipped behaviour** twice: `play_item` stops clearing the queue,
  and the queue cursor becomes consumption. Both are user-visible; call them out
  in the owner playtest ask.
- **The `on` mode's replay loop** is the sharpest correctness hazard. Guard it.
- **Two sources of truth for "what plays next"** is the failure mode this design
  is built to avoid. If a later change moves the melded list into the backend, it
  must move *wholly*, not partially.

## Verification

The repo's standard set (`.agents/repo-guidance.md`): `npm run check`,
`npm run build`, and from `src-tauri/`: `cargo check --locked`,
`cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`.
E2E (`npm run e2e`) is Linux-only — see `.agents/machines.md` for the VM.

## Open questions

- None blocking. Every design decision above is an owner ruling.
