# pl-s3: Vela lacks durable cross-source playlists and sequence-safe playback

**Severity**: MEDIUM — without a durable playlist model, curated sequences do
not survive restart; without session-owned advancement, a delayed playback end
can launch or stamp the wrong item.
**Status**: Verified — coder guards red-proved; two independent external reviewers accepted
**Branch**: `main` (approved playlists Slice 3)
**Commit**: `304f493`

## Evidence

At base `cdfc91a`, Vela had no playlist store, playlist commands, or playlist
surface. The retained EOF notification carried no playback-session identity,
and recent completion matched only an item key. A replaced tracker could
therefore finish into a newer same-key play, while any naive auto-advance
consumer could act on stale EOF state. The exact Slice 3 contract is in
`.agents/plans/playlists.md`.

## Predicted observable failure

A created or edited playlist can disappear after restart, playback can mutate
the playlist file, a removed/offline source can make the entire sequence fail,
or a delayed prior tracker can stamp or advance the replacement session. A
mid-playlist reorder can also be ignored if advancement uses a captured index
instead of re-reading the durable playlist and anchoring on the stable entry.

## What

Add durable Vela-native playlists, a sidebar editor and context-menu add flow,
and mixed-source sequence playback. Make playlist reads fail closed, keep dead
entries visible, keep playback read-only, and bind both recent completion and
auto-advance to the exact playback session.

## Approach

`storage.rs` extracts the config store's atomic, owner-only, cross-process
locked JSON discipline; `playlists.rs` layers a versioned playlist schema and
stable entry identities over it. `commands.rs` owns CRUD, read-time
availability, playlist playback, a session-paired EOF/tracker dispatcher, and
stable-entry re-anchoring after a fresh store read. `recents.rs` stores the play
session and rejects stale completion. `PlaylistsView.svelte` and `+page.svelte`
add the owned-status editor and Add to Playlist submenu without sharing errors
with the loaded library surface.

## Files changed

- `src-tauri/src/storage.rs`, `src-tauri/src/config.rs` — shared fail-closed,
  atomic JSON persistence with process and file locking.
- `src-tauri/src/playlists.rs` — versioned CRUD, stable entries, availability,
  validation, and persistence guards.
- `src-tauri/src/commands.rs`, `src-tauri/src/recents.rs`,
  `src-tauri/src/playback.rs`, `src-tauri/src/lib.rs` — exact-session recents,
  payloadful advancement, playlist cursor, fresh-read playback, and Tauri
  command registration.
- `src/lib/PlaylistsView.svelte`, `src/lib/errors.ts`, `src/lib/types.ts`,
  `src/routes/+page.svelte` — playlist navigation, editing, playback, submenu,
  and independent status ownership.
- `tests/e2e/helpers.mjs`, `tests/e2e/mockjf.mjs`,
  `tests/e2e/scenarios/playlistedit.mjs`,
  `tests/e2e/scenarios/playlistplay.mjs` — persisted editor and real-app
  sequence/race coverage.

## Guard proof

- `storage::tests::only_a_genuinely_missing_file_defaults` and
  `playlists::tests::malformed_or_future_store_fails_closed_without_rewriting`
  guard fail-closed reads and byte preservation.
- `playlists::tests::crud_uses_stable_entries_allows_duplicates_and_checks_bounds`
  and `store_round_trips_entries_and_order` guard durable CRUD and ordering.
- `playlists::tests::unavailable_entries_are_retained_and_marked_in_place`
  guards removed-source retention.
- `playlists::tests::read_only_playback_snapshot_leaves_store_byte_identical`
  and `playlistplay.mjs` guard read-only playback.
- `recents::tests::stale_same_key_finish_cannot_stamp_the_replacement_session`,
  `commands::tests::playback_advance_joins_only_matching_eof_and_tracker_sessions`,
  and `commands::tests::session_comparison_rejects_a_stale_dispatcher` guard
  stale completion and advancement.
