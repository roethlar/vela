# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change.

## Now

- LANDED 2026-07-05 — mpv autocrop bundle (`.agents/plans/mpv-autocrop-bundle.md`,
  owner-approved; decision in `.agents/decisions.md` 2026-07-05 "Ship mpv's
  autocrop.lua behind an opt-in toggle", which REVERSES the same-day crop-drop for
  this narrow bundled-script case only). Owner reopened the dropped crop feature
  (option "C") to ship mpv's own `autocrop.lua` + a Settings control. Three
  commits, each `reviewloop codex`-accepted (plan itself converged r1-r4):
  - slice 1 `95d4b1a`: vendored `autocrop.lua` (mpv `efb70d7f`) + LICENSE.GPL +
    PROVENANCE under `src-tauri/resources/mpv-scripts/`; `tauri.conf.json` resources
    map + `bundle.license = "MIT AND GPL-2.0-or-later"`; PKGBUILD installs to
    `/usr/lib/Vela/mpv-scripts/` + `license=('MIT' 'GPL2')`. Verified: deb AND Arch
    packages ship the files at the resolver path (`.PKGINFO` = MIT+GPL2).
  - slice 2 `c66e680`: tri-state `mpv_autocrop` config (off/manual/auto); resolver
    via `AppHandle::path().resolve(mpv-scripts/autocrop.lua, Resource)`; mode-branched
    `--script` injection in `play()` (manual adds `autocrop-auto=no`; auto = crop on
    start). Guard-proven arg tests.
  - slice 3 `be292e0`: Settings → Advanced mpv three-state selector
    (Off/Manual/Automatic); Automatic carries the D-state/HDR hang warning.
  Off by default; **Automatic auto-fires the live `video-crop` D-state path** (owner
  chose to have it available, disclosed via UI). Version bumped to 0.1.22
  (`f237705`); handoff `607143d`. REMAINING: owner playtest only (does Shift+C /
  Automatic crop cleanly on the real HDR stack). NOTE: rpm `License` tag not locally
  verifiable (no rpm tooling on the Arch dev host).
