# Plan: Post-playback watch-state refresh

Status: DRAFT — not approved for implementation. Covers two `ISSUES.md`
entries (Open - Owner-Reported 2026-07-04): "Continue Watching does not
refresh after playback" and "Card watch state is stale after playback".

## Root cause (confirmed by code reading, 2026-07-04)

The server side is already correct. On mpv exit the Rust trackers post the
final state — Plex `update_timeline("stopped")` plus `update_progress`
(`src-tauri/src/playback.rs:800-823`), Jellyfin `/Sessions/Playing/Stopped`
(`playback.rs:893-904`) — and `get_hubs` is a live, uncached fetch every call
(`src-tauri/src/plex_library.rs:438`; Jellyfin `jellyfin.rs:694`). There is no
Rust-side hub cache.

The staleness is entirely frontend: `src/routes/+page.svelte` fetches hubs
once via `loadHome()` (`+page.svelte:234`, callers: mount `boot()`, source
switch, sources-changed, Plex link completion) and stores them in a `$state`
array that nothing invalidates after playback. Rust emits zero Tauri events
today, and the page has no focus/visibility listener. Restart "fixes" it
because `boot()` re-runs the live fetch. Progress/played per card come
embedded in the same hub/browse payloads (`ItemDto.view_offset_ms/.played`,
`src-tauri/src/source/mod.rs:29-41`), so refreshing the fetch refreshes the
badges and bars too.

## Design

One new Tauri event, one new frontend listener. No caching changes.

1. Rust emits `playback-ended` exactly once per playback session, after the
   final server write for sources that track progress:
   - Thread a `tauri::AppHandle` into `playback::play` (`playback.rs:436`)
     and the tracker tails; emit after the Plex final `update_progress`
     (~`playback.rs:823`) and after the Jellyfin `/Stopped` post
     (~`playback.rs:904`), so a re-fetch triggered by the event is guaranteed
     to see the new server state.
   - Sessions with no tracker (local/SMB files): emit at mpv process exit.
     Exactly-once per session is the invariant; the precise mechanics are an
     implementation detail within it.
   - Payload: `{ sourceId, itemKey }` only — no URLs, no tokens (earned
     practice: nothing token-bearing in new frontend-visible surfaces).
   - Emit happens on the tracker thread after its network IO completes; no
     shared locks held across the emit.
2. Frontend (`src/routes/+page.svelte`): import `listen` from
   `@tauri-apps/api/event`; in `onMount`, subscribe to `playback-ended` →
   `loadHome(++homeGen)` (the existing generation guard makes concurrent
   re-invokes race-safe), and re-fetch the current browse listing when a
   browse view is showing so grid badges update too. Unlisten in `onDestroy`.
3. Queue auto-advance: each finished item ends its own tracker session, so
   each advance emits one event; the Home refresh after the final item is the
   one users see.
4. Manual "Mark watched" already updates card state in place
   (`+page.svelte:569-577`); unchanged.

Non-goals: no hub caching/TTL layer, no periodic polling, no window-focus
refresh in v1 (can be added later as staleness hardening if events prove
insufficient), no mid-playback live progress updates in the UI.

## Verification

- Full CI set (change spans both sides): `npm run check`, `npm run build`,
  and from `src-tauri/`: `cargo check --locked`,
  `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`.
- Rust unit coverage where a seam exists (e.g. event payload assembly);
  the emit/listen wiring itself is not unit-testable — covered manually.
- Owner playtest (required, automation cannot cover it): play a Plex item a
  few minutes, quit mpv → Continue Watching/On Deck and the card progress bar
  update without restart; finish an episode → played badge appears and hubs
  reflect it; repeat once on Jellyfin. Watch the console for event spam
  (expect exactly one event per session).

## Open points to settle at approval

1. Browse-grid refresh scope: re-fetch the whole current listing (simple,
   proposed) vs patch the single item in place (cheaper, more code).
2. Whether the event should also fire on user-abort at position 0 (harmless
   extra refresh, proposed: yes, keep it unconditional).
