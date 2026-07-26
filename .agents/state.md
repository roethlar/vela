# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **Vela 1.0.0 is published**; the shipped version is now 1.0.20. The release
  detail rotated to `docs/history/state-archive.md` (2026-07-25) and its
  canonical evidence lives in `.agents/plans/v1-release-readiness.md`.
- Product behavior remains as settled in `.agents/decisions.md`: Vela is a
  multi-server Plex/Jellyfin/experimental-Emby client, delegates HDR playback
  to external mpv, uses title-level watched state across duplicate copies, and
  offers Prefer Best, Prefer Compatible, Prefer Fastest Source, Ask Every Time,
  and per-title Play Version.
- **Intro/credits/commercial marker skipping is the ACTIVE implementation**
  (owner activated it 2026-07-25). The plan is
  `.agents/plans/skip-credits-intros-v2.md`, now Active v2 revision 7, rebased
  onto the 1.0.4 prerequisite base; the plan records the landed version sequence
  (corrected 2026-07-25 — two review-fix bumps overtook its prediction). Its
  config-integrity prerequisite is satisfied. Slice 1 is implemented, canonically
  verified, committed as
  `c7aa963` at version 1.0.5, and guard-proven: the provider-neutral marker
  model, the shared normalizer, `include_markers` on both resolve entry points,
  Plex `includeMarkers=1` on the existing selected-detail request, Jellyfin
  MediaSegments fetched concurrently with the mandatory item fetch, and Emby
  issuing no request. Fifteen injected regressions each failed for their own
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
  legacy-field preservation each red-proven separately. Slice 4's production
  flip then landed as `5dd3e35` at 1.0.10 with its kind-filter guard at
  `e58d978`/1.0.11, so the play command now derives `include_markers` from
  `skip_policies.any_enabled()` and the Settings controls render — but the
  slice is NOT done: its five behavioural E2E legs have never run (see
  `## Next`). Settled behavior is recorded in
  `.agents/decisions.md`: missing intro, credit, and commercial settings default
  to Button; the external-mpv control is genuinely clickable; and Space
  activates it only while visible, otherwise retaining its normal pause
  behavior. An unknown marker policy does not normalize: it invalidates the
  settings file under the owner-approved app-wide fail-closed recovery rule.
  Commercial ranges are in scope wherever an upstream server publishes them;
  the dated provider evidence and unsupported-provider boundary are canonical
  in the plan.
- The app-wide fail-closed settings prerequisite (`connections.json` split,
  strict boundaries, byte-exact recovery, prior-version rollback, and the Plex
  credential-path work) is COMPLETE and fully landed through version 1.0.4.
  Its slice-by-slice detail rotated to `docs/history/state-archive.md`
  (2026-07-25); canonical evidence stays in
  `.agents/plans/config-integrity-recovery.md` and
  `.agents/review/findings/cir-1.md`.

## Next

- **Transcoding slices 5 and 6 are what remain of that feature**, per
  `.agents/plans/server-transcoding.md`. Slice 5 is Automatic. Its thresholds
  are specified in the plan and its two user-visible choices are ruled (stepping
  is ONE-WAY with no step-up; at most 2 step-downs per play; a short `↓ 4 Mbps`
  mpv OSD notice). Its blocking prerequisite — the IPC reader reading any
  numeric property as a playback position, which corrupted resume points on a
  play that failed early — is FIXED at 1.0.28 (`4f7bc21`). Slice 5 is now
  unblocked and NOT started. It is also the slice that withdraws the `tr-8` gate
  in `Settings.svelte` — the guard in `tests/transcoding-ui.test.mjs` fails the
  moment a `decoder-frame-drop-count` SUBSCRIPTION appears, which is the
  reminder. Slice 6 is Emby best-effort labelling and the README Player notes
  (quality setting, one-off menu, the plain statement that converting forfeits
  HDR and drops container chapters).
