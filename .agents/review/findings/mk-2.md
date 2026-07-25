# mk-2: Asking Plex for markers can be the reason a play fails

**Severity**: MEDIUM — an optional feature is fused into the mandatory request,
so a server that rejects the marker parameter loses playback entirely rather
than losing markers.
**Status**: In progress
**Branch**: none — repo policy is direct commits on `main` (AGENTS.md Git
Safety leaves branch policy to the repo)
**Commit**: `2971672` (version 1.0.7)

## Evidence

`src-tauri/src/source/plex.rs:1318` at head `c7aa963`. `resolve_stream_version`
built its fetch closure as `lib.get_item_detail(item_key, include_markers)` and
retried once through `self.rediscover()`. Both attempts carry
`includeMarkers=1`, and there is no markerless attempt anywhere on the path, so
the `?` on the second attempt fails stream resolution outright.

## Predicted observable failure

`include_markers` is true and a Plex server errors on
`/library/metadata/{id}?includeMarkers=1` while still answering the plain
metadata request. Selecting that item produces a playback error and mpv never
starts — on a server where playback would have worked before the marker
parameter existed.

## What

The approved plan attaches Plex markers to the existing mandatory detail
response specifically to avoid a third request, and reasoned that this leaves
"no independent marker error to propagate". That reasoning holds for *parsing*
but not for the *request*: adding a query parameter creates a new failure mode
where none existed, and that failure mode was allowed to fail the play. This
contradicts the same plan's global contract that a provider marker failure must
degrade to an empty marker list and never refuse playback.

## Approach

The fallback is placed in `PlexLibrary::get_item_detail` rather than in the
source's rediscovery logic, so it covers every caller and leaves the existing
`ensure_ready` / one-rediscovery-retry structure untouched. When the request was
made with `include_markers` and it fails, the method retries the identical
request once without the marker parameter and returns that response with no
markers. A failure of the plain request propagates exactly as before.

This does not add a request to the success path — the extra attempt exists only
on a failure that previously ended the play.

## Files changed

- `src-tauri/src/plex_library.rs` — `get_item_detail` markerless retry, with the
  request construction factored so both attempts share one code path.

## Guard proof

- `src-tauri/src/plex_library.rs::item_detail_falls_back_when_the_marker_request_is_rejected`
  — a server that answers the `includeMarkers=1` request with 500 and the plain
  request with a valid detail body. Asserts the call succeeds, returns no
  markers, and that the second request line carries no `includeMarkers`.
  Red-proven from the committed state on 2026-07-25: disabling the fallback
  guard made the call fail with `reqwest::Error { kind: Status(500, ..) }` and
  the assertion FAIL; restoring it made it PASS. The injection compiled and the
  restore was verified clean.

One implementation note worth carrying: the retry cannot hold the original
`Box<dyn StdError>` across its await, because that type is not `Send` and the
enclosing futures must be. Only the error's message survives the retry.

## Coder dispute (if any)

None on the defect. One scope note recorded rather than disputed: the plan's
Plex paragraph asserts there is "no independent marker error to propagate",
which this fix shows to be inaccurate for the request itself. That sentence is
author-level rationale in the plan, not an owner decision in
`.agents/decisions.md`, so correcting it alongside the fix overturns no ruling.

## Known gaps

The trigger is conditional: `includeMarkers` is a documented Plex parameter and
Plex generally ignores unknown query parameters, so a server that fails only on
the marker-bearing request is plausible but unobserved. The fix is cheap and
strictly fail-safe, so it is worth carrying regardless.

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

Reviewer text: "Plex marker retrieval is fused into the mandatory
selected-detail request without a markerless fallback. If `includeMarkers=1`
itself causes an HTTP failure, the rediscovery path repeats the same
marker-bearing request and ultimately fails stream resolution, contradicting the
provider-neutral contract that marker failures should degrade to an empty marker
list rather than fail playback."
