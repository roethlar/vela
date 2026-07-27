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
  Repeated unrelated Linux E2E harness races prevented a clean 39/39; see its
  plan and finding record. On 2026-07-27 the owner ruled those failures
  non-blocking for this LOW refactor rather than expanding into harness work.
  Real-Plex `live-transcode` then passed 1/1 with clean session and venue
  teardown. Owner-directed Claude Code 2.1.220 reached its isolated
  caller-drift proof over exact `735b591..81d5497` and cleaned up, but its CLI
  returned no structured verdict or transcript. The one allowed re-emission
  could not resume the non-persisted session, so `tr-13` was contested at that
  point. The owner rerouted the replacement review to Grok only with no
  model argument; Grok 0.2.112 and its one fresh retry both failed HTTP 401
  before inference. The owner then selected Kimi Code CLI 0.29.2 /
  `kimi-code/k3` at `max`; its transcript confirmed alias, server model `k3`,
  and effort, and it accepted exact `735b591..81d5497` with matching pins,
  `capability_ok:true`, `guard_confirmed:true`, and no comments. Its
  decision-only missing-`copyts` mutation made the focused guard fail for the
  intended mismatch, exact restoration passed, and the clean worktree was
  removed. `tr-13` and the server-transcoding plan are VERIFIED/CLOSED; the
  Claude and Grok attempts remain transport history.

## Next

- No implementation or review loop is active; await the owner's next scoped
  request.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- None. The unrelated continuation/mpv and `refresh` E2E flakes remain
  recorded and were not retried or repaired under `tr-13`.

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
- `.agents/plans/server-transcoding.md` (complete)
- `.agents/plans/tr-11-plex-client-profile.md` (complete)
- `.agents/plans/tr-10-plex-transcode-header-auth.md` (complete)
- `.agents/plans/tr-12-plex-decision-diagnostics.md` (complete)
- `.agents/plans/tr-13-plex-universal-query-builder.md` (complete)
- `.agents/review/findings/tr-12.md` (complete)
- `.agents/review/findings/tr-13.md` (complete) and
  `.agents/review/tr-13.contested.md` (resolved transport history)
- `.agents/plans/skip-credits-intros-v2.md` (landed; evidence only)
- `.agents/review/index.md`, `.agents/review/findings/tr-11.md`, and
  `.agents/review/findings/tr-10.md`
- `.agents/plans/config-integrity-recovery.md` (landed; evidence only)
- `.agents/plans/v1-release-readiness.md`
- `README.md`, `RELEASE_NOTES.md`, `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
