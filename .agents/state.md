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
- No outstanding playtest asks. Library sorting: owner-verified WORKING
  2026-07-10 on 0.1.41 (last-episode-added follow-up queued in ## Next).
  mpv autocrop: owner-tested 2026-07-10 — PARTIAL PASS, defect queued in
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
- Version 0.1.41 (bumped `4552a66`, 2026-07-09; owner-built
  `Vela_0.1.41_universal.dmg` on the mac host 2026-07-09). As of
  2026-07-10: **github is at `878f9c3` == local HEAD with CI GREEN**
  (verified via `gh run list`); **origin (gitea) is at `c2ab703`** — 8
  commits behind (owner pushes manually — policy
  `.agents/push-policy.md`).

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
- QUEUED (owner, 2026-07-10, autocrop playtest): **Automatic autocrop
  doesn't engage on RESUME — only on a fresh play; the owner has to hit
  Shift+C.** Wiring is not the suspect: auto mode just loads the bundled
  stock script (`playback.rs autocrop_args`), and resume differs only by
  `--start=<seconds>` on the same command line — so the stock
  `autocrop.lua`'s auto trigger vs the start-seek interaction is the place
  to look (the script is bundled in-repo; read it at plan time). Untriaged
  beyond that; no code without a plan + go.
- QUEUED (owner, 2026-07-10, from the sorting playtest — "add that to the
  queue, but don't code"): **TV shows need a "Date Last Episode Added"
  sort.** Sorting is otherwise verified working, but the date-added sort on
  shows appears to use the SERIES' own addedAt (when the show was added),
  so a show whose newest episode just arrived doesn't surface. Plan first;
  per-backend leaf-added semantics (e.g. Plex episode addedAt vs series
  addedAt) are NOT investigated or spec'd — the "it seems" diagnosis is the
  owner's observation, to be code-confirmed at plan time.
- QUEUED (owner, 2026-07-10): **Watched-status changes are masked while an
  item sits in Continue Watching.** Owner report: right-click mark
  watched/unwatched from the carousel "does not mark the video appropriately
  until I also remove it from continue watching"; while a video is in
  Continue Watching the watched status can't be changed *from anywhere*
  until it's removed. Session triage (read-only, 2026-07-10): removing from
  Continue Watching performs no scrobble (`commands.rs remove_from_continue`
  → tombstone + recents drop only), yet the status comes right afterward —
  so the server-side scrobble likely DID land and the defect is a *display
  mask*: the carousel shows Vela's local recents snapshot, which wins the
  hero dedup over the fresh server hub copy (`+page.svelte` `heroItems`,
  "local copy wins") and whose `played`/`view_offset_ms` are frozen at
  playback time; `set_watched` drops the recents entry only on
  played=true and deliberately never on unwatched (`commands.rs
  set_watched`), so the stale in-progress snapshot keeps rendering no
  matter where the state was changed from. **PLAN DRAFTED 2026-07-10:
  `.agents/plans/continue-watching-watch-state.md`** (diagnosis
  code-confirmed; folds in the queued-last one-op curation item) —
  awaiting plan review + owner go. No code without the go.
- Migration-time (not now): plan the one-shot Plex→JF/Emby watch-state copy
  (provider-id matching; both APIs already integrated).
- QUEUED LAST (owner, 2026-07-08, from the 0.1.33 playtest — "add this to the
  bottom of the queue"): **Continue Watching carousel needs a one-op
  curation.** Owner-reported annoyance: "if I mark a video in the carousel as
  unwatched, it stays in the carousel. if I remove it from continue watching,
  the watched status remains. so I have to do two ops to get what I want."
  Design when picked up (plan first; options include a combined context-menu
  action or changing what each action implies) — FOLDED INTO
  `.agents/plans/continue-watching-watch-state.md` (2026-07-10): the drafted
  semantics make mark-unwatched a one-op full reset that also leaves the
  carousel, resolving this by design if the plan is accepted.

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
- `.agents/plans/continue-watching-watch-state.md` (DRAFT — awaiting
  review + owner go)
- `.agents/review/index.md` (durable review trails)
- `docs/history/state-archive.md` (rotated state entries)
- `README.md`, `ISSUES.md` (swept by DLS slice 3, 2026-07-09)

## Unrecorded Repo Memory

- None known.
