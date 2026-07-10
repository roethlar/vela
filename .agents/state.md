# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

## Now

- **PRODUCT DIRECTION (2026-07-08, owner): Vela is a multi-server client.**
  Local/SMB/SSH sources are REMOVED (decision `.agents/decisions.md`
  2026-07-08; plan `.agents/plans/drop-local-sources.md`, plan-review accepted
  r5). The owner is Plex-first today and will eventually migrate to Jellyfin
  or Emby; the watch-state migration goal is a one-shot direct Plex→JF/Emby
  copy in Vela at migration time if simple (no Trakt relay). Nothing of that
  tool is built yet.
- **DLS (drop-local-sources): slice 1 LANDED + owner-playtested 2026-07-08**
  (0.1.33, `6855df5`; loop `dls-s1` clean r1; full detail rotated to
  `docs/history/state-archive.md`). **Slice 3 (docs sweep) LANDED
  2026-07-09, loop `dls-s3` accepted at r5** (`861442f` + four fix commits
  through `ec6a4b9`; trail in `.agents/review/index.md`) — README/ISSUES/
  repo-guidance de-localed, obsolete plans bannered, decision statuses
  closed/amended, config round-trip guard extended to legacy SMB
  credentials (guard-proven); repo-map refresh moot (file retired
  2026-07-08). **Slice 2 (E2E re-home) LANDED 2026-07-09, loop `dls-s2`
  accepted clean r1** (`80dd8e6` app fix + `b223951` suite + `b41703a`
  bump 0.1.40): all scenarios mock-served and nav-flip-aware, suite 10/10
  on the owner's Linux VM. The re-home banked a real regression fix —
  **context-menu Play threw since the nav flip** (Svelte 5 `{@const}` read
  after `closeMenu()`; fixed `80dd8e6`, guard = queue/curation scenarios
  red→green). **THE DLS PLAN IS COMPLETE.** Owner spot-check ask for the
  next build: right-click → Play on a library card now works (0.1.40).
- **ITEM-DETAIL TRACK: COMPLETE and owner-verified through 0.1.41
  (2026-07-10)** — nav flip (`74ff385`), episode navigation polish
  (`f1e36d3`+`cc9f060`), detail crumb trail (`496218e`), context-menu Play
  un-broken (`80dd8e6`, owner-verified 2026-07-09), hero episode Info →
  season page (`d7b938f`+`18c5bcd`, loop `idv-s6` accepted r2;
  owner-verified 2026-07-10 "info goes to series view"). Loops
  idv-s3/s4/s5/s6 in `.agents/review/index.md`; older history rotated to
  the archive and `.agents/plans/item-detail-view.md`. No open defect;
  further Plex polish only on the next owner report. JF/Emby `item_detail`
  stays deferred on an explicit owner go. No automated frontend guard for
  these flows (no JS runner; the mock E2E servers carry no episodes) —
  owner playtests are the behavioral check.
- **PERSON BROWSE (clickable actor/director/writer → filtered grid):
  COMPLETE — owner playtest VERIFIED 2026-07-09 ("works well") on 0.1.39.**
  Plan `.agents/plans/person-browse.md`; slice 1 backend `35fcc67` (loop
  `pb-s1` clean r1), slice 2 frontend `b290b31` (loop `pb-s2` clean r1),
  bumps `62fd927`/`8204a77`. No open defect; JF/Emby person browse stays
  deferred on an explicit owner go (same bar as JF/Emby `item_detail`).