- CURRENT WORK (2026-07-05) — implementing
  `.agents/plans/smb-ssh-playtest-fixes.md`, **owner-approved** after a 3-round
  codex plan-reviewloop (accepted at r3; trail in the plan's Review log). Five
  bugs from the owner's 0.1.21 NAS playtest (SMB share + SSH folder, same host);
  diagnoses done this session, SMB/SSH seek mechanisms adversarially verified.
  **Bug 1 sub-slice 1 LANDED 2026-07-05** (code `941b933`; three
  reviewloop-codex fixups `08fef74`/`79f3979`/`401fd1b`; version bump 0.1.23
  `3e9256d`; all UNPUSHED — owner pushes manually). It: (a) moved the share-root
  `list_dir("")` out of `SmbConnection::connect` (per-seek/stream/browse hot path)
  into `verify_mount` (add-time only) — connect verifies lazily on first op; and
  (b) caches the entity length per proxy token so a seek skips the redundant
  `stat`, via `SmbConnection::open_read_with_len` + `open_raw`. The per-token cache
  is scoped to one playback (cleared on token reuse) and generation-guarded against
  stale writers. `reviewloop codex` converged at r4 (accepted clean) after three
  reopens, each a real distinct defect, all guard-proven — trail
  `.agents/review/index.md` + `findings/sspf-1..3.md`. Hermetic guards in
  `stream_proxy.rs` tests (seek reuses cached len; reuse clears stale len; stale-gen
  store rejected).
  **Bug 1 sub-slice 2 LANDED 2026-07-05** (code `d45ffe3`; reviewloop-codex fixup
  `8f41b90`; version bump 0.1.24 `92b1984`; UNPUSHED). Per-response write deadline
  on the proxy socket in `serve_target` (`stream_proxy.rs`
  `DEFAULT_WRITE_TIMEOUT_MS`, atomic, test-configurable) so a non-draining client
  can't pin a thread + a bounded point for sub-slice 3's cooperative cancel. codex
  reopened once (sspf-4): a short deadline breaks a normal long mpv pause because
  ffmpeg HTTP reconnect is off by default → premature EOF on resume. Fixed by
  enabling ffmpeg reconnect for the loopback proxy stream on the mpv side
  (`playback::proxy_reconnect_args`, `--stream-lavf-o-append=reconnect=1/…`,
  scoped to `http://127.0.0.1:`, asserted after user args) AND raising the deadline
  default to 300s so it's a backstop, not a pause-killer. Trail `findings/sspf-4.md`.
  Owner playtest still owes the end-to-end check (pause past the deadline resumes
  seamlessly — needs mpv + a live drop).
  **Bug 1 sub-slice 3 LANDED 2026-07-06** (code `05ed86b`; five reviewloop-codex
  fixups `c7211e6`/`5a64172`/`dec0121`/`ada9f65`/`ab3f74c`; trail `14b0102`; version
  bump 0.1.25 `503593a`; UNPUSHED). Per-token SMB session reuse — the real seek fix:
  the loopback proxy caches the live `SmbConnection` per token (created once, reused
  by every seek — each connection opens its OWN file handle) instead of rebuilding a
  libsmbclient session per seek. A session is stored only for the current, still-live
  play (`generation` = which play, bumped on replay; `active` = is it live; invariant
  `active==false ⇒ session==None`), freed once at playback-end (generation-guarded
  compare-and-remove) — or by the play path if `play()` fails — always OFF the
  registry lock AND off async workers; evicted entries drop off-lock too. A per-token
  `serve_epoch` cooperatively cancels a superseded in-flight serve (GET bumps, HEAD
  doesn't). `ctx_lifecycle_lock` UNCHANGED (per-seek context churn removed instead).
  `reviewloop codex` converged r1-r5 (r1-r4 reopened, r5 accepted clean) — five real
  distinct defects, all guard-proven (trail `.agents/review/index.md` +
  `findings/sspf-5..9.md`); the fixes built toward a correct session-lifecycle model
  (a healthy converging loop, not a stall). **Bug 1 (SMB seek) is now COMPLETE** —
  all three sub-slices landed. REMAINING: owner NAS playtest — confirm the felt
  seek-freeze is gone on the real share.
  **Bug 3 (source-click dead-end) LANDED 2026-07-06** (code `b9cca81`;
  reviewloop-codex fixup `6837157`; trail `f73cdaa`; version bump 0.1.26 `01c54ec`;
  UNPUSHED). A scoped source whose per-source Home settles empty (no hubs AND no
  hero/recents) but has browsable sections now lands on its content (opens the first
  section) instead of the "Nothing on your home screen yet" dead-end; a server source
  that returns Home hubs keeps its Home unchanged. Implemented as a reactive
  `!loading`-gated `$effect` in `+page.svelte` (NOT a tail of `selectSource`), so it
  covers source click, Home, AND Back uniformly. `reviewloop codex` converged r1-r2:
  r1 reopened sspf-10 (HIGH — the first cut lived only in `selectSource`, so Home/Back
  still dead-ended and re-clicking the source early-returned = trap) + sspf-11
  (MEDIUM — reading `hubs`/`heroItems` after `await loadEverything()` could read a
  superseded load → force-browse a slow server source); both fixed by the
  reactive-effect approach; r2 accepted clean. Guard
  `tests/e2e/scenarios/sourcedeadend.mjs` (both directions + Home-button leg),
  guard-proven red/green — ran HEADED (Xvfb absent on this host; owner-approved
  2026-07-06). REMAINING: owner playtest — clicking the SMB/SSH source on the real NAS
  lands on content.
  **Bug 5 P1 (Connected-tab triplication + erroring Remove) LANDED 2026-07-06**
  (code `9c3597a`+`9379ec5`; reviewloop-codex fixup `0a64cd0`; trail `eb6a85a`;
  version bump 0.1.27 `d88a277`; UNPUSHED). `9c3597a`: the Connected registered-source
  loop now excludes the whole local family (`LOCAL_FAMILY_KINDS = ["local","smb","ssh"]`
  const mirroring the backend), dropping the leaked smb/ssh source row whose Remove
  called `remove_source` and errored (a dead-end) + the triplication. `9379ec5`:
  `remove_smb_folder_in_config` (pure, unit-tested) refuses to remove a mount's last
  folder (a zombie zero-folder invisible share), and the UI `removeSmbFolder` cascades
  a last-folder Remove to a full `unmount_smb` (clean, no error). `reviewloop codex`
  converged r1-r2: r1 reopened sspf-12 (MEDIUM — the frontend filter+cascade shipped
  with no automated guard; codex showed a hermetic guard IS feasible via a native
  mountless SMB seed), fixed by `tests/e2e/scenarios/connectedtab.mjs`; r2 accepted.
  Full CI green (cargo test 102, clippy -D warnings clean). REMAINING: owner playtest
  — one row per SMB/SSH mount on the real NAS, no erroring Remove.
  **Bug 2 (SSH seek) LANDED 2026-07-06** (code `2174d2e`+`0bbff29`; reviewloop-codex
  fixup `314d76c`; trail `d11ce50`; version bump 0.1.28 `49072f8`; UNPUSHED). SSH uses
  the raw sshfs mount (NOT the SMB proxy); its single default SFTP channel
  head-of-line-blocks a seek's read behind the readahead backlog → stall on a latency
  link. Fix: `-o max_conns=4` (parallel SFTP channels) added to the sshfs options,
  now via `sshfs_options_for(target_os)` — **Linux-only** (macOS sshfs-mac 2.10
  rejects max_conns; codex sspf-13 caught the unconditional version as a HIGH macOS
  regression). Owner chose the FUNCTIONAL hermetic guard (loopback sshd+sshfs mounts
  with the option set + reads correctly; gated on sshd/sshfs/ssh-keygen + /dev/fuse,
  skips gracefully) over a latency-repro — localhost has ~0 latency, so the stall
  can't reproduce hermetically. `reviewloop codex` converged r1-r2 (r1 sspf-13, r2
  accepted). Full CI green (cargo test 105, clippy -D warnings clean). REMAINING:
  owner NAS playtest — SSH seek no longer hangs (the authoritative stall-fix check).
  **OWNER REORDER 2026-07-06: Bug 5 P2 done first, THEN Bug 4, THEN metadata rail.**
  The owner chose to do the small Bug 5 P2 (naming/rename) before the larger Bug 4,
  deviating from the plan's "Slice order & commits" (which had Bug 4 before Bug 5 P2).
  Reason: bank the small win and let the three landed P1 fixes (0.1.26–0.1.28) be
  playtested before the large P2 metadata work lands on top.
  **Bug 5 P2 (source naming + rename) LANDED 2026-07-06** (code `c83a1be`+`55a6852`;
  reviewloop-codex fixup `5053d2b`; trail `b7652d0`; version bump 0.1.29 `fce064b`;
  UNPUSHED). **Bug 5 is now COMPLETE** (P1 0.1.27 + P2 here). `c83a1be`: an added
  SMB share / SSH folder now gets a friendly default label — bare share (SMB) / last
  remote-path segment (SSH), disambiguated against existing local-family labels
  (qualify with server/host, then numeric suffix; case-insensitive) via pure
  `unique_mount_name`/`last_path_segment` — instead of the URL-shaped
  `server/share` / `host:remote_path`; plus an optional **Name** field in both add
  forms (passed through the existing `name` param — NO schema change). `55a6852`:
  `rename_smb_mount`/`rename_ssh_mount` commands (pure `rename_*_mount_in_config`
  helpers; propagate the new label to the name copies seeded at add time — the SMB
  share-root folder `path==""` and the SSH-fed local folder — only when that copy
  still equals the OLD mount name, so a user-renamed folder is left alone) + an
  inline rename affordance in the Connected tab. `reviewloop codex` converged r1→r2:
  r1 reopened sspf-14 (MEDIUM — rename **Save** stayed enabled on a blank field, so
  the click surfaced the "A name is required." error, a Bug-5-UX-ruling-forbidden
  error click); fixed `5053d2b` (Save disabled on blank + silent Enter no-op; backend
  still rejects empty defensively). r2 accepted clean. Full CI green (cargo test 118,
  clippy -D warnings clean, npm check + build clean); pure helpers guard-proven
  red/green by the coder (codex ran read-only → `guard_confirmed:false`, its value
  was the code review that surfaced sspf-14). Trail `.agents/review/index.md` +
  `findings/sspf-14.md`. REMAINING: owner playtest — friendly default labels appear
  and rename works on the real NAS.
  **IMMEDIATE NEXT ACTION: Bug 4 (LARGER) — share/mount root shows bare metadata-less cards,
  starves the merged view (P2, the metadata unlock).** The share-root auto-add
  registers the whole share as ONE flat kind-auto folder; a NAS root of category dirs
  (Movies/, TV/…) is mis-classified into one flat section of bare cards (no
  title/year/poster), which can't dedup against server copies in the merged All view.
  Fix (see plan Bug 4 for the full design + the THREE codex-pinned constraints): make
  kind-auto roots category-aware — expand each into per-category effective
  `LocalFolder` roots so `sections()`/`items()` see normal configured folders (the
  `items()` guard at `local.rs:744` rejects non-configured section keys — do NOT
  loosen it); key the detected-kind cache by **source/mount id + path** with a schema
  bump (raw-path keys collide across mounts and a stale root kind preserves the flat
  classification); and run the expansion OFF-LOCK on the blocking pool (NOT under
  `config::update`/`source_lock` — the lock-across-blocking invariant slice 7
  `e7c5231` honored). This is the metadata unlock (`metadata.rs` already resolves
  .nfo/artwork sidecars over the VFS + keyless iTunes/TVmaze once items parse at the
  right level). DESIGN FORK to settle first: lazy effective-root cache inside
  `LocalSource` (expand under `run_blocking` in sections()/items()) vs a
  config-snapshot expand-then-swap.
  **THEN the metadata-gated local/SMB/SSH "Recently added" rail** (last slice; depends
  on Bug 4 — only items with resolved metadata, never blank filename cards).
  STANDING INSTRUCTION: `reviewloop codex` on every slice; bump version per landed
  code slice (routine). Reviewloop mechanics that worked this session (0.1.26–0.1.28):
  codex incantation `codex exec --json -s read-only --output-schema <schema> "<prompt>"
  </dev/null` (final verdict in the last `item.completed` agent_message; schema =
  `{verdict,guard_confirmed,reviewed_sha,base_sha,comments}`, fail-closed); pin base =
  pre-slice SHA, head = the fixup commit each round; per slice: trail commit
  (`review(...)`) + version bump + `handoff:` state commit. This session's three loops
  each reopened once then accepted — every finding was real (sspf-10/11 Home/Back
  dead-end trap, sspf-12 missing frontend guard, sspf-13 macOS max_conns regression).
  E2E/test techniques earned this session (reuse them):
  - **E2E must run HEADED on this host** — Xvfb is NOT installed, so the harness's
    default headless mode fails; run `VELA_E2E_HEADED=1 npm run e2e -- <scenario>`
    (owner-approved 2026-07-06; it pops a window over the desktop). Frontend slices
    rebuild the debug binary to embed the change, so guard-proof = revert + rebuild +
    run RED, restore + rebuild + run GREEN.
  - **Hermetic Connected-tab E2E**: seed a NATIVE SMB mount (`mountpoint:""`) in
    `config.json` — renders from config (get_sources + list_smb_mounts) with NO
    connection (`tests/e2e/scenarios/connectedtab.mjs`).
  - **Hermetic sshfs test**: a non-root loopback sshd (StrictModes no, UsePAM no,
    key-only) + sshfs mount + read, gated on sshd/sshfs/ssh-keygen + /dev/fuse
    (`src/sshfs.rs` `max_conns_option_set_mounts_and_reads_over_loopback_sshd`).
  - **Testable platform gates**: split on an explicit OS string (`sshfs_options_for(os)`
    via `std::env::consts::OS`) instead of `#[cfg]`, so both branches unit-test from
    any host.
  Load-bearing gotchas from the plan-review a
  fresh session must not relearn:
  - SMB seek (slice 1): every mpv seek rebuilds a full libsmbclient session; fix =
    per-token session reuse (own file handle per stream, generation-owned
    compare-and-remove cleanup, cooperative cancel bounded by `OP_TIMEOUT_MS`).
    Do NOT release `ctx_lifecycle_lock` around `smbc_free_context` (codex blocker
    — lifecycle race); remove per-seek context churn instead.
  - SSH seek is DISTINCT (raw sshfs path, NOT the proxy): fix = sshfs mount
    options (max_conns/cache) + a required hermetic loopback sshd+sshfs test.
  - Bug 4 (share-root) expansion must run OFF-LOCK on the blocking pool (never
    `detect_kind`/`read_dir` under `config::update`/`source_lock`); key the kind
    cache by mount id + path with a schema bump.
  - Source-click routing keys on the empty-Home STATE (not "any source") so
    server-source Home hubs aren't regressed.
  - METADATA (owner-gated the whole thing on this): no product shift.
    `metadata.rs` already resolves `.nfo`+artwork sidecar OVER THE VFS (so SMB/SSH
    work) + keyless iTunes/TVmaze online; the blank filename cards ARE Bug 4's
    mis-classification, and Bug 4 is the metadata unlock. The local/SMB/SSH "Recently
    added" Home rail (last slice) is metadata-gated (only items with resolved
    metadata; never blank filename cards).
