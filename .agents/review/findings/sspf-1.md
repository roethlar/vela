# sspf-1: token reuse serves a stale cached length on replay

**Severity**: MEDIUM — a file resized/replaced mid-session serves a wrong
Content-Length/Content-Range on the next play (416 on a now-valid tail, or a
truncated body on a shrunk file), where the prior stat-per-request never could
**Status**: Verified
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: `08fef74`

## Evidence
`src-tauri/src/stream_proxy.rs` `register()` (the token-reuse return path):
when a replay of the same `(mount.id, relative)` reused an existing token it
returned the token without touching `Entry.len`. Combined with the new
sub-slice-1 length cache, a later request took the `Some(len)` path
(`serve_connection`, `open_read_with_len`) instead of re-statting. Filed by
codex on review round 1 of the slice (base `adbeb867`, head `941b933`).

## Predicted observable failure
Play file X (len L1 cached). X is replaced/resized on the server. Replaying X
in the same app session reuses the token, so its first request serves the
stale L1: a tail Range past L1 → 416 even though the bytes now exist; a shrunk
file → Content-Length promises L1 but the body is short. The prior code
re-statted every request, so it always served the current size.

## What
The per-token length cache outlived a single playback: token reuse carried a
stale length into a new play.

## Approach
Reduce the cache's scope to one playback. On the token-reuse path in
`register()`, set `existing.len = None` so each fresh play re-stats once on its
first request; seeks within that playback still reuse the fresh value. One
playback is the only window the seek optimization needs and the only window the
file is guaranteed stable.

## Files changed
- `src-tauri/src/stream_proxy.rs` — reuse path clears `Entry.len`; new guard
  test.

## Guard proof
`stream_proxy::tests::reregistering_a_token_clears_a_stale_cached_length` seeds
a cached len, re-registers the same mount+path, and asserts the len was
cleared. Executed 2026-07-05: red — with `existing.len = None` removed the
stale `Some(4242)` survives (`left: Some(4242), right: None`, FAIL); green —
with the reset the len is `None`. Full suite clean.

## Coder dispute (if any)
None — admitted as filed; a real regression introduced by the length cache.

## Known gaps
A file resized *during a single uninterrupted playback* (same token, no
re-register) still serves the first-request length — but mpv learned that
Content-Length at open and never requests beyond it, so caching it for the
session is consistent, not a defect. sspf-2 covers the concurrent-replay race.

## Reviewer comments
codex (codex-cli 0.142.5), `-s read-only`, JSON mode. Round 1 (2026-07-05):
**reopened** on this finding, reviewed head `941b9338`, base `adbeb86765`,
`guard_confirmed: true`. Final: **accepted** at round 4, reviewed head
`401fd1bc`, base `adbeb86765`, `guard_confirmed: true`, no comments.