- **Slice 5 is PART-BUILT.** Detection (`automatic.rs`, `5e95630`/`7e6fd02`) and
  mpv sampling (`spawn_health_sampler`, `6021e9f`) are landed and guard-proven.
  **The relaunch a verdict must trigger is NOT built**: `PlaySpec::step_down` is
  always `None`, so no play watches itself and Automatic is still inert. The
  next increment needs the same "a background thread causes a new play" plumbing
  `PlaybackAdvance` provides for EOF. Detail and the vacuous-guard lesson from
  that pass are in `.agents/plans/server-transcoding.md`.
- **Nothing in the transcoding feature has been exercised against a real
  server.** Every fix above is guarded by unit and static tests only. The
  quality menu, a real conversion, and a real teardown against the owner's Plex
  need either a playtest or the repaired E2E venue.
- **Finish marker Slice 4's behavioural verification — the slice is NOT done.**
  Its production flip landed as `5dd3e35` at 1.0.10 (PlaySpec fields, policy
  resolution and marker filtering, payload write/cleanup, arg injection,
  Settings controls, README, and the mock MediaSegments route), verified only by
  the canonical set and 278 Rust tests. The five behavioural E2E legs are
  neither written nor run: no skip button has ever rendered, been clicked, or
  been activated by Space, and no auto-skip seek has been observed **through the
  app**. The PLAYER behaviour itself is now verified: 15/15 legs in real mpv on
  the macOS host (button appears with a real hitbox, Space skips then returns to
  pausing, auto-skip seeks for intro and commercial, Off does nothing, and a
  click inside the hitbox skips while a click outside does not). Linux is needed
  only for the webview harness, not for mpv. What remains unproven is the glue:
  a real play resolving policies, fetching markers, writing the payload, and
  launching mpv with those arguments — though the kind filter itself is now
  guarded and red-proven at 1.0.11 (`e58d978`). The macOS host cannot substitute
  for the venue: Tauri WebDriver is Linux/Windows only, and config resolves to
  `~/Library/Application Support` with no XDG override, so an app run here would
  drive the owner's real settings rather than a fixture.
  `tests/e2e/scenarios/markers.mjs` is
  scoped to exactly that glue and has NEVER run. The venue never runs mpv with a
  real video output (owner, 2026-07-25), so the button, hitbox, pointer click
  and Space binding are permanently untestable there and live in the desktop mpv
  check instead — the plan's acceptance list is superseded on that point.
- **The Linux E2E venue is broken and blocks that verification.** On 2026-07-25
  it was synced (103 files checksum-verified), rebuilt, and found unable to
  render the app under WebKitWebDriver — `smoke` fails the same way as
  `markers`, so the suite gates nothing at present. Ruled out: stale tree,
  driver/WebKit version mismatch, app crash, and forced software GL. Full
  diagnosis in `.agents/machines.md`. Repairing the venue is the prerequisite
  for finishing Slice 4; it needs an owner decision, and installing `xdotool`
  there is a separate owner call.