- `commands::tests::next_playlist_position_tracks_the_stable_entry_across_edits`
  plus `playlistplay.mjs` guard fresh-read mid-playlist edits, mixed-source
  advancement, removed-source skipping, and silent resume.
- `playlistedit.mjs` guards UI CRUD, restart persistence, retained unavailable
  entries, and deletion.
- The coder independently injected and restored regressions for every claimed
  storage invariant: missing-only defaulting, dangling-symlink and malformed
  JSON failure, mutation rollback, owner-only data and lock modes, and future
  schema rejection. Each focused guard failed for the intended reason and then
  passed from exact head.
- The coder independently red/green-proved create, rename, duplicate retention,
  reorder bounds and order, remove, delete, name/type validation, load-order
  preservation, unavailable marking, and byte-identical read-only playback.
- The coder independently red/green-proved exact-session completion, stale
  different-key and same-key handling, EOF/tracker session pairing, cursor
  session ownership, stable-entry re-anchoring, start-gate ordering/failure,
  and removed-source recent filtering.
- On the Linux real app, the coder independently broke and restored each editor
  IPC path and UI ownership/availability guard, then independently broke and
  restored arbitrary-start selection, same-key tracker replacement, fresh
  stable-anchor advancement, silent resume, unavailable-source skipping,
  byte-identical playback, and exact-session final completion. Each focused
  scenario went red for the intended observable failure and returned to 1/1.
- Restored committed-tree verification: exact Node 26.5.0/npm 12.0.1,
  `npm ci`, zero npm vulnerabilities, Svelte check 0 errors/0 warnings,
  frontend build, Rust 1.89 and stable checks, Clippy with warnings denied,
  118 Rust tests, zero RustSec vulnerabilities (17 explicitly allowed upstream
  maintenance/soundness warnings), and Linux real-app E2E 20/20.

## Coder dispute (if any)

None pending.

## Known gaps

Configured-but-currently-offline sources are skipped when routing fails but
cannot be pre-marked unavailable because the registry exposes only currently
constructed sources. Removed sources are marked unavailable at read time and
retained. Server-owned playlists remain S4; Continue Playing remains S5.

## Reviewer comments

**r1-A — verdict recorded 2026-07-15T23:59:44Z — accepted.** Grok 0.2.101
(`grok-4.5`, session `019f6815-82aa-7c62-bf3c-1783c76cf866`) reviewed exact
head `304f49305dfcda21dc40228bc13b3b65bd7b9c98` against base
`cdfc91a77d78679eea2059806ac4ae8a6937a27b` in a pre-created detached
worktree. It inspected the complete pinned diff, removed the session match
from recent completion, observed the exact same-key replacement test run one
test and fail on the stale 75-second stamp, restored the head blob, observed
the same one test pass, left the worktree clean, and returned
`guard_confirmed:true` with no comments.

**r1-B — verdict recorded 2026-07-15T23:59:44Z — accepted.** A memory-isolated Grok 0.2.101
(`grok-4.5`, session `019f6817-6f57-7232-a701-53ed1e0e39c6`) independently
reviewed the same exact head and base in a different detached worktree. It
made malformed JSON default to an empty store, observed the exact playlist
fail-closed test run one test and fail because corrupt state was accepted,
restored the head blob, observed the same one test pass, left the worktree
clean, and returned `guard_confirmed:true` with no comments.

Two earlier reviewer-A launches failed closed and do not count. Session
`019f680f-3c4b-77e2-9aac-7c244a991bdc` could not create the mandated worktree
and its restored `--exact` proof was ambiguous about whether a test ran.
Corrective session `019f6812-d2d5-77e1-a426-1548d5aec480` produced an explicit
one-test red and green but had to use a clone after the same sandbox denial.
The orchestrator established that provenance, pre-created real detached
worktrees outside the reviewer sandbox, and then dispatched the two accepted
memory-isolated sessions above.
