# Plan: refresh Home after clean-EOF server watch state settles

Status: **APPROVED 2026-07-18.** The owner directed autonomous execution through
the recorded issue queue and explicitly clarified that `continue` means go,
without another routine check-in. This plan is binding for finding `chr-1`; a
material scope expansion requires a new approval.

## Goal

After a clean playback completion, the final automatic Home refresh must run
after the owning server has accepted or rejected the played-state update. A
server-only next episode that becomes eligible from that update must appear in
Continue Watching without manual Refresh. Playlist/Continue Playing release,
quit/error refresh, local curation, failure behavior, and refresh counts must
remain unchanged.

## Existing race

`play_by_key`'s tracker tail first stamps local recents, emits
`playback-ended`, and joins its tracker-ended signal with clean EOF. That early
event is required for quit/error progress and is necessarily before clean-EOF
admission.

The joined dispatcher in `src-tauri/src/lib.rs::run` then:

1. holds `watch_edit_lock`;
2. admits the exact clean completion and curates local recents/tombstones;
3. advances any backend playlist and emits terminal `continue-playing`;
4. emits its authoritative `playback-ended`; and only then
5. awaits `mark_clean_completion_played`.

Plex hub/On Deck eligibility can change only when that last server mutation
settles. Both automatic Home reads can therefore contain the old hub, and a
new-series next episode remains absent until manual Refresh. A successful
frontend continuation happens to add another post-start refresh, but that does
not cover Continue Playing Off, no successor, a failed successor, or a
hub-only next episode.

## Binding ordering

Keep admission, sequence advancement, and terminal `continue-playing` exactly
where they are. Move the existing dispatcher-owned `playback-ended` payload and
emit block to immediately after the played-state attempt and its error handler.

The admitted clean-completion transaction becomes:

1. local exact-session admission/curation;
2. playlist advancement or terminal continuation release;
3. best-effort owning-server played-state await;
4. exactly one unconditional dispatcher `playback-ended` emit.

The emit occurs after either server success or failure. Do not add a third
dispatcher event, retry the server write, or move continuation below the slow
server await.

## Concurrency and failure semantics

- Retain `watch_edit_lock` across admission, sequence release, the server
  attempt, and the moved emit. A later explicit watched-state edit waits and
  then wins; do not drop/reacquire the lock.
- Intermediate playlist successors and terminal Continue Playing remain
  authorized before synchronization, so an offline server cannot delay the
  next mpv process.
- Success refreshes against both local curation/successor recents and the
  server's settled new hub eligibility.
- Failure logs exactly as today, retains local clean-completion curation and
  sequence state, and still refreshes once so local truth is visible. There is
  no rollback or retry.
- A persistence error or stale/newer-session refusal keeps the existing early
  exit: no sequence action, server mark, or dispatcher refresh.
- Quit/error playback never enters this dispatcher and keeps the tracker-only
  refresh.
- Home generation ownership remains sufficient. If a post-start refresh lands
  after a fast mark, it owns the newer generation; if it lands during a slow
  mark, the moved post-mark refresh becomes newer.

Refresh counts stay exact:

- quit/error: one tracker reload;
- admitted clean EOF with Continue Playing Off or an intermediate playlist:
  two reloads, tracker plus dispatcher;
- successful terminal automatic continuation: three reloads, adding the
  frontend post-start reload.

## Cost accepted by this slice

The authoritative local-curation/intermediate-successor repaint now waits for
the played-state attempt. Jellyfin/Emby requests use a 15-second timeout; Plex
can also rediscover and retry. The earlier tracker event remains immediate but
can race admission and is not authoritative. This latency is preferable to an
extra full multi-source Home fetch on every clean completion.

## Implementation slice

One code/test/version commit on `fix/chr-1-post-mark-refresh`, followed by
finding-specific Claude `codereview`:

- `src-tauri/src/lib.rs` — move the existing dispatcher refresh after the
  played-state result; update ordering comments only.
- `tests/clean-eof-refresh-order.test.mjs` — canonical source-order/count guard
  over the dispatcher block, runnable in a macOS review worktree.
- `package.json` — include that focused guard in `npm run check`.
- `tests/e2e/mockjf.mjs` — add a default-disabled successful-PlayedItems
  transition that can expose a chosen follow-up item through Resume; never fire
  it on a failed edit.
- `tests/e2e/scenarios/completionhub.mjs` — deterministic success and failure
  ordering/count behavior with Continue Playing Off.
- Version surfaces maintained by `scripts/bump.sh`: Vela 0.1.60.

No frontend, playback signal, command helper, playlist, source implementation,
or window-state behavior changes.

## Guard design

### Canonical source guard

Parse the dispatcher slice from `src-tauri/src/lib.rs` and require:

- exactly one dispatcher `playback-ended` emit;
- `advance_playlist` and terminal `continue-playing` precede the played-state
  await;
- the played-state call and its error log precede the dispatcher refresh,
  proving the emit is after the complete attempt and outside the error-only
  branch.

### Real-app success leg

Start with Continue Playing Off and one old episode in Continue Watching. The
mock exposes a different follow-up episode only when delayed successful
PlayedItems settles. After natural EOF:

- one Resume response lands while PlayedItems is pending;
- no second mpv starts and the follow-up is still absent;
- a second Resume response lands after PlayedItems is served;
- there are exactly two post-EOF Resume responses total;
- the completed episode stays suppressed and the follow-up renders without
  manual Refresh.

### Real-app failure leg

After restart, delay a one-shot PlayedItems 401 with a stable fallback card.
Require one pre-response and one post-response Resume reload, exactly two total,
the completed item locally suppressed, the fallback retained, and no
continuation. This proves the moved emit is unconditional after failure.

The existing delayed-PlayedItems leg in `continuetv` remains the continuation-
latency guard: E2 must launch before E1's server response is served.

## Production mutation proofs

After implementation, mutate production only and restore the exact committed
head after each red result:

1. move the dispatcher refresh back before the server await — source order and
   success post-response eligibility fail;
2. emit only on successful server mark — the 401 post-response reload fails;
3. add a duplicate pre-mark dispatcher emit — exact reload counts fail;
4. move terminal `continue-playing` below the server await — existing
   `continuetv` fails because E2 no longer launches while PlayedItems is parked;
5. remove the tracker emit — the completion-hub pre-response count and existing
   quit/progress coverage fail.

## Verification

- Node/npm pin assertion and syntax-check all changed `.mjs` files;
- `node --test tests/clean-eof-refresh-order.test.mjs`;
- `npm run check`, `npm run build`, and the canonical Rust/check/audit gates
  from `.agents/repo-guidance.md`;
- focused fresh-build Linux real-app scenarios: `completionhub`, `continuetv`,
  `continueoff`, `continueon`, `playlistplay`, `serverplaylists`, and
  `watchstate`;
- fresh-build full Linux real-app suite;
- Claude `codereview` over pinned base/head with an independent production-only
  mutation, red/restored-green proof, and exact clean restoration.

## Non-goals

- No additional polling, refresh event, artificial delay, or server retry.
- No change to watch-state authority, local tombstones, explicit edits, or the
  accepted queued-edit race.
- No change to playlist/Continue Playing selection, playback start timing,
  frontend generation rules, or quit/error behavior.
- No Plex/Jellyfin/Emby hub API change and no attempt to make the mock's
  synthetic eligibility transition a product feature.
