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
- The app-wide fail-closed settings prerequisite is the active implementation
  at `.agents/plans/config-integrity-recovery.md`. It specifies independent
  strict boundaries and targeted byte-exact recovery for settings and active
  server connections. Active connection records and plaintext tokens move to
  private `connections.json`; valid connections survive a settings reset
  without reauthorization. The owner rejected an OS credential vault and
  app-managed pretend encryption: owner-account file/backup permissions,
  redacted runtime handling, private request headers, and removal of Plex token
  URLs/query strings are the security boundary. Unknown fields invalidate only
  their whole owning file; documented legacy rollback fields and non-settings
  media payloads remain compatible. Slice 1 is implemented and canonically
  verified at version 1.0.1: active connections now live in private
  `connections.json`, startup and runtime fail closed behind the two-file gate,
  a valid combined 1.0.0 config splits only after an exact verified backup, and
  invalid combined settings are not mined. Native Windows ACL validation and
  the checksum-matched Linux real-app suite passed. Slice 1 landed as `016a958`;
  its mandatory post-commit guard regressions were injected, failed for their
  intended reasons, restored, and rerun green. A vacuous source-write static
  guard found during that pass was strengthened and independently red-proven.
- Slice 2 is implemented and canonically verified at version 1.0.2. Invalid
  settings and connections now offer real Rename/Reconnect and Exit buttons;
  recovery uses an exact private no-replace rename and targeted validated
  default while leaving the other file and playlists unchanged. Damaged legacy
  combined settings yield no connection data and require reconnection. A
  private strict recovery record keeps crashes after the user's click blocked
  across restart and resumes only an exact unambiguous transaction state.
  Checksum-matched Linux real-app coverage passed 35/35, including click, Space,
  Exit no-write, restart, preserved-connection, reconnect, and crash-resume
  cases; native Windows no-replace, ACL, recovery, and resume tests passed.
  Slice 2 landed as `0c9b48f`. Nine behavior guards were independently
  red-proven and restored. A vacuous busy-disabled button check found during
  that pass was strengthened and then failed for the intended regression.
- Slice 2A is implemented, canonically verified, committed, and independently
  red-proven at version 1.0.3. Settings and connections independently retain
  the three newest private, distinct, strictly valid prior versions. A
  damaged-file screen shows all available versions newest first as real dated
  buttons while retaining fresh-file recovery and Exit. Rollback is bound to
  the selected whole file/version, preserves the exact damaged current file
  first, and leaves the other durable file and playlists untouched.
  Checksum-identical native Windows tests and the rebuilt Linux real-app suite
  passed. Production landed as `b09b610`; the guard pass found and strengthened
  three insufficient tests in `ee79573`, `b8d2860`, and `ac65b0f`, then proved
  their exact regressions red and restored green.
- The required plan `openreview` ran over exact range `7a4b5b0..bf3730a` with
  Claude Code 2.1.218 / `claude-opus-4-8` at max and admitted one MEDIUM finding,
  `cir-1`. The owner resolved it on 2026-07-23: damaged settings are renamed
  whole and replaced or Vela exits; damaged connections are renamed and enter
  reconnection or Vela exits; a damaged legacy combined config is not mined for
  connection records and therefore also requires reconnection. Plan revision 4
  records the repair. The owner declined a follow-up Claude review and
  explicitly activated implementation on 2026-07-23; no clean follow-up verdict
  is claimed.

## Next

- Implement Slice 3 Plex token exposure hardening and closeout.
  After all prerequisite slices land, explicitly activate the marker plan
  before marker implementation.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- Marker-skipping implementation remains intentionally gated on completing the
  app-wide config-integrity/recovery prerequisite and explicit marker-plan
  activation.
- Config-integrity/recovery has no unresolved owner or review blocker.

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
- `.agents/review/index.md` and `.agents/review/findings/cir-1.md`
- `.agents/plans/skip-credits-intros-v2.md`
- `.agents/plans/v1-release-readiness.md`
- `README.md`, `RELEASE_NOTES.md`, and `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
