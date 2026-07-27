# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **Vela 1.0.0 is published**; the current source version is 1.0.56. The release
  detail rotated to `docs/history/state-archive.md` (2026-07-25) and its
  canonical evidence lives in `.agents/plans/v1-release-readiness.md`.
- Product behavior remains as settled in `.agents/decisions.md`: Vela is a
  multi-server Plex/Jellyfin/experimental-Emby client, delegates HDR playback
  to external mpv, uses title-level watched state across duplicate copies, and
  offers Prefer Best, Prefer Compatible, Prefer Fastest Source, Ask Every Time,
  and per-title Play Version.
- **Server transcoding implementation is landed through Slice 6.** The six
  slices landed from 1.0.12 through 1.0.52; all seven first-pass `or-*`
  findings and both later HIGH findings are fixed through 1.0.54. The first
  real Plex run exposed two HIGH findings. `tr-11` is VERIFIED / CLOSED at
  `f185449` plus exact-one guard `5c27f89`: both Plex builders select the `Web`
  HLS client profile; all local gates, clean Linux 38/38, real-Plex
  decision/session/play/teardown, independent guard proof, and final plain
  Codex review pass. HIGH `tr-10` is also VERIFIED/CLOSED in 1.0.56:
  implementation `d91e8d2` plus Claude review correction `ca15258` keeps Plex
  transcode tokens out of URLs and carries auth through the private mpv include.
  It passed explicit local gates, independent guard mutations, clean Linux E2E
  38/38, real-Plex `live-transcode` 1/1, and fresh Claude re-review PASS.
  MEDIUM `tr-12` now has draft revision 1 at
  `.agents/plans/tr-12-plex-decision-diagnostics.md`; owner ruling and approval
  remain pending. LOW `tr-13` (duplicated universal-transcode query builders)
  remains separately gated.

## Next

- **First action:** ask the owner to approve or revise `tr-12` draft revision 1,
  including the recommended inline-menu / logged-playback diagnostic split.
  Implementation needs a later explicit go. `tr-13` remains separately gated.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- No evidence blocker is known. `tr-12` implementation is owner-gated on the
  draft ruling/approval and then a separate implementation go.

## Verification

- Canonical commands live in `.agents/repo-guidance.md` (Verification). Do not
  duplicate here.
- Linux live-server venue details live in `.agents/machines.md`.
- Release verification immutable artifact hashes live in
  `.agents/plans/v1-release-readiness.md`.

## Active Sources

- `AGENTS.md` and `.agents/repo-guidance.md`
- `.agents/decisions.md`
- `.agents/machines.md`
- `.agents/push-policy.md`
- `.agents/plans/server-transcoding.md` (`tr-11` and `tr-10` closed)
- `.agents/plans/tr-11-plex-client-profile.md` (complete)
- `.agents/plans/tr-10-plex-transcode-header-auth.md` (complete)
- `.agents/plans/tr-12-plex-decision-diagnostics.md` (draft revision 1)
- `.agents/review/findings/tr-12.md` and
  `.agents/review/findings/tr-13.md` (open follow-ups from the plan review)
- `.agents/plans/skip-credits-intros-v2.md` (landed; evidence only)
- `.agents/review/index.md`, `.agents/review/findings/tr-11.md`, and
  `.agents/review/findings/tr-10.md`
- `.agents/plans/config-integrity-recovery.md` (landed; evidence only)
- `.agents/plans/v1-release-readiness.md`
- `README.md`, `RELEASE_NOTES.md`, `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
