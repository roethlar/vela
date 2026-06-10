> **Superseded (2026-06-10).** Current review action status now lives in
> `.agents/state.md`, and durable decisions from this file live in
> `.agents/decisions.md`. This file is retained as history and is no longer
> updated.

# Deduped Action List - vela-foundation

Status key: `todo`, `in-progress`, `done`, `partial`.

This list merges `ISSUES.md`, the GPT review, and the agent findings for
`vela-foundation`. It has been updated after the first implementation pass.

1. `done` Make Plex connection selection native-client-like enough for remote use.
   - Preserves normalized connection origins plus Plex `local`/`relay` metadata.
   - Probes candidates with short authenticated `/identity` requests and verifies `machineIdentifier`.
   - Prefers reachable direct HTTPS connections and does not select Relay by default.
   - Persists only a candidate that has actually answered.

2. `done` Make Plex API URL construction IPv6-safe.
   - Reuses normalized origins from Plex resource URIs.
   - Brackets IPv6 hosts when restoring legacy/manual saved config.
   - Routes Plex API paths through normalized server origins instead of raw `scheme/host/port` interpolation.

3. `done` Avoid holding the Plex source mutex across rediscovery/probing.
   - Clones the library under the lock, drops the guard for network work, then reacquires only to save the chosen server.

4. `done` Retry stale Plex server recovery consistently.
   - `items`, `children`, and `resolve_stream` now rediscover once on failure, matching sections/hubs/search behavior.

5. `done` Add central renderer-command input validation.
   - Clamps page sizes.
   - Caps search query length.
   - Allow-lists sort tokens and section types before dispatching to sources.
   - Validates renderer-provided Plex path IDs before building Plex paths.

6. `done` Fix frontend CI and verify the production build.
   - Removed the stale `@ts-expect-error` in `vite.config.js`.
   - Added `npm run build` to CI.
   - Added the missing `@types/node` dev dependency required by the existing tsconfig.

7. `done` Add browse/search empty states.
   - Empty browse views and no-result searches now render explicit copy instead of an empty grid.

8. `done` Announce Settings modal errors.
   - Settings modal errors now use `role="alert"`.

9. `done` Improve short-search UX.
   - One-character searches now clear stale search state and show the minimum-length requirement.

10. `done` Filter symlink escapes during local listing/search.
    - Local browse, children, and search skip canonical paths outside their configured root before surfacing results.

11. `done` Improve Jellyfin/Emby media-source selection.
    - Playback now chooses from Jellyfin/Emby `MediaSources` intentionally.
    - Direct-play candidates outrank direct-stream candidates, HDR outranks SDR within the same directness tier, and quality breaks ties.
    - If no direct candidate is advertised, selection falls back to the best-quality source instead of silently taking the first source.

12. `done` Tighten local asset protocol scope.
    - `add_local_folder` validates folder kind and root safety before calling `allow_directory`.
    - Filesystem root and `$HOME` are rejected as media roots.
    - Unsafe persisted local roots are skipped on startup for browsing and asset protocol access.

13. `done` Fail closed on unexpected Plex link HTTP statuses.
    - `link_begin` and `link_poll` now reject unexpected non-2xx statuses instead of treating them as pending/parseable auth responses.

14. `todo` Move local filesystem browsing off async worker threads.
    - Local listing/search still performs synchronous filesystem traversal inside async trait methods.
    - This is lower risk than the network and token issues, but large/slow local mounts can still occupy async workers.

15. `partial` Keep token exposure policy explicit.
    - Current branch deliberately accepts token-bearing poster/stream URLs as a local-only exposure.
    - Backend-only Plex calls were moved toward header auth where practical.
    - Avoid adding new logs or error messages containing token-bearing URLs.
    - Revisit poster/stream token transport only if the threat model changes.

16. `done` Carry over fixed `ISSUES.md` items.
    - Blocking OS/process calls moved off async command bodies.
    - Plex preflight fails closed.
    - CSP restored.
    - mpv IPC path hardened.
    - Config persistence hardened.
    - PIN XML parsing, QR rendering, settings focus management, frontend timers, and CI baseline added.
