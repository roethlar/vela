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

## OPEN FINDINGS

### tr-4 (HIGH) — teardown is detached and can be lost at exit

`commands.rs` spawns the teardown as a detached task from the playback-end
callback. The exit handler kills mpv and returns, so the runtime can terminate
before the DELETE is sent or completes, leaving an encoder running. Needs an
owned active-session record and an awaited shutdown path, not a fire-and-forget
spawn.

### tr-6 (MEDIUM) — teardown ignores the HTTP response

Both providers check only transport errors and never apply `error_for_status`,
so a 401, 429 or 5xx answer is treated as success: nothing is logged and nothing
is retried while the transcode keeps running.

### tr-8 (MEDIUM) — Automatic promises unimplemented behaviour

`Automatic` is selectable in Settings but nothing observes mpv's decoder drops
or cache starvation and nothing steps down. This is the same class as tr-1: a
shipped option that does nothing. Either withhold the value until slice 5 lands
it, or land slice 5 before shipping.

### tr-9 (MEDIUM) — Plex multi-part media truncates under transcode

Every transcode URL hardcodes `partIndex=0`, while direct play joins all parts
as an EDL. A split-file version would transcode only its first part and end at
that boundary. The plan already records multi-part transcoding as an open
question; shipping silent truncation is worse than refusing, so this needs a
decision before the feature is complete.
