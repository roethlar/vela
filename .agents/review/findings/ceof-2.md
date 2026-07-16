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

Not approved. The candidate direction is to preserve one ordered lock ownership
across local admission and automatic played-state synchronization while moving
playlist advancement and its network resolution outside that hold. The exact
ordering, ownership mechanism, and failure guard must be settled in the plan
before code changes.

## Files changed

None.

## Guard proof

Not implemented. A valid guard must park an offline or delayed playlist
advancement, submit an unrelated explicit watched-state edit, and prove that the
edit reaches its source before advancement is released while completion
ordering remains correct.

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
