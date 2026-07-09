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
  `docs/history/state-archive.md`). REMAINING: **slice 2 — E2E re-home to
  mock servers (LINUX HOST ONLY; tests/e2e is knowingly broken until then,
  and the re-home must also rewrite scenarios written against click-to-play
  — since the nav flip, library card clicks open info pages)**; **slice 3 —
  docs sweep** (README, ISSUES, `.agents/repo-guidance.md` Mission Detail +
  the SMB/local Earned Practices bullets — now drift vs code — plus plan
  banners and the 2026-05-23/2026-07-04 decision closures).
- **ITEM-DETAIL TRACK: COMPLETE and owner-verified through 0.1.36
  (2026-07-09)** — nav flip (`74ff385`), episode navigation polish
  (`f1e36d3`+`cc9f060`), detail crumb trail (`496218e`); loops idv-s3/s4/s5
  in `.agents/review/index.md`; full history rotated to the archive and
  `.agents/plans/item-detail-view.md`. No open defect; further Plex polish
  only on the next owner report. JF/Emby `item_detail` stays deferred on an
  explicit owner go. No automated frontend guard (no JS runner; E2E is
  Linux-only) — owner playtests are the behavioral check.
- **PERSON BROWSE (clickable actor/director/writer → filtered grid):
  CODE-COMPLETE at 0.1.39, owner playtest PENDING.** Owner go 2026-07-09
  (defaults accepted: newest-first, full cast, episode-level crew links);
  plan `.agents/plans/person-browse.md` (reviewed r3); slice 1 backend
  `35fcc67` (loop `pb-s1` clean r1), slice 2 frontend `b290b31` (loop
  `pb-s2` clean r1), bumps `62fd927`/`8204a77`. Playtest checklist:
  cast/director/writer clicks open the person grid (newest first,
  movies+shows mixed), results route per the nav flip, Back/crumbs work,
  mark-watched from the person grid keeps the grid populated (the plan's
  refresh case), non-Plex sparse pages stay plain text. The owner last
  BUILT 0.1.37 — 0.1.39 needs a fresh `./scripts/build.ps1`.
- Outstanding owner playtest asks (older builds): (1) library sorting
  0.1.30 — sort dropdown on Plex libraries + merged All view; (2) mpv
  autocrop 0.1.22 — Shift+C / Automatic crop on the real HDR stack.
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
- Version 0.1.39 (bumped `8204a77`, 2026-07-09). **Owner pushed BOTH remotes
  (origin + github) to `926162c` on 2026-07-09**; 9 commits are unpushed as
  of `d3dbb58` (owner pushes manually; policy `.agents/push-policy.md`).
  **GitHub CI is GREEN on the pushed head `926162c`** (verified via
  `gh run list` 2026-07-09) — the 2026-07-05 RED-CI re-triage item is
  CLOSED; the old `05f9594` failures don't reproduce on current code.

## Next

- Owner playtest of person browse (0.1.39 — build first; checklist in the
  entry above). The most likely next action on any host.
- DLS slice 2 (E2E re-home) from a Linux-host session, then slice 3 (docs
  sweep) — scope lists in the DLS entry above.
- Migration-time (not now): plan the one-shot Plex→JF/Emby watch-state copy
  (provider-id matching; both APIs already integrated).
- QUEUED LAST (owner, 2026-07-08, from the 0.1.33 playtest — "add this to the
  bottom of the queue"): **Continue Watching carousel needs a one-op
  curation.** Owner-reported annoyance: "if I mark a video in the carousel as
  unwatched, it stays in the carousel. if I remove it from continue watching,
  the watched status remains. so I have to do two ops to get what I want."
  Design when picked up (plan first; options include a combined context-menu
  action or changing what each action implies) — not spec'd yet.

## Blockers

- None recorded.

## Verification

- Canonical commands live in `.agents/repo-guidance.md` (Verification) —
  frontend `npm run check` / `npm run build`; Rust from `src-tauri/`:
  `cargo check --locked`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked`; `npm run e2e` (Linux only; broken until
  DLS slice 2 re-homes it to mock servers).

## Active Sources

- `AGENTS.md` + `.agents/repo-guidance.md` (governance refreshed 2026-07-08,
  toolkit `6f08a67`; verification commands now live in repo-guidance)
- `.agents/decisions.md`
- `.agents/plans/drop-local-sources.md` (ACTIVE — slice 1 landed, 2-3 open)
- `.agents/plans/item-detail-view.md` (ACTIVE — nav flip landed; polish)
- `.agents/plans/person-browse.md` (IMPLEMENTED — slices 1-2 landed;
  owner playtest pending)
- `.agents/review/index.md` (durable review trails)
- `docs/history/state-archive.md` (rotated state entries)
- `README.md`, `ISSUES.md` (drift-suspect until DLS slice 3 sweeps them)

## Unrecorded Repo Memory

- None known.
