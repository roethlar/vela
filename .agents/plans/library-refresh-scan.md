# Plan: library view refresh + server library-scan trigger (owner ask, 2026-07-12)

## Status
**APPROVED — owner "go" 2026-07-12. Review loop closed at r7 on the
owner's implement-then-code-review call: residual review moves to the
standard codex code reviewloop on the implementation diff; E2E RED
checks settle test-vacuity questions mechanically. Implementation in
progress.** Owner ask (2026-07-12, while testing Jellyfin):
(1) "there's no way to refresh the library view in vela … there needs to be
a refresh option"; (2) "can we trigger a library scan from within vela?"
followed by "plan" on the agent's refresh-first/scan-second scoping.

## Diagnosis (code-confirmed 2026-07-12)
The backend is already live: `get_sections` (`src-tauri/src/commands.rs:977`)
fetches every source's section list on each call (Plex
`get_library_sections`, JF/Emby `/Users/{id}/Views`) — nothing is cached
backend-side. The staleness is purely frontend: `src/routes/+page.svelte`
calls `loadSections` only from `loadEverything` (line 350), which runs at
startup, on source switch/reselect, after Settings source changes, and on
Plex link completion. `goHome` (line 396) re-fetches hubs at most (and only
when the cached list is empty), never sections. A library added on the
server therefore never appears until a source switch or an app restart —
and items added to an already-open library never appear in the visible
grid either.

Server-side scan triggers exist on all three backends:
- **Plex:** `GET {server}/library/sections/{key}/refresh` (the raw section
  key is the numeric Plex section id Vela already holds). Requires the
  server owner's token; a shared user gets 401. The owner's linked account
  owns the server, so this works for the primary use case.
- **Jellyfin:** `POST /Items/{itemId}/Refresh` (with the jellyfin-web
  scan-dialog params) — but the reliable `itemId` is the physical
  library's `VirtualFolderInfo.ItemId` from `GET /Library/VirtualFolders`,
  which is what jellyfin-web's dashboard scan uses. For an ordinary
  (ungrouped) library, the `/Users/{id}/Views` id Vela already holds as
  the raw section key (`jellyfin.rs sections()`,
  `namespace_key(&self.id, &v.id)` at `src-tauri/src/source/jellyfin.rs:784`)
  equals that `ItemId`. But Jellyfin can serve SYNTHETIC grouped user
  views ("group into one view" server option): posting Refresh at a
  synthetic view id returns success while scanning nothing. The scan path
  therefore resolves the target through `/Library/VirtualFolders` first
  (design §Slice 2.3). Both endpoints are admin-gated (RequiresElevation):
  a non-admin user gets 403, which must surface politely, not crash.
- **Emby:** same client code path and same endpoints as Jellyfin.
  **ASSUMPTION** (same class as the JF/Emby notes in
  `.agents/plans/show-last-episode-sort.md`): Emby accepts the identical
  refresh call; a refusal degrades to the error banner, non-fatal. Vela is
  Plex-first and the owner is currently testing JF; no Emby server is
  available to verify. The resolver route is flavor-branched (design
  §Slice 2.3(i)): Emby's documented endpoint is
  `/Library/VirtualFolders/Query` with an `Items` wrapper, unlike
  Jellyfin's bare-array `/Library/VirtualFolders`; Vela follows each
  flavor's documentation, and an Emby mismatch in practice errors visibly
  (no false success).

These are two features that pair: **refresh** re-asks the server what it
already knows (fixes the restart annoyance); **scan** tells the server to go
discover new files, after which a refresh shows the result.

## Design

