# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **Version 0.1.56** (`package.json`, both lockfiles,
  `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and the Arch PKGBUILD
  agree, as of `d3f3636`).

- **UI EMBELLISHMENTS SLICE 3 COMPLETE at 0.1.56 (owner go 2026-07-16).**
  The approved motion and designed-empty-state implementation lands through
  `841f749`. Every focused source family was separately mutation-proven, three
  initially vacuous guards were repaired and re-proven, normal and verified
  reduced-motion focused Linux runs pass, final dark/light screenshots were
  inspected, and the complete fresh-binary real-app suite passes 27/27. Primary
  Claude Fable 5 accepted exact reviewed head `6075f52` with an independent
  red/restored-green guard proof and no material finding. The owner then removed
  Agy and the default secondary-review gate. Final local frontend/Rust gates and
  the fresh `vela v0.1.56` Linux real-app suite pass 27/27. No owner playtest is
  required. Exact evidence:
  `.agents/review/findings/ui-s3.md`.

- **UI EMBELLISHMENTS SLICE 1 COMPLETE at 0.1.53 (owner go 2026-07-16).** The
  theme-correct foundation in `.agents/plans/ui-embellishments.md` now owns
  semantic theme states, six shared visual primitives, and typed SVG icons.
  Implementation landed through `c1c4db4`; version `0ce3629`. Canonical
  frontend checks, separately red-proven source guards, normal and
  reduced-motion focused Linux runs, final Linux E2E 25/25, and dark/light
  screenshot inspection are green. Primary Claude Fable 5 and independent Grok
  4.5 each accepted the exact code range with separate red/green guard proofs
  and no material findings. The owner is unavailable to playtest this track;
  no playtest is required or pending. Slices 2 and 3 are complete below and
  above, respectively.
  Exact review evidence: `.agents/review/findings/ui-s1.md`.

- **UI EMBELLISHMENTS SLICE 2 COMPLETE at 0.1.55 (owner go 2026-07-16).** One
  cache/source-safe reveal action now owns nine media-art surfaces over fixed
  failure underlays; all ten runtime images decode asynchronously and the Plex
  QR remains the sole no-fade exception. Implementation `830cabd`, real-app
  selector repair `c22a07e`, version `49c0dc9`. Focused guards were separately
  proven red, canonical frontend plus complete Rust verification passed, normal
  and reduced-motion Linux runs passed, the full real-app suite passed 26/26,
  and six dark/light screenshots were inspected. Primary Claude Fable 5 and the
  owner-authorized independent Agy/Gemini substitute accepted the exact range
  with separate guard proofs and no findings. No owner playtest is required or
  pending. Slice 3 is complete above. Exact
  evidence:
  `.agents/review/findings/ui-s2.md`.

- **CAROUSEL DUPLICATE PLAY ACTION CORRECTION COMPLETE at 0.1.54 (owner go
  2026-07-16).** Playlist S1 added a primary Play/Resume row beneath the
  Continue Watching cover-flow even though the centered card already performs
  that action. Implementation `507fb2f` removes the complete row, retains card
  click/keyboard semantics, and keeps Play from Beginning in the context menu.
  The exact guard failed with the row restored and passed after restoration;
  affected Linux scenarios and the full 25/25 suite are green. External review
  converged clean with independent Claude and Grok guard proofs. Final 0.1.54
  focused Linux build and screenshot inspection confirm the row is absent. No
  owner playtest is required or pending. Evidence:
  `.agents/review/findings/car-1.md`.

- **FAILED EDIT-ERROR AUTO-DISMISS: OWNER-CONFIRMED ON 0.1.50.**
  Implemented at `01e30cf` from approved plan
  `.agents/plans/edit-error-auto-dismiss.md`. Failed watch-state edit errors
  retain their own line, follow navigation, clear after eight seconds, and
  still clear immediately for a newer edit/source change; captured attempts
  prevent a queued old timer from erasing a newer failure. Four distinct
  regressions were proven red, the restored Linux suite is green, and both live
  server paths passed. Grok independently proved the stale-timer ownership
  guard, restored the head green, and accepted with no findings. Closed review
  record: `.agents/review/findings/eet-1.md`. The owner built 0.1.50 and
  confirmed the exact stopped-Plex timing path: the grid/title stayed present,
  the named red edit line disappeared after about eight seconds, and the item
  remained unwatched/actionable after Plex restart plus Refresh.

- **FAILED-WATCH-EDIT RECOVERY: OWNER-CONFIRMED ON 0.1.49.** Implemented at
  `b5c170a`; Grok accepted r1.
  The owner playtest failed 2026-07-14 on 0.1.48; follow-up plan
  `.agents/plans/failed-watch-edit-recovery.md` is IMPLEMENTED. The stopped-Plex
  test showed a view failure and a named edit failure on separate lines, with no
  raw URL. Recovery failed: the whole loaded Movies grid disappeared, and
  **12 Years a Slave** remained absent after Plex returned even though Plex Web
  showed it present and unwatched. Read-only diagnosis at `310c2ca` confirmed
  Plex returns that exact item at index 5 of Vela's first-page query and Vela
  has no tombstone for it. The frontend's failed-edit catch unnecessarily
  re-enters the browse listing; backend rollback affects only Home
  recents/tombstones. `b5c170a` replaces that browse reload with a Home-only
  repair and strengthens the count-only guards to exact identity. The four
  planned hermetic regressions were each proven red, the restored scenario is
  green, the full Linux suite is 18/18, and the exact live Plex path passed.
  Grok independently red-proved both the old broad recovery and the exact-
  identity guard, restored the head green, and accepted with no findings.
  The owner repeated the exact stopped-Plex path on installed 0.1.49 and
  confirmed the grid/title remain present and the item remains unwatched. The
  follow-up for the indefinite red edit line is implemented and Grok-accepted
  at `01e30cf` under `.agents/plans/edit-error-auto-dismiss.md`. Closed recovery
  review record:
  `.agents/review/findings/fwer-1.md`. Original plan
  `.agents/plans/per-surface-status.md`; decision `.agents/decisions.md`
  (2026-07-14). Every failure now reports on the surface it belongs to: the
  view's banner keeps listing/refresh/search failures, and the watch-state edit
  (`fee7f0e`), the queue (`67358fd`), the mpv bar (`0f41c7b`) and the detail
  page (`40dfc40`) each report their own. Slice 5 (`282702b`) then DELETED the
  whole refereeing apparatus — the `owner` field, `ErrorOwner`, `clearOwned`,
  per-surface clearing, the scope merge — net -67 lines. As of `282702b`: e2e
  18/18, cargo test 95, clippy/svelte-check/build clean.
  - **What it bought:** the defect class that ran for EIGHT review rounds
    (r17-r24, each fix opening the next door, always the same loss — a failure
    the user needed, silently gone) is structurally gone, because the fight over
    one surface is gone. Slice 1 alone collapsed SIX e2e cases, three of which
    had asserted that a failed edit must be SUPPRESSED when the user navigated
    away — never right, just the price of sharing.
  - **The queue and its status slice (`67358fd`) were deleted in playlist S1**
    (`ec5d613`). The rewritten `surfaces` scenario still red-proves the detail
    and edit lines that remain; exact proof and external review:
    `.agents/review/findings/pl-s1.md`.
  - The library-refresh-scan review loop is CLOSED at r24. Its evidence rotated
    to `docs/history/state-archive.md` (2026-07-14); its two standing rules were
    carved out first — the **review protocol** to `.agents/decisions.md` and
    **guard discipline** to `.agents/repo-guidance.md`. Do not reopen the loop
    against the shared banner.

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

- **Completed and owner-verified, rotated to `docs/history/state-archive.md`
  2026-07-14** (do not reconstruct from chat): library-refresh-scan (0.1.45),
  drop-local-sources, item-detail, person-browse, cw-watch-state (0.1.42). Their
  plans remain in `.agents/plans/` as design records.

## Next

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
  is COMPLETE; no push, CI dispatch, or release workflow was triggered. The
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

- **DRIFT FOUND 2026-07-14, NOT FIXED — needs an owner go (it is a code file):**
  `src-tauri/src/source/mod.rs:63` has a comment referencing a "listing-cache"
  that no longer exists (it died with the local-source removal, 2026-07-08).
  Comment-only; fold it into the next slice that touches the file rather than
  making a lone code commit. (`ISSUES.md`'s companion drift — an open P1 for a
  metadata cache that no longer exists — was fixed in the same pass.)

- **v1.0.0 RELEASE TRACK (owner, 2026-07-10, refined 2026-07-15 — ordered LAST,
  behind the functional
  work above; "queue first, v1 polish goes to the bottom", where "queue" means the
  work queue, not the play queue):** (1) UI embellishments — Slice 1 is complete
  at 0.1.53, Slice 2 at 0.1.55, and Slice 3 at 0.1.56; item 1 is complete
  (`.agents/plans/ui-embellishments.md`; vibrancy CUT — Linux/Wayland first;
  motion SUBTLE, binding); (2) docs polish — a README that
  entices users to try it; (3) graphics + screenshots for socials. 2 and 3 are
  gated on 1 and on the functional work emptying; (4) harden the GitHub release
  build so missing platform artifacts fail closed, add the required Arch package
  for AUR publication, and ship unsigned binaries because the owner has no Apple
  or Windows developer credentials; (5) run the deferred final Plex/error smoke
  before publishing. Emby ships explicitly experimental until a real-server
  integration test exists.

- Migration-time (not now): plan the one-shot Plex→JF/Emby watch-state copy
  (provider-id matching; both APIs already integrated).

## Blockers

- None recorded.

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