- **CW WATCH-STATE: COMPLETE — owner playtest VERIFIED 2026-07-10
  ("carousel fix verified") on 0.1.42** (fix `02504be`; plan
  `.agents/plans/continue-watching-watch-state.md`, retained as design
  record). Mark watched/unwatched are one-op curations (recents drop +
  identity tombstone, curate-first with rollback; edits serialized;
  every play path clears tombstones); "Remove from Continue Watching"
  stays the keep-progress dismiss. Decision in `.agents/decisions.md`
  (2026-07-10). Resolved BOTH the 2026-07-10 masking defect and the
  2026-07-08 two-op curation annoyance. Guard: `watchcurate` E2E,
  red→green proven on the VM; full suite 11/11; local CI set green.
  Codex plan-review loop closed at r6 — the one CONTESTED r6 finding
  still awaits owner adjudication (item in ## Next).
- Other playtest state: library sorting owner-verified WORKING 2026-07-10
  on 0.1.41 (last-episode-added follow-up queued in ## Next). mpv
  autocrop: owner-tested 2026-07-10 — PARTIAL PASS, defect queued in
  ## Next (fresh plays crop automatically; resume doesn't).
- **machine-local (Windows dev host, `F:\dev\vela`):** the `ptk` MCP server
  (warm PowerShell runspace, `ptk_invoke`) is the DIRECT shell for agent
  harnesses on this host — probe it before assuming no shell / delegating
  shell work to subagents (2026-07-09 lesson: an entire session ran shell
  through subagent indirection with ptk available the whole time);
  cargo/rustc need valid stdin — `cmd /c "cargo ... < nul"` (rustup shim
  quirk); codex lives at `%APPDATA%\npm\codex.cmd`, headless via
  `codex exec --json --sandbox read-only` with the prompt on stdin;
  unix-cfg-gated cargo tests are excluded here (Linux CI is authoritative —
  don't record the local count, it rots); clippy baseline = 4 pre-existing
  cfg-dead mpv-installer warnings (post-removal; was 13); the E2E harness
  does NOT run here (Linux WebKitWebDriver); checkout is autocrlf=true
  (empty-diff "modified" files are line-ending noise). NOTE: whether this
  block belongs in TRACKED state at all is a filed toolkit defect
  (roethlar/AgentGovernanceBootstrap#2, 2026-07-09 — the handoff operator
  conflicts with the toolkit's own `*.local.*` convention); expect a future
  governance refresh to move it to an untracked `state.local.md`-style home.
- Version 0.1.43 (bumped for the autocrop-resume fix, 2026-07-10;
  0.1.42 was the cw-watch-state fix, owner-built and verified same
  day). Remotes as of the cw-watch-state landing
  (2026-07-10): **origin (gitea) was at `26f460f`** (caught up from the
  earlier 8-behind note) and **github at `878f9c3` with CI green** —
  both now behind local main (the day's plan/fix/docs commits); owner
  pushes manually (policy `.agents/push-policy.md`: always ask).
  machine-local (mac host): the owner's Linux VM clone (`vm` remote) is
  at `26f460f` with the landed slice content applied as a WORKING-TREE
  diff (not pushed — push policy); a later `git push vm main` needs
  `ssh … git checkout -- . && rm tests/e2e/scenarios/watchcurate.mjs`
  first, or the untracked/modified copies will block updateInstead.

## Next

- No pending playtest ask from the 2026-07-09/10 work — both fixes
  (context-menu Play, hero episode Info) are owner-verified on 0.1.40/41.
- **machine-local (mac host `/Users/michael/Dev/vela`):** the owner's Linux
  VM at `michael@192.168.64.5` is the standing E2E venue (Ubuntu 25.10
  aarch64, 12 CPU; fully provisioned 2026-07-09: rustup, tauri-driver,
  Xvfb, bsdtar, webkit2gtk-4.1-dev, vendored arm64 WebKitWebDriver, debug
  binary built). Clone at `~/dev/vela` with `receive.denyCurrentBranch=
  updateInstead`; the mac clone has a `vm` remote — sync is `git push vm
  main` (VM tree must be clean: `ssh … git checkout -- .` first if a diff
  was applied), uncommitted work travels via `git diff | ssh … git apply -`.
- **AUTOCROP-RESUME: IMPLEMENTED 2026-07-10, awaiting owner playtest**
  (fix `c2962a8` on 0.1.43; plan `.agents/plans/autocrop-resume.md`,
  loop closed accepted r3). Root cause probe-CONFIRMED on the mac host:
  the stock script's positional auto_delay makes resumed plays detect
  immediately at file-loaded, before hwdec engages, so its hwdec guard
  misfires and cropdetect gathers nothing (fresh plays only worked
  because the delay deferred detection past hwdec init). Fix per owner
  fork ruling: stock `autocrop.lua` stays byte-identical upstream; new
  Vela-owned `vela-autocrop.lua` shim owns the auto trigger (settle
  delay after every load → invokes the stock public binding). Guard:
  mac probe red→green recorded in the plan; new `autocrop` E2E
  (sed-red proven, load-marker asserts the shim resolved), full suite
  12/12; full local CI set green. **Owner playtest ask (0.1.43):**
  resume a mid-progress letterboxed/HDR title → bars crop automatically
  within ~5s without Shift+C; fresh play still crops; Shift+C still
  toggles; and a manual Shift+C crop+undo right after resume stays
  undone.
- QUEUED (owner, 2026-07-10, from the sorting playtest — "add that to the
  queue, but don't code"): **TV shows need a "Date Last Episode Added"
  sort.** Sorting is otherwise verified working, but the date-added sort on
  shows appears to use the SERIES' own addedAt (when the show was added),
  so a show whose newest episode just arrived doesn't surface. Plan first;
  per-backend leaf-added semantics (e.g. Plex episode addedAt vs series
  addedAt) are NOT investigated or spec'd — the "it seems" diagnosis is the
  owner's observation, to be code-confirmed at plan time.
- OPEN ADJUDICATION (owner, from the cw-watch-state plan-review r6): the
  contested residual queued-edit race class — accept the recorded
  disposition (documented accepted edge) or order the compare-and-swap
  hardening as a follow-up plan. Detail: plan Review log r6 + Accepted
  edges.
- QUEUED (agent-observed during plan review, 2026-07-10 — needs owner
  interest before any plan): queue plays and auto-advance never enter
  Vela's recents (`play_by_key` records nothing; only the frontend
  direct-play path calls `record_recent`), so a queue session stopped
  midway doesn't surface in Continue Watching. Surfaced by codex
  plan-review r1 on `.agents/plans/continue-watching-watch-state.md`;
  that plan fixes only the tombstone-lifecycle slice of it.
- **v1.0.0 RELEASE TRACK (owner, 2026-07-10 — ordered LAST behind the
  functional queue above, "queue first, v1 polish goes to the bottom"):**
  (1) UI embellishments — plan QUEUED with decisions resolved
  (`.agents/plans/ui-embellishments.md`: 3 slices; vibrancy CUT —
  Linux/Wayland first; motion SUBTLE, binding); (2) docs polish — README
  that entices users to try it; (3) graphics + screenshots for socials.
  2 and 3 are gated on 1 and on the functional queue emptying.
- Migration-time (not now): plan the one-shot Plex→JF/Emby watch-state copy
  (provider-id matching; both APIs already integrated).

## Blockers

- None recorded.

## Verification

- Canonical commands live in `.agents/repo-guidance.md` (Verification) —
  frontend `npm run check` / `npm run build`; Rust from `src-tauri/`:
  `cargo check --locked`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked`; `npm run e2e` (Linux only; re-homed to
  mock servers 2026-07-09, 10/10 green at `b41703a` on the owner's VM).

## Active Sources

- `AGENTS.md` + `.agents/repo-guidance.md` (governance refreshed 2026-07-08,
  toolkit `6f08a67`; verification commands now live in repo-guidance)
- `.agents/decisions.md`
- `.agents/plans/drop-local-sources.md` (COMPLETE — all three slices landed
  2026-07-08/09)
- `.agents/plans/item-detail-view.md` (ACTIVE — nav flip landed; polish)
- `.agents/plans/person-browse.md` (COMPLETE — owner-verified 2026-07-09)
- `.agents/plans/continue-watching-watch-state.md` (COMPLETE —
  owner-verified 2026-07-10; r6 adjudication open)
- `.agents/plans/ui-embellishments.md` (QUEUED — v1.0.0 item 1,
  decisions resolved, parked at queue bottom)
- `.agents/plans/autocrop-resume.md` (IMPLEMENTED — awaiting owner
  playtest)
- `.agents/review/index.md` (durable review trails)
- `docs/history/state-archive.md` (rotated state entries)
- `README.md`, `ISSUES.md` (swept by DLS slice 3, 2026-07-09)

## Unrecorded Repo Memory

- None known.
