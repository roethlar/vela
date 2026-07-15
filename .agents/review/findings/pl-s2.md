# pl-s2: Successful playback bypasses Vela recents

**Severity**: MEDIUM — plays launched through the shared backend path can be
absent from Continue Watching, while moving the write too early would create
false recents or clear curation after a failed launch.
**Status**: Verified — two independent external Grok sessions accepted r1
**Branch**: `main` (approved playlists Slice 2)
**Commit**: `c6bc5c1`

## Evidence

At base `4e4eec0`, `play_by_key` resolved and launched playback but relied on a
frontend-only `record_recent` call. Any backend-driven caller therefore skipped
Vela's recent, and the split writers had no ordering boundary against a very
fast mpv exit. The exact Slice 2 contract is in `.agents/plans/playlists.md`.

## Predicted observable failure

A successfully launched title can be missing from Vela's Continue Watching
strip or remain as an unstamped open session. Conversely, a failed resolve or
mpv spawn can create a false recent or clear the user's Continue Watching
tombstone if recording occurs before the launch has fully succeeded.

## What

Make the shared backend playback path the sole owner of play-start recording.
Record the full item snapshot only after playback and tracker setup succeed,
and order that start record ahead of the same session's final-position stamp.

## Approach

`play_by_key` now wraps the completed launch result in
`after_successful_play`, records the full `ItemDto` with
`recents::record_play_start`, and treats a post-launch persistence failure as a
sanitized best-effort error rather than falsely reporting that a running player
failed. `PlayStartGate` holds the tracker callback until the start-record
attempt is complete, with an RAII release for cancellation and every early
error. `record_play_start` preserves Resume state, resets explicit beginning to
zero, and delegates to the existing atomic record/tombstone transaction. The
frontend command and invocation are deleted so there is one writer.

## Files changed

- `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`,
  `src-tauri/src/recents.rs` — backend-owned success boundary, start/end gate,
  start-mode shaping, tests, and deletion of the frontend recording command.
- `src/routes/+page.svelte` — delete the frontend fire-and-forget writer.
- `tests/e2e/scenarios/playback.mjs` — assert the backend-owned open recent and
  successful tombstone clearing before mpv exits, then the final position.
- `tests/e2e/scenarios/surfaces.mjs` — force a deterministic post-resolve mpv
  spawn failure and assert no recent plus exact tombstone preservation.

## Guard proof

- `recents::tests::record_play_start_resets_only_for_an_explicit_beginning`
  failed at `left: Some(30000), right: Some(0)` when beginning no longer reset
  the snapshot, then passed after restoring `c6bc5c1`.
- `commands::tests::successful_play_side_effect_runs_only_for_a_completed_launch`
  failed at `calls: 2` when the callback ran on `Err`, then passed restored.
- `commands::tests::playback_end_waits_until_the_start_record_boundary_opens`
  failed at the closed-boundary assertion when `wait()` was bypassed, then
  passed restored.
- On the Linux real-app venue, deleting the backend record made `playback` time
  out waiting for the backend-owned open recent.
- Recording before spawn made `surfaces` fail because the deterministic failed
  launch created a recent; separately clearing the tombstone before spawn made
  its exact tombstone assertion fail.
- Re-adding the successful play's tombstone after recording made `playback`
  fail the successful-clear assertion.
- Restored verification: all 101 Rust tests pass and focused Linux real-app E2E
  `playback surfaces` passes 2/2. The full local gates and Linux E2E 18/18 had
  already passed on the committed slice before these post-commit proofs.

## Coder dispute (if any)

None. No reviewer raised a material finding.

## Known gaps

S1 left the dispatcher as notification plumbing with no sequence caller, so the
plan's literal "play via the dispatcher" E2E is deferred to S3's playlist
auto-advance guard; S2 proves backend ownership through the same shared
`play_by_key` path without adding test-only queue machinery.

The same-session gate does not solve a pre-existing cross-session stale-finish
race: an old tracker can finish after a replacement starts and re-front the old
item, or a same-key old tracker can stamp the replacement entry. S3 must add a
session identity before auto-advance relies on this path. This is distinct from
the owner-accepted queued watch-edit race.

## Reviewer comments

**r1-A — 2026-07-15T22:12:29Z — accepted.** Grok 0.2.101
(`grok-4.5`, session `019f67cf-a887-7c42-ba30-f7826881e3cc`) reviewed exact
head `c6bc5c1c26eac20959dd0954e590bb44445551cb` against base
`4e4eec026b9c22e874a6e448197c06850d10227e`. It inspected the complete pinned
diff, independently made the failed-launch side effect run on `Err`, observed
the targeted calls-count failure (`2` instead of `1`), restored the test green,
removed its clean detached worktree, and returned `guard_confirmed:true` with
no comments.

**r1-B — 2026-07-15T22:12:29Z — accepted.** A memory-isolated Grok 0.2.101
(`grok-4.5`, session `019f67d2-8ea3-7851-aaab-dceb98d0ca92`) independently
reviewed the same exact head and base without seeing r1-A. It bypassed
`PlayStartGate::wait`, observed the closed-boundary test fail, restored the
targeted test green, removed its clean detached worktree, and returned
`guard_confirmed:true` with no comments.