- E2E slice 11 landed 2026-07-05 (`7c899be`) + review loop e2e-10 CLOSED
  (eh-15 verified, fix `6db391c`): the markwatched scenario now round-trips
  mark-unwatched (DELETE PlayedItems, server flip, badge clears). eh-15
  then hardened BOTH badge legs — each assertion gates on a later
  `/Users/{u}/Items` server refetch and asserts a PRESENT card, so a
  refetch that drops the card or serves stale state can no longer pass on
  the optimistic mutation or the empty-grid refresh gap (the old
  `!card?...watched` unwatch wait was vacuously true while the card was
  missing). Guard-proven with a drop-after-unwatch mock (pre-fix scenario
  GREEN on the dropped card, fixed scenario RED) and codex-accepted; a
  3-lens adversarial pre-review (all `refuted:false`) confirmed the
  optimistic *watched* card never paints (batched Svelte flush), so the
  unwatch leg was the load-bearing hole. Suite: 9 scenarios. The
  locally-testable backlog stays EXHAUSTED; SMB + live scenarios remain
  owner-cred-gated (see Next).
- E2E slice 10 landed 2026-07-05 (`5742789`) + review loop e2e-9 CLOSED
  (eh-14 verified): the merged All view scenario machine-verifies the
  library-rework owner checks — mock JF + local folder dedup to ONE
  "2 sources" card; "Play from Local" plays the file path while "Play
  from Mock JF" plays the mock stream; the per-title override persists
  under the exact canonical key (`title:mockmovie|2020`) and flips with
  the choice. Suite: 9 scenarios. Unblocked follow-on remaining:
  mark-unwatched (small).
