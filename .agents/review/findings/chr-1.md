# chr-1: Clean-EOF refresh precedes server hub eligibility

**Severity**: MEDIUM — a newly eligible next episode can remain absent from
Continue Watching until the user manually refreshes Home
**Status**: Admitted — approved plan; implementation not started
**Branch**: `fix/chr-1-post-mark-refresh`
**Base**: pending branch creation
**Implementation commit**: pending
**Last dispatched head**: pending

## Evidence

The owner reported that finishing an episode from a new series does not add its
next episode until the Refresh icon is clicked. The joined clean-EOF dispatcher
in `src-tauri/src/lib.rs::run` emits its authoritative `playback-ended` before
awaiting `commands::mark_clean_completion_played`. Server hub eligibility may
change only after that mutation settles, leaving no later automatic refetch.

## Predicted observable failure

With Continue Playing Off and a server hub that exposes the follow-up episode
only after PlayedItems succeeds, both automatic Home reloads finish before the
server mutation. The follow-up remains absent after the mutation settles and
appears only after manual Refresh.

## What

Move the existing dispatcher refresh after the played-state attempt while
preserving early sequence release, failure reload, tracker refresh, exact
refresh counts, watch-edit serialization, and stale-session exits.

## Approach

Binding implementation and guard design:
`.agents/plans/clean-eof-hub-refresh.md`.

## Files changed

Pending implementation.

## Guard proof

Pending implementation.

## Coder dispute (if any)

None.

## Known gaps

Real Plex playtest remains deferred to the owner's final pre-release smoke.
The hermetic proof uses the shared Jellyfin mock to model a server-state-
dependent Home eligibility transition; production source APIs remain unchanged.

## Reviewer comments

Pending Claude code review.
