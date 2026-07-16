# pl-s4: server playlists need read-only, source-isolated sequence playback

**Severity**: MEDIUM — flattening source failures can hide healthy playlists,
while treating a server playlist as a single item loses the approved sequence
model and any writeback could mutate server-owned curation.
**Status**: Verified
**Branch**: `main` (approved playlists Slice 4)
**Commits**: `963ef73` (implementation), `4090d73` (integration-guard repair)

## Evidence

At base `97acab1`, `MediaSource` exposed no playlist methods, no source parsed
playlist endpoints, and the UI showed only Vela-owned playlists. The existing
playlist cursor re-read only `playlists.json`, so namespaced item keys alone
could route individual items but could not retain server-playlist sequence
context. The exact Slice 4 contract is in `.agents/plans/playlists.md`.

## Predicted observable failure

One offline server can collapse every server playlist, a selected playlist can
stop after its first item, server order or duplicates can be lost, audio/folder
entries can be offered to mpv, or edit/write affordances can mutate a playlist
whose authority is Plex, Jellyfin, or Emby.

## What

Add read-only server-playlist discovery and item loading for Plex, Jellyfin,
and Emby; retain source-level availability in the sidebar; and extend the
exact-session cursor so a server playlist re-fetches and advances in server
order without writing either server or Vela playlist storage.

## Approach

`MediaSource` gains empty-default playlist methods and a namespaced
`PlaylistDto`. Plex parses both current and legacy playlist XML forms and pages
playlist contents; the shared Jellyfin/Emby client uses their documented video
playlist and playlist-items routes with flavor-specific authentication. The
command layer discovers sources concurrently into per-source availability
groups and adds a server cursor owner that re-fetches items on exact-session
advance. A separate frontend component structurally exposes playback only.

## Files changed

- `src-tauri/src/source/mod.rs`, `source/plex.rs`, `source/jellyfin.rs`,
  `plex_library.rs` — trait/DTO, authenticated parsers, filters, pagination,
  namespacing, and source implementations.
- `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` — isolated source groups,
  item/play commands, Tauri registration, and server-owned cursor advancement.
- `src/lib/types.ts`, `src/lib/ServerPlaylistView.svelte`,
  `src/routes/+page.svelte` — server groups, unavailable state, read-only detail,
  and playlist-root navigation.
- `tests/e2e/helpers.mjs`, `tests/e2e/mockjf.mjs`,
  `tests/e2e/scenarios/serverplaylists.mjs` — server playlist endpoints,
  per-source failure, real playback, sequence, and no-write guards.
- `tests/e2e/scenarios/sortpersist.mjs` — distinguish library listings from
  the new playlist-discovery request so the persisted-sort guard observes the
  request it claims to test.

## Guard proof

- Rust parser/default/query/mapping guards cover Plex, Jellyfin, Emby,
  namespacing, video-only discovery, hostile-id encoding, and empty defaults.
- Linux real-app `serverplaylists` covers healthy plus unavailable groups,
  exact order, no edit affordances, first-item playback, second-item automatic
  advance, and absence of server or local playlist writes.
- The full suite exposed the old `sortpersist` request selector treating
  playlist discovery's `SortBy=SortName` as a library listing. Filtering to
  requests with `ParentId` restores its declared scope; suppressing persisted
  sort application then fails it with `SortBy=SortName`, and exact restoration
  passes.
- The author independently broke and restored the source defaults; Plex row
  compatibility, video filtering, and namespacing; Jellyfin/Emby video query,
  user-data query, safe path encoding, namespacing, and audio filtering. Each
  exact Rust guard failed for the intended assertion and passed after exact
  restoration.
- On the Linux real app, the author independently collapsed per-source failure
  isolation, reversed server order, stopped automatic advancement, injected a
  Remove button, issued server POSTs, and wrote `playlists.json`. The focused
  scenario failed on each distinct observable contract and returned to 1/1
  after each exact restoration.
- The repaired `sortpersist` guard failed both when its `ParentId` selector was
  removed and when persisted sort application was suppressed, then returned to
  1/1 from exact head.
- Restored verification passed exact Node 26.5.0/npm 12.0.1, `npm ci`, zero npm
  vulnerabilities, Svelte 0 errors/0 warnings, frontend build, Rust 1.89 and
  stable checks, Clippy with warnings denied, 123 Rust tests, zero RustSec
  vulnerabilities with 17 allowed upstream maintenance/soundness warnings,
  and Linux real-app E2E 21/21.

## Coder dispute (if any)

None.

## Known gaps

Emby uses the shared documented MediaBrowser API lineage and is intentionally
experimental for v1.0; no live Emby server is available. Plex and Jellyfin use
official endpoint contracts, but only the Jellyfin-shaped real-app mock is in
the hermetic E2E suite; source parser guards cover Plex and both shared JSON
flavors.

## Reviewer comments

**Implementation r1-A — verdict recorded 2026-07-16T00:47:24Z — accepted.**
Grok 0.2.101 (`grok-4.5`) reviewed exact head
`963ef73f0108029a8c25e3db872575c7537c1049` against base
`97acab164ff1fab82adcecab1044fc3ab5dc47a7` in a detached worktree. It removed
Plex `Metadata` playlist-row support, observed the exact parser guard fail with
the missing current-form row, restored exact head, observed GREEN, returned
`guard_confirmed:true`, and reported no comments.

**Implementation r1-B — verdict recorded 2026-07-16T00:47:24Z — accepted.**
An independent, memory-isolated Grok 0.2.101 (`grok-4.5`) session reviewed the
same exact range in a separate detached worktree. It removed the Jellyfin/Emby
`MediaTypes=Video` contract while preserving a runnable query fixture, observed
the exact query guard fail, restored exact head, observed GREEN, returned
`guard_confirmed:true`, and reported no comments.

The subsequent full-suite run exposed a test-integration defect: the existing
sort-persistence scenario selected the new playlist discovery request instead
of a library listing. Commit `4090d73` repairs only that selector and was sent
through two fresh external proofs before closure.

**Integration r1-A — verdict recorded 2026-07-16T00:47:24Z — accepted.**
Grok 0.2.101 (`grok-4.5`, session
`f9275794-8063-4b3b-abc5-db4daf6ca6d7`) reviewed exact head
`4090d732699332a284744918727398da0503c28b` against base
`963ef73f0108029a8c25e3db872575c7537c1049`. It removed the new `ParentId`
selector, observed focused Linux `sortpersist` RED on playlist discovery's
`SortBy=SortName`, restored exact head, observed 1/1 GREEN, returned
`guard_confirmed:true`, and reported no comments.

**Integration r1-B — verdict recorded 2026-07-16T00:47:24Z — accepted.**
An independent, memory-isolated Grok 0.2.101 (`grok-4.5`, session
`d730924e-2ff0-4564-89fa-a3350d425015`) reviewed the same exact range in a
separate detached worktree. It suppressed persisted section-sort application,
observed focused Linux `sortpersist` RED on the true library request, restored
exact head, observed 1/1 GREEN, returned `guard_confirmed:true`, and reported
no comments. Both reviewer worktrees and the VM were checksum-verified at
exact reviewed content before cleanup.