- E2E slice 9 landed 2026-07-05 (`ccc6270`) + review loop e2e-8 CLOSED
  (eh-13 verified after the loop's first reopen→fix→accept round-trip):
  the watch-state scenario machine-verifies the 2026-07-04 owner-reported
  "stale until restart" fix end-to-end — mpv plays THROUGH the mock's
  Range-capable HTTP stream, Start/Stopped check-ins carry correct
  ItemId/MediaSourceId/PositionTicks, and the card gains '% watched'
  without restart. Suite: 8 scenarios. Remaining unblocked follow-ons:
  mark-unwatched; merged All view (mock JF + local folder = two sources).
- E2E slice 8 landed 2026-07-05 (`c706228`) + review loop e2e-7 CLOSED
  (eh-12 verified): NEW HARNESS LEG — a hermetic mock Jellyfin server
  (`tests/e2e/mockjf.mjs`, stateful, fail-closed on the client's Items
  query contract) runs inside the runner; a seeded `sources` entry
  restores it at boot with no auth. The mark-watched scenario asserts
  both the PlayedItems POST and the watched badge surviving the refetch.
  Suite: 7 scenarios. UNBLOCKED FOLLOW-ONS this leg enables without owner
  creds: watch-state-refresh-after-playback scenario (mock + local play),
  mark-unwatched, and a merged All view scenario (mock JF + local folder
  = two sources).
- Slice 7 landed 2026-07-05 (`e7c5231`, loop app-1 CLOSED clean):
  `resolve_stream`'s VFS checks (canonicalize/is_file — network-priced on
  native SMB) moved onto the blocking pool per the async-worker invariant;
  full Kimi-P0 audit against current code came back all-resolved and
  repo-map's stale deferred-issue note was corrected.
