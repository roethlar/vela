# Plan: clean episode completion advances Continue Watching

Status: **DRAFT — Claude r3 review unavailable; not reviewed, surfaced, or
owner-approved.** The owner reported the regression on a locally built 0.1.51
universal macOS DMG:
after an episode finishes, that episode remains the only Continue Watching
card, and manually marking it watched is required before the next episode
appears.

This is a follow-up to completed playlist Slice 5
(`.agents/plans/playlists.md`). It does not reopen the playlist model, Continue
Playing modes, or the accepted queued watch-edit race.

## Owner-visible defect

For a naturally completed episode:

1. mpv emits `end-file` with `reason=eof`.
2. Vela removes the completed local recent once its sampled position crosses
   the configured threshold and immediately reloads Home.
3. Plex still returns the old unplayed episode from Continue Watching / On Deck,
   so the merged hero reintroduces it.
4. Continue Playing starts the successor and records its recent in the backend,
   but the frontend never reloads after that successful automatic start. The
   older end-of-playback reload therefore remains authoritative.
5. Manual Mark watched sends the server's explicit played mutation, writes the
   existing Continue Watching tombstone, and reloads after the successor recent
   exists. That one action supplies both missing state transitions, so the next
   episode finally appears.

The exact paths are:

- final sampled progress and early `playback-ended` publication:
  `src-tauri/src/playback.rs::start_tracking_plex` and
  `src-tauri/src/commands.rs::play_by_key`;
- clean-EOF/tracker join and sequence dispatch:
  `src-tauri/src/commands.rs::PlaybackAdvance` and `src-tauri/src/lib.rs`;
- retained server hub merge and automatic successor launch:
  `src/routes/+page.svelte::heroItems` and
  `handlePlaybackContinuation`;
- explicit played mutation and curation:
  `src-tauri/src/commands.rs::set_watched` and
  `src-tauri/src/recents.rs::hide`.

The backend gap predates playlists: Plex final tracking has reported timeline
and progress, but only explicit Mark watched calls the source's `mark_played`
implementation. Slice 5 made that gap immediately visible by adding automatic
successor starts while retaining the literal rendered hub feed.

## Proposed binding behavior

- A joined clean EOF is a completed play. It marks the exact playback session
  finished locally, suppresses every identity of that completed item from the
  merged Continue Watching feed, and asks the owning server to mark it played.
- User quit, mpv error, replacement by another play, tracker-only exit, and an
  unmatched/stale EOF are not clean completion and never receive this treatment.
- The local suppression is the existing bounded, persistent Continue Watching
  tombstone mechanism. It guarantees the completed card cannot be resurrected
  by a lagging or offline server hub. Replaying the item clears its tombstones
  through the existing successful-play path.
- A clean completion may not remove, tombstone, or server-mark a newer same-key
  playback session. Exact session identity remains the authority.
- Automatic playback start must publish a newer Home refresh after the backend
  has recorded the successor recent. That refresh owns a newer `homeGen`, so an
  older end-of-playback response cannot overwrite it.
- `on` still chooses from the literal already-rendered Continue Watching order;
  it must not wait for or select from a fresh server response. Its no-repeat
  behavior is unchanged.
- Server played-state synchronization must not delay sequence advancement. The
  clean completion first commits local curation and advances/emits continuation;
  the server mutation then runs under the existing watch-edit serialization.
  A later explicit user edit therefore wins once it acquires that lock. Failure
  is logged like final progress-report failure and does not undo a proven clean
  EOF or stop the sequence.
- Continue Playing `off` still stops, but its completed card disappears without
  a manual edit. `only-tv`, `on`, Vela playlists, and server playlists all gain
  the same completed-item curation and post-start repaint at every clean boundary.

## Implementation slice

One code/test slice, independently committed, followed by the ordinary version
bump from 0.1.51 to 0.1.52 through `scripts/bump.sh`.

### Exact clean-completion curation

In `src-tauri/src/recents.rs`:

1. Add an exact-session clean-completion helper. It must:
   - detect a newer same-key/watch-key session and return without changing it;
   - remove the matching old session when it still exists;
   - tombstone the submitted play key, its optional watch key, and every matching
     identity still available on the snapshot;
   - preserve the existing FIFO bound and replay-clears-tombstone rule;
   - return whether the completion was admitted for server synchronization.
