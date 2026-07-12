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
- **Jellyfin:** `POST /Items/{viewId}/Refresh` with
  `Recursive=true&MetadataRefreshMode=Default&ImageRefreshMode=Default&ReplaceAllImages=false&ReplaceAllMetadata=false`
  — the call Jellyfin's own web UI "Scan Library" button makes. The raw
  section key IS the view id (`jellyfin.rs sections()`, `namespace_key(&self.id,
  &v.id)` at `src-tauri/src/source/jellyfin.rs:784`). Admin-gated
  (RequiresElevation): a non-admin user gets 403, which must surface
  politely, not crash.
- **Emby:** same client code path and same endpoints as Jellyfin.
  **ASSUMPTION** (same class as the JF/Emby notes in
  `.agents/plans/show-last-episode-sort.md`): Emby accepts the identical
  refresh call; a refusal degrades to the error banner, non-fatal. Vela is
  Plex-first and the owner is currently testing JF; no Emby server is
  available to verify.

These are two features that pair: **refresh** re-asks the server what it
already knows (fixes the restart annoyance); **scan** tells the server to go
discover new files, after which a refresh shows the result.

## Design

### Slice 1 — library view refresh (frontend-only)
1. **`refreshLibraries()` in `+page.svelte`:** one user action that
   refreshes the section list AND the content the user is looking at.
   - Clear the shared `error` banner once, at action start (the action owns
     its status — see the error-lifecycle contract below).
   - Re-fetch sections: bump `sourceGen`, call `loadSections(sg)` WITHOUT
     clearing `sections` first (unlike `loadEverything`, which blanks the
     sidebar for a source switch — a refresh must not flash the nav; the
     gen counter already discards a stale in-flight response).
   - **Mode `"home"`:** also re-fetch hubs/recents (the `loadHome` fetch
     set) concurrently. Already on Home, so a `mode = "home"` write is a
     no-op.
   - **Mode `"browse"`:** after the CURRENT-generation sections response
     lands (`sg === sourceGen`), reconcile the visible content:
     - active section (or merged type view) still exists → reload the
       visible grid from offset zero with the current sort, replacing the
       items (the `loadGen`-guarded listing machinery; gen-gate the reload
       so navigation performed meanwhile wins and a stale listing response
       is discarded). Without this, a post-scan refresh would update the
       sidebar but leave the grid stale — the exact "scan then refresh"
       pairing this plan promises.
     - active section's key no longer present → the library was deleted
       server-side; `goHome()` and FORCE a hub re-fetch (bump `homeGen`,
       call `loadHome` unconditionally — `goHome`'s hubs-empty conditional
       at line 408 would otherwise show cached rails that may still feature
       the removed library).
   - **Error-lifecycle contract** (observable, guard-tested): (a) clicking
     Refresh clears any prior banner; (b) if any refresh leg fails, the
     banner shows that failure even when a sibling leg succeeded — a
     successful leg must never null a sibling's error (note `loadHome`
     today sets `error = null` on entry, line 374: the hub leg invoked from
     refresh must not route through that clearing, e.g. by parameterizing
     it or inlining the hub fetch); (c) a later successful refresh clears
     the stale failure banner (via (a)); (d) errors publish only if the
     action's generations are still current.
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
   the endpoint shape and the rejections are unit-testable.
   **Version note:** `GET` is the long-standing verb (Plex Web still uses
   it; Plex staff confirm it stays backward-compatible) though newer docs
   prefer `POST` — record only; no dual-verb logic.
3. **Jellyfin/Emby (shared client):** new `post_empty(segments, query)`
   helper mirroring `get_json` (`jellyfin.rs:113`) for status/timeout/auth
   handling but building the URL via the existing `build_url`
   (`jellyfin.rs:239`) — **never raw path interpolation**. `build_url`
   percent-encodes each path segment, so a hostile view id
   (`../System/Shutdown?x=`, backslashes, fragments) cannot escape the
   `/Items/{id}/Refresh` shape into a different authenticated endpoint —
   this matters here more than on the GET paths because the scan call
   carries admin-capable credentials. Status mapping: `UNAUTHORIZED →
   RECONNECT_REQUIRED`, plus `FORBIDDEN → "the server refused the scan
   (administrator permission required)"` so the JF non-admin case reads as
   policy, not failure. `JellyfinSource::scan_library` posts
   `["Items", view_id, "Refresh"]` with query `Recursive=true`,
   `MetadataRefreshMode=Default`, `ImageRefreshMode=Default`,
   `ReplaceAllImages=false`, `ReplaceAllMetadata=false`,
   `RegenerateTrickplay=false` (the set jellyfin-web's scan dialog sends).
   A pure fn returns the param list (`scan_query()`) for unit testing.
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
   routes to the existing `error` banner. No auto-refresh afterward — the
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

## Verification
- **Unit (guard-proven red→green):**
  - Plex `scan_path`: exact `/library/sections/{key}/refresh` shape for a
    valid id, and REJECTION of hostile keys — empty, `all`, `1/../2`,
    `12?force=1`, `12#f`, `1\2`, non-digits (via `validate_plex_id`).
  - JF `scan_query`: the full param set (`Recursive=true`, both refresh
    modes `Default`, both `ReplaceAll*=false`, `RegenerateTrickplay=false`).
  - JF URL construction: a hostile view id (`../System/Shutdown?x=`)
    passed through `build_url(["Items", id, "Refresh"], …)` stays ONE
    encoded path segment — the result still matches
    `…/Items/<encoded>/Refresh` and contains no raw `../` or `?` from the
    id.
- **E2E `libraryrefresh.mjs`:** `mockjf.mjs` gains mutable state the
  scenario can edit live: `state.views` (today hardcoded,
  `mockjf.mjs:100`; initialize to the current single view — existing
  scenarios unaffected), the movie list already in `state`, and a one-shot
  `state.failNextViews` flag (500 on the next `/Users/{id}/Views`).
  Scenario, one app session, four cases in sequence:
  1. *Sidebar:* load → one library → push a second view → Refresh → new
     sidebar entry appears without restart.
  2. *Visible grid:* open the library grid → add a movie to the mock's
     list → Refresh → the new card appears WITHOUT re-entering the library
     (the post-scan pairing case).
  3. *Failure then success:* set `failNextViews` → Refresh → error banner
     shows; Refresh again → banner clears and content is current (the
     error-lifecycle contract, (a)–(c)).
  4. *Deleted active library:* while browsing it, remove its view →
     Refresh → app lands on Home AND the mock log shows a fresh hub fetch
     after the refresh (forced `loadHome`, not the cached-rails
     conditional).
  RED without slice 1 (no refresh control exists to click).
- **E2E `scanlib.mjs`:** mock answers `POST /Items/{id}/Refresh` with 204
  (requests are already recorded, `mockjf.mjs:93`). Scenario: right-click
  the library sidebar entry → click "Scan library" → poll
  `mock.state.requests` for `POST /Items/lib1/Refresh` carrying
  `Recursive=true` and `RegenerateTrickplay=false` → assert the transient
  notice rendered. RED without slice 2 (no menu entry, no request).
- Full local CI set (`npm run check`, `npm run build`; from `src-tauri/`:
  `cargo check --locked`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked`); full E2E suite on the owner's Linux VM
  (venue per `.agents/state.md`).
- **Owner playtest ask (real servers):** add a library in JF → Refresh →
  it appears without restart; right-click a JF library → Scan library →
  JF dashboard shows the scan running; same scan on Plex (owner token) →
  Plex shows the section scanning; delete/hide a library server-side →
  Refresh while browsing it → Vela lands on Home instead of a dead grid.

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
