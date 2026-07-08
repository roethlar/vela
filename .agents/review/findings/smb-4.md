# smb-4: Loopback stream proxy — native SMB playback and artwork

**Severity**: — (planned slice 4 of `.agents/plans/smb-native-client.md`, not a defect)
**Status**: Verified (accepted round 2; awaiting owner-gated merge)
**Branch**: `smb-native` (stacked; commit follows smb-3's `2a283aa`)
**Commit**: `f2a4640c32790aec7bb3e988d4a95cfb51de613c` (slice `f47d256` + fix-ups `602fa89`/`f2a4640`; base `2a283aa7f6e94dcd6d537afa483dd35867536620`)

## Evidence
Approved plan slice 4 (design §3): mpv has no smb:// support on the
owner's system, so native playback requires a localhost HTTP Range proxy
translating byte-range requests into SMB positioned reads.

## Predicted observable failure
Before: playing any native-SMB item errors ("lands in the next update").
After: resolve_stream returns `http://127.0.0.1:<port>/<token>`; mpv
plays, seeks (Range), and resumes; SMB `.nfo` sidecars enrich titles and
sidecar posters render via the same proxy.

## What
`stream_proxy.rs` (new, Linux-family) + the `Vfs::resolve_stream_url`
hook wiring playback and artwork through it; `smb_client.rs` regains the
positioned-read handle (`open_read`/`read_at`) and bounded `read_small`.

## Approach
- Hand-rolled HTTP/1.1 on std TCP (no new deps; plan's "smallest viable"
  choice): one thread per connection, every response `Connection: close`
  (mpv reconnects with a fresh Range on seeks; one round-trip on a LAN).
  Bounded request-head read (8 KiB, 10 s timeout).
- Range handling per the RFC 9110 subset mpv emits: `a-b`/`a-`/`-suffix`
  → 206 with Content-Range, clamped; unsatisfiable → 416 with
  `bytes */len`; multi-range/malformed → 200 full entity (specified safe
  fallback). HEAD supported; non-GET/HEAD → 405; unknown token → 404.
- Security: binds 127.0.0.1 only; token = UUIDv4 (unguessable), maps to
  exactly one file; no paths or credentials in URLs; no request logging;
  registry capped at 64 (oldest evicted), same target reuses its token so
  artwork URLs stay stable. mpv's title shows the URL — the token grants
  reads of one file, loopback-only, for the app's lifetime; accepted and
  documented in the module header.
- Each streaming response opens its own SMB connection (parallel-safe;
  session setup per seek is the plan's accepted cost, readahead noted as
  follow-up if seeks stutter).
- Playback: native `resolve_stream` → proxy URL, `ProgressTarget::None`
  (same client-side recents semantics as local files).
- Sidecars: `SmbVfs::read_to_string` now does a bounded (1 MiB) native
  read, so `.nfo` enrichment works; `metadata::local_artwork` routes
  provider artwork through `resolve_stream_url` (webview CSP already
  allows `http:` images) and drops it if unresolvable, instead of
  emitting an unloadable provider path.

## Files changed
- `src-tauri/src/stream_proxy.rs` — new (proxy + range parser + tests)
- `src-tauri/src/smb_client.rs` — `SmbReadHandle`, `read_small`
- `src-tauri/src/source/vfs.rs` — `resolve_stream_url` default hook
- `src-tauri/src/source/smb_vfs.rs` — hook impl + sidecar reads
- `src-tauri/src/source/local.rs` — native resolve_stream via the hook
- `src-tauri/src/source/metadata.rs` — artwork through the hook
- `src-tauri/src/lib.rs` — module registration

## Guard proof
- `stream_proxy::tests::serves_full_head_range_and_errors_end_to_end`:
  real TCP against the running proxy with an in-memory target — status
  lines, Content-Range, body bytes for 200/206/HEAD, plus 416/404/405.
  (The `Target::Mem` variant exists exactly so HTTP semantics are testable
  without an SMB server.)
- `range_parsing_covers_mpv_forms`: mutation-proven — changing the end
  clamp to `min(len)` fails it; restored, all 70 pass.
- `registry_reuses_tokens_and_evicts_oldest`: token stability + cap.
- Tests serialize on a shared lock (global registry; parallel eviction
  would race).

## Coder dispute (if any)
None.

## Known gaps
- No live SMB streaming test in-session (no credentials in config); the
  owner playtest before merge covers play/seek/resume against the real
  NAS. The proxy's HTTP layer is fully covered by the Mem-target tests.
- Keep-alive deliberately unsupported; if real mpv seek latency
  disappoints, revisit per the plan's readahead note rather than growing
  the hand-rolled server.
- Proxy port changes per app run; recents entries persist only provider
  paths (rating keys), not URLs, so restarts are unaffected.

## Reviewer comments
Round 1 — reopened. Reviewer: codex (codex-cli 0.142.5); reviewed
`f47d256…`, base `2a283aa…`. 2026-07-04 (UTC). guard_confirmed: **true**
(range-clamp mutation failed the parser test; restore passed 70).
Findings, both accepted as correct:
1. `metadata.rs:223` — sidecar artwork minted as loopback token URLs
   during enrichment is persisted via the listing cache (and recents), so
   restarts serve dead `127.0.0.1:<old-port>/<old-token>` posters until
   revalidation.
2. `stream_proxy.rs:46` — the 64-token registry is smaller than one
   poster-rich folder; a large listing evicts live playback/artwork
   tokens → spurious 404s.
Fix direction (round 2): artwork leaves the token proxy entirely —
stable `velasmb://<mount-id>/<path>` custom URI scheme served by a
Linux-only Tauri protocol handler (config-validated mount, normalized
path, image-extension whitelist, bounded read). The token registry then
holds playback targets only, where the cap is ample.

Round-1 fix-up (coder), 2026-07-04:
- Artwork moved off the token proxy onto the stable `velasmb://` scheme
  (`602fa89` + `f2a4640`): async Tauri protocol handler, config-validated
  mount, normalized path, image-extension whitelist (mutation-proven),
  10 MiB bounded read, CSP `velasmb:`; `Vfs::artwork_ref` replaces
  artwork-over-proxy; token registry is playback-only.
- Process incident, recorded for honesty: the first fix-up commit
  (`602fa89`) was BROKEN — during its mutation proof, `git checkout
  <file>` reverted uncommitted work instead of the mutation, and the
  post-restore verification silently didn't run. Caught by re-checking
  the committed tree; corrected in `f2a4640` with verification re-run
  from a clean state. Lesson applied: mutation proofs only against a
  committed baseline.

Round 2 — accepted.
- Reviewed SHA `f2a4640c32790aec7bb3e988d4a95cfb51de613c`, same base.
  2026-07-04 (UTC). Verdict: **accepted**; guard_confirmed: **true**
  (both mutations FAIL-then-PASS in the reviewer's isolated checkout;
  71 green before and after).
- Comments: velasmb artwork handling, async protocol path, CSP, and
  register_smb call paths reviewed; no remaining artwork proxy-token
  minting or restart-unstable SMB poster path found.