### Slice 1 — library view refresh (frontend-only)
1. **`refreshLibraries()` in `+page.svelte`:** one user action that
   refreshes the section list AND the content the user is looking at.
   - **Navigation epoch (new):** a single counter, `navEpoch`, bumped by
     EVERY user navigation — `select`/`selectType`, `goHome`, running a
     search, opening a person view, drilling into children, opening or
     closing the detail surface, and source switches. It exists because
     the current generations don't cover all navigation: in-source
     navigation bumps `loadGen` but not `sourceGen`, and detail
     open/close bumps none of them — a delayed refresh outcome could
     otherwise force Home or publish a banner underneath a view the user
     navigated to meanwhile. The content leg and the final
     error publication are gated on `navEpoch` being unchanged from
     the action-start snapshot; the disappearance fallback is NOT —
     its gate is settlement-time root identity (contract below), never
     the epoch (the sidebar `sections` swap alone stays
     `sourceGen`-gated: a fresher section list is valid regardless of
     navigation).
   - At action start: clear the shared `error` banner once (the action
     owns its status — contract below) and SNAPSHOT the state it will
     reconcile against: the `navEpoch`, the VISIBLE-ROOT KIND — exactly
     one of `home | section-grid | type-grid | search | person | drill |
     detail` — plus the root's identity (`active?.key` for
     `section-grid`, `activeType` for `type-grid`). The kind must be
     derived from what is actually visible, not from residual state:
     `goHome` leaves `active` set (line 396-409) and a search retains
     `activeType` (line 603-623), so "has an active section/type" does
     NOT mean "is looking at that grid".
   - Re-fetch sections: bump `sourceGen`, fetch WITHOUT clearing
     `sections` first (unlike `loadEverything`, which blanks the sidebar
     for a source switch — a refresh must not flash the nav; the gen
     counter already discards a stale in-flight response).
   - **Content-leg precedence — exactly one, chosen by the snapshot's
     visible-root kind:**
     - `home` → re-fetch the Home data set (hubs/recents/tombstones)
       concurrently with sections. The Home leg CLAIMS a new `homeGen`
       (like every existing Home load, `+page.svelte:371-393`) and
       applies data/loading only for that generation plus the captured
       `navEpoch` — without the claim, an older in-flight Home load
       (startup, source switch) could overwrite the refreshed rails
       after settlement, or publish its stale failure over a successful
       refresh. Its error aggregates action-locally (contract
       below), never via `loadHome`'s own publish path — and the claim
       gates the FAILURE too: a leg whose generation was superseded
       contributes neither data nor failure to the action aggregate (a
       newer successful same-root load, e.g. playback-ended, bumps
       `homeGen` without touching `navEpoch`; the refresh's stale
       failure must not publish over it).
     - `section-grid` → after the current-generation sections response
       lands (`sg === sourceGen`) and the section still exists, reload
       the grid from offset zero with the current sort, REPLACING the
       items (not appending; the reset half of the existing
       reset-vs-append listing machinery) — `navEpoch`-gated so
       navigation performed meanwhile wins. Without this, a post-scan
       refresh would update the sidebar but leave the grid stale — the
       "scan then refresh" pairing this plan promises.
     - `type-grid` → same, via the type-listing reload.
     - `search`, `person`, `drill`, `detail` → NO content leg. These
       views are query-scoped, not library-list-scoped; refresh updates
       the sidebar only and leaves the visible content untouched.
       (Reloading here would, e.g., replace filtered search results with
       the full type listing under a Search crumb.)
   - **Disappearance fallback (one forced-Home path):** runs at
     settlement of a COMPLETE single-source sections response (an
     `activeSource` is set, or exactly one source is configured): a
     single-source `get_sections` either errors (→ the error contract;
     no reconciliation) or returns that source's complete list, so
     "missing" really means deleted. A MERGED-scope aggregate is partial
     BY DESIGN — `get_sections` skips failing sources, surfacing an
     error only when the combined result is empty and a source failed
     (`commands.rs:2447-2490` aggregate semantics) — so absence proves
     nothing there and NO disappearance fallback runs for `type-grid`
     roots (which only exist in the merged multi-source scope,
     `+page.svelte:1224`) or any merged refresh: a transient failure of
     one server must not yank the user Home. Accepted edge (recorded in
     Non-goals): a library genuinely deleted mid-merged-view leaves the
     user on its grid until they navigate; the content-leg reload still
     runs and shows the surviving items.
     **The fallback's gate is CURRENT ROOT IDENTITY, not the epoch:** at
     settlement, apply the forced-Home cleanup iff the browse root AT
     THAT MOMENT — the visible one, or the one hidden under an open
     detail — is a section grid rooted on the now-missing key. A pure
     `navEpoch` gate is wrong in both directions here: it would let a
     pre-navigation snapshot force Home after the user moved to B
     (r3's fix), but it would ALSO discard a cleanup the user still
     needs when the only mid-flight "navigation" was opening or closing
     a detail over the SAME dead section — leaving the sidebar without A
     while A's orphaned grid sits on or under the screen. Root-identity
     matching handles both: still rooted on the missing section (bare,
     or under detail) → clean up; moved to B/Home/search/anything else →
     untouched. Never applies to `home`/`search`/`person`/`drill` roots.
     When the dead root is hidden under an open detail, the detail
     surface itself is never touched: reconcile the HIDDEN state to Home
     (the detail crumb bar degrades to its existing Back-only-over-Home
     form), so closing the detail reveals Home instead of the orphan.
   - **Empty-Home redirect deferral:** the existing empty-Home effect
     (`+page.svelte:328-340`) auto-opens the first section when Home has
     no rails — and `select()` clears `detailView`. If the forced-Home
     cleanup lands under an open detail while the scoped Home is empty,
     that effect would CLOSE the detail the rule above promises to
     preserve and open some other library beneath the user. The redirect
     must not run while `detailView` is open; it may fire normally once
     the detail closes. The routine is a SINGLE forced-Home: goHome's state reset
     plus one unconditional Home re-fetch (bump `homeGen`, fetch) — NOT
     `goHome()` followed by `loadHome(++homeGen)`, which would
     double-fetch when hubs happen to be empty, and NOT bare `goHome()`,
     whose hubs-empty conditional (line 408) would keep cached rails that
     may still feature the removed library. (After landing, the existing
     empty-Home redirect at lines 328-340 may auto-open the first
     remaining section when the scoped source has no rails — accepted,
     existing behavior.)
   - **Error-lifecycle contract** (observable, guard-tested): the refresh
     legs are ACTION-LOCAL — no leg writes or clears the shared `error`
     itself (today `loadSections` publishes directly on failure, line
     366-368, `resetAndLoad`-style listing loads and `goHome` clear it on
     entry — the refresh action must not reuse those error paths as-is;
     parameterize or inline). `refreshLibraries` aggregates its legs'
     outcomes and publishes ONCE, at action end: (a) clicking Refresh
     clears any prior banner; (b) if any leg failed, the aggregated banner
     shows that failure even when sibling legs succeeded; (c) a later
     successful refresh clears a stale failure banner (via (a)); (d) the
     aggregate publishes only if the snapshot's `navEpoch` is still
     current AND every failed leg's claimed generation is still
     current — a refresh superseded by ANY navigation (including
     opening a detail page) stays silent (navigation wins), and a
     failed leg superseded by a newer same-root load (which bumps the
     leg's generation, not `navEpoch`) contributes nothing. Both
     orderings are guarded: older load → Refresh, and Refresh → newer
     load (case 14 and its reverse phase).
2. **UI:** an icon button beside the "Library" group heading in the sidebar
   (`.sidegroup`, `+page.svelte:1223` — becomes a flex row), aria-label
   "Refresh libraries", `title` tooltip. New `refresh` glyph in
   `src/lib/Icon.svelte` (circular-arrows). A `refreshing` boolean disables
   the button and spins the icon while the legs are in flight (feedback +
   double-fire suppression; correctness already comes from the gens).
3. No backend change in this slice.

### Slice 2 — server scan trigger (all three backends)
1. **Trait:** `MediaSource` gains
   `async fn scan_library(&self, _section_key: &str) -> Result<(), String>`
   defaulting to `Err("this source doesn't support library scans")` — the
   established opt-in pattern (`person_items`,
   `src-tauri/src/source/mod.rs:314`).
2. **Plex:** new `PlexLibrary` method issuing
   `GET {base}/library/sections/{key}/refresh` with the `X-Plex-Token`
   header (house pattern: `plex_library.rs get_library_sections:729`);
   `error_for_status` maps a non-owner 401 to an error string.
   **Key safety:** the raw section key is frontend-supplied text (the IPC
   command routes it via `registry.route`, which only strips the source
   prefix) — validate it with the existing `validate_plex_id`
   (`plex.rs:689`, digits-only) BEFORE building the path. This also rejects
   the special `all` pseudo-section and any path-shaped input
   (`../`, `?`, `#`). `PlexSource::scan_library` wraps the call in the same
   `ensure_ready` → on-error `rediscover` retry used by `sections()`
   (`src-tauri/src/source/plex.rs:283`). A small pure fn builds the path
   (`scan_path(key) -> Result<String, String>`, validation folded in) so
   the endpoint shape and the rejections are unit-testable — and the
   production request MUST be built through `scan_path` (a hand-formatted
   path beside it would make the test vacuous).
   **Version note:** `GET` is the long-standing verb (Plex Web still uses
   it; Plex staff confirm it stays backward-compatible) though newer docs
   prefer `POST` — record only; no dual-verb logic.
3. **Jellyfin/Emby (shared client)** — one exact production chain, in
   `JellyfinSource::scan_library(view_id)`:
   - **(i) Target resolution:** a scan-scoped client helper
     (`get_virtual_folders`) that mirrors `get_json`'s auth/timeout/401
     handling but ALSO maps `FORBIDDEN` to the same friendly
     administrator-permission message as step (iii) — the resolution GET
     is itself elevation-gated, so a NON-ADMIN'S SCAN DIES HERE, before
     the POST: without this mapping every non-admin would see a raw
     technical 403 error and the step-(iii) friendly message would be
     unreachable in practice. (Admin-gated like the scan itself, so no
     new privilege is required.) **The route branches by `Flavor`
     (`jellyfin.rs:33-40`):** Jellyfin → `GET /Library/VirtualFolders`
     (bare JSON array); Emby → `GET /Library/VirtualFolders/Query` with
     an `Items` envelope, per Emby's own 4.9 REST reference — mandating
     Jellyfin's bare route for both would make every docs-conforming Emby
     server 404 before the POST. The branch itself is a tested
     PRODUCTION selector — a pure `vf_route(flavor) -> (path,
     envelope-kind)` (or equivalent) that `get_virtual_folders`
     consumes — unit-tested for BOTH flavor values with a branch-flip
     guard proof (swap the match arms → both tests fail); envelope parse
     tests alone would stay green while Emby silently rode Jellyfin's
     route. Each envelope also gets a serde parse unit test; the Emby
     branch stays live-unverified (no Emby server available) but is
     aligned WITH its documentation rather than against it, and any
     real-world failure still surfaces visibly.
     Find the `VirtualFolderInfo` whose `ItemId` equals the raw view id —
     for an ordinary library the user-view id IS the virtual folder's
     `ItemId`, and that `ItemId` is what jellyfin-web's own dashboard
     scan posts to. No match means the section is a SYNTHETIC grouped
     user view: return `Err("this library groups multiple server
     libraries; scan them individually from the server dashboard")` — an
     honest, observable refusal instead of the false "Scan started" a
     blind POST at the synthetic id would produce.
   - **(ii) URL construction:** `JellyfinClient::scan_url(&self, item_id)
     -> Result<String, String>` — REJECTS empty, `"."`, and `".."` ids
     before building (the `url` crate's `PathSegmentsMut::extend`, used
     by `build_url`, silently DROPS exact dot segments — url 2.5.8
     documented behavior — so `".."` would otherwise collapse the path to
     `/Items/Refresh` instead of staying one encoded segment); then
     composes `build_url(&["Items", item_id, "Refresh"], &scan_query())`
     (`build_url`, `jellyfin.rs:239`, percent-encodes each segment, so a
     hostile id like `../System/Shutdown?x=` — path-shaped, backslashes,
     fragments — cannot escape the `/Items/{id}/Refresh` shape into a
     different authenticated endpoint; this matters more than on the GET
     paths because the scan call carries admin-capable credentials).
     **Never raw path interpolation.** Unit tests target `scan_url`
     itself, NOT `build_url` in isolation — a hostile-id test against
     the pre-existing generic helper would stay green even if the scan
     path interpolated the raw id (the repo's vacuous-guard rule).
   - **(iii) Dispatch:** `JellyfinClient::post_empty_url(&self, url)`
     owns the POST mechanics — auth headers, 15s timeout (mirroring
     `get_json`, `jellyfin.rs:113`), status mapping `UNAUTHORIZED →
     RECONNECT_REQUIRED` plus `FORBIDDEN → "the server refused the scan
     (administrator permission required)"` so the JF non-admin case reads
     as policy, not failure.
   - Query set: `Recursive=true`, `MetadataRefreshMode=Default`,
     `ImageRefreshMode=Default`, `ReplaceAllImages=false`,
     `ReplaceAllMetadata=false`, `RegenerateTrickplay=false` (the set
     jellyfin-web's scan dialog sends), returned by a pure `scan_query()`
     for unit testing.
   **Version note:** current Jellyfin servers default
   `RegenerateTrickplay=false` and no longer declare `Recursive` on the
   controller; unknown/undeclared params are ignored server-side, so this
   one request shape works across versions. The E2E asserts what VELA
   sends (our client contract), not what the server honors.
4. **Command:** `scan_section(section_key, state)` — resolve via
   `registry.route(&section_key)` (section keys are source-namespaced,
   same routing as `get_items`, `commands.rs:1028`) and call
   `src.scan_library(&raw)`. Register in `lib.rs` `generate_handler` beside
   `set_section_sort` (`src-tauri/src/lib.rs:231`). No new state, no
   persistence.
5. **Frontend:** right-click on a sidebar library entry (the
   `{#each sections}` branch only, `+page.svelte:1231`) opens a small
   section context menu — new `sectionMenu` state alongside the existing
   item `menu` (`+page.svelte:836`), same clamp + `menubackdrop` dismissal
   pattern; opening either menu closes the other. Single entry **"Scan
   library"** → `invoke("scan_section", { sectionKey })`. Success sets a
   transient neutral notice ("Scan started: <title>", auto-clears ~4s,
   rendered beside/like the error banner but not error-styled); failure
   routes to the existing `error` banner. **Per-attempt exclusivity:**
   initiating a scan clears any prior scan-produced banner and notice, so
   a failed attempt followed by a successful retry shows the success
   notice alone, never beside the stale failure. **Latest-attempt
   ownership:** scan attempts carry a generation (`scanGen`); only the
   LATEST attempt's outcome may publish its notice/banner or arm the
   auto-clear timer — otherwise a slow scan A completing after a quick
   scan B would overwrite B's status with stale news (out-of-order
   completions). The auto-clear timer is generation-owned too: starting
   a new attempt CANCELS any already-armed timer, and a firing
   callback clears the notice only if its generation still owns the
   published notice — otherwise a timer armed by an earlier success
   (A) would clear the newer attempt's (B's) notice almost
   immediately. The timer is also cleared on component destruction.
   The menu entry for a section with a scan already in flight is
   disabled as feedback; correctness comes from the generation, not
   the disable. No auto-refresh afterward — the
   scan is asynchronous server-side and completion is unknowable without
   polling (non-goal); the slice-1 button is the companion action.
   Type tabs in the merged multi-source scope get no menu (no single
   section to scan).

## Non-goals
- No automatic/polling refresh, no server-push (websocket) library-change
  subscription.
- No "scan all libraries" / whole-server entry — per-library only.
- No scan progress reporting, completion detection, or post-scan
  auto-refresh.
- No permission pre-flight (Vela can't reliably know scan rights up front);
  the server's refusal surfaces politely instead.
- No capability-gating of the menu entry by source kind — all three
  backends implement it; the default-Err trait impl covers any future kind.
- No mock support for Plex-flavor scan in E2E (the harness's mock is
  JF-shaped; Plex path is covered by unit test + owner playtest, the same
  split the repo already accepts for sort-key mapping).
- No scan of synthetic grouped Jellyfin user views — Vela refuses with an
  explicit message (scanning the underlying physical folders of a grouped
  view is out of scope until an owner asks).
- No disappearance fallback in the merged multi-source scope: its section
  aggregate is partial by design when a source transiently fails, so
  absence there is not evidence of deletion. A genuinely deleted library
  leaves the merged grid in place (content reload shows survivors) until
  the user navigates.

## Verification
- **Unit (guard-proven red→green; every test targets the PRODUCTION
  helper the request is built through — reverting that helper's body to
  naive interpolation/formatting must turn the test RED):**
  - Plex `scan_path` (used by `scan_library`, no hand-formatted path
    beside it): exact `/library/sections/{key}/refresh` shape for a valid
    id, and REJECTION of hostile keys — empty, `all`, `1/../2`,
    `12?force=1`, `12#f`, `1\2`, non-digits (via `validate_plex_id`).
  - JF `scan_url` (the production step (ii) helper, not `build_url` in
    isolation): a valid item id yields `…/Items/<id>/Refresh` carrying
    the full `scan_query` param set (`Recursive=true`, both refresh modes
    `Default`, both `ReplaceAll*=false`, `RegenerateTrickplay=false`); a
    hostile id (`../System/Shutdown?x=`, backslash, fragment) stays ONE
    encoded path segment — the result still matches
    `…/Items/<encoded>/Refresh` with no raw `../`, `?`, or `#` from the
    id; and REJECTION (`Err`) of empty, `"."`, and `".."` ids — the `url`
    crate drops exact dot segments, so without local rejection `".."`
    would silently collapse the path to `/Items/Refresh`. Guard proof:
    replace `scan_url`'s body with raw `format!` interpolation → the
    hostile-id and dot-segment assertions FAIL.
  - JF/Emby `vf_route` (the production resolver-route selector consumed
    by `get_virtual_folders`): both flavor arms asserted (Jellyfin →
    bare `/Library/VirtualFolders`; Emby → `/Library/VirtualFolders/
    Query` + `Items` envelope), plus serde parse tests for each envelope
    shape. Guard proof: swapping the match arms fails both tests.
- **E2E `libraryrefresh.mjs`:** `mockjf.mjs` gains scenario-mutable
  machinery: `state.views` (today hardcoded, `mockjf.mjs:100`; initialize
  to the current single view — existing scenarios unaffected) with
  PER-VIEW movie collections — the Items listing serves each view's own
  collection keyed by the requested `ParentId` (today everything belongs
  to lib1 and other parents are rejected, so a second library would 400
  on open); `state.addMovie(viewId, movie)` /
  `state.removeMovie(viewId, id)` operations that maintain the associated
  `byId`/`userData` entries coherently (the raw arrays are closure-held
  snapshots today, `mockjf.mjs:29-49` — pushing on an exposed array alone
  would leave `toJson` reading missing `userData` and 500 the next
  listing); a one-shot `state.failNextViews` flag (500 on the next
  `/Users/{id}/Views`) and a `state.viewsDelayMs` knob; a mutable Latest
  rail seed; and the Items listing handler HONORS `StartIndex`/`Limit`
  (today it ignores `StartIndex` and returns everything,
  `mockjf.mjs:114-139` — with an ignore-pagination mock, an
  implementation that appends instead of replacing would pass). The
  scenario SEEDS a non-empty Latest rail so Home has cached content to go
  stale. Cases run in ONE app session in this exact order —
  non-destructive first, view-destroying last, with EXPLICIT state
  restoration (re-add the removed view + collection, then run one
  settling Refresh) between the destructive phases; each case asserts
  only after the refresh control re-enables (the action-settled signal):
  1. *Sidebar + Home rails:* on Home → mutate the Latest seed AND push a
     second view (library B, with its own movie collection; A and B both
     exist from here on) → Refresh → the new sidebar entry appears
     without restart AND the Home rail reflects the mutated Latest (the
     `home` content leg re-fetches; a sidebar-only implementation fails
     the rail assertion).
  2. *Mixed-success aggregation (contract (b)), on Home:* set
     `failNextViews` → Refresh → the mock log shows the HOME leg's
     request succeeded, yet the banner reports the sections failure — a
     successful sibling leg must not mask it (on a grid root the content
     leg DEPENDS on the sections result, so only the Home root runs two
     independent legs and can prove this). Refresh again → banner clears,
     content current (contracts (a)+(c)).
  3. *Visible grid replaced from offset zero:* open library A's grid →
     `state.addMovie(A, ...)` AND `state.removeMovie(A, ...)` an existing
     one → Refresh → the post-refresh listing request carries
     `StartIndex=0`, the new card appears, the removed card is GONE, and
     the visible card set matches the mock's current collection exactly
     (count + titles) — WITHOUT re-entering the library. Appending or
     reloading from a stale offset fails the exact-set assertion (the
     post-scan pairing case, non-vacuous by construction).
  4. *Navigation wins (error):* on A's grid, set `failNextViews` +
     `viewsDelayMs` → Refresh → navigate (go Home) while the refresh is
     in flight → the delayed failure must NOT surface a banner after
     navigation (contract (d), `navEpoch`).
  5. *Navigation wins (content leg):* browsing library A (A still
     exists) → set `viewsDelayMs` → Refresh → immediately open library B
     while the response is in flight → after settlement B's own cards are
     visible, unreplaced (a content-leg reload not gated on `navEpoch`
     would overwrite B's grid with A's listing), and no error banner or
     mock contract violation was recorded.
  6. *Detail opened mid-refresh:* browsing A → set `failNextViews` +
     `viewsDelayMs` → Refresh → open an A card's detail while in flight
     (the detail-OPEN half of the interaction `openDetailAndPlay` drives,
     `tests/e2e/helpers.mjs:92` — WITHOUT its play step; factor the
     helper or inline the open) → the detail remains open and the
     delayed failure surfaces NO banner after settlement (detail OPEN
     must bump `navEpoch`; goHome/select-based navigation alone cannot
     prove this — those paths already bump the existing gens).
  7. *Detail closed mid-refresh:* with detail open over A → set
     `failNextViews` + `viewsDelayMs` → Refresh → close the detail (Back)
     while in flight → NO banner is published over the revealed grid
     after settlement (detail CLOSE must bump `navEpoch` too — an
     open-only implementation passes case 6 but fails this one).
  8. *Root-identity mismatch (fallback), destructive:* browsing A → set
      `viewsDelayMs`, remove A from `state.views` → Refresh → open
      library B while in flight → the delayed response must NOT force
      Home; B's grid stays because the settlement-time root (B) does
      not match the missing key (A) — the fallback's root-identity
      gate. (A `navEpoch` gate would pass here too, but would wrongly
      suppress the same-root detail cleanup of cases 11-12.) RESTORE
      A, settling Refresh.
  9. *Positive deleted-library fallback, destructive:* browsing A with NO
     further navigation → remove A's view → record the mock's hub-request
     count → Refresh → app lands on Home (or the standard empty-Home
     redirect) AND the mock log shows a NEW hub fetch after the refresh.
     Because Home rails were seeded non-empty, `goHome`'s hubs-empty
     conditional alone will NOT re-fetch — an implementation with no
     fallback, or bare `goHome()`, fails here (non-vacuous by
     construction). RESTORE A, settling Refresh.
  10. *Detail over a removed library, destructive:* open an A card's
     detail → remove A's view → Refresh → the detail surface STAYS OPEN
     with no forced Home (root kind `detail`) → THEN press Back → it
     reveals HOME (the hidden parent was reconciled), not the dead grid
     of a library with no sidebar entry. RESTORE A, settling Refresh.
  11. *Deletion racing a detail OPEN, destructive:* browsing A → set
     `viewsDelayMs` (success, no failure flag), remove A's view →
     Refresh → open an A card's detail while the response is in
     flight → at settlement the sidebar drops A AND the hidden parent
     reconciles (Back → Home, not A's orphan) — the root-identity gate:
     a pure epoch gate would discard this cleanup because detail-open
     bumped `navEpoch`. RESTORE A, settling Refresh.
  12. *Deletion racing a detail CLOSE, destructive:* open an A card's
     detail → set `viewsDelayMs`, remove A's view → Refresh → press
     Back while in flight → at settlement the revealed root reconciles
     to Home (still rooted on the missing key), not A's orphan grid.
     RESTORE A, settling Refresh.
  13. *Empty-Home redirect deferral, destructive (two sources, explicit
      selection):* the redirect requires `activeSource !== null`
      (`+page.svelte:331`); with a single mock and no selection it is
      ineligible and the deferral assertion is vacuous. Run this case
      with TWO configured mock sources and `activeSource` set to the
      mock containing A and B. Guard-prove eligibility first: empty
      the Latest seed (Home rails empty) with the source selected and
      NO detail open → the `select(sections[0])` redirect
      (`+page.svelte:328-341`) fires. Then the deferral: same
      empty-Home state → open an A card's detail → remove A's view →
      Refresh → the detail STAYS OPEN (the redirect must be deferred
      while `detailView` is open — it would otherwise close the
      detail and open library B underneath) → Back → the redirect may
      now fire (B's grid or Home both valid). RESTORE A, the Latest
      seed, and the selection, settling Refresh.
  14. *Stale older Home load, non-destructive:* arm one-shot
     `state.delayNextLatestMs` + `state.failNextLatest` → trigger a
     plain Home load that consumes them (an in-flight, slow, WILL-FAIL
     Home fetch) → immediately Refresh on Home → the refresh's Home leg
     (new `homeGen`) completes: rails present, and the older load's
     late failure publishes NO banner and overwrites nothing (the Home
      leg must CLAIM a generation; an unclaimed leg lets the stale
      result land). Reverse phase, same case: arm a one-shot slow
      FAILING refresh Home leg → while it is in flight trigger a newer
      successful Home reload (playback-ended path) → the refresh's
      late failure publishes NO banner at settlement (a superseded leg
      contributes neither data nor failure).
  RED without slice 1 (no refresh control exists to click).
  Mock knob additions for the above: `state.viewsDelayMs`, one-shot
  `state.delayNextLatestMs` + `state.failNextLatest`, and the mutable
  Latest seed.
- **E2E `mergedrefresh.mjs` (two mock sources, harness pattern from
  `mergedview.mjs`):** covers the merged-scope behaviors no
  single-source case can:
  1. *Merged type-grid reload:* All scope → open the Movies type grid →
     `addMovie` on mock 2 → Refresh → the new card appears without
     re-entering (the `type-grid` content leg).
  2. *Partial aggregate must not force Home (sole-provider failure):*
      seed mock 1 with only a non-Movies section (shows) so mock 2 is
      the SOLE Movies provider — both mocks default to Movies
      (`mockjf.mjs:100-102`), and with a shared type the merged grid
      would retain Movies from mock 1, letting a faulty
      force-Home-when-the-active-type-disappears implementation pass.
      Browsing the merged Movies type grid → one-shot fail mock 2's
      `/Users/{id}/Views` at refresh time → Movies disappears from
      the refreshed type tabs BUT the user is NOT forced Home (merged
      scope runs no disappearance fallback; a transient one-server
      failure is indistinguishable from deletion in a partial
      aggregate), mock 1's shows content still renders, and a
      follow-up Refresh with mock 2 recovered reloads the Movies
      content leg.
- **E2E `scanlib.mjs`:** mock gains `GET /Library/VirtualFolders`
  (returns one entry with `ItemId: 'lib1'`, matching the served view) and
  answers `POST /Items/{id}/Refresh` with 204 (requests are already
  recorded, `mockjf.mjs:93`), plus one-shot `state.failNextItemRefresh`
  (403 on the POST) and `state.failNextVirtualFolders` (403 on the
  resolution GET) flags. Cases:
  1. *Happy path:* right-click the library sidebar entry → "Scan
     library" → the mock log shows the `VirtualFolders` resolution GET
     followed by `POST /Items/lib1/Refresh` carrying `Recursive=true` and
     `RegenerateTrickplay=false` → the transient notice rendered.
  2. *Grouped view refused:* a second view (`Id: 'grouped1'`) with NO
     matching `VirtualFolders` entry — seeded BEFORE app launch (or
     added live followed by a Refresh) so its sidebar entry exists to
     right-click → scan it → error banner with the grouped-libraries
     message, and NO `POST /Items/grouped1/Refresh` in the mock log (the
     false-success case this guards is a 204 with nothing scanned).
  3. *Non-admin at resolution:* set `failNextVirtualFolders` → scan →
     the FRIENDLY administrator-permission banner (not a raw technical
     403), and NO refresh POST occurs — this is the step every real
     non-admin actually hits, since the resolution GET is elevation-gated
     before the POST is ever reached.
  4. *Retry lifecycle:* set `failNextItemRefresh` → scan lib1 → error
     banner (admin-refusal message class); scan again → success notice
     shown and the stale failure banner gone (per-attempt exclusivity).
  5. *Out-of-order completions:* the mock serves a second scannable
     library (`lib2` view + matching `VirtualFolders` entry) and a
     one-shot `state.itemRefreshDelayMs`. Arm the delay → scan lib1
     (slow) → immediately scan lib2 (fast) → lib2's success notice
     appears; when lib1's delayed response lands, the published status
     must NOT change (latest-attempt `scanGen` ownership — a stale
      attempt may not overwrite the newer one's banner/notice or
      re-arm the auto-clear timer). Second phase, both successful:
      scan lib1 (fast success, timer armed) → immediately scan lib2
      (success) → lib2's notice is still visible after lib1's
      original ~4s deadline passes (lib1's armed timer was cancelled
      or its callback generation-gated; it may not clear lib2's
      notice).
  RED without slice 2 (no menu entry, no request).
- Full local CI set (`npm run check`, `npm run build`; from `src-tauri/`:
  `cargo check --locked`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked`); full E2E suite on the owner's Linux VM
  (venue per `.agents/state.md`).
- **Owner playtest ask (real servers):** add a library in JF → Refresh →
  it appears without restart; right-click a JF library → Scan library →
  JF dashboard shows the scan running; same scan on Plex (owner token) →
  Plex shows the section scanning; delete/hide a library server-side →
  Refresh while browsing it → Vela lands on Home (or, when the scoped
  source has no Home rails, on the app's standard empty-Home redirect to
  the first remaining library) instead of a dead grid.

## Review log
Plan-review loop (playbook `reviewloop`, adapted to plan review; reviewer
codex, headless one-shot, read-only sandbox, mac host). Convergence =
a round with verdict `accepted` / no admitted findings.

**r1 — 2026-07-12 — codex-cli 0.144.1, verdict `reopened`, 3 findings,
all ADMITTED.** Base `5aa560c`, head `489f632`.
1. HIGH — refresh didn't reload the visible library content, breaking the
   plan's own scan-then-refresh pairing (and deleted-library fallback could
   land on stale cached rails). Fixed: slice 1 now reconciles the active
   browse view from offset zero (gen-gated) and forces the hub re-fetch on
   the deleted-library path; E2E extended with grid + deleted-library
   cases.
2. HIGH — raw frontend-supplied section keys interpolated into URL paths
   form an authenticated path-injection primitive (JF POST with
   admin-capable creds could be steered to e.g. `/System/Shutdown`).
   Fixed: JF/Emby URLs built via the existing segment-encoding
   `build_url`; Plex keys validated via the existing `validate_plex_id`;
   hostile-key unit tests specified.
3. MEDIUM — no coherent error lifecycle (stale failure banners; a
   successful sibling leg could mask a failed one). Fixed: explicit
   observable error-lifecycle contract (a)–(d) in slice 1 + E2E
   failure-then-success case.
Non-blocking comments applied: diagnosis call-site wording; Plex GET/POST
and JF `RegenerateTrickplay`/`Recursive` version notes; `.agents/state.md`
now points at this draft while the loop is active.

**r2 — 2026-07-12 — codex-cli 0.144.1, verdict `reopened`, 5 findings,
all ADMITTED.** Base `5aa560c`, head `77dc722`. Three are applications of
the repo's vacuous-guard rule to the plan's own test specs; two are spec
gaps.
1. HIGH — the JF hostile-key test targeted the pre-existing generic
   `build_url`, so an interpolating `scan_library` would still pass.
   Fixed: production `scan_url` helper is the tested surface; explicit
   revert-to-interpolation guard proof; same must-use rule stated for
   Plex `scan_path`.
2. MEDIUM — browse reconciliation ignored real browse variants (search
   retains `activeType`; person/drill views; a merged type whose last
   library vanished). Fixed: explicit content-leg precedence (Home /
   section grid root / type grid root / no-content-leg for search,
   person, drill) + one forced-Home disappearance fallback covering both
   missing section and missing type.
3. MEDIUM — error contract wasn't connected to the helpers that clear
   `error` today (`resetAndLoad`/`goHome`), and clause (d) had no guard.
   Fixed: action-local legs, aggregate-and-publish-once, settle-gated E2E
   assertions, new navigation-wins E2E case with delayed failing Views.
4. MEDIUM — the visible-grid E2E assumed a mutable mock movie list that
   doesn't exist (closure-held snapshots, `userData` preinit). Fixed:
   specified `state.addMovie(movie)` maintaining `byId`/`userData`
   coherently.
5. MEDIUM — the deleted-library E2E was vacuous (default mock rails are
   empty, so `goHome`'s hubs-empty conditional alone would pass it).
   Fixed: seed non-empty Latest, count hub requests, require a new fetch
   after Refresh.
Non-blocking comments applied: single forced-Home fetch path (no
goHome+loadHome double-fetch — folded into the disappearance fallback);
scan per-attempt banner/notice exclusivity + scanlib retry case.

**r3 — 2026-07-12 — codex-cli 0.144.1, verdict `reopened`, 5 findings +
1 LOW, all ADMITTED.** Base `5aa560c`, head `c4d6d50`.
1. HIGH — JF user-view ids are unreliable scan targets: synthetic grouped
   views accept the POST and scan nothing (false "Scan started").
   Fixed: scan resolves the physical library via `/Library/VirtualFolders`
   `ItemId` (jellyfin-web parity, same admin privilege); no match →
   explicit grouped-view refusal (new non-goal records the boundary);
   scanlib E2E gained the grouped-view case.
2. MEDIUM — r2 left an internal contradiction (`post_empty(segments,
   query)` vs a prebuilt `scan_url`; `build_url` lives on the client, not
   the source). Fixed: one exact production chain —
   `client.scan_url(item_id)?` → `client.post_empty_url(&url)` — with
   ownership stated per step.
3. MEDIUM — the disappearance fallback triggered on residual
   `active`/`activeType` (Home retains `active`, search retains
   `activeType`), forcing Home from views the plan promised to leave
   alone. Fixed: snapshot captures an explicit visible-root KIND; the
   fallback applies only to `section-grid`/`type-grid` roots.
4. MEDIUM — generation gating didn't cover all navigation (in-source nav
   bumps only `loadGen`; detail open/close bumps nothing). Fixed: new
   `navEpoch` bumped by every navigation gates the content leg, the
   fallback, and error publication; E2E gained the delayed-success
   navigate-to-B case.
5. MEDIUM — the visible-grid E2E couldn't distinguish replace-from-zero
   from append (mock ignored `StartIndex`). Fixed: mock honors
   pagination; case asserts `StartIndex=0`, exact card set, and that a
   removed movie's card disappears.
6. LOW — `url::PathSegmentsMut::extend` silently drops exact `.`/`..`
   segments, falsifying the one-encoded-segment invariant for `".."`.
   Fixed: `scan_url` is fallible and rejects empty/`.`/`..` ids; unit
   cases added.
Non-blocking comment applied: playtest wording now allows the existing
empty-Home first-section redirect.

**r4 — 2026-07-12 — codex-cli 0.144.1, verdict `reopened`, 3 findings,
all ADMITTED.** Base `5aa560c`, head `2178344`.
1. MEDIUM — the elevation-gated `/Library/VirtualFolders` resolution GET
   meant every real non-admin died there with a raw 403, making the
   step-(iii) friendly message unreachable. Fixed: scan-scoped
   `get_virtual_folders` maps FORBIDDEN to the same friendly message;
   scanlib gained the non-admin-at-resolution case (asserts no POST
   follows).
2. MEDIUM — the delayed-success E2E was unimplementable (the mock serves
   only lib1's `ParentId`, so library B's grid would 400) and no case
   exercised `navEpoch` gating of the SUCCESSFUL content leg. Fixed:
   per-view mock collections; the case split into content-leg (A
   present) and fallback (A removed) phases with B's-cards-unreplaced
   assertions.
3. MEDIUM — the root-kind/navEpoch fixes weren't guard-proven for detail
   navigation (both planned cases used goHome/select, which bump existing
   gens; detail open/close bumps none). Fixed: two detail-focused cases —
   refresh with detail open over a removed section (root classification),
   and detail opened mid-delayed-refresh (epoch bump on detail open).
Non-blocking comments applied: case 1 now also asserts the Home rail
re-fetch (Home content leg); merged type-grid leg recorded as an accepted
E2E gap (shares the section-grid implementation; owner playtest covers
it); Emby `/Library/VirtualFolders/Query` variant recorded as a version
note with visible-failure fallback semantics.

**r5 — 2026-07-12 — codex-cli 0.144.1, verdict `reopened`, 7 findings,
all ADMITTED** (count rose because the r4 amendments created new
surface — including one regression the coder introduced). Base
`5aa560c`, head `fb96562`.
1. MEDIUM — merged-scope disappearance was inferred from a PARTIAL
   aggregate (`get_sections` skips failing sources by design), so a
   transient one-server failure would force Home. Fixed: disappearance
   fallback now requires the single-source completeness precondition;
   merged scope gets NO fallback (recorded non-goal / accepted edge).
2. MEDIUM — r4 dropped the POSITIVE deleted-library case, leaving only
   must-not-run guards (a no-fallback implementation would pass). Fixed:
   restored as case 9 with the hub-request-count assertion.
3. MEDIUM — preserving detail over a removed library orphaned the hidden
   parent grid (Back revealed a dead view). Fixed: hidden-parent
   reconciliation to Home at settlement; case 10 asserts through Back.
4. MEDIUM — detail CLOSE was outside the navEpoch guard proof. Fixed:
   case 7 (close mid-refresh, no banner over the revealed grid).
5. MEDIUM — the one-session case order destroyed state later cases
   needed. Fixed: explicit order (destructive last) + restore-and-settle
   steps between destructive phases.
6. MEDIUM — the failure-then-success case ran on a grid root, where the
   content leg depends on sections, so contract (b) (sibling success
   must not mask failure) was never exercised. Fixed: moved to Home
   (case 2), asserting the Home leg succeeded while the banner reports
   the sections failure.
7. MEDIUM — the Emby resolver mandated Jellyfin's bare route against
   Emby's own documented `/Library/VirtualFolders/Query` + `Items`
   envelope. Fixed: flavor-branched resolver with per-envelope serde
   unit tests; Emby branch remains live-unverified but docs-aligned,
   failures visible.

**r6 — 2026-07-12 — codex-cli 0.144.1, verdict `reopened`, 6 findings,
all ADMITTED; codex confirmed the r5 fixes sound.** Base `5aa560c`,
head `4b9686f`. LOOP PAUSED after this round on the owner's
instruction (session restart); resume = fix-verify round r7.
1. MEDIUM — a pure `navEpoch` gate on the disappearance fallback
   SUPPRESSES cleanup when the mid-flight "navigation" was a detail
   open/close over the same dead section (sidebar drops A, A's orphan
   grid remains). Fixed: the fallback's gate is now CURRENT ROOT
   IDENTITY at settlement (visible or hidden under detail), not the
   epoch; cases 11-12 guard the two transition races.
2. MEDIUM — the empty-Home `select(sections[0])` redirect would CLOSE a
   preserved detail and open another library when the reconciled Home is
   empty. Fixed: redirect deferred while `detailView` is open; case 13.
3. MEDIUM — the Home refresh leg claimed no `homeGen`, so an older
   in-flight Home load could overwrite refreshed rails or publish a
   stale failure. Fixed: the leg claims a generation like every Home
   load; case 14 with one-shot delay/fail knobs.
4. MEDIUM — concurrent scans could publish out-of-order status. Fixed:
   latest-attempt `scanGen` ownership of notice/banner/timer (+ menu
   disable as feedback); scanlib case 5.
5. MEDIUM — the Emby route branch wasn't guard-proven (envelope parse
   tests pass while Emby rides Jellyfin's route). Fixed: production
   `vf_route(flavor)` selector with branch-flip guard proof.
6. MEDIUM — merged type-grid reload and no-fallback had NO verification
   step anywhere (claimed playtest didn't list them). Fixed: new
   two-mock `mergedrefresh.mjs` (reload + partial-aggregate-no-Home).
Non-blocking comments applied: aggregate error-semantics wording
corrected (errors when the combined result is empty AND a source
failed); case 6 opens detail without `openDetailAndPlay`'s play half;
scanlib's grouped view seeded before launch (or refreshed into the
sidebar) before right-clicking it.

**r7 — 2026-07-12 — codex-cli 0.144.1 (gpt-5.6-sol), verdict `reopened`,
5 findings, all MEDIUM, all ADMITTED.** Base `5aa560c`, head `0c852c9`.
1. MEDIUM — the overview and case 8 still called the disappearance
   fallback `navEpoch`-gated, contradicting r6's root-identity contract
   (an epoch gate also suppresses the same-root detail cleanup of cases
   11-12). Fixed: overview excludes the fallback from the epoch rule;
   case 8 rewritten as a root-identity-mismatch test.
2. MEDIUM — action-local leg errors weren't generation-owned: a newer
   same-root load (bumps `homeGen`, not `navEpoch`) couldn't stop a
   superseded refresh leg's failure from publishing. Fixed: aggregate
   clause (d) now also requires current leg generations; case 14 gained
   the reverse-ordering phase.
3. MEDIUM — `scanGen` gated publishing/arming but not an already-armed
   timer: A's leftover auto-clear could wipe B's fresh notice. Fixed:
   new attempts cancel armed timers, callbacks are generation-gated and
   cleared on component destruction; scanlib case 5 gained a
   success-then-success timing phase.
4. MEDIUM — case 13 was vacuous: the empty-Home redirect requires
   `activeSource !== null` (`+page.svelte:331`), never eligible in a
   single-mock run. Fixed: two-source variant with explicit selection
   and an eligibility guard-proof before the deferral assertion.
5. MEDIUM — mergedrefresh case 2 couldn't detect its target regression:
   both mocks default to Movies, so the active type never disappears.
   Fixed: mock 1 is non-Movies-only, mock 2 the sole Movies provider;
   assert tabs lose Movies, no forced Home, recovery reload.
Non-blocking comments applied: aggregate-semantics citation corrected to
`commands.rs:2447-2490`; external JF/Emby/Plex route claims validated by
the reviewer against upstream docs; the `scenarios.js` path in the review
prompt was the operator's error — the loader is `tests/e2e/run.mjs` +
`tests/e2e/scenarios/`.

## Code review log
The IMPLEMENTATION diff's review loop (playbook `reviewloop.md`, batch
adaptation: one round over a pinned commit range, no per-finding branches).
Reviewer codex, headless one-shot, read-only sandbox, mac host. Distinct
from the plan-review loop above, which reviewed the PLAN.

**r1 — 2026-07-12 — verdict `reopened`, 4 findings, all ADMITTED and
fixed (`e9edac8`).** Trail was in the commit message only until r2; recorded
here retroactively.
1. The refresh Home leg orphaned the `loading` flag it stole by claiming
   `homeGen` — a plain Home load pending at click time could no longer clear
   it (its finally is generation-gated), stranding the skeleton and blocking
   the `!loading`-gated empty-Home redirect.
2. `mockjf` bound its Latest fail/delay one-shots at RESPOND time, so a
   concurrent request could steal a flag armed for another.
3. `scanlib` did not wait for lib1's parked POST before scanning lib2.
4. Escape closed the detail before the context menus, and the two menus were
   not mutually exclusive.

**r2 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 8 findings, all
MEDIUM.** Base `63560a6` (plan APPROVED), head `ca84f5b` (both slices + the
r1 fixes + the two E2E commits). `guard_confirmed:false` — read-only, and
the E2E suite is Linux-only so the reviewer could not run it. Fresh eyes
over the whole implementation, r1's fixes included. Triage: **7 ADMITTED,
1 SPLIT (half admitted, half DECLINED)**. Exactly one finding is a
behavior defect; the other seven are the repo's vacuous-guard rule applied
to this plan's own required tests — the app is correct, but nothing would
catch it ceasing to be.

- **lrs-1 (ADMITTED) — the empty-Home redirect silently swallows a refresh
  failure.** When a refresh's sections leg fails AND its Home leg returns
  empty, `hubs` becomes `[]`, the empty-Home `$effect`
  (`+page.svelte:329`) fires `select(sections[0])`, and `select`'s
  `navEpoch++` makes the action's publish gate (`+page.svelte:599`) read
  "the user navigated" — so the sections failure is suppressed and the user
  lands in a grid built from the STALE section list with no error. The
  plan's navigation-wins rule (contract (d)) was written for USER
  navigation; an APP-initiated redirect satisfies it by accident. Contract
  (b) — a failed leg must surface even when a sibling succeeded — loses.
- **lrs-2 (ADMITTED) — refresh case 5 cannot detect its target.**
  `loadMore` reads LIVE `active`/`activeType` (`+page.svelte:727`), so an
  ungated content leg landing after the user opened library B would simply
  clear and reload B with B's own cards; every assertion still passes.
- **lrs-3 (ADMITTED) — refresh case 14 is missing the plan's required
  reverse-ordering phase** (slow FAILING refresh leg, then a newer
  successful same-root Home reload). The leg-failure generation ownership
  added by plan-review r7 finding 2 therefore has no guard at all.
- **lrs-4 (ADMITTED) — the r1 `loading` fix has no guard.** Case 14 asserts
  rails and the refresh control; neither observes `loading` stuck true.
  Delete the Home leg's `finally` (`+page.svelte:562`) and the suite stays
  green while a later empty-Home refresh hangs on the skeleton forever and
  never redirects.
- **lrs-5 (ADMITTED) — the scan out-of-order case only covers a stale
  SUCCESS.** The stale-FAILURE gate (`+page.svelte:1119`) is unguarded, and
  the mock cannot even express the case: `failNextItemRefresh` is consumed
  at RESPOND time (`mockjf.mjs`), so a fast scan B steals the failure armed
  for a delayed scan A — the same binding bug r1 fixed for the Latest flags.
- **lrs-6 (ADMITTED) — `scan_url` is never tested with a hostile id.**
  `scan_url_shape_and_rejections` (`jellyfin.rs:1156`) covers only `""`,
  `"."`, `".."`. The plan REQUIRED slash/backslash/query/fragment ids
  (`../System/Shutdown?x=`). Replace segment-safe construction with raw
  interpolation, keep the dot precheck, and every assertion stays green —
  on the one request that carries admin-capable credentials.
- **lrs-7 (ADMITTED) — the `scan_query` assertion is tautological.** The
  unit test derives its expected pairs from `scan_query()` itself
  (`jellyfin.rs:1164`) and the E2E checks only two of the six params, so
  flipping `ReplaceAllMetadata` to `true` — turning a cheap scan into a
  destructive metadata rewrite on the owner's real server — stays green.
- **lrs-8 (SPLIT).**
  - *ADMITTED half:* refresh case 13's deferral assertion is vacuous. The
    fallback's Home re-fetch is fire-and-forget (`forceHomeForRemovedRoot`,
    `+page.svelte:481`), so `settle()` (which waits only on the refresh
    control) returns before it lands; the scenario can assert "detail still
    open" and press Back while `loading` is still true — passing even with
    the detail-deferral guard removed.
  - *DECLINED half:* the reviewer also called `loadHome`'s direct error
    publish (`+page.svelte:405`) a defect of this work ("a late error can
    appear over a subsequently opened detail"). That is pre-existing
    `goHome`→`loadHome` semantics, unchanged by this diff and reachable
    without any refresh; the fallback's re-fetch is a normal Home load and
    the plan specifies it as such ("goHome's state reset plus one
    unconditional Home re-fetch"). Declined as out of scope for this loop,
    not as wrong — if the owner wants Home-load errors epoch-gated, that is
    its own plan.

Fixes land one finding per commit, each guard-proven red→green on the Linux
VM, then r3 re-reviews.

**r3 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 4 MEDIUM + 1 LOW,
ALL ADMITTED.** Base `63560a6`, head `b160ef1`. `guard_confirmed:false`
(read-only; Linux-only suite). Fresh eyes over the whole implementation,
r1/r2 fixes included. THREE are behavior defects — r2's vacuity sweep had
hardened the guards, and with the tests no longer blind, r3 found what they
were failing to see.

- **r3-1 (`afca8c5`) — the empty-Home redirect fired MID-ACTION.** The Home
  leg deliberately never raises `loading` (a refresh must not blank the UI),
  so a sections leg landing first with a newly added library satisfied the
  effect (`sections > 0`, `hubs` still empty, `!loading`): the user was
  thrown into the new library while the Home leg was in flight, and the
  redirect's `resetAndLoad` bumped `homeGen`, so the arriving rails were
  discarded as stale — and a Home leg that FAILED was dropped silently for
  the same reason. Fixed: the effect is gated on `!refreshing` and
  re-evaluates when the action settles. Guard: refresh case 17. The
  instrumented VM run reproduced the defect exactly (`hg=45` vs
  `homeGen=46`, epoch unchanged, `hLen=1` fetched and thrown away).
- **r3-2 (`538ce78`) — a dying listing bannered over fresh cards.** The
  content leg claimed `loadGen` only after the sections response, so an
  ordinary listing load that failed during a slow sections fetch published
  its own error (loadMore's direct-publish path) AFTER the action had
  cleared the surface — and the action's successful reload contributed no
  failure to clear it. Fixed: the action claims the generation at the CLICK
  (`gridGen`). Guard: refresh case 16 + mock one-shots `failNextItems` /
  `itemsDelayMs` (bound at ARRIVAL).
- **r3-3 (`58dfa0b`) — a Plex scan could hit the WRONG SERVER.** Section
  keys are server-local numeric ids; `rediscover()` returns the first
  REACHABLE server on the account. A scan that failed on server A could be
  retried against server B's section with the same number — an unrelated
  library — and report success for the one the user clicked. Fixed: the
  retry goes through `may_retry_scan_on` and proceeds only on a provably
  identical machine id. Guard:
  `scan_retry_never_crosses_to_another_server`. **Recorded gap:** the READ
  paths share the same blind rediscover-and-retry shape (pre-existing, out
  of scope; the scan was fixed first because it ACTS on the server).
- **r3-4 (`948762b`) — per-attempt exclusivity was proven one way only.**
  Only failure→success was covered, so deleting `scanNotice = null` at
  attempt start left a failing scan's banner sitting NEXT TO the previous
  attempt's "Scan started".
- **r3-5 (`91fa2b5`, LOW) — nothing proved a notice ever EXPIRES.** The
  timer phases only proved an older timer cannot clear a newer notice, so a
  dead auto-clear passed while "Scan started" stuck on screen forever.

**r4 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 3 MEDIUM + 1 LOW,
ALL ADMITTED.** Base `63560a6`, head `615c94c`. TWO of the four are
REGRESSIONS THE r3 FIXES INTRODUCED — the loop catching its own tail, which
is exactly what a fresh-eyes round is for.

- **r4-1 (`c485e6d`) — the action orphaned the loading flags it stole.**
  r3-2's click-time `loadGen` claim orphans the in-flight listing, whose own
  release is generation-gated — so on every early return (sections failed,
  root gone, navigation won) NOBODY released `loading`/`loadingMore`. A
  refresh whose sections fetch failed left the grid stuck on its skeleton,
  unable to paginate, until the user navigated away. Fixed: the leg owns the
  flags and releases them in a `finally`. Guard: refresh case 18.
- **r4-2 (`f282a74`) — a scroll could load on the ACTION's generation.**
  `onScroll` calls `loadMore()` with the DEFAULT generation, which after
  r3-2 is the action's own: a scroll during a slow sections fetch started a
  second load on that generation, appending a page at the pre-reset offset
  (corrupt order/offset) or publishing its failure over the action's result.
  Fixed: the action takes `loadingMore` at the click. Guard: refresh case 19
  (70 movies in library B so PAGE=60 actually paginates, then a real scroll).
- **r4-3 (`4504a8b`) — the r3-3 fix was INCOMPLETE.** `rediscover()` installs
  AND PERSISTS its chosen server before returning, so refusing a cross-machine
  retry afterwards was too late: the source was already repointed at server B,
  and the NEXT scan would succeed on its first attempt against B's
  same-numbered unrelated library. Fixed: discovery is filtered BEFORE the
  choice — `rediscover_on(machine)` via `same_machine_candidates()`. Guard:
  `scan_rediscover_only_considers_the_same_machine`.
- **r4-4 (`ba4f4a1`, LOW) — the notice-expiry deadline had too much slack.**
  Measured from the wrong attempt's t0 plus a 9s window, so a timer stretched
  to 8-10s passed. Fixed: measured from when the owning notice is armed, 6s
  budget (4s promise + jitter).

**r5 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 4 MEDIUM + 1 LOW,
ALL ADMITTED.** Base `63560a6`, head `befbd86`. The round that found the
DESIGN error, not just its symptoms.

- **r5-2 (`7da85e6`) — r3-2's design was WRONG, and r4-1/r4-2 were props under
  it.** Claiming `loadGen` at the click invalidated the in-flight listing; when
  the action then returned early (sections failed), that listing's result was
  discarded with NOTHING to replace it — a healthy library rendered EMPTY
  ("Nothing in this view yet"), unable to paginate, until the user navigated
  away. r4's case 18 passed straight through it because it asserted only that
  the skeleton disappeared: a released flag is not a usable grid. Fixed by
  reverting the design: the listing is left alone, the leg claims its
  generation only when it actually resets, and the narrow problem the early
  claim was for (an orphaned load publishing its banner over the action's
  result) is solved narrowly with `gridActionActive` suppression. Case 18 now
  requires CARDS; case 19 asserts no stale page survives the reset.
- **r5-1 (`a232e1d`) — a slow refresh stranded a source the user switched to.**
  `refreshing` was a GLOBAL gate on the empty-Home redirect, so an action still
  running against source A blocked source B's auto-open for A's whole timeout.
  Fixed: the gate is scoped to the action's root (`refreshEpoch === navEpoch`).
  **`navEpoch` also had to become `$state`** — the gate short-circuits on it, so
  as a plain `let` the effect registered NO dependency on navigation and never
  re-ran on a source switch; the scoped gate was inert until that changed. Both
  halves are independently red-proven (case 20).
- **r5-3 + r5-4 (`136ed21`) — r4-3 was pinned in the wrong place, and its guard
  was vacuous.** Only the RETRY was pinned, so a scan's FIRST attempt still went
  wherever an unrelated read's rediscover had repointed the source; and
  reverting the production call to the unfiltered rediscover left both unit
  tests green. Fixed at the root: `rediscover()` itself derives its pin from the
  installed server, so a source can never silently swap machines under the
  server-local ids it has handed out. This also closes the read-path class r4-3
  recorded as a known gap. Remaining gap (recorded): the async call site has no
  unit coverage — discovery/reachability are network calls with no fake in this
  repo — so the footgun was removed structurally instead.
- **r5-5 (`53b862a`, LOW) — the expiry clock still started ~2.8s late**, so a
  4s→7-8s timer regression still passed. Now measured from when the owning
  notice is armed.

**r6 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 4 MEDIUM, ALL
ADMITTED.** Base `63560a6`, head `4fd181a`. Two are regressions from the r5
fixes; one of those would have broken Plex outright.

- **r6-3 (`7f6919f`) — the r5-3 pin BROKE Plex recovery.** The startup path
  restores a saved host/port via `set_server_manual`, which stores NO machine
  identifier (empty string). Pinning rediscovery on "" matched nothing, filtered
  every candidate away, and left browsing and scanning dead after a saved
  address went stale — until the user relinked. Fixed: `rediscovery_pin()`
  treats an empty id as UNKNOWN → discover freely, as before r5-3.
- **r6-4 (`7f6919f`) — unpinned rediscoveries could clobber each other.** Two
  first-connect calls can race, choose DIFFERENT machines, and both install; the
  loser repoints the source under section keys the winner already handed out.
  Fixed: `should_install()` — an unpinned call installs only while nothing is
  installed.
- **r6-1 (`6fc8c4f`) — a regression from r5-2.** The grid-action banner
  suppression was GLOBAL, so after the user navigated away the still-running
  action went on swallowing the NEW view's errors while its own outcome was
  discarded on the epoch mismatch: library B rendered empty and silent. Fixed:
  the suppression is scoped to the action's root, like the redirect gate. Guard:
  case 21.
- **r6-2 (`44a5b44`) — a Settings source change reset the view without bumping
  `navEpoch`**, so an in-flight refresh kept owning the epoch and went on
  blocking the redirect over a view it no longer related to. Fixed: bump it.
  **Recorded gap:** no automated guard — driving Settings add/remove mid-refresh
  needs a real source-add flow the mock harness does not have; the mechanism
  itself is guard-proven by case 20.

**r7 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 3 MEDIUM, ALL
ADMITTED.** Base `63560a6`, head `43a412e`. Narrower than r6 — the loop is
converging.

- **r7-1 (`163b958`) — an endpoint of UNKNOWN identity could still drift.** r6-3
  correctly stopped pinning on a restored endpoint's empty machine id, but that
  left such a source free to be repointed: a failed scan rediscovered UNPINNED,
  could install account server B, and although the retry was refused, the NEXT
  scan's first attempt would send A's still-visible section key to B. Fixed at
  the root: the source LEARNS its machine (one `/identity` call at first contact,
  lock never held across it), so rediscovery is pinned from then on; and while
  identity is unknown a scan does not rediscover at all.
- **r7-2 (`e04a880`) — Plex linking is navigation.** The link screen replaces the
  root and its completion calls `loadEverything()`, neither bumping `navEpoch`,
  so an obsolete refresh kept owning the epoch. Both points now bump it.
  **Recorded gap:** no automated guard (linking needs plex.tv; the harness is
  hermetic). Same class as r6-2's Settings gap.
- **r7-3 (`0220ffc`) — the mock accepted UNAUTHENTICATED scans.** The scan routes
  are the only writes Vela makes and carry admin-capable credentials, yet the
  mock never checked the Authorization header: dropping auth from either
  production request left every scan guard green while a real server answers 401
  and Scan Library is unusable. The mock now demands auth on both routes.

**r8 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 4 MEDIUM. 3
ADMITTED, 1 DECLINED.**  Base `63560a6`, head `b2b19db`.

- **r8-1 (`3bb70ce`) — a FAILED identity probe still left the source unpinned.**
  r7-1 learns the machine at first contact, but the probe can fail and
  ensure_ready returned the endpoint anyway; a later rediscover could then
  install another account server, and the scan's FIRST attempt would hit it with
  this server's section key. Fixed: if Vela cannot say which server a library
  belongs to, it refuses to scan and says so.
- **r8-2 (`c4dcb08`) — the PIN screen transition is the navigation.** `beginLink`
  bumped `navEpoch` before awaiting `link_begin`, but Settings closes at once, so
  a Refresh started during that await still owned the epoch and could render an
  obsolete library error over the link screen. The bump now also happens when the
  PIN is assigned. (Guard gap unchanged: linking needs plex.tv.)
- **r8-3 (`baaac10`) — the suppression silenced NEWER same-root loads too.** A
  watch-state edit runs `refreshWatchState` → `resetAndLoad`, claiming a higher
  generation; that load WINS (the action's leg is dropped as stale), so swallowing
  its failure left an empty grid with no banner. Fixed: the action records the
  generation current at the click (`gridActionBaseGen`) and may silence only
  loads at or below it — exactly the ones it orphaned. Guard: case 22.
- **r8-4 — DECLINED (recorded, not silently dropped).** *Claim:* opening a detail
  while page 2 of a library is loading, when that page then FAILS, leaves
  `hasMore = false` while the error is (correctly) suppressed because the user
  navigated away; on Back the library looks silently truncated until re-entered.
  *Reason for declining:* the consequence is recoverable (re-entering the library
  runs `resetAndLoad` and restores full pagination), no wrong data and no wrong
  action result, and `hasMore = false` on a failed page is PRE-EXISTING
  pagination-failure semantics this work did not introduce. Fixing it properly
  means reworking those semantics — out of scope for this plan. If the owner
  wants failed pages to remain retryable, that is its own plan.

**r9 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 2 MEDIUM, both
ADMITTED.** Base `63560a6`, head `b40728f`. Narrowing again (4 → 2).

- **r9-1 (`a0ef142`) — a same-root RE-RUN was read as navigation.**
  `refreshWatchState()` re-enters the current root to pick up new watch state,
  but on a search/person root it went through helpers that bumped `navEpoch`
  unconditionally — so an in-flight Refresh treated it as the user navigating
  away and silently dropped its own failure: spinner stops, sidebar stale, no
  error, though the user never moved. Fixed: `runSearch`/`runPersonView` take a
  `rerun` flag. Guard: case 23.
- **r9-2 (`e9eeba1`) — a scan must reach the server its KEY came from.** r8-1
  proved only that the current server could be NAMED. If a restored server's
  /identity probe failed while its sections loaded, a later read failure could
  install account server B, and A's still-visible key would pass the check and
  scan B's same-numbered library while Vela reported success. Fixed: the source
  records which machine served the section list (`sections_machine`), and a scan
  fires only when that machine and the current one are both known and IDENTICAL
  (`scan_target_ok`).

Operator note (process, not code): two guard proofs in this round initially
came back GREEN against an injected regression — both times the harness was
at fault, not the guard. A proof script ended with `git checkout --
src/routes/+page.svelte`, which silently reverted the UNCOMMITTED fixes
under test; and one injection removed the late `loadGen` claim while leaving
the new click-time claim in place, so the pre-fix behavior was never
restored. Rules now: land the fix BEFORE injecting, and restore from a file
backup, never `git checkout`.

**r10 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 2 MEDIUM. 1
ADMITTED, 1 DECLINED.** Base `63560a6`, head `7e81619`.

- **r10-2 (`8e474a7`) — the section list's server was recorded before the list
  existed.** `sections()` stamped `sections_machine` from the PRE-REQUEST client.
  An attempt that failed — or was merely still in flight — therefore relabelled
  the keys the user was still looking at: they came from A, the source had since
  drifted to account server B (an unpinned rediscovery after a failed read), and
  `scan_target_ok(B, B)` then passed, rescanning B's same-numbered library while
  reporting success for the one the user clicked. The same bug refused legitimate
  scans in the other direction (an A-fail → B-retry-success stamped neither
  server that could have served the list). Fixed: the stamp comes from the client
  that ACTUALLY SERVED the returned list, written only once that list is in hand;
  a failed attempt leaves it alone. Guard:
  `a_scan_never_reaches_a_server_that_did_not_serve_the_key` — the FIRST
  end-to-end guard over the scan-safety family (two mock Plex servers on
  loopback, no network and no new deps; A serves the list, the source drifts to
  B, B's refresh is parked mid-flight, and the scan must be refused). Proven red:
  with the stamp back in its pre-fetch position the scan SUCCEEDS against B.
  Rationale for the harness: every previous finding in this family (r3-3, r4-3,
  r6, r7-1, r8-1, r9-2) could only ever be guarded by a pure decision helper —
  which is precisely why the ORDERING of the call into those helpers kept
  regressing undetected. Extend this mock rather than adding another pure test
  when the next scan-safety finding lands.
- **r10-1 — DECLINED (recorded, not silently dropped).** *Claim:*
  `refreshWatchState()` passes `rerun: true` off residual `searchTerm`/
  `personView`, but `searchTerm` survives child drills and the re-entry helpers
  clear an open detail — so on a drilled/detail root the re-run changes what is
  on screen WITHOUT bumping `navEpoch`, letting a failing in-flight Refresh
  publish its banner over the "new" root instead of navigation winning. *Reason
  for declining:* on every root the claim covers, `visibleRootKind()` returns
  `detail`/`search`/`person`, which take NO content leg and never set
  `gridActionActive` — the action's only publishable failure there is the
  sidebar `get_sections` fetch, a library-LIST failure that is equally true
  whichever browse root is on screen, and the disappearance fallback cannot fire
  (`currentSectionRootKey()` returns null whenever `searchTerm`/`personView` is
  set). So the predicted outcome is a TRUTHFUL banner reporting the failure of a
  Refresh the user themselves clicked, shown over the same query's results. No
  wrong data, no wrong action result, nothing stranded or swallowed. The
  re-entry is APP-initiated, and the navigation-wins contract is explicitly about
  USER navigation (see the `select({auto})` comment, lrs-1); treating an
  app-initiated detail-close as navigation would resurrect exactly the silent
  failure-swallowing that r9-1 fixed. Note also that closing a detail / popping a
  drill on a watch-state edit is PRE-EXISTING behavior (base `63560a6` called
  `runSearch(searchTerm)` unconditionally); this work only removed the epoch bump.

**r11 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 2 MEDIUM, both
ADMITTED.** Base `63560a6`, head `5c9dce6`.

This round overturned the DESIGN of the r10-2 fix, which is why the loop was
still worth running after two straight rounds of one admitted finding.

- **r11-1 (`4af195d`) — a section key's origin must travel WITH the key.** r10-2
  had the source record which server served the section list. r11 showed no such
  record can be right: the key a caller holds need not come from the list
  currently on screen. A right-click menu opened on server A's library outlives
  the refresh that replaces the sidebar with B's; a failed refresh leaves A's
  listing up after the source has moved on. In both cases the record truthfully
  says "B served the last list I returned", so a scan of A's still-visible
  "Movies" passed `scan_target_ok(B, B)` and rescanned B's same-numbered library,
  reporting success for the one the user clicked. A Plex section key is only a
  number — B has a section 2 of its own — so the source cannot tell the two apart
  from its own state, at any point in time. Fixed: `SectionDto::provenance`
  records the issuing server, the frontend hands it back unchanged with the scan
  (`scan_section`), and Plex refuses any key it cannot prove came from the server
  it is talking to now. `sections_machine` and its ordering hazards are DELETED.
  A source that could not name its server when it served the list issues `None`,
  which fails closed exactly as r8-1 intended. Jellyfin/Emby ignore provenance:
  one fixed server address for the source's life, and library ids are
  server-issued GUIDs, not small server-local numbers. Guard:
  `a_stale_key_never_scans_the_server_that_replaced_it` (extends the r10-2 mock
  harness; both mocks serve a section "2", and the assertion is that B never
  RECEIVES the request, not merely that the call returned an error).
- **r11-2 (`8916388`) — a refresh must retract the banner of a load it
  superseded.** A watch-state edit mid-refresh claims a newer listing generation;
  if that load fails it banners (correctly — r8-3). But when the refresh's
  sections leg lands, its content leg claims a generation HIGHER still, replaces
  the cards and succeeds. Nothing of the action's own failed, so settlement
  published nothing and never took the stale message down: fresh cards under a
  "couldn't load" banner. Fixed: `loadMore` tags a banner with the generation
  that published it, and settlement retracts exactly one the action superseded
  (`errorGen <= claimedGen`) — a NEWER load supersedes the ACTION in turn, and
  its failure is the one the user needs. Guard: refresh case 24.

Guard-quality note (the reusable lesson of this round): case 22 already ran
r11-2's exact interleaving and passed, because it asserts only the INTERMEDIATE
state (the banner appears) and never the settled one. A guard that stops
watching before the action settles will sit and watch the bug happen. When a
case's subject is an action with a settlement step, assert the SETTLED state —
what the user is finally left looking at — not just the transient it passes
through.

**r12 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 2 MEDIUM, both
ADMITTED** (r12-1 only after an independent adjudication OVERTURNED my decline —
see below). Base `63560a6`, head `4f7ac59`.

- **r12-2 (`17d13f0`) — text equality is not evidence of banner ownership.**
  r11-2's retraction was scoped by generation AND by text: it remembered what the
  superseded load wrote and cleared only while the banner still said exactly
  that. Two different failures can say exactly that — a 401 on a LISTING and a
  401 on a SCAN both surface as `RECONNECT_REQUIRED`, which `friendlyError`
  renders as one constant sentence. So a scan failure could wear the superseded
  load's text and be retracted by a refresh that never superseded it: the scan
  failed and the user was left with no status at all. Fixed: every write to
  `error` goes through `setError()`, which clears the tag unless the write IS the
  tagged listing publish. The tag can no longer outlive the message it describes,
  and no future banner write has to know the tag exists to stay correct —
  ASSIGNING `error` DIRECTLY IS NOW A BUG. Guard: refresh case 25, plus mock
  one-shots `unauthNextItems` / `unauthNextItemRefresh` (401 is the one failure a
  listing and a scan report with identical text, which is what makes the case
  writable at all).
- **r12-1 (`b56dca7`) — a rebound source's keys are not the keys it issued
  before.** A browse root was identified by its section key alone, but a Plex
  section key is a server-local number: a source whose `/identity` probe never
  answered leaves rediscovery unpinned, so a later read failure can REBIND it to
  another account server — whose list also has a section 2. The disappearance
  check saw the key still present, kept the user rooted on it, and the grid
  filled with the NEW server's cards under the OLD library's title. Fixed: the
  source carries a `binding`, bumped only on an install it cannot prove is the
  same server (`rebind_voids_keys`), and stamps it on every key it issues; a
  section is the same library only if key AND binding match (`sameSection`).
  Provenance cannot decide this — it is `None` exactly when the machine is
  unknown, which is exactly when a rebind is possible, so a frontend watching it
  would see `None -> Some(A)` and could not tell a REBOUND source from one whose
  probe merely RECOVERED on the same server. The backend can: recovery touches
  only `ensure_ready`, a rebind is an unpinned `rediscover()` that installs.
  Guards: `an_identity_probe_that_recovers_is_not_a_rebind` (the false positive,
  red-proven), `sections_are_stamped_with_the_binding_that_issued_them`,
  `only_an_unprovable_rebind_voids_outstanding_keys`. GUARD GAP: the frontend half
  (`sameSection`) has no end-to-end guard — the E2E mock is Jellyfin, which never
  rebinds, and the real rebind path needs plex.tv discovery, which the Rust mock
  harness cannot reach. Every INPUT to it is guarded; the comparison itself rests
  on inspection.

  **r13-1 later found a defect IN this fix** — the binding was read apart from the
  client it describes. See r13.

  **Process note — I was wrong, and the record should say so.** I first moved to
  DECLINE r12-1, arguing that the only available fix was a frontend provenance
  comparison, that it would false-positive on a benign `/identity` recovery, and
  that the ambiguity was therefore *irreducible*. The owner asked whether that was
  based on code or assumption. It was assumption, in the load-bearing place. An
  independent adjudication (grok, read-only, given both the finding and my decline
  and told to attack the decline) upheld the finding and named the error: I had
  promoted "the FRONTEND cannot disambiguate" into "NOTHING can" — but the backend
  distinguishes the two cases by construction, and a correct fix follows directly
  from that. Claims 1-4 of the decline were sound (and are now load-bearing
  comments in the fix); claim 5 was motivated reasoning that made stopping feel
  clean after eleven rounds. **Lesson for this loop: a decline that rests on "no
  correct fix exists" is a claim about the whole design space, and it must be
  adjudicated by someone other than its author.** The two earlier declines (r8-4,
  r10-1) rest on code-verified claims about reachability and consequence, not on
  design-space impossibility — but they were also self-adjudicated, and are worth
  re-examining on the same standard if they ever become load-bearing.

**r13 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 2 MEDIUM. 1 ADMITTED
and fixed, 1 OPEN as a follow-up (out of this plan's scope — owner decision).**
Base `63560a6`, head `8d4e5d7`.

- **r13-1 (`381955a`) — the binding was read apart from the client it
  describes.** `sections()` took a CLONE of the client from `ensure_ready`, then
  loaded the binding. Between the two, another task's failed read can rediscover
  and rebind the source, bumping it — so a list correctly served by the OLD
  server was stamped with the NEW server's binding, and the frontend took the old
  server's library for the new one's: it evicts the live root the user is standing
  on and offers that library as if it belonged to the server that replaced it.
  Exactly what the binding exists to prevent, reintroduced by how it was read.
  Fixed: the client and its binding are captured in ONE critical section
  (`ensure_ready_bound` / `rediscover_bound`), including across the `/identity`
  probe. Guard: `a_list_carries_the_binding_of_the_server_that_served_it` (parks
  the probe, rebinds mid-flight; red-proven).

- **r13-2 — OPEN, deferred to a follow-up plan. NOT a decline.** *Claim:* the
  binding is only checked when a new sections list arrives. Reads do not carry it:
  `get_items` / `get_children` / pagination / reselect send a bare section or
  rating key, which the source routes to whatever server it is bound to NOW. So
  between refreshes, a rebound source serves the NEW server's items under the OLD
  library's title, and the user can browse, play and curate them.
  *Assessment (code-checked, not assumed):* REAL, and NOT introduced by this work —
  `items()` has taken only a section key since before this plan (`git show
  63560a6:src-tauri/src/source/plex.rs`), so a rebind has always made stale keys
  address the new server. This plan NARROWED the hazard (r12-1 reconciles the root
  to Home on the next refresh) and did not widen it. Closing it completely means
  the binding must travel with EVERY key the source issues — `ItemDto` too, plus
  the read commands and the frontend paths that hold them — or a rebind must emit
  a signal that resets the view wholesale. Either is a design change beyond this
  plan's approved scope, which is why it is recorded here rather than fixed here.
  *Reachability:* a Plex server restored from config whose `/identity` never
  answers, on a multi-server account, where a read failure rediscovers onto a
  different server. Scans are already refused throughout (r11-1).

**r14 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 2 MEDIUM + 2 LOW, all
four ADMITTED.** Base `63560a6`, head `7110890`. The round that found the loop's
own guards decaying.

- **r14-1 (`3922913`) — a list from a server we are no longer bound to was still
  served.** r13-1 made the client and its binding agree with each other; it did
  not make them agree with REALITY. While one caller talks to A (the `/identity`
  probe failing is the widest window), another task's failed read can install B.
  A's fetch then succeeds, carrying A's keys and A's binding — internally
  consistent, and a report about a server this source has stopped being. A's
  libraries sit in the sidebar while every read behind them routes to B. Fixed: a
  rebind landing during the fetch invalidates the fetch (`sections_once` returns
  `None`); `sections()` asks again, of whoever it is bound to now, bounded to two
  attempts. Guard: `a_list_from_a_server_we_no_longer_are_is_not_served` — B is
  really INSTALLED mid-probe, the production state r13's guard could not reach.
  It is red BOTH without the staleness check AND with the client/binding read
  apart, so it SUBSUMES the r13 guard, which is deleted.

  Note what r13's guard got wrong, because the shape recurs: it simulated a rebind
  by bumping the counter WITHOUT installing a server — a state production never
  reaches. It passed, and it could not have failed. A guard whose setup cannot
  occur in production tests nothing.

- **r14-2 (`4cb6b2a`) — a scan must not erase a listing's failure.** `scanSection`
  cleared the banner unconditionally, borrowing the refresh's "the action owns its
  status" convention. But a refresh RELOADS the cards its banner is about; a scan
  reloads nothing. The banner EXPLAINS the empty grid on screen, and wiping it left
  the user with an empty view and a cheerful "Scan started" accounting for none of
  it. Fixed: `setError` records WHO published the banner (`errorOwner`), and a scan
  may only take down a previous scan's.

- **r14-3 (`4cb6b2a`) — CASE 16 HAD GONE VACUOUS, and this is the lesson of the
  round.** Case 16 guards r3-2: a doomed listing must not banner over the refresh
  that superseded it. It arms a listing that dies ~600ms in and a refresh that
  settles ~1400ms in, then asserted no banner AFTER settlement. When r11-2/r12-2
  taught the refresh to RETRACT a banner it superseded, that retraction began
  tidying away the very banner case 16 was watching for — so deleting the
  suppression still passed, while the false failure sat over the grid for 800ms.
  **A fix in one round silently disarmed a guard written six rounds earlier, and
  nothing failed.** Fixed: the case now watches the WHOLE in-flight window (polls
  while the control is disabled) and asserts the doomed listing really was served
  its failure, so the window cannot prove nothing. Re-proven red.

  **Reusable rule: when a fix teaches the app to CLEAN UP a bad state, every guard
  that asserts the absence of that state after settlement is now suspect — it may
  be observing the cleanup, not the prevention. Re-prove those guards red, do not
  assume they still bite.**

- **r14-4 (`4cb6b2a`) — removing the last source could strand a banner on the
  Welcome screen.** `onSourcesChanged` bumped `navEpoch` only AFTER awaiting the
  source list, so a refresh settling during that await published against a matching
  epoch; the no-source teardown then cleared every surface except the banner,
  leaving a dead server's failure with nothing on screen to explain it and no way
  to clear it. Fixed: the bump happens before the await, and the teardown clears
  the banner.

Process note: r14-2, r14-3 and r14-4 were committed TOGETHER in `4cb6b2a`, which
violates the repo's one-item-per-commit rule for findings lists (AGENTS.md, Git
Safety). Flagged to the owner rather than rewritten (history rewrite needs an
explicit go). Not repeated.

**r15 — 2026-07-13 — codex-cli 0.144.1, verdict `reopened`, 2 MEDIUM + 4 LOW, all
six ADMITTED.** Base `63560a6`, head `ee4e4ce`. HALF of this round was the loop
auditing its own guards, and finding them wanting.

Guard defects (the tests, not the code):

- **r15-1 (`5787873`) — production's ONLY binding increment had no coverage.**
  `rebind_voids_keys` was unit-tested as a predicate, but nothing exercised the
  place that CALLS it: the real path reaches it only through plex.tv discovery.
  Delete the increment and all 91 Rust tests stayed green — while an unpinned A→B
  install kept binding 0, so the frontend accepted B's colliding section key as A's
  library and showed B's content under A's title, durably. Fixed: the install is
  extracted to `install_under_lock` (rediscover_bound's only install site, behavior
  unchanged) so it can be driven directly; the guard walks all three cases (first
  connect, pinned recovery, unpinned rebind). Red-proven.
- **r15-2 (`eeb45d0`) — the rebind guard could not tell a refetch from a
  restamp.** Both mock servers answered the SAME section body and B's request log
  was discarded, so an implementation that served A's completed list and merely
  relabelled it with B's provenance/binding would have passed. Fixed: the mocks now
  serve DIFFERENT libraries behind the SAME key (the collision is the hazard; the
  title distinguishes them), and the guard asserts B was actually ASKED.
- **r15-5 (`171b828`) — the only listing-banner-vs-scan case forced the scan to
  FAIL**, so restoring the bug still passed: the scan's own failure simply replaced
  the banner it had just erased. A SUCCESSFUL scan replaces nothing, which is
  exactly when the loss shows. Guard: refresh case 26.

Code defects:

- **r15-3 + r15-4 (`171b828`) — one root cause, fixed as one.** The view's error
  banner and the scan's status shared a slot. That coupling had already produced
  r14-2; here it produced two more (a scan's failure permanently DESTROYING a
  listing's diagnostic by overwriting it, so a later successful scan left an empty
  grid with no explanation; and a scan completing after its source was removed
  republishing over the Welcome screen). Fixed at the root: a scan's status has its
  own surface. Failure is an alert that stays until the next scan and sits ALONGSIDE
  any listing failure — both are true, and the listing's is the one that explains the
  grid. The last-source teardown invalidates in-flight scans. `errorOwner` (the r14-2
  patch) is deleted: scans no longer touch `error` at all.
- **r15-6 (`0b49c09`) — my r14-4 fix opened the window it closed.** Moving
  `onSourcesChanged`'s `navEpoch` bump BEFORE the await stopped a settling refresh
  publishing into the teardown — but Settings does not await `onChanged`, so a
  refresh STARTED during that await then owned the post-change epoch and blocked the
  empty-Home redirect until it settled or timed out. Fixed with the double bump the
  link flow already uses (r8-2): declare the intent before the await, the fact after.

**The lesson of r14 and r15 together, and the one most worth carrying out of this
plan: a guard is not a guard until it has been proven red AGAINST THE BUG IT NAMES,
and it stops being one the moment surrounding behavior changes.** Three guards here
were vacuous for three different reasons — a setup production cannot reach (r13's),
an assertion that could not distinguish the fix from the bug (r14-1's), and a
scenario that hid the loss behind a second failure (r14-2's). None of them failed.
Nothing warned. Re-prove guards when the behavior around them moves.
