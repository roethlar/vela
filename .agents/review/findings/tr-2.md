# tr-2: Plex transcode requests ignore which version was selected

**Severity**: MEDIUM — capability and conversion are asked about the wrong copy
whenever a title has more than one Plex Media entry, which is exactly the case
the per-title quality menu is built around.
**Status**: Verified
**Branch**: none — repo policy is direct commits on `main`
**Commit**: `c08d27d` (version 1.0.15)

## Evidence

`src-tauri/src/plex_library.rs::transcode_decision` (added `499ab0b`) and
`::transcode_url` (added `bac110d`) both hardcode `mediaIndex` and `partIndex`
to `0` and take no version identity, while `PlexSource::resolve_stream_version`
and `playback_versions` already select among `detail.media` entries by
`version_id`.

## Predicted observable failure

A Plex title whose second Media entry is selected — a second copy on the same
server, or one chosen through Play Version — has its capability evaluated
against entry zero. The quality menu then lists the FIRST copy's options, and
choosing one either fails for the selected copy or transcodes and plays the
wrong version, so the user watches a different file than the one they picked.

## What

Plex addresses a copy positionally within an item, so a request that omits the
index silently means "the first one". Both new endpoints inherited that default
from the probe script they were derived from, which only ever tested
single-version titles.

## Approach

Both methods take an explicit `media_index: usize`, and the caller derives it
from the position of the selected `version_id` within `detail.media` — the same
list `playback_versions` enumerates, so the index and the version identity
cannot disagree. `partIndex` stays 0: Vela joins multi-part versions as an EDL
for direct play, and how a multi-part version should transcode is a separate
open question recorded in the plan rather than guessed at here.

## Files changed

- `src-tauri/src/plex_library.rs` — `media_index` on both methods.

## Guard proof

- `plex_library::tests::transcode_requests_target_the_selected_version` — builds
  URLs for index 0 and index 1 and asserts `mediaIndex` differs and matches.
  Red-proven 2026-07-25 from the committed state: hardcoding both request sites
  back to `0` failed on "the selected version must reach the server"; restoring
  passed. The injection compiled and the restore was verified clean.

## Coder dispute (if any)

None.

## Known gaps

`partIndex` remains 0. Multi-part (split-file) Plex versions are already a
recorded open question for transcoding in
`.agents/plans/server-transcoding.md`; this finding does not close it.

## Reviewer comments

`Reviewer: codex / (harness default model, default effort) / standard` — see
`tr-1.md` for the dispatch note; both findings came from the same review.

Reviewed head `b94fcd13ae2a6596937b57e6acdc622560e848e0`, base
`72e0f48f6c7ddeda603cea253951c4a93932e709`, both echoed and matched. Verdict:
finding raised (MEDIUM), admitted at intake. 2026-07-25 UTC.

Reviewer text: "The Plex decision request hardcodes mediaIndex and partIndex to
zero and accepts no selected version identity, even though Vela already supports
multiple Plex Media versions and the planned quality menu is defined per exact
copy. Consequently this API cannot determine capabilities for any selected
non-first version."
