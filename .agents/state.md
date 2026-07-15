# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **Version 0.1.49** (`package.json`, `src-tauri/tauri.conf.json`,
  `src-tauri/Cargo.toml` all agree, as of `b5c170a`).

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
  remaining polish is the indefinite red edit line: the owner approved an
  attempt-owned eight-second auto-dismiss in
  `.agents/plans/edit-error-auto-dismiss.md`. Closed recovery review record:
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
  - **The queue's slice (`67358fd`) is about to be deleted** along with the
    queue itself (`.agents/plans/playlists.md` S1). The `surfaces` E2E scenario
    must keep red-proving the surfaces that REMAIN.
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

- **IMPLEMENTING: failed edit-error auto-dismiss.** Approved plan
  `.agents/plans/edit-error-auto-dismiss.md`, one code slice. Failed watch-state
  edit errors remain on their own line, follow navigation, then auto-dismiss
  eight seconds after publication; the next edit/source change still clears
  immediately and stale timers cannot erase newer failures. Active review loop:
  `eet-1`; see `.agents/review/index.md`.

- **AWAITING OWNER GO: `.agents/plans/playlists.md`** (drafted 2026-07-14, no code
  written). Product model and the two durable rulings: `.agents/decisions.md`
  (2026-07-14 — no play queue; video stays external). Five slices.
  - **THE PLAY QUEUE IS BEING DELETED** (owner ruling). Ephemeral queues are a
    music idiom; the only preset video sequence worth having is a show binge, and
    there the sequence IS the show's episode order — which Continue Playing walks.
    Anything larger is a named playlist. Infuse's model, and the owner's. S1
    deletes the chip, the drawer, the six `queue_*` commands, and
    per-surface-status slice 2 (`67358fd`) with them. **This is why the queue step
    of the playtest above is gone: never ask the owner to test a surface that is
    being removed.**
  - **S2 (every play records a recent) is independent of playlists and can land
    first.** It is the real defect: the Continue Watching carousel reflects nothing
    played through the dispatcher, because `play_by_key` records no recent
    (`commands.rs:2365` says so outright), so Vela's half of the hero merge stays
    empty and only the server's hub half moves. This absorbs the 2026-07-10 QUEUED
    defect, which is no longer tracked separately.
  - **Already true, and it is what makes this cheap:** item keys are namespaced
    `<source_id>:<raw>` and `Registry::route` (`source/mod.rs:414`) dispatches per
    item, so a list mixing Plex and Jellyfin items already plays today.
    Mixed-source playlists need no new dispatch machinery. Episode walking for
    `only-tv` needs no new server API either — `children()` plus the season/show
    keys already on `ItemDto` cover it.
  - **The sharpest hazard:** the `on` continue-playing mode can replay an item the
    server never marks watched, forever. It needs a no-repeat guard, and it must
    walk the SAME Continue Watching list the carousel renders — a second source of
    truth for "what plays next" is exactly the failure class per-surface-status was
    built to kill.
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

- **AUTOCROP-RESUME: IMPLEMENTED 2026-07-10, awaiting owner playtest** (fix
  `c2962a8` on 0.1.43; plan `.agents/plans/autocrop-resume.md`, loop closed
  accepted r3). Root cause probe-CONFIRMED: the stock script's positional
  auto_delay makes resumed plays detect immediately at file-loaded, before hwdec
  engages, so its hwdec guard misfires and cropdetect gathers nothing (fresh plays
  only worked because the delay deferred detection past hwdec init). Fix per owner
  fork ruling: stock `autocrop.lua` stays byte-identical upstream; a new
  Vela-owned `vela-autocrop.lua` shim owns the auto trigger. Guards: mac probe
  red→green recorded in the plan; the `autocrop` E2E (sed-red proven).
  **Owner playtest ask:** resume a mid-progress letterboxed/HDR title → bars crop
  automatically within ~5s without Shift+C; fresh play still crops; Shift+C still
  toggles; and a manual Shift+C crop+undo right after resume stays undone.

- **SHOW-SORT + PER-LIBRARY PERSISTENCE: LANDED 2026-07-10 (`9cd3323` on 0.1.44),
  awaiting owner playtest** (plan `.agents/plans/show-last-episode-sort.md`). Show
  libraries get "Last episode added" (Plex `episode.addedAt` LIVE-VERIFIED against
  the owner's server; JF/Emby `DateLastContentAdded`; show-only, excluded from the
  merged view), and every library's sort now persists across restarts
  (`section_sorts` config map). Guards: 3 new unit tests + the `sortpersist`
  restart E2E, all proven red→green. **Owner playtest ask (real Plex):** show
  library → "Last episode added" → a show with a fresh episode tops the list; movie
  libraries don't offer that option; set different sorts on two libraries, restart
  → each reopens on its own sort.

- **OPEN ADJUDICATION (owner, from the cw-watch-state plan-review r6):** the
  contested residual queued-edit race class — accept the recorded disposition
  (documented accepted edge) or order the compare-and-swap hardening as a follow-up
  plan. Detail: that plan's Review log r6 + Accepted edges.

- **DRIFT FOUND 2026-07-14, NOT FIXED — needs an owner go (it is a code file):**
  `src-tauri/src/source/mod.rs:63` has a comment referencing a "listing-cache"
  that no longer exists (it died with the local-source removal, 2026-07-08).
  Comment-only; fold it into the next slice that touches the file rather than
  making a lone code commit. (`ISSUES.md`'s companion drift — an open P1 for a
  metadata cache that no longer exists — was fixed in the same pass.)

- **v1.0.0 RELEASE TRACK (owner, 2026-07-10 — ordered LAST, behind the functional
  work above; "queue first, v1 polish goes to the bottom", where "queue" means the
  work queue, not the play queue):** (1) UI embellishments — plan QUEUED with
  decisions resolved (`.agents/plans/ui-embellishments.md`: 3 slices; vibrancy CUT
  — Linux/Wayland first; motion SUBTLE, binding); (2) docs polish — a README that
  entices users to try it; (3) graphics + screenshots for socials. 2 and 3 are
  gated on 1 and on the functional work emptying.

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
  owner playtest confirmed; landing waits on the timeout polish)
- `.agents/plans/edit-error-auto-dismiss.md` (APPROVED — implementation active;
  Grok code review required)
- `.agents/plans/playlists.md` (DRAFTED — awaiting owner go; five slices)
- `.agents/plans/per-surface-status.md` (COMPLETE — owner playtest outstanding)
- `.agents/plans/autocrop-resume.md` (IMPLEMENTED — awaiting owner playtest)
- `.agents/plans/show-last-episode-sort.md` (LANDED — awaiting owner playtest)
- `.agents/plans/ui-embellishments.md` (QUEUED — v1.0.0 item 1, parked at the
  bottom)
- `.agents/plans/library-refresh-scan.md` (COMPLETE + owner-playtested; the
  r1-r24 two-reviewer log is its `## Code review log` — the standing rules it
  produced now live in decisions.md and repo-guidance.md)
- `.agents/plans/continue-watching-watch-state.md` (COMPLETE — r6 adjudication
  open)
- `.agents/plans/drop-local-sources.md`, `.agents/plans/item-detail-view.md`,
  `.agents/plans/person-browse.md` (all COMPLETE — design records)
- `.agents/review/index.md` (durable review trails)
- `docs/history/state-archive.md` (rotated state entries)
- `README.md`, `ISSUES.md`

## Unrecorded Repo Memory

- None known.
