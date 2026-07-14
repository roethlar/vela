- **PER-SURFACE-STATUS: COMPLETE — all five slices landed 2026-07-14 (0.1.46), awaiting
  owner playtest.** Plan `.agents/plans/per-surface-status.md`; decision
  `.agents/decisions.md` (2026-07-14). Every failure now reports on the surface it belongs
  to: the view's banner keeps listing/refresh/search failures, and the watch-state edit
  (`fee7f0e`), the queue (`67358fd`), the mpv bar (`0f41c7b`) and the detail page
  (`40dfc40`) each report their own. Slice 5 (`282702b`) then DELETED the whole refereeing
  apparatus — the `owner` field, `ErrorOwner`, `clearOwned`, per-surface clearing, the scope
  merge — net -67 lines. e2e 18/18, cargo test 95, clippy/svelte-check/build clean.
  - **What it bought:** the defect class that ran for EIGHT review rounds (r17-r24, each fix
    opening the next door, always the same loss — a failure the user needed, silently gone)
    is structurally gone, because the fight over one surface is gone. Slice 1 alone
    collapsed SIX e2e cases, three of which had asserted that a failed edit must be
    SUPPRESSED when the user navigated away — never right, just the price of sharing.
  - **GUARDED after all: slices 2 and 4** (`surfaces` scenario, `537ba70`, red-proven four
    ways). I had recorded them as unguardable and that was WRONG, twice, both times because
    I reasoned about the code instead of reading it — the mock CAN fail a Play, because
    `play_by_key` resolves the stream before it spawns mpv. **Before recording anything as
    unguardable, go and read the failure path.** Building the guard then found a real bug in
    slice 2 (closing the drawer abandoned an in-flight action, dropping a failure the user
    needed and making the chip's mark dead code).
  - **OWNER PLAYTEST ASK (0.1.46).** Slice 3 (the mpv bar) is the only one automation cannot
    reach — `install_mpv` cannot be made to fail. The rest is confirmation: (1) kill the
    server mid-edit, mark watched — the failure lands on its own line, follows you when you
    navigate, and a second edit replaces it; (2) a failed mpv install reports on the mpv bar
    and a search no longer wipes it.
  - The library-refresh-scan review loop is CLOSED at r24; its log is the evidence for this
    plan. Do not reopen it against the shared banner.
# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

## Now

- **LIBRARY-REFRESH-SCAN: COMPLETE — owner playtest VERIFIED on REAL PLEX
  2026-07-14 (0.1.45).** Refresh button + per-library server scan trigger. Plan:
  `.agents/plans/library-refresh-scan.md`. As of `22dad8b`: E2E 17/17 on the VM;
  `cargo test` 95; clippy `-D warnings`, svelte-check, npm build all clean. (CI runs on
  the `github` remote only, and local main is ahead of it — the owner pushes.)
  - **Owner playtest (all six green, real Plex):** refresh while in a library;
    library RENAMED then refresh (sidebar + breadcrumb both update); library
    DELETED while standing in it then refresh (reconciled to Home); right-click →
    Scan Library (Plex really scans — **the Plex scan path had never touched a real
    server before this**); general use with no ~5s stalls (confirms the r16-2
    `/identity` fix); footer 0.1.45. Detail in the plan's `## Owner playtest`.
  - **Where the trail lives:** the plan's `## Code review log` — every round, every
    finding, every fix commit, the declines (r10-1 UPHELD; r8-4 and r12-1 OVERTURNED on
    independent adjudication), the guard gaps, and the process disclosures. Do not
    reconstruct any of it from chat.
  - **r19 LANDED (2026-07-14).** codex 1 HIGH + 3 MEDIUM, grok 1 MEDIUM — and BOTH
    reviewers, independently, found the same defect in the r18 `setWatched` fix. Every
    r19 finding was in code written during r17/r18; nothing in the original slices was
    faulted. Fixes: `563b2fb` (HIGH — an identity probe's answer was written onto the
    server that REPLACED the one it described, pinning rediscovery to a lie: the third
    distinct route to the wrong-server scan in this plan, and the second one opened by
    a fix for the previous one) and `91045cb` (a failed edit stomped the banner
    explaining its own empty grid, and could paint itself on a root the user had left).
  - **r20 LANDED (2026-07-14).** codex 3 MEDIUM + 1 LOW, grok 2 MEDIUM — converging
    independently, for the third round running, on the same defects. Both were in the
    r19 fixes, and the first was **r19's own bug returning through a door r19 opened**:
    the combined banner inherited the listing's generation tag, so the refresh's
    RETRACT erased the user's failed edit (`64972ac` — the banner is now a list of
    OWNED parts, and a retract takes only what it superseded). The second: the r19
    currency gate was wrong in BOTH directions — the Plex link screen replaces the whole
    view while bumping no load generation, and re-entering the library you are standing
    in bumps both counters while going nowhere (`39ead92` — `rootSig()` asks the view
    what root it is on, instead of inferring it from counters).
  - **r21 + r22 LANDED (2026-07-14).** r21: codex 5 MEDIUM + 3 LOW, grok 1 HIGH + 3
    MEDIUM + 1 LOW. r22: codex 3 MEDIUM, grok 1 MEDIUM. Both rounds' top finding was
    found independently by BOTH reviewers, and both were in the PREVIOUS round's fixes.
    Nine fix commits, `d7cb3ef`..`74fc3ad`, each with a red-proven guard where the
    harness can reach it. Detail in the plan's `## Code review log`.
  - **r23 LANDED (2026-07-14).** codex 4 MEDIUM + 1 LOW, grok 3 MEDIUM + 1 LOW — and all
    three top findings were in ONE commit: the `linking` flag invented two commits
    earlier, which shipped three defects of its own (it dropped an edit made on a grid the
    user never left; it stuck true forever after any source change abandoned a link; it
    could not retract a banner already on screen). Replaced by a SCOPE on each banner part
    (`0a79013`) — which is what the model needed from the start. Also `78f7ba0` (skipping
    the repaint was skipping a needed heal — Continue Watching kept showing a rolled-back
    item as gone) and `f61bc71` (the delivery witness overstated what it saw).
  - **First reviewer-vs-reviewer disagreement of the loop (r23):** grok blessed the r22-2
    early return, codex found it skipped a heal. Both positions were satisfiable at once,
    so it was FIXED, not escalated. Escalate to the owner only when they genuinely cannot
    both hold.
  - **r24 LANDED (2026-07-14).** codex 6 MEDIUM, grok 3 MEDIUM. Every finding was in an
    r23 fix or an r23 guard. The r23 scope was defeated on the very next navigation
    (`setError(null)` — which every load start calls — still wiped app-scoped parts): the
    SEVENTH door into the same silent loss, opened by the fix for the sixth. Banner parts
    now name the SURFACE that owns them (`da99a46`), and the heal retracts what it repairs
    and stays off Welcome (in `da99a46`; guards `49a2141`).
  - **THE LOOP IS CLOSED AT r24, and its conclusion is now an approved plan.** The findings
    had migrated out of this feature into a pre-existing design weakness: one shared error
    banner carrying failures from four surfaces with four different lifetimes. The owner was
    asked and chose the durable fix: **per-surface status**
    (`.agents/plans/per-surface-status.md`, decision 2026-07-14). The r17-r24 log is the
    evidence for it — read it before touching the banner again.
  - **A THIRD process violation, disclosed, NOT rewritten:** `da99a46` is MIS-DESCRIBED —
    its message covers only the banner-owner model, but it also carries two production
    fixes to the heal. Root cause, for the third time: staging a whole path instead of the
    hunks just written.
  - **REVIEW PROTOCOL (owner, 2026-07-14) — now standing:** TWO independent
    reviewers (`codex` and `grok`) on the same pinned diff, neither seeing the
    other's findings; the author writes the fixes and runs every guard, red-proof
    and E2E run. **An author may NEVER adjudicate their own decline** — it goes to
    the reviewer that did not raise it. That rule exists because author
    self-adjudication was tested twice and failed twice (r12-1, r8-4, both
    overturned). Reviewer-vs-reviewer disagreement goes to the owner.
  - **Why it is still running (r17-r22 evidence):** in this subsystem the author's
    FIXES carry defects at the same rate as the original code. EIGHT rounds running, the
    newest fix has carried a defect of the same CLASS it was fixing, through another
    door — and the class never changes: **a failure the user needs is silently lost.**
    It has now been reached through the publish door, the ordering door, the retract
    door, the dedup door and the setError door, each opened by the fix for the last
    (plus, twice, a wrong-server scan reintroduced by the fix for the previous
    wrong-server scan). The two reviewers have converged, independently, on the same top
    finding in FOUR straight rounds. A single reviewer — or the author alone — ships
    every one of them.
  - **THE FIX IS THE MOST DANGEROUS CODE IN THE REPO.** Not the original. Review the
    newest fix hardest, and never treat "this one is simple" as a reason to skip it.
  - **A SELF-AUDIT IS NOT A CHECK.** r20-2 is a defect the author looked straight at
    during his own r19 audit, in a message claiming every writer had been traced, and
    waved through on an assumption (that the Plex link screen was a modal over the grid;
    it REPLACES the view) that one grep would have falsified. That is the third
    unverified author assumption a reviewer has overturned (r12-1, r8-4, r20-2). When
    the author's reasoning says "this one is fine", that is the moment to go and look.
  - **Guard discipline (the transferable lesson):** a long and still-growing list of
    guards in this plan turned out VACUOUS — disarmed by the author's own later fixes,
    written vacuous while actively trying not to, or left guarding a behavior that could
    be deleted outright with the suite green. The plan's review log owns the roll-call.
    **Not one ever failed or warned. Every one was caught only by injecting the
    regression and demanding the test go red.** Re-prove a guard whenever behavior
    around it changes, and prove each behavior a fix claims SEPARATELY.
  - **OPEN, recorded, not fixed:** r13-2 (reads carry no binding — owner-DEFERRED to a
    follow-up plan, do not re-raise in review); the tall-viewport request storm is
    untestable at the harness viewport; no guard on scan invalidation when a source is
    removed (r16-3).
  - **FIXES THE HARNESS CANNOT GUARD** (fixed, verified only by inspection — say so
    rather than implying coverage): `sameSection` and `rootSig`'s section BINDING (the
    E2E mock is Jellyfin — GUID ids, never rebinds); the drilled-below-a-search repaint
    gate (the mock resolves ParentId only against VIEWS, so no scenario can drill); and
    everything on the Plex link/pin screen (`link_begin` needs plex.tv). Each is a
    standing hole a future regression could walk through unseen.
  - **Two process violations, disclosed, NOT rewritten** (rewrite needs owner go):
    `4cb6b2a` batches three findings; `878c92e` is MIS-DESCRIBED — a `git add -A`
    swept two production fixes and an E2E case into a commit whose message covers
    only a test fix. Root cause both times: `git add -A` instead of naming paths.
