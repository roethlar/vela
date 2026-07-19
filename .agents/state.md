# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **`chr-1` MERGED to `main` — accepted by owner-directed in-session Claude
  review.**
  The clean-EOF dispatcher now emits its authoritative Home refresh after the
  best-effort server played-state attempt while releasing playlist/Continue
  Playing work first. Implementation `6ec2ba6`; exact-head evidence/docs
  `fe8eebe`; version 0.1.60; local canonical gates and fresh Linux real-app E2E
  29/29 passed, with five independent production mutation proofs restored
  exactly. After the Claude Code 2.1.214 MCP reviewer transport was proven
  unable to inherit launch-granted permissions, the owner waived headless
  dispatch and directed an interactive in-session Claude Code 2.1.215 review;
  it accepted `6ec2ba6` at head `5a7cabf` with no material issues (same-vendor
  caveat recorded). Verdict and evidence: `.agents/review/findings/chr-1.md`.
  The governance defect and its prevention are filed as
  `https://github.com/roethlar/AgentGovernanceBootstrap/issues/6`. The owner
  approved the merge on 2026-07-19; `main` fast-forwarded to `5248fe6`.

- **`pws-1` IMPLEMENTED AND EXTERNALLY ACCEPTED.** Exact automatic successors
  retain the completed mpv process's actual fullscreen and maximized state;
  manual starts retain configured defaults. Claude Code 2.1.214 /
  `claude-opus-4-8` / high accepted exact head `8d4c7bc` with an independent
  red/restored-green stale-session proof and no comments. Exact status lives in
  `.agents/review/findings/pws-1.md`.

