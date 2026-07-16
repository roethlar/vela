# ceof-2: Playlist advancement holds the global watch-edit lock across network waits

**Severity**: LOW — watched-state edits can appear frozen for a long but bounded
offline playlist-advance window; no deadlock or wrong final state is predicted.
**Status**: Open — admitted from primary Claude plan open review
**Branch**: Not started; a revised plan and owner approval are required
**Commit**: None

## Evidence

`src-tauri/src/lib.rs` acquires `watch_edit_lock` before
`admit_clean_completion`, retains it across `advance_playlist`, and releases it
only after `mark_clean_completion_played`. `advance_playlist` can fetch a server
playlist and both Vela/server playlist paths call `play_by_key` sequentially for
unavailable entries. Stream resolution is network work and can consume the
source client timeout for each candidate. `set_watched` waits on the same
app-wide lock.

## Predicted observable failure

If a playlist source becomes unreachable at a clean item boundary, automatic
advancement can walk multiple unavailable entries while holding the global
watch-edit lock. During that interval, Mark watched/unwatched on any source
waits without feedback, potentially for minutes on a long playlist.

## What

The clean-completion transaction's ordering lock covers more work than its
state-ordering requirement needs. Local completion and the automatic server
played write require serialization with explicit edits; sequence discovery and
playback resolution do not.

## Approach

Not approved. The smallest safe candidate is to retain the acquired lock across
local admission and move that same guard into an automatic played-state future,
while sequence advancement/publication runs concurrently outside the guard's
effective lifetime. The played future must explicitly release the guard when
its write finishes even if advancement remains parked. Dropping and reacquiring
around advancement is unsafe: an explicit unwatched edit could land in the gap
and then be overwritten by the later automatic watched write. Exact ownership
and join behavior still require plan approval before code changes.

## Files changed

None.

## Guard proof

Not implemented. Extend the server-playlist mock to park its post-EOF item
refetch. While that GET remains unserved, require the automatic PlayedItems POST
to complete, then submit a later explicit DELETE and require it to complete,
leaving final state unwatched before the playlist GET is released. Current code
fails before either write; a concurrent implementation that retains the guard
until advancement also finishes fails on the later DELETE.

## Coder dispute (if any)

None. Intake ADMITTED the finding because it has concrete code evidence, a
predicted observable stall, and justified LOW severity.

## Known gaps

The owner has not chosen whether to repair this before release or accept it as a
known narrow offline-path risk.

## Reviewer comments

Claude Code 2.1.211 (`claude-fable-5`) returned this finding from the neutral
plan `openreview`, recorded at 2026-07-16T16:20:40Z, over exact base
`b42b3a74cd8d9ad5e5b16f153d87d169fff8a408` and head
`07ecb4674e4fab696d6f80f1b028669530dc332c`. The structured verdict matched the
required schema and exact SHAs.
