# rev-3: Watched-state actions on merged cards route to a watch-incapable source

**Severity**: MEDIUM — a visible action (Mark watched/unwatched) errors every time for the common local+server merged title.
**Status**: In progress
**Branch**: `fix/rev-3-watch-routing` (stacked on rev-2)
**Commit**: `59770c2ef20e071106439c46ab5c49b23bed1b11`

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
Merged entries now carry an explicit watch identity: `rank_backings` sets a
new `ItemDto.watch_key` to the first server backing's key (plex/jellyfin/
emby) whenever the ranked play face can't take watched-state actions, absent
when the face itself can. The frontend's `setWatched` invokes with
`item.watchKey ?? item.ratingKey`, so the visible action routes where it can
actually be recorded while playback identity stays untouched. Root cause
(play identity and watch identity diverging on merged cards) addressed in
the DTO, not patched around in the UI.

## Files changed
- `src-tauri/src/source/mod.rs` — `watch_key` field (+ all constructors).
- `src-tauri/src/commands.rs` — `rank_backings` computes it; guard test.
- `src/routes/+page.svelte` — `setWatched` routes via `watchKey`.

## Guard proof
- `commands::merge_tests::merged_watch_key_routes_to_a_server_backing` —
  smb face + plex backing → `watch_key` = the plex key; server-only face →
  `watch_key` absent. Neutering the watch_key computation makes it FAIL
  (verified); restoring makes it PASS (verified).

## Coder dispute (if any)
None.

## Known gaps
Stacked on rev-2's branch; merge order rev-1 → rev-5.

## Reviewer comments
(pending)
