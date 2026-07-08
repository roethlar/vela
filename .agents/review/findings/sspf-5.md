# sspf-5: stale request caches an SMB session after its play already released it

**Severity**: HIGH — a live libsmbclient session (socket + memory + a server-side
session slot) is orphaned with no owner to free it; rapid replaces accumulate
them up to the registry cap, and a NAS with per-client session limits can start
refusing connections.
**Status**: Verified
**Commit**: `c7211e6`

## Evidence
`src-tauri/src/stream_proxy.rs`, `get_or_create_session` commit step (sub-slice 3,
head `05ed86b`). A serve captures `generation` at its registry lookup, then takes
the slow path and blocks in `connect_mount` (which can wait on
`ctx_lifecycle_lock` while another connection's blocking `smbc_free_context`
finishes). During that window the play that spawned the request ends (user stops,
or a new play kills the old mpv), firing `on_end` → `release_session(token, gen)`,
which finds `session == None` and is a no-op. The request then finishes connecting
and commits `entry.session = Some(created)`. That session's play is already over,
so no `on_end` will ever free it; it survives until eviction (cap 64) or app exit.

## Predicted observable failure
Register a token, capture its generation `g0`, run `release_session(token, g0)`
(the play ends before the session is cached), then call
`get_or_create_session(token, g0, …)` (the stale request commits). Before the fix
it stores a session with no owner (`entry_has_session == true`, unfreeable). After
the fix the commit is refused and no session is stored.

## What
The commit-if-absent path stored a newly built session unconditionally, and
`release_session` did not mark the play-epoch closed. A request whose play ended
mid-connect could therefore cache a session that nothing would ever release.

## Approach
Make the session's `generation` a live-play-epoch guard on the *store*, not just
on the free: (1) `release_session`, on a generation match, bumps the generation
after taking the session — closing that token's play-epoch even when there was no
session to take; (2) `get_or_create_session` takes the caller's captured
`generation` and, in the commit, stores only if `entry.generation` still matches —
otherwise the play ended or was replaced, so it drops the freshly built session
off-lock and returns a "superseded" error. The fast path is unchanged: returning a
live session to a superseded request is harmless (its serve fails on the closed
socket), and the leak only ever happened at the commit.

## Files changed
- `src-tauri/src/stream_proxy.rs` — `release_session` bumps generation on match;
  `get_or_create_session` gains a `generation` parameter and a generation-guarded
  commit (`Outcome::Superseded`); `serve_connection` passes its captured
  `generation`.

## Guard proof
- `stream_proxy::tests::a_create_after_the_plays_release_is_refused_not_orphaned`
  — asserts a create with the pre-release generation is refused and stores no
  session. Reverting the commit's generation guard (store regardless) makes it
  FAIL (the orphan is stored); restoring makes it PASS.

## Reviewer comments
- **r1** 2026-07-05 `codex` (codex-cli 0.142.5), `codex exec --json`, reviewed head
  `05ed86b3` base `21cd8909`, `guard_confirmed:false`, verdict **reopened**:
  "A stale GET can cache a session after its playback-end release already ran …
  the stale request stores created into entry.session with no future owner to free
  it; repeated rapid replaces leak live SMB contexts until eviction/app exit —
  HIGH". Admitted (real TOCTOU orphan). Fix below; re-review pending.
