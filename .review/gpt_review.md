> **Superseded (2026-06-10).** Current implementation status and durable review
> decisions now live in `.agents/state.md` and `.agents/decisions.md`. This file
> is retained as history and is no longer updated.

# GPT Review - vela-foundation

Review target: `vela-foundation` at `da45bd1` after fetching `origin`.
Date: 2026-05-24.

This branch is materially newer than `tauri-ui`; most of the prior Kimi issue
queue in `ISSUES.md` is already addressed. The remaining risks are concentrated
around Plex connection selection/recovery, renderer-controlled command bounds,
and a few frontend/CI gaps.

Implementation status after the first pass:

- Fixed: Plex remote/direct endpoint probing, IPv6-safe Plex origins, stale Plex
  rediscovery retries, Plex renderer path validation, renderer command bounds,
  frontend CI/build, empty states, settings error announcement, short-search UX,
  local symlink escape listing, local asset scope validation, Plex link HTTP
  failure handling, and Jellyfin/Emby media-source selection.
- Deferred: moving local filesystem traversal out of async trait methods.
- Accepted for now: token-bearing poster/stream URLs remain a deliberate
  local-only exposure; new backend-only Plex calls avoid query-token use where
  practical.

## High

1. Plex server selection persists candidates before proving reachability.
   `discover_servers()` asks Plex for HTTPS, Relay, and IPv6 connections, but
   `PlexServer` only stores scheme/host/port and discards `local`/`relay`/full
   URI metadata. `pick_server()` then chooses the first HTTPS `.plex.direct`
   endpoint or falls back to the first server, and `rediscover()` persists it
   immediately.

   Impact: off-LAN use can still fail unlike the native Plex client. One stale
   LAN `.plex.direct`, dead IPv6, or Relay endpoint can be saved and retried
   forever. Relay can also be selected even though HDR/direct-play over Relay is
   not a sensible default.

   References:
   - `src-tauri/src/plex_library.rs:180`
   - `src-tauri/src/plex_library.rs:249`
   - `src-tauri/src/source/plex.rs:36`
   - `src-tauri/src/source/plex.rs:282`

   Fix: preserve normalized connection origins plus `local`/`relay`; probe
   candidates with short authenticated `/identity` requests; verify
   `machineIdentifier`; prefer reachable direct HTTPS; leave Relay behind an
   explicit opt-in.

2. Plex URL construction is IPv6-unsafe and loses normalized origins.
   Discovery parses connection URIs with `url::Url`, then stores `host_str()`.
   Later calls rebuild URLs as `{}://{}:{}`. For IPv6, this produces invalid
   authorities like `https://2001:db8::1:32400`.

   References:
   - `src-tauri/src/plex_library.rs:263`
   - `src-tauri/src/plex_library.rs:286`
   - `src-tauri/src/plex_library.rs:436`
   - `src-tauri/src/plex_library.rs:548`
   - `src-tauri/src/plex_library.rs:672`
   - `src-tauri/src/plex_library.rs:720`

   Fix: store a normalized origin from the Plex connection URI and join paths
   from that origin. For manually restored config, bracket IPv6 hosts when
   reconstructing the origin.

3. Frontend CI is currently red on the branch as checked out.
   `vite.config.js` still has an unused `@ts-expect-error`; `svelte-check`
   reports it. CI runs `npm run check`, so this blocks the frontend job.

   References:
   - `vite.config.js:4`
   - `.github/workflows/ci.yml:52`

   Fix: remove the stale directive. A clean `npm ci` may also be required after
   branch switching because local `node_modules` was from the old checkout.

## Medium

4. Plex rediscovery holds the source mutex across network I/O.
   `ensure_ready()` clones the client when a server is ready, but `rediscover()`
   takes `self.lib.lock().await` and keeps it while calling plex.tv discovery.

   Impact: concurrent Plex calls queue behind network discovery/probing. This is
   exactly the stall pattern the newer architecture otherwise avoids.

   References:
   - `src-tauri/src/source/plex.rs:36`

   Fix: clone the `PlexLibrary` under the mutex, drop the guard, discover/probe,
   then reacquire briefly to set the chosen server.

5. Stale Plex server recovery is inconsistent.
   `sections()`, `hubs()`, and `search()` rediscover and retry on failure, but
   `items()`, `children()`, and `resolve_stream()` fail directly if the saved
   endpoint is stale.

   References:
   - `src-tauri/src/source/plex.rs:146`
   - `src-tauri/src/source/plex.rs:183`
   - `src-tauri/src/source/plex.rs:192`

   Fix: use the same rediscover-once retry wrapper for all Plex API paths that
   depend on a selected server.

