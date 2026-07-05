# cw-2: remove_from_continue holds the registry lock across a network await

**Severity**: LOW — bounded (15s reqwest timeout) but user-visible: hub
loads, browsing, and play routing block behind a best-effort network call.
**Status**: In progress
**Branch**: n/a — single fix commit on `main`.
**Commit**: (pending)

## Evidence
`src-tauri/src/commands.rs` `remove_from_continue`:
`if let Ok((src, raw)) = state.registry.lock().await.route(&rating_key)`
— under Rust 2021 the scrutinee temporary (the registry MutexGuard) lives
through the if-let body, which awaits `src.remove_from_continue(&raw)` (a
Plex HTTP PUT with a 15s client timeout). Every other command that routes
(`get_hubs`, `play_by_key`, `set_watched`) drops the guard at the end of the
routing statement before any network call.

## Predicted observable failure
Click "Remove from Continue Watching" while the Plex server is slow or
unreachable: unrelated UI actions needing the registry (hub refresh, play)
stall up to 15s. Violates the recorded repo practice "do not hold shared
locks across blocking network work".

## What
Lock scope bug: the guard should end with the route lookup.

## Approach
Bind the routing result in a `let` statement (guard temporary drops at the
statement end, same pattern as `set_watched`), then await the network call
lock-free.

## Files changed
- `src-tauri/src/commands.rs` — `remove_from_continue` lock scope.

## Guard proof
Lock lifetime isn't observable from a unit test without instrumenting the
registry mutex; this is a compile-time-shape fix. Manual check instead:
code inspection against the `set_watched` pattern + full suite green. (Per
playbook: genuinely untestable, reason stated.)

## Coder dispute (if any)
None — the Rust 2021 scrutinee-lifetime reading is correct.

## Known gaps
None.

## Reviewer comments
(pending)
