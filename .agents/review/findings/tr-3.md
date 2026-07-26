# tr-3..tr-9: transcoding slice 3 review

Seven findings from one review; recorded together because they share a range,
a dispatch, and a theme. Reviewer: `codex` at its own default model and effort
(owner: "codereview with codex, no model or effort specified"), MCP transport,
read-only sandbox, over exact range
`b94fcd13ae2a6596937b57e6acdc622560e848e0..e0e5fc7aebcd00af262f761e4f1e6546fb738802`.
Schema-valid verdict, both pins echoed. 7 candidates, 7 admitted, 0 declined.
2026-07-25 UTC.

Reviewer summary: "No. The default Original path can silently transcode, and
several teardown paths can leave server encoders running; Automatic playback and
Plex split-file transcoding are also incomplete."

**ALL SEVEN ARE CLOSED as of 2026-07-25** (versions 1.0.20 through 1.0.25, owner
go per finding). No follow-up external review has run on the fixes; that remains
open. Nothing here has been exercised against a real server.

## tr-3 (HIGH) — Original could silently transcode. FIXED `049ed78` (1.0.20)

`PlaybackOptions::resolve` delegated Original to `direct_or_best_available`,
which picked a conversion whenever the server reported no direct-play
capability. Jellyfin's `SupportsDirectPlay`/`SupportsDirectStream` are optional,
so a server that omits them while advertising transcoding would convert for a
user still on the default — losing HDR and container chapters they never
offered up.

The author introduced that fallback on the reasoning that "refusing to play is
worse than converting". That trade was never put to the owner and it contradicts
the stated contract that users who do not want conversion keep exactly the
playback they have. **Original and Automatic are now unconditional**, and
`direct_or_best_available` is deleted rather than left to be reached again.

Guard: `source::tests::original_never_converts_even_without_reported_direct_play`
plus the `(false, true, 1080)` shape added to
`original_is_the_untouched_file_for_every_shape_of_copy`.

## tr-7 (MEDIUM) — redundant PlaybackInfo on the Original path. FIXED `049ed78`

Jellyfin resolution called `playback_options_for` unconditionally, issuing a
second PlaybackInfo request on every play including Original, whose 15-second
timeout could delay an ordinary launch. This also falsified the author's own
report to the owner that Original was "byte-identical, no capability request,
no extra round trip" — true for Plex, false for Jellyfin. The capability lookup
now happens only when a real tier is requested.

## tr-5 (HIGH) — sessionless Jellyfin transcode. FIXED `049ed78`

`deliver` built a transcode URL even when PlaybackInfo omitted
`PlaySessionId`, recording `None` as the teardown handle and so starting a
delivery nothing could ever stop. It now refuses to transcode without a session
and plays direct instead.

## tr-4 (HIGH) — teardown was detached and could be lost at exit. FIXED `d24224b` (1.0.21)

`commands.rs` spawned the teardown as a detached task from the playback-end
callback. The exit handler kills mpv and returns, so the runtime could terminate
before the DELETE was sent, leaving an encoder running.

The session is now owned in `AppState` as `active_transcode`, registered before
launch, and claimed by session id so a superseded play's tail cannot stop the
encoder the user is now watching. Three paths can be the last to run and each
issues the teardown itself: the tracker tail (blocking, on its own thread), the
launch-failure path (awaited — no tracker ever runs there), and the exit sweep
(blocking, and it WAITS, unlike the local mpv kill beside it). Registration
returns whatever it displaced so the superseding play stops that encoder too.
Bounded by a 10s deadline so an unreachable server delays shutdown rather than
blocking it.

Guards: five Rust tests in `commands::tests` (exit-sweep drain, session-matched
claim, displaced record, single claimant, deadline) plus five static wiring
assertions in `tests/transcoding-ui.test.mjs`. Ten regressions injected
separately, each failing for its own reason.

## tr-6 (MEDIUM) — teardown ignored the HTTP response. FIXED `996c417` (1.0.22)

Both providers checked only transport errors, so a 401, 429 or 5xx answer was
treated as success: nothing logged, nothing retried, encoder still running.

