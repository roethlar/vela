# Plan: library view refresh + server library-scan trigger (owner ask, 2026-07-12)

## Status
**DRAFT — in codex plan-review loop; not owner-approved; no implementation
until the owner approves.** Owner ask (2026-07-12, while testing Jellyfin):
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
     navigated to meanwhile. ALL refresh reconciliation — the content
     leg, the disappearance fallback, and the final error publication —
     is gated on `navEpoch` being unchanged from the action-start
     snapshot (the sidebar `sections` swap alone stays
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
       concurrently with sections.
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
   - **Disappearance fallback (one forced-Home path):** applies ONLY when
     the snapshot's visible-root kind is `section-grid` AND the refresh
     ran in a SINGLE-SOURCE scope (an `activeSource` is set, or exactly
     one source is configured) AND the section key is missing from the
     new list. Completeness is the precondition: a single-source
     `get_sections` either errors (→ the error contract; no
     reconciliation) or returns that source's complete list, so "missing"
     really means deleted. A MERGED-scope aggregate is partial BY DESIGN —
     `get_sections` skips failing sources and errors only when all fail
     (`commands.rs:964-996` aggregate semantics) — so absence proves
     nothing there and NO disappearance fallback runs for `type-grid`
     roots (which only exist in the merged multi-source scope,
     `+page.svelte:1224`) or any merged refresh: a transient failure of
     one server must not yank the user Home. Accepted edge (recorded in
     Non-goals): a library genuinely deleted mid-merged-view leaves the
     user on its grid until they navigate; the content-leg reload still
     runs and shows the surviving items. Never for
     `home`/`search`/`person`/`drill` roots, whatever residual
     `active`/`activeType` state they retain. Gated on `navEpoch`
     unchanged.
   - **Detail over a vanished library:** when the snapshot root is
     `detail`, the detail surface itself is never touched — but if the
     HIDDEN browse state beneath it is a single-source-scope section grid
     whose section vanished (same completeness precondition as above),
     reconcile that hidden state to Home at settlement (same snapshot
     epoch gate): the detail stays open, and closing it reveals Home
     (the detail crumb bar degrades to its existing Back-only-over-Home
     form) instead of an orphaned grid for a library that no longer
     exists. The routine is a SINGLE forced-Home: goHome's state reset
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
     current — a refresh superseded by ANY navigation (including opening
     a detail page) stays silent (navigation wins).
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
     server 404 before the POST. Each envelope gets a serde parse unit
     test; the Emby branch stays live-unverified (no Emby server
     available) but is now aligned WITH its documentation rather than
     against it, and any real-world failure still surfaces visibly.
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
   notice alone, never beside the stale failure. No auto-refresh afterward — the
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
     (the harness already drives the detail surface —
     `openDetailAndPlay`, `tests/e2e/helpers.mjs:92`) → the detail
     remains open and the delayed failure surfaces NO banner after
     settlement (detail OPEN must bump `navEpoch`; goHome/select-based
     navigation alone cannot prove this — those paths already bump the
     existing gens).
  7. *Detail closed mid-refresh:* with detail open over A → set
     `failNextViews` + `viewsDelayMs` → Refresh → close the detail (Back)
     while in flight → NO banner is published over the revealed grid
     after settlement (detail CLOSE must bump `navEpoch` too — an
     open-only implementation passes case 6 but fails this one).
  8. *Navigation wins (fallback), destructive:* browsing A → set
     `viewsDelayMs`, remove A from `state.views` → Refresh → open
     library B while in flight → the delayed response must NOT force
     Home; B's grid stays (the disappearance fallback is
     `navEpoch`-gated). RESTORE A, settling Refresh.
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
     of a library with no sidebar entry.
  RED without slice 1 (no refresh control exists to click).
  Recorded gap: the merged `type-grid` content leg is not E2E-covered
  (needs a second mock server à la `mergedview.mjs`); it shares the
  reconcile implementation with `section-grid`, and the owner playtest is
  the behavioral check — same accepted class as the item-detail flows.
  The merged scope's NO-fallback rule is likewise playtest-covered.
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
  2. *Grouped view refused:* add a second view (`Id: 'grouped1'`) with NO
     matching `VirtualFolders` entry → scan it → error banner with the
     grouped-libraries message, and NO `POST /Items/grouped1/Refresh` in
     the mock log (the false-success case this guards is a 204 with
     nothing scanned).
  3. *Non-admin at resolution:* set `failNextVirtualFolders` → scan →
     the FRIENDLY administrator-permission banner (not a raw technical
     403), and NO refresh POST occurs — this is the step every real
     non-admin actually hits, since the resolution GET is elevation-gated
     before the POST is ever reached.
  4. *Retry lifecycle:* set `failNextItemRefresh` → scan lib1 → error
     banner (admin-refusal message class); scan again → success notice
     shown and the stale failure banner gone (per-attempt exclusivity).
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
