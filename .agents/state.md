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
  onto the 1.0.4 prerequisite base; the plan's own predicted version sequence
  (1.0.5-1.0.8) was overtaken by the two review-fix bumps and is stale there —
  the landed versions are named per slice below. Its config-integrity
  prerequisite is satisfied. Slice 1 is implemented, canonically verified,
  committed as
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

- **Close the four open transcoding findings before anything else in that
  feature.** Detail in `.agents/review/findings/tr-3.md`; all four came from the
  2026-07-25 codex review of slice 3 and are admitted, not disputed.
  1. `tr-4` (HIGH) — teardown is a detached `tauri::async_runtime::spawn` from
     the playback-end callback in `commands.rs`, and app exit kills mpv and
     returns, so the DELETE can be lost and a transcode left running on the
     user's server. Needs an owned active-session record and an awaited
     shutdown path.
  2. `tr-6` (MEDIUM) — both providers' teardown checks only transport errors,
     never `error_for_status`, so 401/429/5xx reads as success with no log and
     no retry.
  3. `tr-8` (MEDIUM) — `Automatic` is selectable in Settings but nothing
     observes mpv or steps down. Same defect class as `tr-1`. Either withhold
     the value or land slice 5 first; do not ship it selectable and inert.
  4. `tr-9` (MEDIUM) — every Plex transcode URL hardcodes `partIndex=0`, so a
     split-file version transcodes only its first part and ends there. The plan
     records multi-part transcoding as an open question; silent truncation is
     worse than refusing, so this needs a decision.
- **Then build transcoding slice 4's UI.** The backend command
  `quality_options` exists (`de80b8a`); the menu does not. Shape is settled in
  `.agents/decisions.md` (2026-07-25): `Play Version >` with servers expanding
  to that server's deliverable qualities when a title has two or more copies,
  `Play at Quality >` listing qualities directly when it has one, never both,
  absent entirely when the only copy cannot be converted. Every entry shows its
  bitrate — two ladder tiers share a label. Resolve options LAZILY when the
  submenu opens: for Plex it is a decision round trip per version. The choice
  applies to the play it starts and persists nothing.
- Slices 5 (Automatic) and 6 (Emby labelling, README) follow, per
  `.agents/plans/server-transcoding.md`.
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

  **Slices 3 and 4 (partial) are landed but NOT clean.** Slice 3 (`e0e5fc7`,
  1.0.18) wired the play path, both transcode URL builders and teardown; slice
  4's backend (`de80b8a`, 1.0.19) added the `quality_options` command. A codex
  review of slice 3 returned SEVEN findings, all admitted
  (`.agents/review/findings/tr-3.md`). Three are fixed in `049ed78` (1.0.20):
  Original could silently transcode on a server that omits Jellyfin's optional
  direct-play flags (tr-3), Original paid a redundant PlaybackInfo round trip
  (tr-7), and a Jellyfin transcode could start with no session id and so never
  be stoppable (tr-5). FOUR REMAIN OPEN and block calling this feature done:
  tr-4 (teardown is a detached task the exit path can lose, leaving an encoder
  running), tr-6 (teardown ignores the HTTP response, so a 401/429/5xx reads as
  success), tr-8 (Automatic is selectable but nothing implements it — the same
  defect class as tr-1), and tr-9 (Plex multi-part media transcodes only its
  first part). Slice 4's UI — the `Play Version > server > quality` nesting and
  the collapsed single-version form — is NOT built.

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
- The Linux E2E venue is BROKEN (2026-07-25) and gates nothing at present:
  `smoke` fails the same "timed out waiting for app render" as `markers` on a
  clean build of current `main`. It blocks marker Slice 4's behavioural
  verification and needs an owner decision. Diagnosis:
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
