# Plan: a failed watch-state edit never reloads or loses the browse grid

Status: **DRAFTED 2026-07-14 — awaiting owner approval before code.** The
owner playtest of 0.1.48 failed on the real Plex path. This follow-up is one
code slice. The per-surface-status implementation remains complete; this plan
fixes the recovery work that runs underneath its correctly separated status
lines.

## Owner-visible defect

Reproduction on the real Plex server:

1. Open the Plex Movies library with its grid loaded.
2. Stop Plex without pressing Refresh.
3. Right-click **12 Years a Slave** and choose **Mark watched**.

Observed:

- Vela correctly reports the view failure and the named edit failure on
  separate surfaces, with no raw URL.
- The entire loaded grid disappears while the failed edit recovers.
- After Plex is restarted, the attempted title is absent from Vela's grid.
- Plex Web still contains the title and reports it unwatched.

Read-only diagnosis at `310c2ca` eliminated the backend and Plex as the source
of the omission:

- Plex returns the title in the exact query Vela issues for the first Movies
  page, at index 5 of 60. It is not a pagination-boundary item.
- The item is unwatched and has no persisted Continue Watching tombstone.
- `commands.rs::get_items` routes directly to `PlexSource::items`; neither
  recents nor Continue Watching tombstones participate in a library listing.
- Failed `commands.rs::set_watched` rolls back only `recents` and
  `hidden_from_continue`. The frontend mutates the clicked card only after the
  command succeeds.

The item therefore remains true and visible at every backend source of truth.
Its loss is frontend listing state.

## Root cause

`setWatched` catches the backend failure and invokes the same broad
`refreshWatchState()` used after successful edits and `playback-ended`.
Outside Home, that helper re-enters the visible browse root through
`resetAndLoad({ preserve: true })`.

The preservation added in `b88d9c9` is compensating rather than structural:
it snapshots `items` / `offset` / `hasMore`, still publishes `items = []`
while the server request runs, and restores the snapshot only when the request
returns an explicit failure while its `loadGen` is still current. Consequences:

- a dead Plex server makes the whole grid visibly blank for the full
  rediscovery/timeout window;
- another repaint can claim a newer generation while the blank is published,
  preventing the original full snapshot from restoring;
- recovery needlessly discards loaded pages and scroll on success;
- the view emits a second failure only because Vela made a listing request the
  failed edit did not need.

The existing mock and live-Plex guards sample only after settlement and compare
card counts. They cannot detect the long-lived intermediate blank, and a
same-cardinality substitution passes. The live recovery assertion requires
only that some cards return and targets the first card in the first available
library, not the reported Movies item.

## Binding behavior (pending owner approval)

A failed watch-state edit does not reload a browse, search, person, drill, or
detail listing. The loaded items, pagination, and scroll remain exactly as the
user had them. Only the edit's own failure line appears; no view failure is
manufactured by a recovery request Vela no longer makes.

Home still heals after the backend rolls back its temporary curation:

- if Home is visible when the edit fails, reload Home's hubs, recents, and
  tombstones;
- otherwise invalidate the hidden Home data so the next visit reloads it;
- the failed edit line continues to follow the user until the next edit, as
  settled by the per-surface-status decision.

Successful edits and `playback-ended` keep their current visible-state refresh.
Those paths have new server truth to display; the failed path does not.

## Implementation slice

One independently committed slice, with the ordinary version bump via
`scripts/bump.sh`.

### Frontend recovery

In `src/routes/+page.svelte`:

1. Separate the failed-edit Home repair from `refreshWatchState()`.
   `refreshWatchState()` remains the success / playback-ended path.
2. Add a narrow failed-edit repair:
   - set `hubs = []` so a later `goHome()` must reload;
   - when `authenticated && mode === "home"`, await `loadHome(++homeGen)`;
   - never call `resetAndLoad`, `runSearch`, or `runPersonView`.
3. Call that repair from `setWatched`'s catch after backend rollback and before
   publishing the edit status.
4. Delete `rootSig`, `myRoot`, and their recovery gate if they have no remaining
   caller. Their sole purpose is to decide whether the failed path may re-enter
   a browse root; the new rule never does.
5. Keep `resetAndLoad({ preserve: true })`. It still protects legitimate
   repaint failures after a successful edit or `playback-ended`; removing or
   broadening it is outside this defect.

Do not alter backend curation, rollback, Plex rediscovery, explicit Refresh,
or offset-zero refresh semantics.

### Hermetic E2E guard

Strengthen `tests/e2e/scenarios/pagefail.mjs` case 4:

1. Snapshot the exact visible card-label/key set, including the card used for
   Mark watched.
2. Park the edit's failed recovery window with mock response controls and hold
   the exact card set visible throughout. The current implementation must go
   red while it publishes `items = []`.
3. Assert the failed edit makes no listing request. A failed backend edit has no
   new browse truth to fetch; an armed next-listing failure must remain
   unconsumed and no view banner should appear.
4. After the edit settles, assert the exact set and attempted card remain, not
   merely the same count.
5. Run a healthy explicit Refresh and assert the attempted card remains
   unwatched and actionable.
6. Keep the existing Home transient-state cases green; they prove the narrow
   repair still heals recents/tombstones when Home could have observed them.

Red-proof the identity assertion separately by temporarily removing the
attempted key while backfilling a different uniquely keyed card. The old count
check must remain green and the exact-identity check must fail for the intended
reason.

### Real Plex guard

Strengthen `tests/e2e/live/plex.mjs` without making the live suite gating:

1. Select the Movies library and target **12 Years a Slave**, the exact reported
   path, rather than the first card of the first library.
2. Snapshot that card's identity and continuously assert it remains visible
   from the Mark watched click until the named failure lands.
3. Assert it remains immediately after failure and after Plex starts plus an
   explicit Refresh.
4. Open its context menu and verify **Mark watched** is still offered; do not
   click it.
5. Preserve every existing real-service cleanup, watchdog, and restore-on-exit
   rail.

The Jellyfin mock cannot reproduce Plex trusted-HTTPS rediscovery. The live
scenario remains opt-in/non-hermetic evidence, never part of the gating suite.

## Verification

- Prove every changed/new guard red for the intended reason, restore from the
  committed state, then prove green.
- `npm run check`
- `npm run build`
- From `src-tauri/`: `cargo check --locked`
- From `src-tauri/`: `cargo clippy --all-targets --locked -- -D warnings`
- From `src-tauri/`: `cargo test --locked`
- Full Linux `npm run e2e`
- Opt-in `npm run e2e:live -- live-plex` with the existing service-restoration
  rails.
- Two independent code reviewers (`codex` and `grok`) on the same pinned diff;
  apply the standing adjudication and guard-discipline rules.
- Owner playtest: repeat the exact report. The grid and **12 Years a Slave**
  never disappear; only the named edit line appears while Plex is down; after
  Plex starts and Refresh runs, the item remains unwatched and actionable.

## Non-goals

- No backend or Plex API changes.
- No change to successful watch-state edits.
- No change to `playback-ended` refresh behavior.
- No change to explicit Refresh resetting the listing to offset zero.
- No attempt to infer whether an ambiguous lost HTTP response was applied by a
  server; this report was not ambiguous, and Plex confirmed no mutation.
- No playlists/queue work.

## Risks

- A Home snapshot taken during curate-first must still be repaired after
  rollback. The existing Home race guards are mandatory and must be re-proven.
- Removing the browse recovery also removes its view-level error line. That is
  intentional: there is no browse request and therefore no browse failure.
- Hard-coding a live title is owner-environment-specific. It belongs only in
  the opt-in live-Plex scenario; the hermetic guard remains generic.
