# cw-1: Curation actions miss merged items' second key (local front / server watch key)

**Severity**: MEDIUM — the batch's own headline behaviors (mark-watched drops
from hero; removal sticks) silently fail for merged titles fronted by a
local/SMB copy with server-owned watch state.
**Status**: Verified
**Branch**: n/a — fixes land as single commits on `main` (owner no-branches
direction); see Commit.
**Commit**: `5ce26db`

## Evidence
- `src-tauri/src/commands.rs` (merged listing) sets a merged card's
  `rating_key` to the ranked play target (often local/SMB) with the server
  backing in `watch_key`.
- `src/routes/+page.svelte` `play()` records the played item as-is, so the
  recents entry's `rating_key` is the LOCAL key, `watch_key` the server key.
- Mark-watched sends `item.watchKey ?? item.ratingKey` (server key), but
  `recents::unrecord` (`src-tauri/src/recents.rs`) matches exact
  `rating_key` only → the local-keyed recents entry survives.
- Remove-from-continue sends only `item.ratingKey`; `hide()` tombstones that
  one key, and the hero tombstone filter is exact-key → the server hub copy
  (server key) reappears, and Plex server-side removal is attempted on a key
  the server routing may not own.

## Predicted observable failure
For a merged title playing from a local/SMB copy with Plex watch state:
mark-watched marks the server item but the hero still shows the entry after
refresh; remove-from-continue hides only the local copy while the server hub
copy comes back, and the Plex-side removal never targets the server item.

## What
`unrecord`/`hide` and the server-side removal treat the submitted key as the
item's only identity, but merged items have two (play key + watch key).

## Approach
Backend-only fix, no API change. `unrecord` drops entries whose `rating_key`
OR `watch_key` matches the submitted key. `hide` resolves the matching
recents entry first and tombstones the full identity set (submitted key,
entry `rating_key`, entry `watch_key`, deduped) before dropping the entry.
`remove_from_continue` resolves the entry's `watch_key` inside the same
config mutation (config::update is generic over the closure's Ok type) and
prefers it for the server-side removal route.

## Files changed
- `src-tauri/src/recents.rs` — key-set matching in `unrecord`/`hide`;
  `hide` returns the preferred server key.
- `src-tauri/src/commands.rs` — `remove_from_continue` routes server
  removal via the resolved watch key.

## Guard proof
- `recents::tests::unrecord_matches_watch_key_too` — FAILS with the exact-key
  matching restored.
- `recents::tests::hide_tombstones_every_key_of_a_merged_entry` — FAILS with
  single-key tombstoning restored.

## Coder dispute (if any)
None — verified against the code; the merged-card scenario is real and the
new slice-2/3 code was written to handle exactly these actions.

## Known gaps
A hub-only item (no recents entry) with a differing server key on another
source's hub is still single-key; out of scope — the merge dedup treats
those as distinct items today.

## Reviewer comments
- Reviewer: codex (codex-cli 0.142.5), dispatched 2026-07-05, verdict recorded 2026-07-05T10:27:19Z
- reviewed_sha `5ce26dbc6ecba209ec67af14c8a7120a488267e1`, base_sha `147bf7455a6cdf0a695ceec21440e1a8680775d0`
- guard_confirmed: true (reviewer ran revert-FAIL/restore-PASS in its own worktree)
- Verdict: **accepted**
- Comments: full `cargo test --locked` could not run to completion in the
  reviewer's sandbox (stream-proxy tests need to bind a local socket, denied
  there); the focused cw-1 guard tests passed after restore. Full suite (80)
  passes in the coder environment.
