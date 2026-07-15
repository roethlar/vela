# pl-s1: Ephemeral queue survives where explicit video playback contexts belong

**Severity**: MEDIUM — queue state vanished on restart and its ambiguous Play
path could not express an intentional start-over without discarding resume
semantics.
**Status**: Verified — two independent external Grok sessions accepted r1
**Branch**: `main` (approved playlists Slice 1)
**Commit**: `ec5d613`

## Evidence

At base `7f8a2c2`, `AppState.queue` and `queue_index`, six `queue_*` commands,
and the queue chip/drawer represented an in-memory sequence that disappeared on
restart. `play_item` replaced that queue with one item and exposed no explicit
start-over mode. The approved product model and exact Slice 1 contract are in
`.agents/plans/playlists.md`.

## Predicted observable failure

A user can curate an apparent sequence that silently disappears after restart,
while an in-progress title exposes an ambiguous Play action that cannot promise
either resume or start-over consistently across the context menu, detail page,
and Continue Watching.

## What

Delete the ephemeral queue completely and make playback context explicit: a
single item offers Play when fresh, or Resume plus Play from Beginning when it
has progress. Preserve the neutral mpv EOF notification seam for the durable
playlist cursor introduced by Slice 3.

## Approach

`play_item` now accepts the full `ItemDto` and an explicit beginning flag;
`playback_start_ms` makes that flag override both server and local offsets while
retaining server-first resume authority otherwise. The frontend passes an
explicit `PlayIntent`, exposes both in-progress verbs on every playback surface,
and removes all queue state, commands, verbs, chip, drawer, polling, and status.
The queue E2E scenario is replaced by a real-mpv play-verbs scenario, while the
remaining detail/edit status ownership checks stay in `surfaces`.

## Files changed

- `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`,
  `src-tauri/src/playback.rs` — plain single-item playback, explicit start mode,
  deleted queue model, neutral EOF seam.
- `src/routes/+page.svelte`, `src/lib/ItemDetail.svelte`,
  `src/lib/SeasonDetail.svelte`, `src/lib/types.ts`, `src/lib/Icon.svelte` —
  explicit verbs on all playback surfaces and deleted queue UI.
- `tests/e2e/scenarios/playverbs.mjs`, `tests/e2e/scenarios/surfaces.mjs`, and
  dependent scenarios — real start-position assertions, queue absence, retained
  error-surface guards, and selector updates.

## Guard proof

- `commands::tests::playback_start_mode_honors_resume_authority_and_forced_beginning`
  failed at `left: 7000, right: 0` when forced beginning returned the server
  offset, then passed after restoring `ec5d613`.
- On the Linux real-app venue, separate production regressions made
  `playverbs` fail for each promised seam: resurrected queue UI; missing context
  Resume/start-over verbs; a beginning intent sent as resume (actual start near
  7.46s instead of below 2s); missing detail start-over; and missing Continue
  Watching start-over. Each exact production file was restored before the next
  proof.
- Separate production regressions made `surfaces` fail when a detail Play error
  was routed to the view banner and when a failed watch edit was routed away
  from its own edit line.
- Restored verification: targeted Rust guard green; `npm run check`, `npm run
  build`, `cargo check --locked`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked`, and Rust 1.89 compatibility all green; full
  Linux real-app E2E 18/18, with the final focused `playverbs surfaces` 2/2.

## Coder dispute (if any)

None. No reviewer raised a material finding.

## Known gaps

The real-app verb guard opens the movie detail component. The parallel season
detail implementation is compile- and inspection-covered but has no hermetic
episode hierarchy in the current Jellyfin mock; final live/manual playback
verification remains part of the release smoke.

## Reviewer comments

**r1-A — 2026-07-15T21:38:54Z — accepted.** Grok 0.2.101
(`grok-4.5`, session `019f67a9-9e28-7171-9d4d-c5b5a1f63680`) reviewed exact
head `ec5d6132df29226e879b91b077e9d3a045e8f075` against base
`7f8a2c2047c980f6fe599e17b2518717c4c2b564`. It independently injected the
forced-beginning regression in a detached worktree, observed the targeted Rust
failure, restored green, removed the worktree, and returned
`guard_confirmed:true` with no comments.

**r1-B — 2026-07-15T21:38:54Z — accepted after fail-closed correction.** A
separate Grok 0.2.101 / `grok-4.5` session initially claimed a guard in one turn
without tool activity; the orchestrator rejected that result. On the one allowed
corrective reprompt, session `019f67b5-f282-7423-9825-a5b71a61a950` inspected
the pinned diff, created its own detached worktree, produced the same targeted
red, restored green, removed the worktree, and returned exact SHAs,
`guard_confirmed:true`, and no comments.

Claude Code 2.1.210 / `claude-fable-5` did **not** contribute a verdict. Its
first attempt hit a Bash permission denial after creating but not completing a
proof; its permitted retry produced neither a worktree nor a verdict across the
stall threshold. Both attempts were failed closed, their disposable state was
removed, and neither is counted among the two acceptances.