- **Version 0.1.60 on `main`** (`package.json`, both lockfiles,
  `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and the Arch PKGBUILD
  agree, as of implementation `6ec2ba6`, merged at `5248fe6`).

- **v1.0 README POLISH COMPLETE; OWNER-REPORTED ISSUE QUEUE IS NEXT.** The
  user-first README rewrite landed at `d4d9e95`, correcting first-run, server-
  parity, multi-Plex, privacy, build, and support claims while surfacing the
  current product features. Functional and UI implementation through OLED
  Black is complete. The owner ran
  `./scripts/build.sh` on 2026-07-17 and produced the unsigned universal macOS
  0.1.57 DMG successfully; GitHub CI passed at `1300f00`, and the fresh Linux
  real-app suite passed 27/27. On 2026-07-18 the owner directed autonomous work
  through the open queue before returning to release-track graphics. The
  deferred real-Plex completion/error smoke remains the final pre-publication
  gate.

- **LIVE E2E (`npm run e2e:live`) — landed 2026-07-14, owner-approved.** Drives
  the app against the owner's REAL Plex and REAL Jellyfin from the Linux VM.
  Opt-in; NEVER part of the gating suite (non-hermetic). Venue, access grants and
  restore-on-exit rules: `.agents/machines.md`.
  - **Why:** the owner's manual playtests found FOUR defects in two sessions that
    18 mock scenarios and 24 rounds of two-reviewer review all missed. Real
    servers say things mocks do not.
  - **What it closed:** the Plex scan path had NEVER been exercised — the mock is
    Jellyfin (GUID ids, never rebinds), so a real Plex section key (a
    server-LOCAL number) had never appeared in a test. `live-plex` now browses
    real libraries and scans one, red-proven.
  - **STILL OPEN, and unclosable here:** a Plex REBIND needs a SECOND Plex
    server, which does not exist. `sameSection` and the section-binding
    comparison remain inspection-only. (The release question this raises is
    answered in `## Next`.)

- **CI: a known vulnerability FAILS the build** (owner ruling 2026-07-14, "known
  vulnerabilities should fail so we can keep current"). The `audit` job used to
  be `continue-on-error: true` and had been swallowing two RUSTSEC DoS advisories
  in `quick-xml` — the crate that parses every Plex response off the network —
  into green runs (fixed `5cb467f`, gate flipped `2ba5f95`). Advisories with no
  upstream fix get an explicit `--ignore RUSTSEC-XXXX-NNNN` with a reason, never
  a blanket re-disable. The remaining entries are WARNINGS (unmaintained GTK 0.18
  crates via Tauri 2, one unsoundness note); `cargo audit` does not fail on those.
  - CI runs on the `github` remote only, not the gitea `origin` — see
    `.agents/repo-guidance.md` (Verification). Run the checks locally; do not
    assume CI covered them.

- **PRODUCT DIRECTION (2026-07-08, owner): Vela is a multi-server client.**
  Local/SMB/SSH sources are REMOVED (decision `.agents/decisions.md` 2026-07-08;
  plan `.agents/plans/drop-local-sources.md`, COMPLETE). The owner is Plex-first
  today and will eventually migrate to Jellyfin or Emby; the watch-state
  migration goal is a one-shot direct Plex→JF/Emby copy in Vela at migration time
  if simple (no Trakt relay). Nothing of that tool is built yet.

- **Completed and owner-verified work rotates to
  `docs/history/state-archive.md`** (rotations 2026-07-14 and 2026-07-17; do
  not reconstruct from chat). The archive owns that list; plans remain in
  `.agents/plans/` as design records.

## Next

- **Reviewer-transport cleanup COMPLETE (2026-07-19):** the `chr-1` temporary
  local artifacts recorded under the macOS host in `.agents/machines.md` are
  cleaned with owner approval; the disposable worktree was confirmed clean at
  `fe8eebe` and removed. Do not retry Claude MCP review on Claude Code
  2.1.214: the exact child-tool capability is proven unavailable. `chr-1`
  merged to `main` at `5248fe6` on 2026-07-19; no chr-1 gate remains besides
  the deferred real-Plex smoke.

- **THEN: multi-Plex — plan DECIDED; Slices 1-3 COMPLETE, Slice 4
  (two-source verification) is the resume point.** Read-only tracing
  (re-confirmed on
  post-5248fe6 main)
  established that the data plane already separates distinct source IDs, while
  account discovery, persistence, restore, link/unlink, and Settings still
  assume one literal `plex` source. The plan at `.agents/plans/multi-plex.md`
  has ALL owner decisions answered (2026-07-19): multiple accounts, repeatable
  link flow (one account + one server per link, pinned at birth), full re-key
  with a `"plex"`-sweeping migration, credentials on per-source `sources`
  entries, per-row Remove with no account-wide disconnect, and — the last open
  question, closed at `13827e4` — duplicate copies keep collapsing while WHICH
  copy plays is an explicit user choice in Settings (rejected: added-order
  default, automatic best-copy heuristics, first-play remembered picker; the
  control's exact shape gets drafted in the collapse slice and shown to the
  owner before build). Implementation slices are at the end of the plan file:
  config foundation + migration, repeatable link flow, Settings per-row
  Remove, then verification. Slice 1 landed at `a0c2d14` with live-persistence
  guard hardening at `ef0bca4`: legacy singleton credentials migrate into a
  minted per-source Plex row, every persisted `"plex"` route is re-keyed with
  crash-safe retry, and restored/rediscovered sources retain their exact
  machine pin. Twenty-two production mutations failed their intended guards;
  restored MSRV/stable/clippy/test/audit gates pass. Slice 2 landed at
  `64291bb` with guard hardening at `54fe020`: every repeatable link mints an
  independent source, auto-connects one reachable direct HTTPS machine or
  pauses on a credential-free multi-machine picker, holds pending credentials
  only in bounded expiring backend memory, and routes Plex removal through the
  normal exact-id path. Twenty-two Slice 2 production mutations failed their
  intended assertions and restored exact. Exact-head frontend and Rust gates
  pass (167 Rust tests; both audits at zero known vulnerabilities, with the
  accepted Cargo warning-class notices). Slice 3 landed at `bfe1a2c`: Settings
  now removes every provider row by its exact source ID, contains no Plex
  account-wide unlink path, and leaves Link Plex available for repeat use.
  Three UI mutations failed their intended guard and restored exact; frontend
  check/build pass. Slice 4 adds the planned two-mock-Plex separation,
  independent-removal, collapse, and override verification. The dedicated
  worktree was recreated from current `main` at `34ad47c`; worktree host facts
  live in `.agents/machines.md`. `ISSUES.md` owns the queue.

- **IMMEDIATE NEXT: work the open issue queue one item at a time.** The
  owner reports from 2026-07-18 are code-traced at the top of `ISSUES.md`; the
  older macOS `build.sh --native` failure follows them. Each code item gets its
  own durable plan, guard proof, commit, and Claude `codereview` before the
  next begins. Outbound actions remain governed by `.agents/push-policy.md`.

- **CLEAN-EOF CAROUSEL REGRESSION — IMPLEMENTED; REVIEW AND OWNER PLAYTEST OPEN.** The owner's 0.1.51
  macOS playtest found that a naturally completed episode remains the only
  Continue Watching card until it is manually marked watched. Diagnosis found
  a pre-existing missing explicit server played mutation plus Slice 5's
  end-refresh/automatic-start repaint race. Draft plan:
  `.agents/plans/clean-eof-carousel.md`. Claude r1 and r2 reopened concrete test
  design issues, which are revised at `43957b1`; r3 then failed without a
  verdict because the local bridge refused connection and its one direct retry
  hit the Claude session limit. The next owner-authorized attempt was discarded
  before verdict when the owner replaced Claude's standing review prompt with a
  neutral best-way-to-achieve-the-goal question (`85e7cd0`). Goal-only r5 then
  accepted `0dd5001` with no material finding and two compatible hardening
  comments. Goal-only r6 accepted the clarified `f57d6a4` with no material
  finding, and goal-only r7 accepted the final `a13942b` plan with no material
  finding. The owner approved the plan on 2026-07-16. Code/test slice
  `8894ca6` is implemented; all seven planned mutations failed for the right
  reason and restored green, the canonical local gates and fresh-build Linux
  E2E 24/24 pass, and two independent Grok secondary reviews accepted after
  separately proving the newer-session and tombstone guards. Version 0.1.52
  landed at `52e1a67`, and its unsigned universal macOS DMG is checksum-valid
  with both architectures. The refreshed primary Claude plan `openreview`
  admitted one LOW lock-scope finding. Primary Claude `codereview` then
  independently red-proved the newer-session and tombstone guard families,
  restored all 140 Rust tests green, and accepted the implementation; exact
  status lives in `.agents/review/index.md`. The owner's final real-Plex
  completion playtest remains unresolved; verification and artifact details
  live in the plan.

- **OWNER-GATED FOLLOW-UP: publish dependency refresh 0.1.51.** Implementation
  is COMPLETE. The
  owner selected Node 26 over current Node 24 LTS, approved a scoped
  patched-cookie override plus fail-closed npm audit, and authorized aligning
  only Node/npm on the existing Linux E2E VM. Slice 1 landed at `7fef89a`; its
  Windows assertion review fix landed at `adc0104`, and Grok accepted the
  corrected full slice at r2. Node 26.5/npm 12.0.1 now drive the repo and E2E
  VM; the VM's Ubuntu packages remain untouched. Slice 2 then landed at
  `986fa2e`, changing
  only the Ubuntu appindicator prerequisite to the current Ayatana package;
  Grok accepted it with no comments. Slice 3 landed at `28159ea`: the compatible
  Vite 8 / Svelte / TypeScript 6 / Tauri JS graph, scoped patched-cookie
  override, explicit optional-hook denial, and fail-closed npm audit gates.
  Local canonical checks, the security red proof, Vite HMR protocol, Tauri
  build, and Linux real-app E2E 18/18 passed; Grok accepted the pinned diff with
  no comments. Per the owner's 2026-07-15 correction, Codex-authored code is
  reviewed externally by Grok or Claude; Codex CLI self-review does not count.
  Slice 4 landed at `770bfba`: the compatible Cargo graph is current, the Rust
  1.89 floor is now checked in CI, audit is at zero vulnerabilities, and local
  plus Linux real-app verification passed. Grok accepted the pinned diff with
  no comments. Slice 5 landed at `8559c59`: `directories` 6 preserves the exact
  macOS/Linux/XDG config paths, audit and all compiler gates pass, Linux E2E is
  18/18, and Grok accepted the pinned diff with no comments. Slice 6 landed at
  `fa3d04f` plus live-coverage commits `69a8f83`/`3a002fa`: every Plex scalar
  mapping now uses serde XML 0.8 attribute syntax while repeated child elements
  remain children. Six mapping regressions were proven red; Rust, audit,
  frontend, Linux E2E 18/18, signal cleanup, and live Plex
  browse/detail/episode/play/watch/scan/offline/restart all passed. Independent
  post-run checks found the watch fixture clean, credentials removed, and both
  Plex services active. Grok and Claude Fable 5 each independently red-proved
  a guard and accepted the exact Slice 6 diff with no comments. Slice 7 landed
  at `1d619fd`: reqwest 0.13.4 retains native TLS without ALPN, the direct 0.12
  duplicate is gone, and the required local, Linux E2E/live-server, Linux
  package, and macOS universal package checks passed. Grok independently
  red-proved the explicit `query` feature and accepted the pinned diff with no
  comments. Slice 8 then closed four integration findings: `4cba5db` enforces
  the pinned Node/npm pair in local, CI, and release install paths; `ec7c43e`
  replaces the old WebKit driver/ICU fixture with Ubuntu's exact 2.52.3 match;
  `5532e93`/`8366b4f` make the live Jellyfin library wait deterministic; and
  `bff2905` prevents stale installers from being recopied into `dist/`.
  Their guards, real integrations, and external Grok/Claude reviews passed.
  `dc73627` bumps Vela once to 0.1.51. The final local canonical suite, Linux
  E2E 18/18, live server checks, current-only Linux deb/rpm packages, and
  checksum-valid macOS universal DMG are green. Claude Fable 5 independently
  accepted the complete pinned Slice 8 integration/version range with its
  guard confirmed and no comments. No implementation gate remains; publishing
  or triggering GitHub-hosted Windows/CI proof requires a separate owner go.
  Exact evidence lives in
  `.agents/plans/dependency-lts-refresh.md`.

- **PLAYLIST IMPLEMENTATION COMPLETE: `.agents/plans/playlists.md`** (all five
  approved slices landed, were red-proven, passed canonical verification, and
  were externally accepted by 2026-07-16).
  Product model and the two durable
  rulings: `.agents/decisions.md`
  (2026-07-14 — no play queue; video stays external). Five slices.
  - **THE PLAY QUEUE IS DELETED** (owner ruling; `ec5d613`, two independent
    Grok acceptances at r1). Ephemeral queues are a
    music idiom; the only preset video sequence worth having is a show binge, and
    there the sequence IS the show's episode order — which Continue Playing walks.
    Anything larger is a named playlist. Infuse's model, and the owner's. S1
    deleted the chip, the drawer, the six `queue_*` commands, and
    per-surface-status slice 2 (`67358fd`) with them; it also added explicit
    Resume / Play from Beginning behavior. Exact verification and fail-closed
    review trail: `.agents/review/findings/pl-s1.md`. **This is why the queue step
    of the playtest above is gone: never ask the owner to test a surface that is
    being removed.**
  - **S2 (every successful play records a recent) is complete** (`c6bc5c1`, two
    independent Grok acceptances at r1). The shared backend path is now the sole
    play-start writer; failed launches create no recent and preserve tombstones,
    and the matching end callback cannot overtake its start record. Exact guard
    and review trail: `.agents/review/findings/pl-s2.md`.
  - **S3 (durable Vela playlists and session-safe sequence playback) is
    complete** (`304f493`, two independent Grok acceptances at r1). The
    fail-closed JSON store, stable entries, editor, cross-source routing,
    retained unavailable entries, and exact-session/fresh-anchor advancement
    were independently red-proved; restored Rust passed 118 tests and Linux
    real-app E2E passed 20/20. Removed sources are pre-marked unavailable;
    configured-but-offline sources are skipped at route time. Exact guard and
    review trail: `.agents/review/findings/pl-s3.md`.
  - **S4 (read-only server playlists) is complete** (`963ef73`; integration
    guard repair `4090d73`; independent Grok acceptances at r1). Plex and the
    shared Jellyfin/experimental-Emby implementation discover video playlists,
    retain per-source unavailable groups, preserve exact server order, and
    re-fetch on exact-session sequence advancement. The separate detail surface
    has no edit affordance, and playback writes neither the server playlist nor
    `playlists.json`. Source/unit guards passed 123 Rust tests; every UI,
    sequence, isolation, and no-write leg was red-proven; restored Linux
    real-app E2E passed 21/21. Exact trail:
    `.agents/review/findings/pl-s4.md`.
  - **S5 (Continue Playing) is complete** (`6938c0f`; guard repairs
    `18ae3d4`/`0da06b7`; two independent Grok acceptances at r1). The persisted
    `off` / `on` / `only-tv` policy defaults to `only-tv`; continuation requires
    the exact cleanly-ended session; `on` walks the literal rendered Continue
    Watching list with a per-run no-repeat guard; and `only-tv` walks watched or
    unwatched episodes across seasons while respecting the Specials boundary.
    Restored local gates passed 132 Rust tests and zero npm/RustSec
    vulnerabilities; the exact 13-file VM sync and fresh-build Linux real-app
    E2E passed 24/24. Exact trail: `.agents/review/findings/pl-s5.md`.
    No playlist implementation gate remains. The owner deferred the final live
    Plex smoke to release preparation; Emby remains explicitly experimental.
  - **The design churned hard before settling** (an "Up Next" consumption queue, a
    melded carousel, a 5-second resume countdown — all proposed, all rejected by
    the owner the same day). The decisions entry records what was rejected and why;
    do not resurrect any of it from git history without reading that first.

- **RELEASE (owner asked 2026-07-14): a second Plex server is NOT needed.** The
  dangerous half of the rebind path IS guarded — `src-tauri/src/source/plex.rs`
  spins up TWO mock Plex servers (machine-A / machine-B), both serving a section
  "2" with DIFFERENT libraries behind it, and drives the real rebind scenarios.
  What has no end-to-end guard is the FRONTEND `sameSection` comparison, because
  the E2E mock is Jellyfin. **A rebind cannot happen at all on a single-server
  account** (it needs 2+ Plex servers on one account AND the saved one becoming
  unidentifiable), so it is inert for this owner. Ship without it and keep it
  recorded. Closing it would need a TLS-capable mock Plex in the harness (the app
  only restores an `https` Plex server, so it needs a trusted cert) or a second
  real instance.

- **AUTOCROP-RESUME: OWNER-CONFIRMED 2026-07-15** (fix
  `c2962a8` on 0.1.43; plan `.agents/plans/autocrop-resume.md`, loop closed
  accepted r3). Root cause probe-CONFIRMED: the stock script's positional
  auto_delay makes resumed plays detect immediately at file-loaded, before hwdec
  engages, so its hwdec guard misfires and cropdetect gathers nothing (fresh plays
  only worked because the delay deferred detection past hwdec init). Fix per owner
  fork ruling: stock `autocrop.lua` stays byte-identical upstream; a new
  Vela-owned `vela-autocrop.lua` shim owns the auto trigger. Guards: mac probe
  red→green recorded in the plan; the `autocrop` E2E (sed-red proven).
  The owner playtested the shipped behavior and confirmed autocrop on 2026-07-15.

- **SHOW-SORT + PER-LIBRARY PERSISTENCE: OWNER-CONFIRMED 2026-07-15**
  (`9cd3323` on 0.1.44; plan `.agents/plans/show-last-episode-sort.md`). Show
  libraries get "Last episode added" (Plex `episode.addedAt` LIVE-VERIFIED against
  the owner's server; JF/Emby `DateLastContentAdded`; show-only, excluded from the
  merged view), and every library's sort now persists across restarts
  (`section_sorts` config map). Guards: 3 new unit tests + the `sortpersist`
  restart E2E, all proven red→green. **Owner playtest ask (real Plex):** show
  library → "Last episode added" → a show with a fresh episode tops the list; movie
  libraries don't offer that option; set different sorts on two libraries, restart
  → each reopens on its own sort. The owner confirmed the playtest on 2026-07-15.

- **QUEUED WATCH-EDIT RACE ACCEPTED FOR v1.0 (owner, 2026-07-15):** the
  contested r6 residual race is rare, bounded and self-healing; closing it is not
  a quick fix because it needs persisted per-entry epochs and compare-and-swap
  curation. Ship it as a known potential issue in the v1.0 release notes. Detail:
  `.agents/plans/continue-watching-watch-state.md` Review log r6 + Accepted edges.

- **DRIFT FIXED (owner go, this session):** `src-tauri/src/source/mod.rs:63`
  comment referenced a "listing-cache" that died with the local-source removal
  (2026-07-08). Owner gave the go ("1 go"); comment now reads "recents
  persistence round-trip" only — verified `Deserialize` on `ItemDto` survives
  solely for `recents.rs` config embedding. (`ISSUES.md`'s companion drift was
  fixed earlier, 2026-07-14 pass.)

- **v1.0.0 RELEASE TRACK (owner, 2026-07-10, refined 2026-07-15 — ordered LAST,
  behind the functional
  work above; "queue first, v1 polish goes to the bottom", where "queue" means the
  work queue, not the play queue):** (1) UI embellishments — Slice 1 is complete
  at 0.1.53, Slice 2 at 0.1.55, and Slice 3 at 0.1.56; item 1 is complete
  (`.agents/plans/ui-embellishments.md`; vibrancy CUT — Linux/Wayland first;
  motion SUBTLE, binding); (2) docs polish — complete at `d4d9e95`; (3)
  graphics + screenshots for socials, deferred until the newly prioritized open
  issue queue is empty; (4) harden the GitHub release
  build so missing platform artifacts fail closed, add the required Arch package
  for AUR publication, and ship unsigned binaries because the owner has no Apple
  or Windows developer credentials; (5) run the deferred final Plex/error smoke
  before publishing. Emby ships explicitly experimental until a real-server
  integration test exists.

- Migration-time (not now): plan the one-shot Plex→JF/Emby watch-state copy
  (provider-id matching; both APIs already integrated).

## Blockers

- **None for `chr-1`:** the branch is merged to `main` (`5248fe6`,
  owner-approved 2026-07-19). The final real-Plex smoke remains deferred to
  release preparation by owner choice.

## Verification

- Canonical commands live in `.agents/repo-guidance.md` (Verification). Do not
  copy them here — that file owns them.
- `npm run e2e` is Linux-only; the venue and its quirks are in
  `.agents/machines.md`. The scenario count is owned by `tests/e2e/scenarios/`,
  not by this file.

## Active Sources

- `AGENTS.md` + `.agents/repo-guidance.md` (governance refreshed 2026-07-08,
  toolkit `6f08a67`; verification commands and Guard discipline live there)
- `.agents/decisions.md`
- `.agents/machines.md` (host-specific facts; the E2E venue)
- `.agents/push-policy.md` (always ask — including the `vm` remote)
- `.agents/plans/failed-watch-edit-recovery.md` (IMPLEMENTED — Grok accepted r1;
  owner playtest confirmed)
- `.agents/plans/edit-error-auto-dismiss.md` (IMPLEMENTED — Grok accepted r1;
  owner confirmed 0.1.50)
- `.agents/plans/dependency-lts-refresh.md` (COMPLETE — final canonical,
  native-package, live-server, and external integration review green)
- `.agents/plans/playlists.md` (COMPLETE — five slices; externally accepted)
- `.agents/plans/clean-eof-carousel.md` (IMPLEMENTED — 0.1.52 universal DMG
  built; owner real-Plex completion playtest outstanding)
- `.agents/plans/per-surface-status.md` (COMPLETE — owner playtest outstanding)
- `.agents/plans/autocrop-resume.md` (IMPLEMENTED — owner-confirmed)
- `.agents/plans/show-last-episode-sort.md` (LANDED — owner-confirmed)
- `.agents/plans/ui-embellishments.md` (COMPLETE at 0.1.56 — v1.0.0 item 1)
- `.agents/plans/multi-plex.md` (IN PROGRESS — Slice 1 complete; Slice 2 next)
- `.agents/plans/library-refresh-scan.md` (COMPLETE + owner-playtested; the
  r1-r24 two-reviewer log is its `## Code review log` — the standing rules it
  produced now live in decisions.md and repo-guidance.md)
- `.agents/plans/continue-watching-watch-state.md` (COMPLETE — owner accepted
  the r6 residual race for v1.0 release-note disclosure)
- `.agents/plans/drop-local-sources.md`, `.agents/plans/item-detail-view.md`,
  `.agents/plans/person-browse.md` (all COMPLETE — design records)
- `.agents/review/index.md` (durable review trails)
- `docs/history/state-archive.md` (rotated state entries)
- `README.md`, `ISSUES.md`

## Unrecorded Repo Memory

- None known.
