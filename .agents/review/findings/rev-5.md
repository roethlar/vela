# rev-5: Merged watch state adopts the first server's state, hiding real progress

**Severity**: LOW — an in-progress title can show no progress bar in the merged view when another backing reported "unwatched" first.
**Status**: Open
**Branch**: `fix/rev-5-adopt-most-progressed`
**Commit**: (pending)

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
(pending)

## Files changed
(pending)

## Guard proof
(pending)

## Coder dispute (if any)
None (coder-originated finding).

## Known gaps
Stacked on rev-4's branch; same function as rev-2.

## Reviewer comments
(pending)