- **LIVE E2E (`npm run e2e:live`) — landed 2026-07-14, owner-approved.** Drives the app
  against the owner's REAL Plex and REAL Jellyfin from the Linux VM. Opt-in; NEVER part of
  the gating suite (non-hermetic). Venue, access grants and restore-on-exit rules:
  `.agents/machines.md`.
  - **Why:** the owner's manual playtests found FOUR defects in two sessions that 18 mock
    scenarios and 24 rounds of two-reviewer review all missed. Real servers say things
    mocks do not.
  - **What it closed:** the Plex scan path had NEVER been exercised — the mock is Jellyfin
    (GUID ids, never rebinds), so a real Plex section key (a server-LOCAL number) had never
    appeared in a test. `live-plex` now browses real libraries and scans one, red-proven.
  - **STILL OPEN, and unclosable here:** a Plex REBIND needs a SECOND Plex server, which
    does not exist. `sameSection` and the section-binding comparison remain inspection-only.
- **CI: a known vulnerability FAILS the build** (owner ruling 2026-07-14, "known
  vulnerabilities should fail so we can keep current"). The `audit` job used to be
  `continue-on-error: true` and had been swallowing two RUSTSEC DoS advisories in
  `quick-xml` — the crate that parses every Plex response off the network — into green runs
  (fixed `5cb467f`, gate flipped `2ba5f95`). Advisories with no upstream fix get an explicit
  `--ignore RUSTSEC-XXXX-NNNN` with a reason, never a blanket re-disable. The ~18 remaining
  entries are WARNINGS (unmaintained GTK 0.18 crates via Tauri 2, one unsoundness note);
  `cargo audit` does not fail on those.
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

- **OWNER PLAYTEST OF 0.1.48 — THE ONE THING OUTSTANDING. Remind the owner of these steps
  (they asked, 2026-07-14).** Per-surface-status is COMPLETE (all five slices) and every
  automated check is green; what is left is the judgement automation cannot make. The owner
  has NOT seen the mpv bar and does not need to — it only renders when mpv is MISSING, which
  would mean renaming their binary. SKIP IT. Three tests, one sitting:

  SETUP: open a Plex library so the grid is full; right-click a poster -> "Add to queue"
  (BEFORE stopping the server); then stop Plex. Do not Refresh — the grid stays loaded from
  memory, which is the point.

  1. THE EDIT'S OWN LINE (the one that matters — it is the visible behaviour change).
     Right-click a poster -> Mark watched. Expect: a line NAMING it ("Couldn't mark “…”
     watched — the server could not be reached"), a SEPARATE line for the grid's own
     failure, the LIBRARY STILL THERE (posters on screen), and NO url anywhere. Then mark a
     DIFFERENT poster: the first failure is REPLACED, not stacked. Then switch library: the
     EDIT line FOLLOWS them, the grid's banner does not. (The old build silently swallowed
     the edit failure on navigation — that is the change they will notice.)
  2. THE QUEUE. Click the queue chip -> drawer -> click the queued item. Expect the failure
     INSIDE the drawer, not on the main banner. Close the drawer -> it goes with it. Reopen,
     click again, and CLOSE THE DRAWER WHILE IT IS STILL TRYING -> the chip takes a red mark.
     Navigate away -> the chip mark SURVIVES.
  3. THE DETAIL PAGE. Left-click a poster -> Play. Expect the failure ON the detail page, not
     on the grid underneath. Back -> it goes with the page.

  THEN restart Plex and Refresh: the library reloads and the grid's banner clears, but the
  EDIT line STAYS (a refresh repairs the VIEW; it does not un-fail the user's edit). A
  successful mark-watched clears it.

  WRONG LOOKS LIKE: anything landing on the MAIN banner that belongs to another surface; two
  messages where one erases the other; a raw `http://…` on screen; or a blank library.

- **RELEASE (owner asked 2026-07-14): a second Plex server is NOT needed.** The dangerous
  half of the rebind path IS guarded — `src-tauri/src/source/plex.rs` spins up TWO mock Plex
  servers (machine-A / machine-B), both serving a section "2" with DIFFERENT libraries behind
  it, and drives the real rebind scenarios. What has no end-to-end guard is the FRONTEND
  `sameSection` comparison, because the E2E mock is Jellyfin. **A rebind cannot happen at all
  on a single-server account** (it needs 2+ Plex servers on one account AND the saved one
  becoming unidentifiable), so it is inert for this owner. Ship without it and keep it
  recorded. Closing it would need a TLS-capable mock Plex in the harness (the app only
  restores an `https` Plex server, so it needs a trusted cert) or a second real instance.
  - Reviewer incantations: `codex exec --sandbox read-only -o <out.json> "$(cat
    <prompt>)" < /dev/null` (stdin MUST be closed or it hangs; it has hung once) and
    `grok --sandbox read-only -p "$(cat <prompt>)"`. **grok has twice returned only its
    preamble with no JSON verdict — that is a FAILED run, not a clean pass. Re-dispatch
    it; never read silence as agreement.** E2E is Linux-only: see `.agents/machines.md`
    for the VM (login-shell cargo).
  - E2E without pushing: the push policy is ASK, and that includes the `vm` remote.
    Sync the changed files with `scp` into `~/dev/vela` and verify by checksum before
    running — no `git push`, no `git checkout -- .` on the VM's tree.
- **RED-PROOF EVERY GUARD, ALWAYS.** Every vacuous guard in this plan (the log has the
  roll-call) was caught this way and no other — never by review, CI or a green run. Land
  the fix, THEN inject the regression, THEN demand the test fail for the RIGHT reason —
  and prove EACH behavior the fix claims separately (r19's frontend fix claimed three and
  needed three injections; r20's needed four). Restore from a committed state, never a
  stale file backup (that silently reverted work once).
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
- **SHOW-SORT + PER-LIBRARY PERSISTENCE: LANDED 2026-07-10 on the
  owner's explicit "go" (`9cd3323` on 0.1.44), awaiting owner playtest**
  (plan `.agents/plans/show-last-episode-sort.md`). Show libraries get
  "Last episode added" (Plex `episode.addedAt` LIVE-VERIFIED against
  the owner's server; JF/Emby `DateLastContentAdded`; show-only,
  excluded from the merged view), and every library's sort now persists
  across restarts (`section_sorts` config map). Guards: 3 new unit
  tests + the `sortpersist` restart E2E, all proven red→green; full
  suite 13/13 on the VM; local CI green. The plan-review governance
  question (recorded "don't code" vs "continue") was resolved by the
  owner's explicit landing go. **Owner playtest ask (0.1.44, real
  Plex):** show library → "Last episode added" → a show with a fresh
  episode tops the list; movie libraries don't offer that option; set
  different sorts on two libraries, restart → each reopens on its own
  sort.
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
- **QUEUE PERSISTENCE + VELA PLAYLISTS (owner feature request, 2026-07-14 —
  needs a plan and owner approval of scope before any code).** The in-app play
  queue does not survive an app restart, which limits how useful it can be: the
  backend holds it as pure in-memory state (`src-tauri/src/lib.rs:61`,
  `queue: Arc<Mutex<Vec<commands::QueueItem>>>`), and nothing in `config.rs`
  ever writes it to disk. Owner direction, two parts and the second is the
  larger one:
  - Persist the queue across restarts (the queue's `QueueItem`s already carry
    the source id, rating key and display fields — `commands.rs:2148` — so the
    data needed to rebuild one is already in hand).
  - **Vela-owned playlists that span sources.** Not Plex/Jellyfin/Emby
    playlists — a Vela-native saved list whose entries may come from DIFFERENT
    servers in one list, which no single server's playlist API can represent.
    This is the reason the feature is not just "serialize the queue".
  - Open questions for the plan (not decided): whether a saved playlist and the
    live queue are one concept or two; what happens to an entry whose source is
    removed, offline, or whose rating key no longer resolves; and whether these
    persist in `config.json` (owner-only perms, atomic save, fail-closed parse
    — see repo-guidance Earned Practices) or in their own store.
  - Related, already queued: queue plays never enter Vela's recents (previous
    item). Both touch the queue; sequence them deliberately.
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
- `.agents/plans/library-refresh-scan.md` (COMPLETE + owner-playtested; two-reviewer
  code-review loop r1-r19 recorded in its `## Code review log`; r20 in flight)
- `.agents/review/index.md` (durable review trails)
- `docs/history/state-archive.md` (rotated state entries)
- `README.md`, `ISSUES.md` (swept by DLS slice 3, 2026-07-09)

## Unrecorded Repo Memory

- None known.
