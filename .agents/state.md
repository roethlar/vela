# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **Vela 1.0.0 is published**; the current source version is 1.0.55. The release
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
  real Plex run exposed two HIGH findings. `tr-11` is implemented at
  `f185449`: both Plex builders now select the `Web` HLS client profile, all
  local gates and the clean Linux 38/38 suite pass, and the real-Plex scenario
  passed decision/session/play/teardown end to end. External code review is
  pending before closure. `tr-10` remains open: mpv still receives the Plex
  token in its transcode URL, although header-only delivery is live-proven safe.
  The plan review's nonblocking follow-ups remain `tr-12` (silent decision
  failures) and `tr-13` (duplicated universal-transcode query builders).

## Next

- **First action:** run plain `codereview codex` over exact base
  `78ace3dde59c6ee998a4525ec55520d6e49c6902` and evidence head
  `a0c513a` for `tr-11`. If accepted, close its records in one commit.
  `tr-10` remains the next repair; `tr-12` and `tr-13` remain separate.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- **`tr-11` closure awaits external code review.** The tested owner's Plex now
  transcodes successfully. `tr-10` remains a HIGH credential blocker.

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
- `.agents/plans/server-transcoding.md` (`tr-11` review pending; `tr-10` open)
- `.agents/plans/tr-11-plex-client-profile.md` (implemented and verified)
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
