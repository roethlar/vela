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
- **DLS slice 1 LANDED 2026-07-08 (0.1.33)** — the turn-off-and-delete commit
  `6855df5` (22 files, +297/−8087): ten Rust modules deleted (source/{local,
  vfs,smb_vfs,metadata,listing_cache}.rs, smb.rs, smb_client.rs, sshfs.rs,
  stream_proxy.rs, ui_events.rs), 15 local-family commands + lib.rs
  registrations + startup remount/refresh paths gone, velasmb scheme + CSP
  token gone, proxy plumbing out of play_by_key/playback.rs, server-only
  `kind_rank`/`detail_rank` (plex 0, jf/emby 1; they now coincide, so
  `detail_key` only appears under a per-title play override), Settings
  local/SMB/SSH forms + Connected mount rows + Folders tab removed, packaging
  deps (smbclient/sshfs/libsmbclient/pavao-sys) dropped, packaged descriptions
  de-SMB'd (`97d4467` got the crate description). **Compat rails, guard-proven
  red/green:** config's `local_folders`/`smb_mounts`/`ssh_mounts` are inert —
  parsed, ignored, PRESERVED on save (legacy migrator deleted; SmbMount
  `kind`/`local_folder_id` now round-trip) — and recents from dead sources are
  filtered at read time (`filter_live_recents`), never stripped from config.
  Reviewloop `dls-s1` accepted CLEAN r1; an ultracode 13-agent audit found 8
  inert-only findings (4 hardened anyway). Trail `c11a458`; bump `e66bf7c`.
  REMAINING: **slice 2 — E2E re-home to mock servers (LINUX HOST ONLY;
  tests/e2e is knowingly broken until then)**; **slice 3 — docs sweep**
  (README, ISSUES, `.agents/repo-guidance.md` Mission Detail + the SMB/local
  Earned Practices bullets — now drift vs code — plus plan banners and the
  2026-05-23/2026-07-04 decision closures); owner playtest (below).
- **ITEM-DETAIL TRACK (Plex-first, owner amendment 2026-07-08):** slice 1
  (backend DetailDto + Plex parse, 0.1.31), amended slice 2 (info surfaces
  + `detail_key` routing, 0.1.32, loop `idv-s2` accepted r2), and **amended
  slice 3 — the uniform nav flip — LANDED 2026-07-08** (`74ff385`, 0.1.34;
  loop `idv-s3` accepted clean r1, trail `.agents/review/index.md`):
  library/home-rail clicks open the detail surface for every source
  (movie/video → info page; season/episode → shared episode page; show
  keeps the seasons drill through `detail_key`), the CW cover-flow center
  click plays directly, the context-menu "Info" entry is ungated
  (`devDetail` flag removed), and the poster-card hover play overlay is
  gone. Full earlier detail in `docs/history/state-archive.md` and
  `.agents/plans/item-detail-view.md`. JF/Emby `item_detail` stays deferred
  (local permanently, per the removal). **0.1.34 PLAYTEST (owner,
  2026-07-08): "otherwise, successful test"** — one defect: a home-rail
  episode click opened a single-episode page with no season/show context.
  **Fixed in polish round idv-s4 (0.1.35)** — `f1e36d3` + r1 fix `cc9f060`,
  loop accepted r2 (trail `.agents/review/index.md`): episodes carry
  namespaced parent/grandparent keys (Plex + JF/Emby), episode clicks open
  the shared season page with the episode selected however arrived at, and
  the season page heading links to the show (seasons drill) and, in
  single-episode mode, to the full season page. **NEXT: owner re-playtest
  (0.1.35) — rail episode click lands on the season page; heading links —
  then Plex polish continues.** No automated frontend guard (no JS runner;
  E2E is Linux-only) — the playtest is the behavioral check.
- **DLS slice 1 PLAYTEST SUCCESSFUL (owner, 2026-07-08, 0.1.33 Windows NSIS
  build):** Plex-only sidebar, no dead hero cards, playback unchanged. The
  item-detail Info pages were NOT exercised — release builds cannot show the
  dev-gated Info entry (no `devtools` feature, so no console for the
  localStorage flag); the owner knows clicking tiles still plays by design
  until the nav flip. Remaining owner playtest asks: (1) library sorting
  0.1.30 — sort dropdown on Plex libraries + merged All view; (2) mpv
  autocrop 0.1.22 — Shift+C / Automatic crop on the real HDR stack.
- **machine-local (Windows dev host, `F:\dev\vela`):** cargo/rustc need valid
  stdin — `cmd /c "cargo ... < nul"` (rustup shim quirk); codex lives at
  `%APPDATA%\npm\codex.cmd`, prompt via stdin redirect (`</dev/null` is
  POSIX-only); cargo test runs 62 here (unix-cfg tests excluded — Linux CI is
  authoritative); clippy baseline = 4 pre-existing cfg-dead mpv-installer
  warnings (post-removal; was 13); the E2E harness does NOT run here (Linux
  WebKitWebDriver); checkout is autocrlf=true (empty-diff "modified" files
  are line-ending noise).
- Version 0.1.35 (bumped `5ec20ad`, 2026-07-08; BUILD_DATE reads 2026-07-09
  — the script stamps UTC, which was past midnight). Everything since the
  last owner push is UNPUSHED as of `5ec20ad` (owner pushes manually; policy
  `.agents/push-policy.md`).

## Next

- DLS slice 2 (E2E re-home) from a Linux-host session, then slice 3 (docs
  sweep) — see the DLS entry above for the sweep list. The re-home must
  also update scenarios written against click-to-play: since the nav flip
  (`74ff385`), library card clicks open info pages, not playback.
- Item-detail: owner re-playtest of 0.1.35 (rail episode → season page
  with episode selected; show/season heading links), then Plex polish
  rounds; JF/Emby `item_detail` resumes only on an explicit owner go.
- QUEUED (owner-parked 2026-07-05 — "after current work"): GitHub CI was RED
  on the last PUSHED commit `05f9594` (`cargo audit` advisory noise + an
  untriaged `cargo check --locked` failure on the runner). Stale-risk: local
  code has since changed enormously (unpushed); re-triage only after the next
  owner push gives CI something current to run.
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
- `.agents/plans/item-detail-view.md` (ACTIVE — amended slice 3 pending)
- `.agents/review/index.md` (durable review trails)
- `docs/history/state-archive.md` (rotated state entries)
- `README.md`, `ISSUES.md` (drift-suspect until DLS slice 3 sweeps them)

## Unrecorded Repo Memory

- None known.
