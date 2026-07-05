# cw-3: A failed play still clears the Continue Watching tombstone

**Severity**: LOW — wrong hero content after an error path; no data loss.
**Status**: In progress
**Branch**: n/a — single fix commit on `main`.
**Commit**: (pending)

## Evidence
`src/routes/+page.svelte` `play()`: `invoke("record_recent", …)` fires
fire-and-forget BEFORE `await invoke("play_item", …)`. `recents::record`
clears the item's tombstone and inserts a recents entry immediately. If
`play_item` then fails (mpv missing, stale source path), the snapshot and
the tombstone clear stand although nothing played.

## Predicted observable failure
Remove an item from Continue Watching, later click Play on it from
Library/Search on a machine where mpv is missing → play fails with an error,
yet the item reappears in the hero (tombstone cleared + fresh recents entry)
despite no playback having happened.

## What
Recording happens at "user clicked play", not at "a play session actually
started".

## Approach
Reorder in `play()`: await `play_item` success first, then fire
`record_recent`. `play_item` resolves at mpv spawn (session start), so the
snapshot still lands at the start of the session, before any finish event.

## Files changed
- `src/routes/+page.svelte` — `play()` ordering.

## Guard proof
Frontend-only ordering; no JS test runner in the repo. Manual check: code
inspection (record is now behind the awaited success), svelte-check + build.
The E2E harness plan's failed-play scenario will cover it end-to-end.

## Coder dispute (if any)
None.

## Known gaps
`playFrom()` and queue auto-advance paths record via the backend dispatcher,
not this function; reviewed — they don't share the defect.

## Reviewer comments
(pending)
