# rev-5: Merged watch state adopts the first server's state, hiding real progress

**Severity**: LOW — an in-progress title can show no progress bar in the merged view when another backing reported "unwatched" first.
**Status**: In progress
**Branch**: `fix/rev-5-adopt-most-progressed` (stacked on rev-4)
**Commit**: `49bfe87` (code); doc recorded in a follow-up commit

## Evidence
`src-tauri/src/commands.rs` `dedup_across_sources`: watch state is adopted
only when `group.played.is_none()` — the first backing reporting anything
(e.g. Plex: `played=Some(false)`, no offset) locks the group, so a later
backing's real progress (Jellyfin: `viewOffsetMs=30min`) is ignored.

## Predicted observable failure
A title unwatched on Plex but half-watched on Jellyfin renders in the merged
All view with no progress bar and no in-progress ranking signal.

## What
"First Some wins" is order-dependent (registry order), not
information-preserving. The merged card should reflect the most-progressed
known state.

## Approach
A total order over watch states replaces first-Some-wins: `watch_rank`
scores finished (3) > in-progress (2, deeper offset breaking ties) >
known-unwatched (1) > unknown (0). Both adoption sites in
`dedup_across_sources` use it — the incremental adopt when a backing joins a
group, and the restore after a richer-display swap (whose kept state is
already the running maximum, so restoring when it outranks the new face's
own state preserves the invariant). Order-independence follows from max
being commutative.

## Files changed
- `src-tauri/src/commands.rs` — `watch_rank` helper; both adoption sites;
  guard test.

## Guard proof
- `commands::merge_tests::dedup_adopts_the_most_progressed_watch_state` —
  plain-unwatched reported before real progress must not hide the progress;
  finished elsewhere beats barely-started. Reverting the initial adoption to
  first-Some-wins makes it FAIL (verified); restoring makes it PASS
  (verified).

## Coder dispute (if any)
None (coder-originated finding).

## Known gaps
Stacked on rev-4's branch; same function as rev-2.

## Reviewer comments
(pending)