- Original Slice 4 scope, retained for reference:
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
- **Server-side transcoding is the ACTIVE implementation** (owner activated it
  2026-07-25). `.agents/plans/server-transcoding.md` has all seven owner
  decisions ruled
  (recorded 2026-07-25 in `.agents/decisions.md`), the Plex contract verified
  against the owner's live server, and six implementation slices written
  (1.0.12-1.0.17). Slice 1 is committed as `499ab0b` at version 1.0.12: the
  quality ladder, `PlaybackOptions`, Jellyfin capability parsing, and the Plex
  decision call, with seven guards each red-proven separately. Nothing calls it
  yet by design, so the play path is unchanged; the scoped
  `#[allow(dead_code)]` markers must be removed in slice 3. Slice 2 is committed
  as `9f87475` at 1.0.13: `playback_quality` on `AppConfig` (missing means
  `original`, valid set derived from the ladder), the `MpvAdvanced` boundary,
  and the Settings > Player control, with five guards each red-proven
  separately. Both settings now state which question they answer, per the
  Prefer Compatible ruling. Still inert at play time.

  Slice 3 (`e0e5fc7`, 1.0.18) wired the play path, both transcode URL builders
  and teardown; slice 4's backend (`de80b8a`, 1.0.19) added the
  `quality_options` command. A codex review of slice 3 returned SEVEN findings,
  all admitted (`.agents/review/findings/tr-3.md`); three were fixed in
  `049ed78` (1.0.20).

  **All seven review findings are now closed, and slice 4 is complete** (owner
  go per finding, 2026-07-25). `tr-4` at 1.0.21 (`d24224b`): the transcode is
  owned in `AppState`, claimed by session id, and torn down by whichever of the
  tracker tail, the launch-failure path, or the exit sweep runs last — the exit
  sweep now WAITS, bounded by a 10s deadline. `tr-6` at 1.0.22 (`996c417`) plus
  a guard strengthening at 1.0.23 (`512d67f`): teardown classifies the answer,
  retries 429/5xx twice with backoff, reports 401/403 once, treats 404 as
  already-gone, and describes transport failures WITHOUT reqwest's `Display` —
  which embeds the full URL, so the log printed what its own comment forbade (a
  pre-existing defect found while fixing tr-6; first recorded here as a token
  leak, which was WRONG — both teardowns authenticate by header, so the exposure
  was the server address and session handle. A 2026-07-25 sweep confirmed every
  request Vela makes is header-authenticated and found no further instance). `tr-8` at 1.0.24
  (`47255a8`): `Automatic` is no longer offered; it stays selectable only for a
  config that already holds it, so no stored value is silently rewritten and no
  document is invalidated. `tr-9` at 1.0.25 (`a53da15`): `conversion_possible`
  is false for anything but exactly one part, `transcode_url` returns `None` so
  a truncating URL cannot be built at all, and the menu never offers conversion
  for a split-file version; real multi-part transcoding is DEFERRED and recorded
  as such in the plan. Slice 4's UI landed at 1.0.26 (`f236d38`) with a guard
  strengthening at 1.0.27 (`696ec7e`): quality nests under version, `Play at
  Quality >` is the else-branch so the two labels cannot co-occur, options
  resolve only when a submenu opens, and the one-off choice is validated against
  the setting's own closed set and never persisted. 30 regressions were injected
  separately across the five fixes and each failed for its own reason; two
  vacuous guards were found during those passes, strengthened, and re-proven.

  Two facts worth not rediscovering: Plex's
  `/video/:/transcode/universal/ping` and `/stop` DO NOT EXIST (both 404) and
  teardown is `DELETE /transcode/sessions/<uuid>`; and Plex filters its quality
  ladder by resolution only, never by bitrate. The 2026-07-19 Prefer Compatible
  mode is inert for single-copy libraries — acceptable, not a defect.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- Reviewer routing note for the next dispatch: the `codex` entry in
  `.agents/review/harnesses.local.json` has no `tiers` block (it predates the
  tier schema — codex-cli 0.142.5, verified 2026-07-04) and this clone has no
  `.agents/model-map.json`, so tier resolution failed closed. The owner cleared
  it on 2026-07-25 by naming `gpt-5.6-sol` at xhigh as a literal slug requiring
  no mapping. Per the playbook that is a session-only inline pin: it was NOT
  written to the harness cache or any map, and any later dispatch needs the
  owner to name it again or to confirm a durable codex tier entry.
- The Linux E2E venue outage is RESOLVED (2026-07-25): the venue was never
  broken. `--skip-build` had been reusing a binary produced by a plain `cargo
  build`, which embeds no frontend and so loads `devUrl`; the webview was
  sitting on a connection-refused page. Rebuilt with `tauri build --debug`,
  `smoke` passes. Full diagnosis and the standing caution are in
  `.agents/machines.md`.

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
- `.agents/plans/server-transcoding.md` (active implementation)
- `.agents/plans/skip-credits-intros-v2.md` (active, Slice 4 unfinished)
- `.agents/review/index.md` and `.agents/review/findings/tr-3.md`
- `.agents/plans/config-integrity-recovery.md` (landed; evidence only)
- `.agents/plans/v1-release-readiness.md`
- `README.md`, `RELEASE_NOTES.md`, and `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
