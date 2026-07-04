# rev-3: Watched-state actions on merged cards route to a watch-incapable source

**Severity**: MEDIUM — a visible action (Mark watched/unwatched) errors every time for the common local+server merged title.
**Status**: Open
**Branch**: `fix/rev-3-watch-routing`
**Commit**: (pending)

## Evidence
`src-tauri/src/commands.rs` `rank_backings`: the merged entry's
`rating_key`/`source_id` point at the playback winner (local family ranks
first by policy), while `played` was adopted from a server backing.
`src/routes/+page.svelte` `setWatched` invokes `set_watched` with
`item.ratingKey`; `src-tauri/src/source/mod.rs` `mark_played` default errors
("this source doesn't support marking watched state") for local-family
sources.

## Predicted observable failure
A title backed by both a local/SMB file and a Plex/Jellyfin copy shows the
Mark watched menu item (played state came from the server), but clicking it
sends the local ratingKey and surfaces the unsupported error banner instead
of marking anything.

## What
Play identity and watch identity diverge on merged entries; the frontend used
the play identity for both.

## Approach
(pending)

## Files changed
(pending)

## Guard proof
(pending)

## Coder dispute (if any)
None.

## Known gaps
Stacked on rev-2's branch; merge order rev-1 → rev-5.

## Reviewer comments
(pending)