6. Renderer-controlled browse inputs are not bounded or allow-listed.
   `get_items()`, `get_children()`, and `search()` pass renderer-supplied
   pagination, sort, section type, and query values to sources with no central
   clamp/validation.

   Impact: a compromised or buggy webview can request huge pages, unknown sort
   tokens, or unsupported section types. Local sources cap some paths, but remote
   backends still receive unbounded requests.

   References:
   - `src-tauri/src/commands.rs:727`
   - `src-tauri/src/commands.rs:741`
   - `src-tauri/src/commands.rs:763`

   Fix: clamp page sizes, cap search length, and allow-list sort and section
   types before dispatching to sources.

7. CI does not run the production frontend build.
   The frontend job only runs `npm ci` and `npm run check`; it never runs
   `npm run build`, even though Tauri uses that command before packaging.

   References:
   - `.github/workflows/ci.yml:51`
   - `package.json:8`
   - `src-tauri/tauri.conf.json:9`

   Fix: add `npm run build` to CI.

8. Jellyfin/Emby direct-play selection takes the first media source.
   `playback_info()` uses `media_sources.first()` and then constructs a static
   direct stream URL. Multi-version items, unavailable/offline sources, remote
   constraints, and non-direct-playable sources can choose the wrong media.

   References:
   - `src-tauri/src/source/jellyfin.rs:143`
   - `src-tauri/src/source/jellyfin.rs:165`

   Fix: inspect media-source playability/version metadata and prefer direct
   playable HDR/highest-quality candidates, falling back intentionally.

9. Local asset protocol scope can be broader than the media intent.
   `add_local_folder()` previously called `allow_directory()` before validating
   kind and did not reject broad roots. Startup also restored all persisted
   local folders into the asset protocol.

   Impact: a bad or old config could expose a filesystem or home root to the
   asset protocol when only a specific media folder should be reachable.

   References:
   - `src-tauri/src/commands.rs:262`
   - `src-tauri/src/lib.rs:104`

   Fix: validate before allowing asset access, reject filesystem/home roots,
   and skip unsafe persisted roots on startup.

10. Plex link flow should fail closed on unexpected HTTP statuses.
    `link_begin()` parsed the response body without first checking success.
    `link_poll()` treated only a few statuses as hard failures, so other
    unexpected non-2xx statuses could look like an unauthenticated pending PIN.

    References:
    - `src-tauri/src/commands.rs:583`
    - `src-tauri/src/commands.rs:718`

    Fix: reject all unexpected non-2xx statuses before parsing link bodies.

## Low

11. Browse/search empty states can render as a blank panel.
   Once loading finishes in browse mode, the UI always renders crumbs plus an
   empty grid. Empty sections and no-match searches should show explicit copy.

   Reference: `src/routes/+page.svelte:575`

12. Settings modal errors are not announced to assistive tech.
    Main app errors use `role="alert"`, but settings errors are a plain div.

    Reference: `src/lib/Settings.svelte:231`

13. Short searches fail silently.
    Queries under two characters return immediately without clearing stale
    state or explaining the minimum length.

    Reference: `src/routes/+page.svelte:377`

14. Local browsing can surface symlinked files that playback later rejects.
    Listing/search can walk outside a configured root through symlinks, while
    `resolve_stream()` correctly rejects those paths with `within_roots()`.
    Security posture is acceptable, but the UX can show unplayable results.

    References:
    - `src-tauri/src/source/local.rs:269`
    - `src-tauri/src/source/local.rs:400`
    - `src-tauri/src/source/local.rs:382`

15. Local filesystem traversal still runs inside async trait methods.
    Local source browse/search paths use synchronous directory and metadata
    sidecar reads. The work is bounded, but slow local disks or mounts can still
    occupy async workers.

    References:
    - `src-tauri/src/source/local.rs:257`
    - `src-tauri/src/source/local.rs:188`

    Fix: wrap local traversal in blocking tasks or split the local source into
    synchronous worker helpers.

## Already Addressed From ISSUES.md

- Blocking SMB mount/unmount and mpv launch paths have been moved off async
  command bodies.
- Plex stream preflight fails closed for non-2xx statuses, with `405 HEAD`
  handled as an explicit exception.
- Many Plex API paths now use `error_for_status()`.
- Jellyfin/Emby stream and poster URL query values are built through `Url`.
- Svelte dynamic `{#each}` blocks are keyed.
- Tauri CSP is restrictive again.
- mpv IPC sockets use a private unpredictable runtime path.
- Plex PIN parsing uses `quick-xml`.
- `rust-version`, release profile settings, and CI exist.
- QR rendering avoids raw `{@html}`.
- Settings focus management and frontend timer cleanup are present.
