# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **Vela 1.0.0 is published.** Annotated tag `v1.0.0` targets
  `06df6812d7fe81185213778669fcaa87680ac83b`; the public Latest release is
  `https://github.com/roethlar/vela/releases/tag/v1.0.0`. It contains the
  universal macOS DMG, Windows NSIS and MSI installers, Linux AppImage/deb/rpm,
  Arch package, and the verified checksum manifest.
- The release closed the native Bash 3 wrapper, fail-closed artifact inventory,
  Arch packaging, real-Plex completion/refresh, 1.0 docs/graphics, cross-platform
  package, and Windows install-over gates. Exact commits, guard red proofs,
  live-state restoration, workflow evidence, artifact hashes, and the GitHub
  permission recovery are canonical in
  `.agents/plans/v1-release-readiness.md`.
- The tag's first release jobs exposed missing GitHub release-write permission
  before publication. The future-tag fix is `9f97355`; repository workflow
  defaults remain read-only. The 1.0 release itself was created and populated
  with `gh` from the successful exact-tag-commit rehearsal, then downloaded
  and checksum-verified before publication.
- Product behavior remains as settled in `.agents/decisions.md`: Vela is a
  multi-server Plex/Jellyfin/experimental-Emby client, delegates HDR playback
  to external mpv, uses title-level watched state across duplicate copies, and
  offers Prefer Best, Prefer Compatible, Prefer Fastest Source, Ask Every Time,
  and per-title Play Version.
- Intro/credits marker skipping is the candidate next product goal, but remains
  planning-only. The hardened draft is
  `.agents/plans/skip-credits-intros-v2.md`. Settled behavior is recorded in
  `.agents/decisions.md`: missing intro/credit settings default to Button; the
  external-mpv control is genuinely clickable; and Space activates it only
  while visible, otherwise retaining its normal pause behavior. An unknown
  marker policy does not normalize: it invalidates the settings file under the
  owner-approved app-wide fail-closed recovery rule. No code has been
  implemented.

## Next

- Continue the marker plan's owner gates one at a time. Next, decide
  commercial-marker handling, followed by live IPC marker refresh. Before
  marker implementation, draft and approve the app-wide config-integrity and
  recovery work required by the new fail-closed decision, then explicitly
  activate the marker plan in state.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- Marker-skipping implementation is intentionally gated on the two remaining
  owner decisions, an approved plan for the app-wide config-integrity/recovery
  prerequisite, and explicit marker-plan activation. Current code still
  normalizes several invalid constrained values and sometimes substitutes a
  default config after load failure; that lower-authority behavior conflicts
  with the 2026-07-22 owner decision and must not be copied into this feature.

## Verification

- Canonical commands live in `.agents/repo-guidance.md` (Verification). Do not
  duplicate them here.
- Linux and live-server venue details live in `.agents/machines.md`.
- Release verification and immutable artifact hashes live in
  `.agents/plans/v1-release-readiness.md`.

## Active Sources

- `AGENTS.md` and `.agents/repo-guidance.md`
- `.agents/decisions.md`
- `.agents/machines.md`
- `.agents/push-policy.md`
- `.agents/plans/skip-credits-intros-v2.md`
- `.agents/plans/v1-release-readiness.md`
- `README.md`, `RELEASE_NOTES.md`, and `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
