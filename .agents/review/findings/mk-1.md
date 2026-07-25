# mk-1: An unresponsive Jellyfin marker endpoint delays mpv launch

**Severity**: MEDIUM — an optional best-effort feature can hold a play back by
the full 15-second HTTP timeout, which the user experiences as Vela hanging.
**Status**: Verified
**Branch**: none — repo policy is direct commits on `main` (AGENTS.md Git
Safety leaves branch policy to the repo)
**Commit**: `be32bde` (version 1.0.6)

## Evidence

`src-tauri/src/source/jellyfin.rs:1363` (and the same shape in `resolve_stream`)
at head `c7aa963`. Both Jellyfin resolve paths ran:

```rust
let (item, markers) = tokio::join!(item_fetch, self.markers_if_enabled(..));
let item: BaseItem = item?;
...
let info = self.client.playback_info_response(item_key).await?;
```

`tokio::join!` yields only when every future completes, and the marker request
inherited the general 15-second per-request timeout from `get_json_url`. The
mandatory `playback_info_response` call was therefore not even started until the
marker future finished.

## Predicted observable failure

`include_markers` is true, the item endpoint answers promptly, and
`/MediaSegments/{id}` accepts the connection then stalls. Vela waits ~15 seconds
before requesting playback info, so the mpv launch is delayed by the full
optional-marker timeout even though markers are documented as best-effort and
"degrade, never refuse play".

## What

Marker collection is specified as best-effort work that rides alongside the
mandatory resolve work. As implemented it was neither bounded independently of
the general timeout nor overlapped with all of the mandatory work, so a slow or
black-holing marker endpoint sat directly on the playback critical path.

## Approach

Two changes in `src-tauri/src/source/jellyfin.rs`. `media_segments` now wraps its
request in `tokio::time::timeout(MARKER_LOOKUP_TIMEOUT, ..)` (4 seconds), so a
marker endpoint that cannot answer quickly yields `[]` instead of holding the
launch — the bound is deliberately far below the 15-second general timeout
because no marker is worth a visible launch delay. Separately, both resolve
paths now join the marker future against **all** the mandatory work (item fetch
*and* playback-info) in one async block rather than against the item fetch
alone, so in the normal case markers overlap the whole resolve and add no
latency at all.

The plan's rule that markers use a non-detached future is preserved: the future
is still awaited within the resolve, just bounded.

## Files changed

- `src-tauri/src/source/jellyfin.rs` — `MARKER_LOOKUP_TIMEOUT` const;
  `media_segments` timeout wrapper; `resolve_stream` and
  `resolve_stream_version` join markers against the full mandatory async block.

## Guard proof

- `src-tauri/src/source/jellyfin.rs::media_segments_are_bounded_when_the_endpoint_stalls`
  — points the client at a server that accepts the connection and never
  responds, asserts the call returns empty markers in well under the general
  15-second timeout. Red-proven from the committed state on 2026-07-25:
  widening the wrapper's bound to 60 seconds made the call take **16.16s** and
  the assertion FAIL — which also confirms the reviewer's mechanism, since the
  unbounded path really does run to the general request timeout. Restoring the
  bound made it PASS. The injection compiled and the restore was verified
  clean.

Only the bounded-latency behavior is claimed and guarded. The join widening is
a latency optimization with no separate behavioral claim: it changes when the
marker request overlaps mandatory work, and its worst case is already covered by
the same bound.

## Coder dispute (if any)

None. The finding is correct as to mechanism and consequence.

## Known gaps

The bound means a genuinely slow-but-working marker endpoint can still add up to
4 seconds before playback in the worst case. Removing that entirely would
require a detached future, which the approved plan forbids
(`.agents/plans/skip-credits-intros-v2.md`, Jellyfin concurrency rule).

## Reviewer comments

`Reviewer: codex / gpt-5.6-sol / xhigh / (inline, session-only)` — the owner
named the slug literally on 2026-07-25 and stated no map resolution was needed;
it was not written to the harness cache or any map.

Harness: codex MCP transport (`mcp__codex__codex`), read-only sandbox,
`approval-policy: never`, `model_reasoning_effort: xhigh`.
Reviewed head `c7aa9637c34b75350c20b41c29415fa8be06526c`, base
`e7ea7dca8ee7ed730bbc22efd9df5fff8e2785f3` — both echoed back by the reviewer
and matched against the dispatched pins.
Verdict: finding raised (MEDIUM), admitted at intake. 2026-07-25 UTC.

Reviewer text: "Both Jellyfin resolution paths wait for the optional
MediaSegments future through `tokio::join!`, whose result cannot be consumed
until every future completes. Because this optional request inherits the general
15-second HTTP timeout and playback-info resolution starts only afterward, a
slow marker endpoint remains on the critical path despite markers being
described as best-effort."