2. Keep `finish_session` for sampled quit/progress semantics. Do not reinterpret
   a non-EOF position as a clean completion.

Extend `PlaybackCompletion` in `src-tauri/src/commands.rs` with the optional
watch identity needed after `finish_session` may already have removed the
snapshot. The payload remains ids only; never include URLs, tokens, or stream
metadata.

In the joined clean-EOF dispatcher:

1. Acquire the existing `watch_edit_lock` before admitting the completion.
2. Commit exact-session local curation.
3. Advance an intermediate playlist item, or emit terminal
   `continue-playing`, without waiting for the server mutation.
4. Emit the existing refresh-only playback state event after any backend-owned
   successor has been recorded, so Home can read both the tombstone and the new
   recent.
5. For an admitted completion, route the namespaced key and call the source's
   existing `mark_played(raw, true)` while retaining watch-edit serialization.
   Log failure without rolling back local completion or sequence state.
6. Release the watch-edit lock. Do not call the public `set_watched` command:
   its user-edit undo/publication behavior is wrong for an already-proven clean
   EOF.

The dispatcher must emit terminal continuation before awaiting the server
write, so a slow/offline source cannot stall the next play. Holding the
watch-edit lock across that write orders any later explicit watched-state edit
after automatic completion, making the user's later choice authoritative.

### Post-start frontend repaint

In `src/routes/+page.svelte`:

1. After `handlePlaybackContinuation` receives a non-null session from
   `play_item`, call `refreshWatchState()` and await/own that refresh under the
   continuation attempt. It must claim a newer `homeGen` than the early
   `playback-ended` reload.
2. A stale/superseded `play_item` returning `null` must not refresh or install a
   continuation session.
3. Preserve the selected `next` item before awaiting any refresh. The `on` mode
   may not consult the response it triggered.
4. Backend-owned playlist intermediate advancement receives its post-start
   refresh from the dispatcher, because those plays bypass
   `handlePlaybackContinuation`.
5. Keep successful manual-play behavior and non-Home surface refresh semantics
   unchanged; do not add polling, a delay, an optimistic fake item, or a second
   carousel source of truth.

### Hermetic guards

Strengthen the existing mock and scenarios instead of adding a disconnected
test-only flow.

1. `tests/e2e/mockjf.mjs`
   - retain the faithful Resume hub and stateful PlayedItems behavior;
   - expose separate arrival/served evidence for a delayed automatic
     PlayedItems response using the existing `playedDelayMs` machinery;
   - make the existing arrival-bound, one-shot `playbackInfoDelayMs` delay a
     successful PlaybackInfo response too, not only the current forced-failure
     branch. This lets a scenario hold a successor before its local recent is
     recorded without changing the response body or failing playback;
   - keep `state.requests` as the authoritative arrival-order log for Resume
     and PlaybackInfo provenance;
   - do not make Stopped implicitly set `played`, because that would hide the
     explicit clean-completion contract.
2. `tests/e2e/scenarios/continuetv.mjs`
   - serve the real Resume hub;
   - complete E1 naturally while delaying its PlayedItems response;
   - require E2 to launch before that server response settles;
   - require the rendered hero to contain E2 and exclude E1 without a manual
     edit, both before and after the delayed server response;
   - require the server eventually records E1 as played;
   - retain quit-no-continuation, rollover, Specials, show-end, and stale-session
     assertions.
3. `tests/e2e/scenarios/continueon.mjs`
   - retain the initial Alpha/Beta/Charlie rendered-order proof and the parked
     early post-Alpha Resume response;
   - prove Beta's literal rendered-list selection by request arrival order:
     parked early Resume, then Beta PlaybackInfo, then the post-start Resume.
     Do not retain the old total-served-count assertion at Beta's socket,
     because the newly required post-start refresh may already have served;
   - use the following Beta-to-Charlie transition for deterministic generation
     coverage. Charlie is hub-only in the existing fixture. Before Beta EOF,
     zero Charlie's mock server resume point without refreshing the UI, park
     the early post-Beta Resume response, and delay Charlie's successful
     PlaybackInfo response long enough for the old load's local reads to settle.
     The retained literal carousel must still select Charlie;
   - after Charlie starts, its newly recorded open recent makes the post-start
     load center Charlie even though the live Resume hub omits it. Wait for both
     post-Beta Resume responses, then hold that Charlie remains the centered
     card; the delayed older snapshot contains neither a Charlie recent nor a
     Charlie hub item;
   - retain the full Alpha/Beta/Charlie no-repeat assertion in this same run.
