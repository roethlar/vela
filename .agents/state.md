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
- **Intro/credits/commercial marker skipping is the ACTIVE implementation**
  (owner activated it 2026-07-25). The plan is
  `.agents/plans/skip-credits-intros-v2.md`, now Active v2 revision 7 with its
  slice version sequence rebased onto the 1.0.4 prerequisite base: the four code
  slices land as 1.0.5 through 1.0.8. Its config-integrity prerequisite is
  satisfied. Slice 1 is implemented, canonically verified, committed as
  `c7aa963` at version 1.0.5, and guard-proven: the provider-neutral marker
  model, the shared normalizer, `include_markers` on both resolve entry points,
  Plex `includeMarkers=1` on the existing selected-detail request, Jellyfin
  MediaSegments fetched concurrently with the mandatory item fetch, and Emby
  issuing no request. The play command still passes `include_markers = false`;
  nothing reads markers until the Slice 3 config boundary and the Slice 4
  product flip land. Fifteen injected regressions each failed for their own
  reason and were restored from the committed state. External review then ran
  and returned two admitted MEDIUM findings, both about best-effort marker I/O
  on the playback critical path: `mk-1` bounded the Jellyfin marker lookup and
  overlapped it with all mandatory work (`be32bde`, 1.0.6), and `mk-2` made a
  failed Plex marker-bearing detail request retry once without the parameter
  (`2971672`, 1.0.7). Both repairs are independently red-proven and the full
  canonical dual-side set passed at 1.0.7 with 271 Rust tests; no follow-up
  review ran, so no clean verdict is claimed. The full evidence is canonical in
  the plan and in `.agents/review/findings/mk-1.md` and `mk-2.md`. Settled
  Slice 2 is committed as `42ab254` at version 1.0.8: the Vela-authored MIT
  `vela-markers.lua` plus its `PROVENANCE.md` entry. It was verified against
  real mpv 0.41.0 (payload read, parse, `loaded` property, self-unlink, and both
  inert degrade paths) because no automated harness covers Lua; button
  rendering, the hitbox, the Space binding, seek, and the entry latch are
  deferred to Slice 4's real-app E2E as that slice specifies, and no repo test
  guards this file yet. Slice 3 is committed as `f62345d` at version 1.0.9: the
  closed `SkipPolicy` enum, the three `AppConfig` fields, and the `MpvAdvanced`
  get/set boundary, with missing-field defaulting, unknown-value rejection, and
  legacy-field preservation each red-proven separately. No Settings control
  renders the policies yet, by design. Settled behavior is recorded in
  `.agents/decisions.md`: missing intro, credit, and commercial settings default
  to Button; the external-mpv control is genuinely clickable; and Space
  activates it only while visible, otherwise retaining its normal pause
  behavior. An unknown marker policy does not normalize: it invalidates the
  settings file under the owner-approved app-wide fail-closed recovery rule.
  Commercial ranges are in scope wherever an upstream server publishes them;
  the dated provider evidence and unsupported-provider boundary are canonical
  in the plan.
- The app-wide fail-closed settings prerequisite is COMPLETE at
  `.agents/plans/config-integrity-recovery.md`. It specifies independent
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
- Slice 3 production is implemented, independently reviewed, canonically
  verified, and committed at version 1.0.4 as `21ecbe8`. Plex artwork,
  progress, timeline, and playback now keep credentials in backend/header
  paths; legacy persisted Plex artwork is converted or removed; provider Part
  keys containing the active credential fail closed; and mpv's private
  per-launch include is cleaned on partial write, replacement, confirmed exit,
  and app exit. Both independent review passes returned findings (two HIGH,
  five MEDIUM, five LOW total), every finding was admitted and resolved, and no
  clean verdict is claimed. Canonical local verification passed with 51 Node
  tests and 259 Rust tests; checksum-identical native Windows passed 255/255
  after one nonreproducible transient history-test failure, and the rebuilt
  Linux real app passed 37/37 E2E scenarios.
- Slice 3's post-commit guard pass is complete (2026-07-24). Beyond the
  restored regressions that proved progress and timeline header auth; settings
  and playlist legacy-artwork sanitation; embedded provider-Part credential
  refusal; frontend protocol conversion and Windows CSP; artwork dimension,
  MIME, traversal, query, header-auth, redirect, declared-size, and streamed
  size bounds; mpv ACL-before-write, partial-write cleanup, process-query
  retention/reaping, replacement ordering, and exit-queue cleanup; discovery
  body nonreflection; and exact/embedded mock-log redaction, the three
  real-app multiplex behaviors were red-proven separately on the Linux E2E
  venue: a transcode query token failed the Plex mock contract, and
  query-token progress and query-token timeline each failed the
  source-token-header assertion. Every regression was restored from its
  committed state and reran green; one restored-green run needed an immediate
  rerun after a transient Settings-dialog timeout on the identical binary.
  Closeout verification passed with the exact Node/npm toolchain, 51 Node
  tests, 259 Rust tests, and the rebuilt Linux real app at 37/37 E2E
  scenarios. The full evidence paragraph is canonical in
  `.agents/plans/config-integrity-recovery.md`; the docs-only closeout
  (evidence plus this state entry) landed as `8b550d6`. That work has since
  reached `origin`; push policy remains ASK (`.agents/push-policy.md`).
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

- Implement marker-skipping Slice 4 per
  `.agents/plans/skip-credits-intros-v2.md` — the atomic product flip, and the
  first slice where any of this is user-visible. Pass the resolved policy into
  selected resolution, policy-filter the returned markers, add the `PlaySpec`
  policy/marker/script fields, resolve `vela-markers.lua` through the same
  Resource resolver autocrop uses, write and clean the private per-launch
  payload non-fatally, inject policy args plus the child-only environment, and
  add all three Settings → Player controls in the same commit that makes them
  work. Extend the Jellyfin mock with the real MediaSegments route, add the five
  behavioral E2E legs, update the README Player notes, and bump to 1.0.10. Play
  must still succeed with a missing script, empty markers, a marker endpoint
  failure, a payload write failure, or a payload parse failure. Red-prove every
  behavior the E2E claims separately; this slice needs the Linux E2E venue.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- No blocker. Marker Slice 1's external review is complete and both findings are
  closed. Routing note for the next dispatch: the `codex` entry in
  `.agents/review/harnesses.local.json` has no `tiers` block (it predates the
  tier schema — codex-cli 0.142.5, verified 2026-07-04) and this clone has no
  `.agents/model-map.json`, so tier resolution failed closed. The owner cleared
  it on 2026-07-25 by naming `gpt-5.6-sol` at xhigh as a literal slug requiring
  no mapping. Per the playbook that is a session-only inline pin: it was NOT
  written to the harness cache or any map, and any later dispatch needs the
  owner to name it again or to confirm a durable codex tier entry.
- Config-integrity/recovery has no unresolved owner or review blocker — that
  plan is fully landed, verified, and guard-proven.

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
