# pl-s5: Continue Playing must advance only the exact cleanly-ended session

**Severity**: HIGH — a stale or non-EOF continuation can replace a newer manual
play, while incorrect episode or playlist boundaries can loop, skip, or launch
the wrong video without another user action.
**Status**: In progress
**Branch**: `main` (approved playlists Slice 5)
**Commits**: `6938c0f` (implementation), `18ae3d4` (backend-race guard repair),
`0da06b7` (server-playlist terminal guard)

## Evidence

At base `21ae7a`, playlist playback ended with the last playlist item and there
was no Continue Playing setting, continuation event, active single-item session
identity, or provider-neutral episode walker. The exact approved contract is
Slice 5 of `.agents/plans/playlists.md`.

## Predicted observable failure

A user quit can be mistaken for clean EOF; a delayed lookup can replace a newer
manual player; a normal-season binge can enter Specials or stop before the next
season; watched episodes can be skipped; `on` can repeat forever or consult a
different feed than the rendered carousel; or a playlist can hand off before
its final item or fail to hand off after it.

## What

Add the three-state Continue Playing policy (`off`, `on`, `only-tv`, default
`only-tv`), session-safe clean-EOF continuation, ordered cross-season episode
walking for Plex/Jellyfin/experimental Emby, a Settings control, and frontend
selection from the literal retained Continue Watching feed with a per-run
no-repeat set.

## Approach

The backend joins mpv's clean-EOF signal to the matching final tracker result,
retains an active UUID for both single-item and playlist playback, and accepts
an optional expected session on automatic replacement. It emits a dedicated
terminal event only after a single item or the true end of either Vela or server
playlist playback. The frontend owns policy because it owns the rendered
Continue Watching merge; `only-tv` delegates hierarchy walking to a namespaced
provider-neutral backend command.

## Files changed

- `src-tauri/src/config.rs`, `commands.rs`, `lib.rs` — persisted mode,
  completion/session arbitration, terminal dispatch, and episode selection.
- `src-tauri/src/source/mod.rs`, `source/jellyfin.rs` — exact episode hierarchy
  identity for Plex and the shared Jellyfin/Emby implementation.
- `src/lib/Settings.svelte`, `types.ts`, `src/routes/+page.svelte` — settings,
  event payload, literal rendered-list selection, and no-repeat state.
- `tests/e2e/mockjf.mjs`, `tests/e2e/scenarios/continueoff.mjs`,
  `continueon.mjs`, `continuetv.mjs`, `serverplaylists.mjs` — hierarchy,
  timing, mode, race, and both playlist-owner boundaries.

## Guard proof

- Focused Rust guards independently fail for fallback normalization, mismatched
  EOF/tracker sessions, completion item identity, stale expected sessions,
  watched-next selection, rollover, end/no-repeat, Specials skip/honor, Plex
  detail hierarchy, Jellyfin/Emby namespacing, and hostile item IDs.
- Linux real-app `continueoff` independently fails when the Settings callback is
  removed, Off advances like On, or restart reads a value other than the
  persisted mode.
- Linux real-app `continuetv` independently fails when quit implies EOF,
  `only-tv` skips episode lookup, rollover is disabled, Specials are admitted,
  show end falls back to Continue Watching, or the backend stale-session guard
  is removed. The race guard was repaired after its first version proved
  vacuous: a manual playlist play now bypasses the page attempt counter, and
  removing the backend comparison launches E2 over that manual movie.
- Linux real-app `continueon` independently fails when selection waits for a
  fresh server feed, the completed key is not retained in the no-repeat set, a
  Vela playlist terminates after its first item, or its final item emits no
  terminal continuation.
- Linux real-app `serverplaylists` independently fails when a server playlist
  terminates after its first item or its final item emits no continuation.
- Every injection was restored from committed state; each exact Rust guard and
  each focused Linux scenario returned green after restoration.

## Coder dispute (if any)

None.

## Known gaps

Emby shares the Jellyfin/MediaBrowser implementation and is intentionally
experimental for v1.0; no live Emby server exists. A final real Plex playtest is
deferred by the owner until release preparation. The accepted queued watch-edit
race is unrelated and remains a v1.0 release-note item.

## Reviewer comments

Pending two independent, memory-isolated Grok reviews of the same pinned range.
