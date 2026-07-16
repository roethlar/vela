# ceof-1: Clean EOF must curate the completed item and repaint the successor

**Severity**: HIGH — a naturally completed episode can remain the only visible
Continue Watching card and require a manual watched-state edit before the next
episode appears.
**Status**: Verified
**Branch**: `main` (retrospective review after the owner clarified the reviewer
hierarchy)
**Commit**: `8894ca6baf268a9c3962aaac1f3417e57ec08339`

## Evidence

At base `07ecb4674e4fab696d6f80f1b028669530dc332c`, Plex final tracking reported
progress but did not explicitly mark a cleanly completed item played. The early
end refresh could therefore merge the still-unplayed server hub item back into
Home, while automatic successor playback recorded a new recent without a later
frontend refresh. The approved behavioral record is
`.agents/plans/clean-eof-carousel.md`.

## Predicted observable failure

After a natural episode EOF, the completed card can remain or reappear, Plex can
retain it as unplayed, and the automatically started successor can fail to own
the carousel until the user manually marks the old card watched. Quit, error,
or stale-session paths must not receive clean-completion side effects.

## What

Treat an exact joined clean EOF as watched completion: curate and tombstone all
matching local identities, best-effort sync played state to the owning source,
advance the exact sequence, and publish a post-successor Home refresh without
letting stale sessions, quit, or errors take that path.

## Approach

The backend joins mpv EOF to the matching tracker completion, admits it only
against the exact active session, persists bounded identity tombstones, advances
the current playlist or emits terminal continuation, refreshes Home, and routes
the watch-state identity to the owning source. The frontend awaits a successful
automatic play's new Home refresh so its generation supersedes the older
end-of-playback load.

## Files changed

- `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`,
  `src-tauri/src/recents.rs` — exact clean-completion admission, curation,
  sequence dispatch, refresh, and played-state synchronization.
- `src/routes/+page.svelte` — post-successor generation-owning refresh.
- `tests/e2e/mockjf.mjs`, `tests/e2e/scenarios/continueon.mjs`,
  `continuetv.mjs`, `playlistplay.mjs`, `serverplaylists.mjs` — delayed server
  state, exact identity, quit, generation, and both playlist-owner guards.

## Guard proof

- Disabling the automatic source played mutation fails the server-played
  assertion while local curation remains green.
- Removing exact-session tombstones resurfaces the completed item while the
  delayed server hub is parked.
- Removing the successful successor refresh leaves the new mpv play absent from
  the hero.
- Defeating the frontend generation guard lets the older tuple displace the
  successor.
- Removing the newer-session admission guard fails the Rust stale-completion
  test.
- Treating every tracker tail as EOF makes the quit leg continue and mutate
  watched state.
- Omitting the dispatcher refresh after intermediate playlist advancement keeps
  the successor out of the rendered carousel.
- Every mutation was restored from committed state and returned green; canonical
  local verification and the fresh-build Linux real-app suite passed.

## Coder dispute (if any)

None.

## Known gaps

The real Plex completion playtest is not yet durably resolved. The separate LOW
lock-scope issue admitted by the refreshed plan open review is `ceof-2`.

## Reviewer comments

Two Grok 0.2.101 / `grok-4.5` secondary reviews independently confirmed the
newer-session and tombstone guards against exact base `07ecb4674e4fab696d6f80f1b028669530dc332c`
and head `8894ca6baf268a9c3962aaac1f3417e57ec08339`; both returned accepted with
`guard_confirmed:true` and no comments.

**Primary implementation review — recorded 2026-07-16T16:28:08Z — accepted.**
Claude Code 2.1.211 (`claude-fable-5`) reviewed exact head
`d6bcb12cbf8e686aa587cb15a161e93d41937f0b` against base
`07ecb4674e4fab696d6f80f1b028669530dc332c` in its own disposable worktree. It
independently ran two mutations:

1. Disabling the `has_newer_match` refusal made the three stale-completion Rust
   guards fail at the predicted assertions; restoring the head made them pass.
2. Removing `add_tombstones` made the three identity guards fail with an empty
   `hidden_from_continue` set while admission guards remained green; restoring
   the head made them pass.

The restored full Rust library suite passed 140 tests, the worktree was clean at
the exact reviewed head, and the structured result returned
`guard_confirmed:true`. Verdict: `accepted`. Claude's only comment was a
non-defect scope caveat: the Linux-only E2E guard legs were source-reviewed but
not rerun on the macOS reviewer host; their existing fresh-build suite and
individual red proofs remain the execution evidence. The disposable worktree
was removed after the orchestrator independently confirmed it clean.
