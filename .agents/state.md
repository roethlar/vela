# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change.

## Now

- Review loop e2e-2 (2026-07-05) CLOSED: 3 findings verified — eh-5 (the
  app bug below), eh-6 (scenario raced the seeded source render), eh-7
  (the guard couldn't tell IPC quit from natural EOF; now a socket
  connectability probe + [3000,8000] stamp bound). Trail:
  `.agents/review/index.md` + `findings/eh-*.md`.
- E2E slice 2 landed 2026-07-05 (`d2be263`): mpv-IPC playback scenario —
  seeded local source (ffmpeg clip), real poster-click play, mpv socket
  probe (path/seek/quit), recents `viewOffsetMs` stamp, hero assertion.
  It immediately caught a REAL app bug (eh-5, fixed in `b4b4ebb`): the
  Continue Watching hero was hub-gated, so local-only setups never saw
  it despite the 2026-07-04 recents-fed-hero decision. Harness knowledge
  gained: `mpv_extra_args` parses one option per LINE; recents items
  serialize camelCase (`viewOffsetMs`); local folder `kind` is singular
  (`movie`/`show`).
- Review loop e2e-1 (2026-07-05) CLOSED: slice-1 review found 2 defects
  (codex) + 2 coder-filed during live diagnosis — signal-orphaned process
  groups; silent false-green on typo'd scenario filters; unbounded
  requests hiding stalls for 300s; and the big one, eh-4: on the live
  Wayland desktop, screenshots hang whenever the test window opens
  unfocused (no frame callbacks). The harness now runs HEADLESS on a
  managed Xvfb display by default (`VELA_E2E_HEADED=1` to watch live;
  `VELA_E2E_DEBUG=1` for per-call timing) — runs are deterministic and
  never pop windows over the owner's work. All 4 fixes verified
  (`.agents/review/index.md`, `findings/eh-*.md`).
- E2E harness slice 1 landed 2026-07-05 (plan `.agents/plans/e2e-harness.md`,
  approved via the 2026-07-04 delegation): `npm run e2e` drives the real
  debug binary via `tauri-driver` + a **vendored Debian WebKitWebDriver
  2.50.6** — verified fact: no distro ships a driver for webkit2gtk 2.52
  (Arch/Fedora/openSUSE drop it; Debian tops out at 2.50.6); the version
  skew was probe-validated live and the deviation (plus the no-WDIO,
  zero-dep WebDriver client) is recorded in the plan. Throwaway
  `XDG_CONFIG_HOME` per scenario; screenshots + driver logs in
  `tests/e2e/artifacts/`; smoke scenario green and red-proven (broken
  assertion → exit 1). Owner standing instruction 2026-07-05: run
  `playbook reviewloop codex` on EVERY slice.
- Review loop cw-1..cw-3 (2026-07-05) CLOSED: codex batch pass over
  `ec94715..a055556` found 3 real defects (merged-key miss in curation
  actions; registry lock across network await; failed play clearing
  tombstones), all fixed as single commits on `main` and independently
  verified. Trail: `.agents/review/index.md` + `findings/cw-*.md`.
