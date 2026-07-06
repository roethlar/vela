# sspf-7: a late request after playback-end can still orphan an SMB session

**Severity**: HIGH — same live-context leak class as sspf-5, reopened: the sspf-5
release-bump left an ownerless generation a straggler request could store under.
**Status**: Verified
**Commit**: `dec0121`

## Evidence
`src-tauri/src/stream_proxy.rs`, `release_session` + `get_or_create_session`
commit (after the sspf-5 fix, head `5a64172`). The sspf-5 fix closed the
play-epoch by bumping `entry.generation` on a matching release. But the bumped
generation stays on the entry with no play owning it. A late GET for that token
after playback-end (e.g. an ffmpeg reconnect straggler, or an old connection when
a new play replaces the old) looks the token up, captures the *bumped* generation,
and reaches the commit; the commit only checked `entry.generation == generation`,
which now holds, so it stores a fresh `SmbConnection`. No `on_end` owns that
generation, so nothing frees it — stopping/replacing SMB playback leaks live
contexts until eviction/app exit.

## Predicted observable failure
After a full play cycle (register → cache a session → `release_session` at end),
the token still sits in the registry. A late `get_or_create_session` capturing the
current (post-release) generation must be refused and store nothing. Before the
fix it stores an orphan (`entry_has_session == true`, unfreeable).

## What
The generation bump-on-release closed the epoch against a request that captured
the *pre-release* generation (sspf-5) but not against one that captures the
*post-release* generation. Generation alone cannot express "no play is active".

## Approach
Split the two concerns the generation was overloading. `generation` now means
*which play* and is bumped ONLY on a same-file replay (register reuse). A new
`Entry.active: bool` means *is the current-generation play still live*: set true
at register (new and reuse), set false in `release_session` on a matching release
(replacing the sspf-5 generation-bump). `get_or_create_session` stores only for
the current, still-live play — `entry.generation == generation && entry.active` —
so a create is refused both when superseded by a newer play (sspf-5 mid-connect)
and when no play is active (sspf-7 straggler). Invariant: `active == false` implies
`session == None` (release frees the session as it clears the flag), so the
unchanged fast path never hands out a session for an inactive token.

## Files changed
- `src-tauri/src/stream_proxy.rs` — `Entry.active`; `register` sets it (new +
  reuse); `release_session` clears it instead of bumping the generation;
  `get_or_create_session` commit guards on `generation` match AND `active`.

## Guard proof
- `stream_proxy::tests::a_late_request_after_a_completed_play_does_not_orphan_a_session`
  — a full play cycle then a late create capturing the current generation; asserts
  it is refused and stores nothing. Dropping the `active` half of the commit guard
  makes it FAIL (the orphan is stored); restoring makes it PASS.
- `a_create_after_the_plays_release_is_refused_not_orphaned` (from sspf-5, updated
  to the active model) still guards the mid-connect case.

## Reviewer comments
- **r2** 2026-07-05 `codex` (codex-cli 0.142.5), `codex exec --json`, reviewed head
  `5a64172` base `21cd8909`, `guard_confirmed:false`, verdict **reopened**:
  "after release_session closes a play by bumping generation, a late GET that looks
  up the old token after playback-end captures that new ownerless generation,
  passes the commit guard, and can store a fresh SmbConnection that no future
  on_end owns; stopping/replacing SMB playback can leave contexts open until
  registry eviction/app exit — HIGH". Admitted (real; the sspf-5 mechanism was
  insufficient). Fix below; re-review pending.
