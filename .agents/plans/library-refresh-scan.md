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
startup, on source switch, and on Plex link completion. `goHome` (line 396)
re-fetches hubs at most, never sections. A library added on the server
therefore never appears until a source switch or an app restart.

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
1. **`refreshLibraries()` in `+page.svelte`:** bump `sourceGen` and call
   `loadSections(sg)` WITHOUT clearing `sections` first (unlike
   `loadEverything`, which blanks the sidebar for a source switch — a
   refresh must not flash the nav; the gen counter already discards a stale
   in-flight response). If `mode === "home"`, also re-fetch hubs via
   `loadHome(++homeGen)` (already on Home, so its `mode = "home"` write is a
   no-op). If, after the new list lands, `mode === "browse"` with a section
   active (`active?.key`) whose key is no longer present, call `goHome()` —
   the library was deleted server-side and the grid under the user is dead.
2. **UI:** an icon button beside the "Library" group heading in the sidebar
   (`.sidegroup`, `+page.svelte:1223` — becomes a flex row), aria-label
   "Refresh libraries", `title` tooltip. New `refresh` glyph in
   `src/lib/Icon.svelte` (circular-arrows). A `refreshing` boolean disables
   the button and spins the icon while the fetch is in flight (feedback +
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
   `PlexSource::scan_library` wraps it in the same
   `ensure_ready` → on-error `rediscover` retry used by `sections()`
   (`src-tauri/src/source/plex.rs:283`). A small pure fn builds the path
   (`scan_path(key) -> String`) so the endpoint shape is unit-testable.
3. **Jellyfin/Emby (shared client):** new `post_empty(path, query)` helper
   on the client mirroring `get_json` (`jellyfin.rs:113`) — auth headers,
   15s timeout, `UNAUTHORIZED → RECONNECT_REQUIRED`, plus
   `FORBIDDEN → "the server refused the scan (administrator permission
   required)"` so the JF non-admin case reads as policy, not failure.
   `JellyfinSource::scan_library` posts `/Items/{view_id}/Refresh` with the
   query params above; a pure fn returns the param list
   (`scan_query() -> Vec<(&str, String)>`) for unit testing.
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
- **Unit (guard-proven red→green):** Plex `scan_path` (exact
  `/library/sections/{key}/refresh` shape) and JF `scan_query`
  (`Recursive=true`, both refresh modes `Default`, both `ReplaceAll*=false`).
- **E2E `libraryrefresh.mjs`:** `mockjf.mjs` serves `/Users/{id}/Views`
  from mutable `state.views` (today hardcoded, `mockjf.mjs:100`; initialize
  to the current single view — existing scenarios unaffected). Scenario:
  load app → assert one library in the sidebar → push a second view into
  `mock.state.views` → click the refresh button → poll until the new
  library appears without restart. RED without slice 1 (no refresh control
  exists to click).
- **E2E `scanlib.mjs`:** mock answers `POST /Items/{id}/Refresh` with 204
  (requests are already recorded, `mockjf.mjs:93`). Scenario: right-click
  the library sidebar entry → click "Scan library" → poll
  `mock.state.requests` for `POST /Items/lib1/Refresh` with
  `Recursive=true` → assert the transient notice rendered. RED without
  slice 2 (no menu entry, no request).
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

(rounds appended below as they complete)