- OWNER DELEGATION 2026-07-04 (decision recorded): progress must not block
  on the owner. The two locked-choice plans were approved via that
  decision and are now IMPLEMENTED, one commit per slice, all
  guard-proven, full suite green (78 Rust tests; svelte-check + build
  clean):
  - SMB share-root auto-add (`f05919e`): adding a share auto-selects its
    root as a library folder — the zero-folder invisible-share trap is
    closed. Existing zero-folder shares (owner's `zoey`) are NOT
    migrated by design; re-add the share or add a folder once.
  - Continue Watching curation (`d2ea1a7`, `cf5af95`, `d259213`): On
    Deck folds into the hero cover-flow via Vela's own
    `/library/onDeck` fetch (synthetic hub `vela.ondeck`), everything
    interleaved by recency (`lastWatchedAtMs`); mark-watched drops the
    recents entry and re-fetches; "Remove from Continue Watching"
    (hero context menu) tombstones the key (`hidden_from_continue`,
    FIFO cap 200, cleared on replay) + best-effort Plex server-side
    removal. Implementation notes + deviations in the plan file.
  - Open verification residual: the Plex removal route's real-item
    effect is unverified (permissions layer correctly refused a live
    mutating probe); route existence IS live-verified. Non-fatal by
    design. Also: hero merge ORDERING is frontend-only and has no unit
    test (no JS runner in repo) — E2E harness covers it later.
  - E2E harness plan drafted (`.agents/plans/e2e-harness.md`,
    approved-in-principle via the delegation decision): tauri-driver +
    WebdriverIO UI driving, mpv-IPC playback probes, env-gated live
    smoke, throwaway config dir, creds via env only. NOT yet built.
  - Versioning clarified by the owner 2026-07-05: bumping is routine —
    just run `scripts/bump.sh` when code lands (per the 2026-06-20
    decision); it is NOT an owner gate. Bumped to 0.1.10.
- MERGED to `main` 2026-07-04 (owner go; merge commit `e9f6029`; the
  `smb-native` branch is deleted — owner direction: no branches without
  his explicit word in future). `.agents/plans/smb-native-client.md` is
  IMPLEMENTED. All six slices codex-verified
  (`.agents/review/index.md`; smb-2/3/4/5 took 2-3 rounds each — every
  reopen was a real finding, recorded in the finding docs). Linux SMB is
  now native and mountless: libsmbclient in-process (via pavao-sys, one
  context per connection), share browsing/listing/search through the
  local-family Vfs provider, playback via a loopback HTTP Range proxy
  (127.0.0.1, tokenized), sidecar posters via the stable `velasmb:`
  scheme, .nfo enrichment over the wire. gvfs/kio machinery deleted;
  macOS/Windows keep OS mounts. PKGBUILD depends on `smbclient`; deb/rpm
  on `libsmbclient`.
  NEXT: (1) owner playtest on the real NAS — add share 10.1.10.206/media
  with credentials in Settings (native, no mount), browse, add a folder,
  play/seek/resume, check hero recents and posters; (2) version bump
  (not done on the branch — owner's release call). Env-gated live probe:
  `VELA_SMB_LIVE=server/share cargo test --lib live_probe -- --nocapture`.
- Vela is a Tauri 2 + SvelteKit + Rust desktop media client for Plex,
  Jellyfin, Emby, local folders, SMB shares, and SSH/SFTP mounts. It plays
  media through the system `mpv` binary for HDR passthrough.
- Version 0.1.10 (bumped 2026-07-05). Remote `github` is current; remote `origin`
  (q:3000) is 3 commits behind — the owner pushes manually (push policy:
  ask, `.agents/push-policy.md`).
- 2026-07-04 landed a large batch, all owner-approved and verified:
  - All five approved plans implemented (see `.agents/plans/*`): post-playback
    watch-state refresh (`playback-ended` event); platform-aware sshfs
    guidance in the add-SSH UI; each SMB/SSH mount registered as its own
    named source; split artwork policy (16:9 resume surfaces, 2:3 catalog
    rows with series posters); library rework phases A-D (persistent listing
    cache for local/SMB, consolidated type-based All nav, cross-source dedup
    via provider ids with backing lists, kind-ranked playback with per-title
    override persisted in `merged_overrides`).
  - The batch then passed a cross-harness review loop (playbook
    `reviewloop`, reviewer codex): 5 findings fixed, guard-proven,
    independently verified, merged. Durable trail:
    `.agents/review/index.md` + `findings/`. Notable outcome: the merged
    All view pages from an immutable `MergedSnapshot` in `AppState`
    (stateless merged pagination was proven unsound in review).
  - Post-review owner-directed UI changes (decisions recorded 2026-07-04 in
    `.agents/decisions.md`): the Continue Watching hero is a cover-flow
    (~30% window height, older items fanned behind-left, newer behind-right,
    always-visible arrows) fed by Vela's OWN recents — semantic: "recently
    played and not finished = Continue Watching", any source, any duration
    (`src-tauri/src/recents.rs`; snapshot at play, position stamped at mpv
    exit via `EndNotify(u64)`, finished entries dropped at
    `watched_threshold_percent`, default 95%). Library nav moved to a left
    sidebar (Home / Library / Sources groups, Infuse reference
    `reference_screens/infuse-home-reference.png`).
- Token/credential stance: poster URLs (all backends) and Jellyfin/Emby
  stream URLs are accepted local-only exposures; SMB mount arguments remain
  one only on macOS/Windows (Linux credentials never leave the process —
  libsmbclient auth callback); Plex stream auth rides as an `X-Plex-Token` header via an
  owner-only mpv include file. Add nothing new that logs or displays
  token-bearing URLs. Recents snapshots in `config.json` carry poster URLs —
  same exposure class as the config's stored tokens.
- macOS SSH live testing is parked (brew macFUSE/sshfs-mac unstable on the
  owner's machine); the shipped in-UI guidance is the decided handling.
- Known accepted v1 gaps: backend queue auto-advance plays are not
  snapshotted into recents; local-source series artwork deferred (portrait
  cards fall back to episode still/no-art); a merged card's progress bar can
  reflect server state while the ranked play target is a local copy (the
  per-title override is the escape hatch).
- `scripts/build.sh` takes ~2.5 min cold on macOS; the session's `!`
  foreground runner kills at 2 min mid-DMG (leaves a mounted staging volume
  and no final dmg) — run builds via the agent (no cap) or a real terminal.
  The `.app` exists only transiently during bundling; the DMG is the
  artifact.

## Next

- E2E harness next slices (skeleton is done): (a) SMB add→browse→play
  scenario — needs owner NAS creds via `VELA_E2E_SMB`/`_USER`/`_PASS` env
  at run time; (b) the mpv-IPC playback probe leg; (c) hero/curation
  scenarios from the plan backlog. The harness replaces most of the owner
  playtest list below; the owner keeps visual flip-throughs and HDR
  judgment.
- Owner (or harness, once built) checks for the new batch: share-root
  auto-add against the real NAS; On Deck item (*Blood and Bone* class)
  appearing in the flow; mark-watched dropping an item live;
  remove-from-continue sticking across restart and replay restoring it.
- Owner playtest sweep of the whole 2026-07-04 batch (v0.1.9 dmg is built):
  sidebar nav; cover-flow hero — a few-seconds play should appear centered
  after mpv closes (recents semantic), and a >60s Plex play should also
  reach the server hub; watch-state refresh without restart; named SMB
  share in Sources; merged All view listing (scroll depth, "N sources"
  cards, context-menu "Play from" persisting an override); sshfs panel
  guidance.
- Finish live smoke tests: Emby, local folders, SMB browse/playback depth.
- Plex stream header auth residuals: owner eyeball check on a real play
  (title bar / Shift+I clean), EDL split-file exercised only by unit tests,
  Jellyfin/Emby stream-URL parity follow-up.
- If updating broader governance metadata, refresh `.agents/repo-map.json`
  and `.agents/artifact-manifest.json` from their old `validated_against`
  commit.

## Blockers

- None recorded.

## Verification

- See `.agents/repo-map.json` for the current automated verification
  commands (npm check/build; cargo check/clippy/test from `src-tauri/`).
  Rust suite: 71 tests; clippy `-D warnings` clean.
- Rust verification on Linux needs the Tauri/WebKitGTK system dependencies
  used by CI.

## Active Sources

- `AGENTS.md`
- `.agents/repo-guidance.md`
- `.agents/repo-map.json`
- `.agents/decisions.md`
- `.agents/plans/` (all five 2026-07-04 plans carry implementation notes)
- `.agents/review/index.md` (completed review loop, durable trail)
- `README.md`
- `ISSUES.md`
- `.review/deduped_action_list.md` and `.review/gpt_review.md` as historical
  evidence only

## Unrecorded Repo Memory

- None known.
