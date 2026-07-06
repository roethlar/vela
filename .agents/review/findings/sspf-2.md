# sspf-2: a late store_len repopulates a length a replay just cleared (TOCTOU)

**Severity**: MEDIUM — under overlapping same-file plays plus a mid-session
resize, a stale length is written back after the clear and the new play serves
a wrong Content-Length/Content-Range
**Status**: Verified
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: `79f3979`

## Evidence
`src-tauri/src/stream_proxy.rs` `store_len()` wrote the discovered length back
by token with no versioning. A request that saw `cached_len = None` at lookup
could finish its slow `open_read`/stat and then `store_len` the old length
AFTER a concurrent replay's `register()` had cleared it (sspf-1 fix). The new
play's first request then reads the repopulated stale `Some(len)`. Filed by
codex on review round 2 (base `adbeb867`, head `08fef74`).

## Predicted observable failure
Play 1's request Ra reads `None`, begins a slow stat returning old size L1. The
file is replaced (new size L2). Play 2 (replay) `register()`s → clears len. Ra
finishes and `store_len(L1)` lands after the clear. Play 2's first request
reads `Some(L1)` and serves a stale length for the L2 file (416 on a now-valid
tail, or a truncated body). The plan requires each sub-slice be race-correct on
its own, so the size cache must resist this.

## What
The write-back path had no ownership check: a stale in-flight writer could
clobber a clear performed for a newer play.

## Approach
A per-token generation. `Entry.generation` is bumped on the `register()`
token-reuse path; `serve_connection` captures the generation at lookup;
`store_len(token, generation, len)` writes only if the entry's generation still
matches. A stale-generation writer is dropped, so the next request re-probes.
This closes the concurrent-replay case; sspf-1 closed the sequential case. The
read path is protected by sspf-1's clear (a bumped generation alone would still
let a `Some` be read), so both fixes are load-bearing.

## Files changed
- `src-tauri/src/stream_proxy.rs` — `Entry.generation`; reuse bumps it;
  generation-guarded `store_len`; `serve_connection` captures and threads it;
  new guard test.

## Guard proof
`stream_proxy::tests::a_stale_generation_store_len_is_ignored`: after a reuse
bumps the generation, a `store_len` under the OLD generation is rejected (len
stays `None`) while a current-generation store is accepted. Executed
2026-07-05: red — with the `entry.generation == generation` check removed the
stale store lands (`left: Some(4242), right: None`, FAIL); green — with the
check the stale store is dropped. Full suite clean.

## Coder dispute (if any)
None — admitted. It is the same token-reuse-race class the plan foreshadows for
sub-slice 3's session cache, but it is a real defect in sub-slice 1's own size
cache, and the plan requires each sub-slice be independently guarded.

## Known gaps
None for the size cache. Sub-slice 3's session cache will need its own
generation-owned cleanup (already in the plan); this generation may be reused
or unified there.

## Reviewer comments
codex (codex-cli 0.142.5), `-s read-only`, JSON mode. Round 2 (2026-07-05):
**reopened** on this finding, reviewed head `08fef741`, base `adbeb86765`,
`guard_confirmed: true`. Final: **accepted** at round 4, reviewed head
`401fd1bc`, base `adbeb86765`, `guard_confirmed: true`, no comments.
