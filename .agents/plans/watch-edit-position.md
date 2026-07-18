# Plan: successful watch edits preserve browse depth and position

Status: **IMPLEMENTED; CLAUDE CODEREVIEW PENDING 2026-07-18.** The owner
directed autonomous execution through the recorded work queue, with each code
change reviewed through the Claude `codereview` playbook. This plan remains
binding for finding `wsp-1`; a material scope expansion requires a new plan.

## Owner-visible problem

After Mark watched or Mark unwatched succeeds in a library listing, Vela calls
the general watch-state refresh. Its browse path empties the listing, reloads
only the first page, and recreates the scroll container at position zero. A
user who loaded and scrolled through a large library loses both the loaded
depth and their place.

The clicked card's local mutation cannot replace server revalidation. In an
All-sources listing, watched state is the most-progressed state across every
backing. Unwatching one backing may correctly leave the merged title watched
because another backing is watched. Server-side Recently played ordering can
also move a title. The listing must stay server-authoritative without visibly
tearing down the grid.

## Binding behavior

- A successful manual watched-state edit revalidates its originating browse
  root only while that same root still owns the screen.
- Paginated section, merged-type, and drilled listings rebuild a fresh snapshot
  from offset zero through the depth loaded when revalidation starts. Existing
  cards, pagination state, and the scroll container stay mounted while those
  requests run.
- A successful buffer publishes once, restores the prior `scrollTop` after the
  DOM update, and leaves the next page loadable when it was loadable before. A
  shorter refreshed result clamps the restored position to its new maximum.
- A revalidation failure retains the confirmed local card state, every loaded
  card, prior pagination state, and scroll position. It publishes a listing
  failure on the view banner, not a false edit failure.
- Search and person roots retain their existing live reruns, which already keep
  old results mounted; their manual-edit reruns explicitly restore grid scroll.
- Home retains its hubs-and-recents refresh so Continue Watching curation is
  immediate. Playlist views retain their current invalidation behavior. Hidden
  Home data is invalidated after every successful edit.
- Navigation, a newer edit, playback completion, explicit Refresh, and newer
  pagination supersede an older buffered reload. Stale work cannot publish into
  another root or release a newer load's guard.
- Failed watched-state edits, `playback-ended`, and explicit Refresh keep their
  existing semantics. This slice changes only successful manual edits.

## Implementation slice

One code/test/version implementation commit plus one test-only guard-hardening
commit on `fix/wsp-1-preserve-watch-edit-position`, followed by
finding-specific Claude `codereview`. The ordinary code-change bump advances
Vela from 0.1.57 to 0.1.58 through `scripts/bump.sh`.

### Stable listing request

In `src/routes/+page.svelte`:

1. Extract the request selection embedded in `loadMore` into a typed immutable
   listing descriptor and page-fetch helper. Capture every scalar defining the
   request and root identity: child key; merged type/source/sort; or section
   key/type/binding/sort.
2. Make `loadMore` use the helper without changing ordinary navigation, error,
   infinite-scroll, tall-viewport, or Refresh behavior.
3. Compare descriptors structurally before publication. A same-looking Plex
   section with a different binding is a different root.

### Buffered manual-edit revalidation

Add a manual-edit-only browse reload that:

1. snapshots loaded offset/depth, `hasMore`, `gridEl.scrollTop`, the listing
   descriptor, `navEpoch`, and a new `loadGen` owner;
2. invalidates older pagination and holds `loadingMore`, but never blanks or
   partially replaces `items` and never enables the skeleton;
3. fetches `PAGE`-sized pages from zero until reaching the old depth or a short
   page. Offset zero is mandatory because a merged continuation page reuses the
   prior immutable backend snapshot;
4. after each await and before publication, requires exact generation,
   navigation epoch, and listing ownership;
5. atomically assigns buffered items, offset, and `hasMore`, then after `tick()`
   restores scroll clamped to the new range;
6. releases `loadingMore` only when its generation still owns the flag; and
7. on failure, publishes a listing error under this generation while leaving
   the prior grid/depth/pagination/scroll intact.

Capture the originating browse identity before `set_watched` awaits. After
success, retain the confirmed local `played` and resume-offset mutation,
invalidate hidden Home, and dispatch the appropriate refresh only if the
origin still matches. Completion after navigation must not re-enter or replace
the destination.

### Guards

- Add `tests/watch-edit-position.test.mjs` to the canonical frontend check. It
  owns the source invariants needed for a macOS Claude worktree proof: manual
  edits use the preserved path, buffering never blanks, starts at zero,
  restores scroll after `tick()`, and gates publication on root/navigation/load
  ownership.
- Add `tests/e2e/scenarios/watchposition.mjs` with one mock server and two
  libraries. In a 130-title library, load 120 cards, set nonzero scroll, and
  edit a page-two card while the first revalidation response is delayed.
  Require continuous exact cards/depth/scroll, requests at offsets 0 then 60,
  restored position, refreshed badge, and working page-three pagination.
- In that scenario, force the first revalidation page to fail after a
  successful edit. Require confirmed local state, exact depth/scroll and
  page-three capability to survive, with a view failure and no edit-failure
  line.
- Also navigate to the second library while an old-root buffer is delayed and
  require the destination to remain exact after the stale response settles.
- Update `markwatched.mjs` to retain server mutation and authoritative refetch
  witnesses while requiring the card to remain present during delayed
  revalidation.
- Extend `mergedview.mjs`: both duplicate backings begin watched, Mark
  unwatched targets one, and the revalidated merged card remains watched from
  the other. This prevents an optimistic-only implementation from passing.

After implementation, prove each behavior separately by mutating production
only and restoring from the committed head:

1. stop refill after page zero — loaded depth/page-three fails;
2. remove scroll restoration — exact position fails while depth passes;
3. blank `items` before buffering — continuous-grid evidence fails;
4. remove publication ownership checks — delayed old-root work replaces the
   destination;
5. begin merged revalidation at nonzero offset or skip it — merged backing
   authority/refetch fails;
6. clear or partially publish on listing failure — failure preservation fails;
7. remove the confirmed local `played` mutation — the failure-path badge guard
   fails without a server response to repair it.

## Verification

- `node scripts/check-js-toolchain.mjs`
- syntax-check changed `.mjs` files
- focused `node --test tests/watch-edit-position.test.mjs`
- `npm run check` and `npm run build`
- from `src-tauri/`: MSRV/stable checks, stable clippy with warnings denied,
  stable tests, and Cargo audit per `.agents/repo-guidance.md`
- Linux real-app focused scenarios: `watchposition`, `markwatched`,
  `mergedview`, `pagefail`, `refresh`, and `watchcurate`
- fresh-build full Linux `npm run e2e`
- Claude MCP `codereview` at pinned base/head, with an independent
  production-only mutation of the focused macOS-capable guard and exact
  restore/green/clean proof

## Non-goals

- No backend watched-state, merged-dedup, sort, or snapshot-policy changes.
- No changes to explicit Refresh, playback-ended refresh, failed-edit recovery,
  the edit race accepted for v1.0, Home curation semantics, or page size.
- No optimistic-only UI, targeted page splice, scroll-index heuristic, polling,
  artificial delay, or test-only production hook.
