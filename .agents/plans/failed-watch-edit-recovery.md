# Plan: a failed watch-state edit never reloads or loses the browse grid

Status: **APPROVED (owner, 2026-07-14) — implementation authorized.** The owner
admitted and ordered correction of the remaining r2 red-proof defect, then
explicitly said to start coding. Implementation commit: `b5c170a`; Grok code
review is active. See `## Plan review log`. The owner playtest of 0.1.48 failed
on the real Plex path. This follow-up is one code slice. The
per-surface-status implementation remains complete; this plan fixes the
recovery work that runs underneath its correctly separated status lines.

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

## Binding behavior (approved)

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

Strengthen `tests/e2e/scenarios/pagefail.mjs` case 4 with a committed green path
that never waits for a recovery listing:

1. Snapshot the exact ordered visible poster `aria-label` values, which are
   unique in this fixture, including the card used for Mark watched; establish
   that the view banner is clear.
2. Record both Items-request arrival and served-response counts. Arm the doomed
   edit with `unauthNextPlayed`, plus `failNextItems` and a 6000 ms
   `itemsDelayMs`. Do **not** wait for the listing controls to bind: on the fixed
   path they must remain armed because no recovery Items request exists.
3. From the Mark watched click until the named edit failure appears, sample the
   exact identity continuously. A missing, substituted, or empty set is an
   immediate assertion failure, not a polling condition to wait through.
4. After the edit settles, require the exact identity and attempted card to
   remain, both Items counters to be unchanged, both listing controls still to
   be armed, and the view banner still to be absent. The failed edit line must
   be the only new failure surface.
5. In `finally`, and before pressing Refresh on the passing path, explicitly
   return `unauthNextPlayed`, `failNextItems`, `unauthNextItems`, and
   `itemsDelayMs` to their neutral values. Assert the controls are neutral, then
   run a healthy explicit Refresh and require its listing to be served
   successfully; it must not be the request that consumes the guard. Assert
   the attempted card remains unwatched and actionable.
6. Keep the existing Home transient-state cases green; they prove the narrow
   repair still heals recents/tombstones when Home could have observed them.

Prove the committed guard red with separate temporary regressions, restoring
the committed tree after each:

1. Reinsert the old catch-to-`refreshWatchState()` call with the delayed Items
   response armed. The continuous identity guard must fail while that call
   publishes `items = []`; do not turn the green case into a wait for the
   recovery request.
2. Reinsert the old call with no response delay. The card set may restore too
   quickly to witness the blank, but the Items-arrival/non-consumption assertion
   must fail for the unnecessary request.
3. Publish a synthetic view failure from the failed-edit catch without making
   a listing request. The no-view-banner assertion must fail independently of
   the request and identity guards.
4. Temporarily inject a same-length UI-writing regression in `setWatched`'s
   failed-edit catch: replace the attempted entry in `items` with a copied item
   whose title/key produce a distinct poster `aria-label`, without clearing or
   changing the array length. The exact-identity assertion must fail while
   `cardCount === 60` remains green. Mutating only the mock catalog is invalid
   here because the fixed path makes no Items request and would never publish
   that mutation into the rendered grid.

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
- Grok `reviewloop` on the pinned code slice and every review-fix slice, with no
  round cap; apply the standing adjudication and guard-discipline rules.
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

## Plan review log

Plan-review loop (playbook `reviewloop`, adapted to design review; Grok and
Claude, headless one-shot, read-only tools). A design review cannot execute an
unimplemented guard, so `guard_confirmed` is recorded as `false`; convergence
requires both reviewers to return `accepted` with no material findings on the
same pinned plan.

**r1 — 2026-07-15T02:13:48Z — base `310c2ca`, head `a481f5d`; round verdict
`reopened`.**

- Grok 0.2.101 (`5bc4b5dfadcf`) returned `reopened`,
  `guard_confirmed: false`, with two ADMITTED findings:
  1. HIGH — case 4 leaves `failNextItems` armed and then requires a healthy
     Refresh. The Refresh would consume the one-shot and fail even after a
     correct production change; a surviving flag could also poison later
     cases. The plan must explicitly disarm listing failure/delay controls
     after proving non-consumption and before Refresh or case exit.
  2. MEDIUM — the hermetic steps conflate the old-path red proof, which must
     park a recovery Items request to observe the blank, with the fixed-path
     green proof, which must prove that request never exists. The plan must
     specify separate red and green phases.
- Claude Code 2.1.209 returned `accepted`, `guard_confirmed: false`, with no
  findings after checking the plan against the referenced frontend, backend,
  mock, and E2E paths. Its first process result was rejected fail-closed because
  denied read-only tool calls yielded placeholder fields; the recorded verdict
  is the substantive retry using only `Read`, `Glob`, and `Grep`.

Round outcome: both Grok findings are evidence-backed and jointly satisfiable;
they are admitted for plan revision before r2.

Finding 1 disposition: ADDRESSED — the hermetic case now disarms every related
listing one-shot before healthy Refresh and in case cleanup, and requires the
Refresh itself to be served successfully.

Finding 2 disposition: ADDRESSED — the committed green case never waits for a
recovery listing; separate temporary regressions prove card continuity,
no-request/non-consumption, view-banner absence, and exact identity red for
their intended reasons.

**r2 — 2026-07-15T02:24:57Z — base `310c2ca`, head `ad401ed`; round verdict
`reopened`, two-round cap exhausted.**

- Grok 0.2.101 (`5bc4b5dfadcf`) returned `reopened`,
  `guard_confirmed: false`, with one ADMITTED finding:
  1. HIGH — substitution red proof 4 mutates only the mock catalog, but the
     fixed failed-edit path intentionally makes no Items request. The rendered
     in-memory poster buttons therefore never receive the replacement, so the
     exact-identity assertion stays green and the purported same-cardinality
     red proof is vacuous. A valid proof must temporarily write the substituted
     same-length array into the UI, either by restoring a successful recovery
     listing for that proof or by assigning the array in the catch, then require
     exact identity to fail while `cardCount === 60` remains green.
- Claude Code 2.1.209 returned `accepted`, `guard_confirmed: false`, with no
  findings. Claude judged the four temporary regressions independently
  executable using the existing mock mutation and request/served controls; that
  conclusion conflicts with Grok's evidence that an unrequested mock mutation
  cannot change the already-rendered grid.

Outcome: there is no reviewer consensus, so the owner's conditional approval
did not fire. The plan remains unapproved at the requested two-round cap. The
r2 finding is evidence-backed and admitted, but correcting it would produce a
new unreviewed head; no third round or implementation starts without new owner
direction.

**Owner disposition — 2026-07-14.** The owner ordered the r2 finding corrected
as above and explicitly authorized implementation, with Grok `reviewloop` on
each code slice and no round limit. This supersedes the stopped status in the
preceding outcome paragraph: the corrected plan is APPROVED. Code-review
acceptance still requires Grok to independently confirm the implemented guard
proof with `guard_confirmed: true`.
