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
  `.agents/decisions.md`: missing intro, credit, and commercial settings default
  to Button; the external-mpv control is genuinely clickable; and Space
  activates it only while visible, otherwise retaining its normal pause
  behavior. An unknown marker policy does not normalize: it invalidates the
  settings file under the owner-approved app-wide fail-closed recovery rule.
  Commercial ranges are in scope wherever an upstream server publishes them;
  the dated provider evidence and unsupported-provider boundary are canonical
  in the plan. No code has been implemented.
- The app-wide fail-closed settings prerequisite now has a planning-only draft
  at `.agents/plans/config-integrity-recovery.md`. It specifies independent
  strict boundaries and targeted byte-exact recovery for settings and active
  server connections. Active connection records and plaintext tokens move to
  private `connections.json`; valid connections survive a settings reset
  without reauthorization. The owner rejected an OS credential vault and
  app-managed pretend encryption: owner-account file/backup permissions,
  redacted runtime handling, private request headers, and removal of Plex token
  URLs/query strings are the security boundary. Unknown fields invalidate only
  their whole owning file; documented legacy rollback fields and non-settings
  media payloads remain compatible. No code has been implemented.

## Next

- Externally review and explicitly activate the app-wide
  config-integrity/recovery plan, then implement and land that prerequisite.
  Afterward, explicitly activate the marker plan in state before marker
  implementation.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- Marker-skipping implementation is intentionally gated on an approved plan for
  the app-wide config-integrity/recovery prerequisite and explicit marker-plan
  activation. Current code still
  normalizes several invalid constrained values and sometimes substitutes a
  default config after load failure; that lower-authority behavior conflicts
  with the 2026-07-22 owner decision and must not be copied into this feature.
- Config-integrity/recovery implementation is gated on required external plan
  review and explicit activation in state.

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
- `.agents/plans/config-integrity-recovery.md`
- `.agents/plans/skip-credits-intros-v2.md`
- `.agents/plans/v1-release-readiness.md`
- `README.md`, `RELEASE_NOTES.md`, and `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
