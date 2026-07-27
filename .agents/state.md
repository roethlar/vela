# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **Vela 1.0.0 is published**; the current source version is 1.0.58. The release
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
  MEDIUM `tr-12` is VERIFIED/CLOSED in 1.0.57 at `9cde6b2`: safe typed decision
  failures reach the quality-menu alert, explicit tiers retain a logged
  Original fallback, and valid refusals stay quiet. All local/mutation gates,
  Linux E2E 39/39, and real-Plex `live-transcode` 1/1 passed; owner-directed
  Claude accepted exact `a7d792e..9cde6b2` with an independent guard proof and
  no comments. LOW `tr-13` implementation landed at `81d5497` in 1.0.58: one
  typed, token-blind Plex universal-transcode query contract now serves both
  production endpoints, and the local gate plus six isolated mutations passed.
  It is not verified/closed because repeated unrelated Linux E2E harness races
  prevented the required clean 39/39; see its plan and finding record.

## Next

- **First action:** owner decides whether to authorize a separate narrow plan
  to stabilize the pre-existing continuation/mpv and `refresh` E2E races. Then
  resume `.agents/plans/tr-13-plex-universal-query-builder.md` at the required
  full Linux 39/39 gate before live Plex or Claude review.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- `tr-13` is blocked at Linux verification: unchanged continuation scenarios
  intermittently receive `mpv: property unavailable`, while `refresh`
  repeatedly loses its one-shot mock hold window and once held WebDriver for
  38 minutes. The repo's stall threshold is crossed; another unchanged retry
  is not authorized evidence.

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
- `.agents/plans/server-transcoding.md` (`tr-11`, `tr-10`, and `tr-12` closed;
  `tr-13` open)
- `.agents/plans/tr-11-plex-client-profile.md` (complete)
- `.agents/plans/tr-10-plex-transcode-header-auth.md` (complete)
- `.agents/plans/tr-12-plex-decision-diagnostics.md` (complete)
- `.agents/plans/tr-13-plex-universal-query-builder.md` (approved; active)
- `.agents/review/findings/tr-12.md` (complete)
- `.agents/review/findings/tr-13.md` (open follow-up from the plan review)
- `.agents/plans/skip-credits-intros-v2.md` (landed; evidence only)
- `.agents/review/index.md`, `.agents/review/findings/tr-11.md`, and
  `.agents/review/findings/tr-10.md`
- `.agents/plans/config-integrity-recovery.md` (landed; evidence only)
- `.agents/plans/v1-release-readiness.md`
- `README.md`, `RELEASE_NOTES.md`, `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