4. `tests/e2e/scenarios/playlistplay.mjs` and `serverplaylists.mjs`
   - after one backend-owned intermediate advance, require the successor recent
     to render and the completed item to remain suppressed;
   - retain the byte-identical/no-write and exact-session boundaries.
5. Rust unit tests
   - matching clean session removes/tombstones every identity;
   - already threshold-removed session still tombstones supplied identities;
   - newer same-key session makes the stale completion a no-op;
   - replay clears completion tombstones;
   - quit/sampled finish alone does not invoke clean-completion curation.

Red-prove each claimed behavior separately after the fix is committed:

1. Disable the automatic source `mark_played` call: the server-played assertion
   fails while local UI curation remains green.
2. Disable exact-session tombstoning while the PlayedItems response is parked:
   E1 resurfaces from the server Resume hub and the identity assertion fails.
3. Remove the post-successor refresh: E2 starts in mpv but never becomes the
   rendered hero card.
4. Let the older post-Beta refresh reuse/win Charlie's successor generation in
   `continueon`. Charlie's server resume point was zeroed before the old request
   and its successful PlaybackInfo was held until that load's local reads
   settled, so the old tuple contains no Charlie identity. Publishing it after
   the newer post-start load removes Charlie from the center and fails the held
   identity assertion. The restored generation guard must keep that same tuple
   stale and Charlie centered.
5. Remove the newer-same-key guard: the Rust test loses the active replacement
   session/tombstone state.
6. Treat every tracker end as clean: the existing quit leg performs a
   PlayedItems write or suppresses E1 and fails independently.
7. Remove the dispatcher refresh after an intermediate playlist start: the
   playlist successor exists in config/mpv but not in the rendered hero.

Every injection is restored from the committed slice, never from a backup.

## Verification

- Syntax-check every changed `.mjs` file.
- Run the canonical local command set in `.agents/repo-guidance.md`, including
  exact Node/npm, clean install, both audits, frontend checks/build, Rust 1.89
  and stable checks, Clippy with warnings denied, and all Rust tests.
- Run each changed Linux real-app scenario for its red/restored-green proof,
  then the complete fresh-build Linux `npm run e2e` suite.
- Run two independent Grok `reviewloop` sessions on the same pinned code slice;
  each must independently red/restore/green a different guard and return
  `guard_confirmed: true`. Repeat on every review-fix slice until both accept.
- Owner playtest on the 0.1.52 universal macOS build: naturally finish a real
  Plex episode. The completed episode disappears, Plex records it watched, the
  next episode auto-starts under `only-tv`, and that successor appears in the
  carousel without a manual watched-state edit.

## Non-goals and known gaps

- No change to episode ordering, watched-next selection, Specials handling,
  playlist persistence, server playlist writing, or Continue Playing modes.
- No retry queue for a failed automatic server played-state write. The durable
  local tombstone preserves Vela's UI across restart; another Plex/Jellyfin
  client can still observe stale server state until a later successful edit.
- No live automated Plex completion fixture: safely completing and restoring a
  real episodic watch-state transition is not currently isolated. The owner's
  exact playtest is the live evidence.
- Emby remains experimental and shares the Jellyfin/MediaBrowser played-state
  implementation; no live Emby server exists.
- The owner-accepted queued watch-edit race remains the v1.0 release-note item.
  This slice uses the existing serialization but does not add persisted
  per-entry epochs or compare-and-swap curation.

## Risks

- The completion arrives after sampled `finish_session` may have removed its
  recent, so the event must carry both play and watch identities; recovering
  identity by querying a fresh server item would reopen the race and can fail
  offline.
- A duplicate/stale clean completion must be idempotent and must not tombstone
  a newer replay of the same key.
- Refresh ordering is part of correctness: a correct backend state can still
  render stale if an older Home response owns the final generation.
- Server synchronization runs after continuation is released. The watch-edit
  lock is therefore load-bearing: removing it permits a later explicit unwatch
  to be overwritten by the older automatic played write.