`source::stop_transcode_request` now classifies the answer — 2xx and 404 settle
(404 means the session is already gone), 429 and 5xx retry twice with 200ms/600ms
backoff, other 4xx are reported once — and returns the outcome so a settled
teardown is distinguishable from an abandoned one.

**A second defect was found while fixing this one and is fixed here too.** The
old code logged `{error}` for transport failures under a comment promising
"Never print the URL". reqwest 0.13's `Display` renders the full request URL, so
that line printed exactly what the comment forbade.
`describe_transport_failure` is now built from the error's own predicates and
can never contain a URL; a guard asserts the premise (the raw error DOES render
the URL) alongside the fix.

**CORRECTION (2026-07-25).** This was first recorded here as leaking the Plex
`X-Plex-Token` and the Jellyfin `api_key`. **That overstated it and was wrong.**
Both teardown requests authenticate with HEADERS, so their URLs carry no
credential: what the log exposed was the server address and the transcode
session handle. The fix stands and the comment it restored was still being
violated, but the severity claim was not verified before it was written down.
The sweep that established this is recorded below.

**Sweep for the same class elsewhere (2026-07-25): none found.** Every HTTP
request VELA ITSELF makes sends the Plex token via an `X-Plex-Token` header and
the Jellyfin/Emby token via `auth_headers()` — `plex_library.rs`, `plex_api.rs`
(where `.query(&params)` carries only non-credential parameters), `jellyfin.rs`,
and `artwork.rs` alike. No reqwest error Vela can produce has a token in its
URL, so no other `{error}` print can leak one. The query-string token exposure
that does exist is confined to URLs handed to mpv (Plex transcode and Jellyfin
`master.m3u8`) and to the webview (Jellyfin poster/backdrop), which is the
accepted local-only exposure recorded in `.agents/repo-guidance.md` — those URLs
are never requested by Vela and so never appear in a reqwest error.

Guards: six tests in `source::teardown_tests`, four of them driving a loopback
server that counts requests. Four regressions injected separately.

**A guard-the-wiring sweep on 2026-07-25 found this fix's WIRING unguarded.**
Deleting the Plex call site to `stop_transcode_request` left every test green:
the classifier, the retries and the credential-free failure text were all proven
in isolation, and nothing proved a teardown reached them. Same defect class that
shipped two dead behaviours in slice 5. Closed by asserting both providers' call
sites, red-proven separately for each. The sweep also covered tr-4, tr-9 and
slice 4, whose wiring was already guarded — this was the only gap. The post-commit
pass found the 404 guard VACUOUS — request count alone cannot separate "settled"
from "refused" — so `stop_transcode_request` was made to return its outcome and
the guard re-proven (`512d67f`, 1.0.23).

## tr-8 (MEDIUM) — Automatic promised unimplemented behaviour. FIXED `47255a8` (1.0.24)

Owner ruled 2026-07-25: withhold the value rather than land slice 5 first.

`Automatic` is no longer offered by the Settings picker. It remains selectable
ONLY when the stored value already is `automatic`, labelled "not implemented
yet; plays as Original" — so opening Settings never silently rewrites a stored
value, and the config layer still accepts it so no existing document is
invalidated and a rollback build still honours it. At play time it already
resolved to Original (tr-3). The help text no longer describes a step-down.

Guards: one test in `tests/transcoding-ui.test.mjs`, including an assertion that
FAILS once a decoder-drop observer appears — the reminder to withdraw the gate
in slice 5. Three regressions injected separately.

## tr-9 (MEDIUM) — Plex multi-part media truncated under transcode. FIXED `a53da15` (1.0.25)

Owner ruled 2026-07-25: refuse to convert, defer real multi-part transcoding.

`PlexLibrary::conversion_possible` is true only for exactly one part.
`transcode_url` checks it and returns `None`, so a truncating URL cannot be
constructed at all rather than depending on caller discipline.
`playback_options` reports no transcoding for such a version, so the menu never
offers it, and it skips the decision round trip whose answer could not change
the outcome. A Settings-level quality request degrades to Original with a log
naming the reason. The deferral is recorded in
`.agents/plans/server-transcoding.md`.

Guards: two Rust tests in `plex_library::tests` and two static assertions.
Five regressions injected separately.
