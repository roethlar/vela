# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **Vela 1.0.0 is published**; the current source version is 1.0.54. The release
  detail rotated to `docs/history/state-archive.md` (2026-07-25) and its
  canonical evidence lives in `.agents/plans/v1-release-readiness.md`.
- Product behavior remains as settled in `.agents/decisions.md`: Vela is a
  multi-server Plex/Jellyfin/experimental-Emby client, delegates HDR playback
  to external mpv, uses title-level watched state across duplicate copies, and
  offers Prefer Best, Prefer Compatible, Prefer Fastest Source, Ask Every Time,
  and per-title Play Version.
- **Server transcoding implementation is landed through Slice 6.** The six
  slices landed from 1.0.12 through 1.0.52; all seven first-pass `or-*`
  findings and both later HIGH findings are fixed through 1.0.54. The one live
  review finding still open is `tr-10`: a Plex transcode URL gives mpv the
  token in its URL instead of in request headers. Canonical implementation
  evidence is in `.agents/plans/server-transcoding.md`; the live finding is
  `.agents/review/findings/tr-10.md`.

## Next

- **First action:** run `npm run e2e:live transcode`, then directly verify
  whether Plex accepts `X-Plex-Token` headers for the HLS playlist and segment
  requests. That evidence decides the safe `tr-10` repair; no code change or
  live run is authorized by this catchup.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- None. `tr-10` is open work, not externally blocked; the exact four-command
  live-control sudoers allowlist is present on the Plex host.

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
- `.agents/plans/server-transcoding.md` (landed; `tr-10` remains open)
- `.agents/plans/skip-credits-intros-v2.md` (landed; evidence only)
- `.agents/review/index.md` and `.agents/review/findings/tr-10.md`
- `.agents/plans/config-integrity-recovery.md` (landed; evidence only)
- `.agents/plans/v1-release-readiness.md`
- `README.md`, `RELEASE_NOTES.md`, `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