## Plan review log

Plan-review loop (playbook `reviewloop`, adapted to design review; Claude Code
with `claude-fable-5`, headless one-shot, read-only tools). An unimplemented
design has no runnable new guard, so `guard_confirmed` is recorded as `false`;
convergence requires Claude to return `accepted` with no material findings on a
pinned plan revision after checking it against current code and tests.

**r1 — 2026-07-16T05:01:06Z — base `b42b3a7`, head `b609303`; round verdict
`reopened`.**

- Claude Code 2.1.211 (`claude-fable-5`) returned `reopened`,
  `guard_confirmed: false`, with two ADMITTED findings:
  1. HIGH — the retained `continueon` served-count assertion would race the
     required post-start refresh. A correct implementation can serve that new
     Resume response after Beta starts but before the scenario observes Beta's
     socket, producing a false failure. The proof must use request-arrival
     provenance instead.
  2. MEDIUM — generation regression injection 4 would remain green because the
     delayed Resume body is computed at response time, after Alpha's automatic
     PlayedItems write normally removes it from the server hub. The scenario
     must keep that write parked until after the stale Resume lands.
- The first Claude process result was rejected fail-closed because model
  safeguards refused its final structured response. The one permitted fresh
  retry produced the substantive verdict recorded above on the same pinned
  base/head.

Round outcome: both findings cite current mock/scenario behavior and predicted
observable failures, so both are admitted for revision before r2.

Finding 1 disposition: ADDRESSED — the plan now replaces the racy response
count with the ordered request proof `early Resume < Beta PlaybackInfo <
post-start Resume`, which permits the post-start response to finish promptly
without weakening rendered-list selection provenance.

Finding 2 disposition: ADDRESSED — the plan now requires `playedDelayMs` to
outlast `delayNextResumeMs` and proves the old Resume is served first. The
stale snapshot therefore still carries Alpha, making generation-guard removal
observable while the restored guard keeps Alpha suppressed.

**r2 — 2026-07-16T05:40:32Z — base `b42b3a7`, head `3b99fbb`; round verdict
`reopened`.**

- Claude Code 2.1.211 (`claude-fable-5`) returned `reopened`,
  `guard_confirmed: false`, with one ADMITTED finding:
  1. MEDIUM — r1's second disposition controlled the delayed server hub but not
     the concurrent local recents/tombstone reads. Completion curation usually
     wins that race, so the old snapshot can already contain Alpha's tombstone;
     removing the generation guard then leaves Beta centered and the required
     mutation proof falsely green or flaky.

Round outcome: the finding cites the separate Promise legs and dispatcher
ordering and predicts failure of a required red proof, so it is admitted for
revision before r3. It supersedes r1 finding 2's incomplete disposition; the
r1 record remains intact as the history of that reviewed revision.

Finding 1 disposition: ADDRESSED — the generation proof no longer depends on
whether the old local reads precede completion curation. It uses the existing
Beta-to-Charlie transition: Charlie is hub-only, its server resume point is
zeroed without repainting the retained carousel, and its successful
PlaybackInfo response is parked until the old local reads settle. The old
tuple therefore has no Charlie, while the newer tuple has Charlie's open
recent; wrongly publishing the old tuple deterministically removes the
centered Charlie. The same run retains the three-key no-repeat proof.

**r3 — 2026-07-16T06:09:57Z — base `b42b3a7`, head `43957b1`; round verdict
unavailable (fail-closed).**

- The first Claude Code 2.1.211 (`claude-fable-5`) process reached zero model
  turns and returned `ConnectionRefused` because the configured local Claude
  bridge was no longer listening. It supplied no review evidence.
- The one playbook-permitted fresh-session retry kept the same model, prompt,
  read-only tools, schema, base, and reviewed SHA while using Anthropic's direct
  endpoint. It completed 34 review turns but terminated with HTTP 429 / Claude
  session limit before returning the required structured verdict.
- Neither process result is a review verdict. No r3 findings were admitted or
  dismissed, and r2's revised disposition has not yet been externally accepted.

Round outcome: review remains fail-closed. Do not surface or implement this
plan until the owner authorizes a later retry after the Claude limit resets or
selects another external Claude model.