- E2E slice 6 landed 2026-07-05 (`fc902f4`) + review loop e2e-6 CLOSED
  (clean pass, no findings): search scenario — short-query validation
  error, hit/miss result filtering, play-from-results over mpv IPC.
  Suite: 6 scenarios. The harness's locally-testable backlog is now
  EXHAUSTED — remaining scenarios (SMB, merged All view, server-gated
  mark-watched/watch-state/Plex-removal) are blocked on owner-provided
  creds/test-server env (see Next).
- E2E slice 5 landed 2026-07-05 (`9274ac2`) + review loop e2e-5 CLOSED
  (eh-11 verified): the queue scenario proves backend auto-advance live —
  clip A to natural EOF over IPC, "Play next"-queued clip B spawns in a
  fresh mpv session with no UI interaction. eh-11 hardened it against the
  EOF-races-UI-window flake (A paused across the menu/screenshot window).
  Suite is now 5 scenarios: smoke, playback, curation, resume, queue.
- E2E slice 4 landed 2026-07-05 (`2f5bba8`) + review loop e2e-4 CLOSED:
  the resume scenario caught eh-10 (`4527613`), a HIGH app bug —
  **Continue Watching restarted local/SMB/SSH items from 0:00** (local
  provider resolves resume_ms 0; play_by_key ignored Vela's own stamp).
  play_by_key now falls back to `recents::resume_stamp_ms` when the
  provider resolves 0; server positions still win. Queue auto-advance
  inherits the fallback (routes through play_by_key). Codex batch pass on
  the slice itself: clean, no findings.
- E2E slice 3 landed 2026-07-05 (`ee01101`) + review loop e2e-3 CLOSED
  (eh-8, eh-9 verified): the curation scenario drives
  remove-from-continue via the hero's real context menu, proves tombstone
  APPLICATION across a true app restart (session recycle; recents entry
  reinserted while the app is down so only the tombstone can keep the
  hero empty; environ-scoped pid guard immune to the owner's own Vela),
  and replay-restore. This live-verifies the cw-3 semantics that were on
  the owner-check list. Mark-watched stays server-gated (local items have
  `played: null` — no menu entry).
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
- Version 0.1.29 (bumped 2026-07-06, `fce064b`, for Bug 5 P2 naming + rename).
  The owner pushes manually (push policy:
  ask, `.agents/push-policy.md`); treat remote positions as owner-managed.
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

- QUEUED (owner-parked 2026-07-05 — "after current work", do NOT dig into it
  before then): GitHub CI is RED on the last PUSHED commit `05f9594`. Two jobs
  failed: `cargo audit` (quick-xml RUSTSEC-2026-0194/0195, published 2026-06-29 —
  an advisory-DB update, NOT from our commit; that job is `continue-on-error:
  true`) AND `Rust (src-tauri)` on `cargo check --locked`. Local `cargo check
  --locked` PASSES on 0.1.21, so the CI Rust-job failure is unexplained (clean-env
  / network / clean-build?) and not yet triaged. The plan commits after `05f9594`
  are docs-only and UNPUSHED, so CI hasn't seen them (owner pushes manually).
- The current SMB/SSH playtest-fix work (see the CURRENT WORK entry at the top of
  Now) is the active track; the older E2E-harness/playtest items below are
  largely superseded or owner-cred-gated.
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
  clippy runs `-D warnings`. (Exact test count lives with the suite / CI, not
  restated here — an earlier "71" vs "78" in this file was drift.)
- Local `cargo check --locked` passes on 0.1.21; but GitHub CI is currently RED —
  see the QUEUED CI item in Next (untriaged `cargo check --locked` failure on the
  CI runner + the advisory-only `cargo audit` job).
- Rust verification on Linux needs the Tauri/WebKitGTK system dependencies
  used by CI.

## Active Sources

- `AGENTS.md`
- `.agents/repo-guidance.md`
- `.agents/repo-map.json`
- `.agents/decisions.md`
- `.agents/plans/` — the 2026-07-04 plans (implementation notes) plus the ACTIVE
  `.agents/plans/smb-ssh-playtest-fixes.md` (owner-approved, in implementation)
- `.agents/review/index.md` (completed review loop, durable trail)
- `README.md`
- `ISSUES.md`
- `.review/deduped_action_list.md` and `.review/gpt_review.md` as historical
  evidence only

## Unrecorded Repo Memory

- None known.
