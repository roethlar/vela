# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- IN FLIGHT since 2026-11-12: the embedded player plan
  `.agents/plans/embedded-player.md`, APPROVED that day, running
  unattended (the owner is away). Where it stands is in that file's
  "Implementation log"; read it before doing anything, and its "Owner
  checks outstanding" and "For the owner" lists before telling the owner
  anything. As of 2026-11-13:
  - Slice 0 (probes), 0b (the library build), 1 (the launch seam) and 2
    (selection and fallback) have landed and been reviewed by codex.
  - Slice 3's core landed at `0a63799`: a play can run inside Vela's
    window on Linux/Wayland, from a development build pointed at a built
    library set, with `"player": "embedded"` in the config file. Proven by
    E2E under a headless Wayland compositor (`npm run e2e -- --embedded`)
    and by runs on `continuum`'s desktop. A Close button and nothing else.
    Its codex review returned NINE findings (`ec-1` to `ec-9`), all
    admitted and all fixed, one commit each, and codex has verified all
    nine (`.agents/review/index.md`).
    Packaging the library into the Linux packages landed later, as an
    opt-in (below); released packages do not contain it, and the release
    workflow is not touched.
  - Slice 4 (HDR) landed at `e8db400`. Its codex review returned THREE
    findings (`hd-1` to `hd-3`), all three fixed and verified by codex
    (`.agents/review/index.md`).
    Their design, with what was measured against the shipped library and
    what the fixes changed about it, is
    `.agents/review/hd-3-hd-2.design.md`. The slice's acceptance is a
    by-eye comparison only the owner can make.
  - WHERE THINGS STAND (2026-11-14). What has landed since Slice 4, and
    what is still on local branches of this clone (worktrees beside it;
    NOT on `main`, not pushed):
    - Packaging the mpv library into the Linux packages LANDED at
      `4b8293e`, opt-in: `scripts/build.sh --with-libmpv`. Released packages
      do not contain it; the release workflow is untouched. Its codex
      review returned SIX findings, `pk-1` to `pk-6`, all admitted and
      fixed on `main` (`53f2ecd` to `1348782`: thirteen commits, three of
      them for defects found while the fixes were attacked). codex has
      verified all six; it had reopened `pk-1` and `pk-6`, each for a case
      with a symbolic link, and accepted their repairs (`e3ac36e`,
      `b26f6b6`): `.agents/review/index.md`. That review is closed. Since
      those commits the check never runs an AppImage, and it loads a
      package's library only when the package holds, byte for byte, the
      set named with `--set`. The AppImage reading has been run once
      against a real `unsquashfs` (in a container; `pk-1`'s record).
    - Slice 5, controls and cropping, LANDED at `3d2fef6`; see the plan's
      log. Its codex review returned TWO findings, both admitted and fixed
      (`a909c20`, `4118fea`). codex verified `pc-1`. `pc-2` is HALTED FOR
      THE OWNER since 2026-11-14, after a third reopen; do not repair it
      further without the owner's word. Three repairs are on `main`
      (`4118fea`, `c872dad`, `060a5fd`). The third changed the rule: what
      the page is told when a run ends is decided from the app's state at
      that moment, not from which reports of a play's end came first. It
      closed all four orders of events the reviewer had named, and the
      reviewer then reopened it for one order the third repair itself
      introduced: a source choice the user opened by hand, cancelled in the
      moment between a play's clean end and Vela dealing with that end,
      takes the window out of fullscreen just before the next play starts.
      Nothing is left fullscreen with nothing playing any more. The record
      says what would close it and puts the question
      (`.agents/review/findings/pc-2.md`, "Halted for the owner"). The
      slice's cropping script
      is built under
      an assumption the owner has not ruled on: the plan's "For the
      owner", item 3.
    - Slice 6 (parity), the TEST SIDE, LANDED at `ac3e563`: every row of
      the plan's parity list is shown in both players by the end-to-end
      suite, or recorded as exempted, with a guard in `npm run check` and
      a verdict over two full runs (`npm run e2e:verdict`; the plan's log
      has the table). No product code changed. Two rows have no
      end-to-end proof in either player (the quality step-down and its
      notice) and one is red in the EXTERNAL player because of a defect
      these tests found there (Blockers, "own mpv configuration"). The
      rest of Slice 6 is the switch-on, which is the owner's. Its codex
      review returned EIGHT findings, `pa-1` to `pa-8`, all admitted, all
      about tests that can pass without what they stand for
      (`.agents/review/codereview-2026-11-14-embedded-parity.md`). All
      eight were fixed on `main` (tests only, no product code), and that
      review is CLOSED (2026-11-14; `.agents/review/index.md`). Codex
      accepted `pa-8`, `pa-3`, `pa-2` and, after four rounds, `pa-1`.
      `pa-6` closed on the agent's review of a repair codex wrote. The
      owner closed `pa-4`, `pa-5` and `pa-7` as they stand, when setting
      tests aside until a release is in view (next point); codex's last
      comments on those three are known limits in their records. A
      further repair of `pa-5` (codex, worktree `cx-pa5r5`) and of `pa-7`
      (helper agents, local branch `pa7-r3`) was being written at that
      moment; neither was reviewed or landed, and both exist only in
      worktrees on `continuum`. Notes kept
      from the work: `.agents/plans/embedded-player.slice6-notes.md`.
    The owner's by-eye checks and the switch-on (Settings control, release
    notes, version bump) are NOT the agent's to do.
  - OWNER, 2026-11-14: TESTS ARE NOT A PRIORITY until the app is done and
    a release is in view (`.agents/decisions.md`, that date; read it
    before doing anything about tests). Keep the checks and the
    end-to-end suite green and run them before calling work done; give a
    product fix the test that shows it; spend no review round and no
    repair on a finding, or a reviewer's comment, that is about tests
    alone: write it into the record for the release's test pass.
  - OWNER, 2026-11-14: WHAT IS OBVIOUSLY BROKEN IS FIXED WITHOUT ASKING
    (`.agents/decisions.md`, that date, which also says what is still
    brought to the owner). Under it these are being fixed, each of which
    this file or a record had as waiting for a go: the second half of
    `pw-9`; the external player idling at the end of a file under the
    user's own mpv configuration (FIXED, see the commit "a video ends
    when its file ends in the external player"); the order of events left
    open in `pc-2` (FIXED, and `pc-2` closed);
    the Arch package not installing `vela-markers.lua` (FIXED); the lockfile-only
    `npm audit fix` (FIXED; GitHub CI's Frontend job should pass again from
    that commit on: look before relying on it). Their entries under Blockers still say "needs a go":
    read them with this point until each is fixed and its entry removed.
  - Active review loop: see `.agents/review/index.md`. Nothing is waiting
    for the owner there: `ps-2` closed as it stands (it is about a test),
    and `pc-2` closed with a fourth repair under the point above.
  - On 2026-11-13 the owner switched on multi-agent workflows for the
    session that is doing this work. The agent reads that as the approval
    the 2026-08-20 constraint below asks for before more than one Claude
    session is started, for that session only. Reviews of record still go
    to codex, and no agent is cut short. Later the same day the owner
    asked that agents started from then on run on Opus 5.5.
  - On 2026-11-14 the owner asked, in the session doing this work, for
    coding tasks to be handed to codex (gpt-6-astra at max effort, which is
    what a plain call resolves to on `continuum`) and reviewed by the
    agent, "to see if we can make some progress by splitting effort".
    Five were handed over, one at a time, each in a worktree of its own
    under `~/dev/vela-agent-keep/wt/` (`cx-pw9a`, `cx-race`, `cx-pa6r3`,
    `cx-pa5r5`, `cx-pw7r4`), uncommitted there until the agent lands them:
    the first half of `pw-9` (LANDED at `0b313a2`); the other end-to-end
    scenarios checked for the race `pw-8` had, with the wait in one helper
    (LANDED at `71395c3`: sixteen scenario files changed; the survey and
    the agent's review are `.agents/review/pw-8.other-scenarios.md`);
    `pa-6`'s second repair (LANDED at `3d2ecca`); `pa-5`'s fourth (set
    aside unlanded when the owner put tests off); and `pw-7`'s fourth
    (being written). Codex cannot be the
    reviewer of code it wrote, so for these the agent reviews (reads the
    diff, runs the checks and the end-to-end runs codex's sandbox cannot,
    red-proves the tests) and lands them; their records say so. The owner
    DECIDED on 2026-11-14 that the agent's review closes a finding whose
    last repair codex wrote (`.agents/decisions.md`, that date). `pa-6` is
    closed under it.
  After the embedded plan's slices came
  `.agents/plans/mpv-window-geometry.md` Slices 1-3, also approved: LANDED
  2026-11-14 at `9815e10` as one commit, Vela 1.0.64 (not released, not
  tagged). Every play in a run now opens mpv's window as the previous one
  left it, size included, and Settings has a "Player window" choice for
  the first window after Vela starts. Its log says why it is one commit,
  what could not be tested here (a real window size), and lists the
  owner's acceptance steps on real desktops, none of them done. Going back
  to an older Vela after playing anything in 1.0.64 needs Settings
  recovery (the release note says so). Its codex review, one hunt per
  slice, returned SEVEN findings, `pw-1` to `pw-7`, all admitted
  (`.agents/review/codereview-2026-11-14-player-window.md`): the Prefer
  Compatible policy can rank for a different screen than the window opens
  on; a size can be carried that the window never had; quitting waits
  without limit for the settings file; an older window can be saved over a
  newer one; and three smaller ones. All seven are FIXED on `main`
  (`55be8fc` to `ac25240`, ten commits, 2026-11-14; no version change, no
  release note). Where each stands (2026-11-14, evening): `pw-1` to
  `pw-7` are CLOSED. Codex accepted `pw-3` to `pw-6` at once, and `pw-1`
  and `pw-2` in their third round, after two joint repairs (`52138d0` to
  `eb2f1c2`, then `c9e7245` to `79384d9`): a Prefer Compatible copy is
  ranked for the screen its window is asked to open on, decided once
  before the ranking; every other policy decides the screen after the
  stream is resolved, as the plan says; a remembered screen that is
  known to be gone is passed over for the screen Vela's window is on.
  What that changes for a user, and its known gaps (no macOS or Windows
  build, no real second screen), are in `pw-1`'s record. `pw-7` closed on
  the agent's review of a repair codex wrote (`0cc2e03`: saves of the
  Player window choice go one at a time). One fix departs on purpose
  from what was first decided: what a play's end and Vela's exit save is
  the last window of mpv's own, not the latest play's (`pw-6`'s record
  says why). Do not release 1.0.64 before the two findings below are
  closed.
  Two findings were added after the review (its document, "Raised after
  the intake"). `pw-8`: the scenario `windowsetting` used to fail one
  external run in four; the scenario was fixed (`26660a9`), every other
  scenario was gone through for the same race (`71395c3`,
  `.agents/review/pw-8.other-scenarios.md`), and codex then reopened it
  on a fault in the product: Vela, quitting, saved the window before its
  reader had read all that mpv sent. Repaired at `0ae5b64` to `bb3b84e`
  (quitting waits at most half a second, inside the two seconds it
  already allowed, until the reader has finished) and WITH CODEX. `pw-9`:
  its first half is fixed (`0b313a2`); its second half (a play whose mpv
  has already gone is treated as running for five seconds) and the reader
  dropping lines it had received when a play is cancelled are being
  fixed by codex as a coding task (worktree `cx-pw9b`), under the owner's
  word that what is obviously broken is fixed.
  The parity verdict has not passed on `main` since the fixes began to
  land; the runs made along the way are in the records. A full run of
  each suite, each building its app, and the verdict are being made at
  `77d4dc3` and are owed again once the fixes still in flight have
  landed. `embeddedownconfig` is no longer a known failure in the
  external player (`67536da`); `pagefail` still is, in both.
  Rules the owner set for this run (decision `.agents/decisions.md`
  2026-11-12, "plans are approved"): every slice with repo code gets a
  `codereview codex` pass; push each commit; steps only the owner can do
  (by-eye picture checks) are left as a list in the plan's log, and until
  the owner has done them the embedded player gets no Settings control, no
  version bump and no release note.
  Standing working rule from the owner (2026-11-12): settle
  implementation, failure-mode and licensing details yourself and record
  them; bring the owner only what changes Vela for its users, briefly, and
  do not repeat a pending ask. Licensing is closed (`.agents/decisions.md`).
- Windows mpv idle-exit fix, 2026-11-12, `bf1b1c3` (1.0.62; plan
  `.agents/plans/windows-mpv-idle-exit.md`). On Windows, mpv started by Vela
  idled at end of file instead of exiting, so Continue Playing, playlist
  advancement, the final watched-state write and the recents stamp never
  ran (owner report on 1.0.59; reproduced with a bare mpv probe on
  netwatch-01). Vela now passes `--idle=no` after every user-controlled
  option. The plan passed an owner-directed openreview (agy,
  gemini-4-argon-high: `best_approach`). Local JS and stable-Rust checks
  and five red-proofs pass; a netwatch-01 probe shows the option wins over
  a user `--idle=yes`. NOT done: E2E, any Windows build, and the owner's
  acceptance on the affected install. GitHub CI run 43302281260 at
  `6dba9bf` passed every job, the MSRV check and `cargo audit` included
  (a first run failed on an unrelated Rust 1.100 clippy lint, fixed at
  `9e76606`; codex codereview of the fix: no material issue). Workaround for
  older releases: add `--idle=no` to the Advanced
  mpv arguments.
- Missing-npm build diagnostic, 2026-11-12 (`.agents/plans/build-missing-npm-diagnostic.md`;
  owner ruled D1 "A" and gave the go). A build host without `npm` used to
  get a Node stack trace. `scripts/check-js-toolchain.mjs` now reports a
  missing or out-of-range npm in one line (`e58a17e`), and `scripts/build.sh`
  names a missing `node` or `npm` before the toolchain check (`b519083`);
  nothing is installed for the builder. Seven red-proofs, `npm run check`
  (78 pass) and the manual repro on the CachyOS host pass. GitHub CI run
  43265242786 at `f2c6449` passed every job
  (Frontend, Rust, cargo audit). Windows was not exercised. Awaiting owner
  confirmation.
- SvelteKit removal, 2026-10-23. The owner approved
  `.agents/plans/remove-sveltekit.md` and ruled G1 "rename"; the decision is
  `.agents/decisions.md` 2026-10-23. Slices 1-4 are implemented on branch
  `remove-sveltekit`, which is NOT merged to `main`. The branch is at
  version 1.0.62, and on 2026-10-24 it merged the reconciled `main`
  (`bc12bc5`, 1.0.61) without a rebase; the plan's 2026-10-24 amendment
  carries the consequences. The approved plan text, that amendment, the
  implementation log and the 2026-10-23 decision entry exist only on the
  branch; `main` carries the plan's pre-approval draft plus its "Obligation
  recorded 2026-11-12" section.
  - The frontend is plain Svelte 5 and Vite. Root `index.html` loads
    `src/main.ts`, which mounts `src/App.svelte`. Static files are in
    `public/`, and `$lib` is defined in tsconfig `paths`.
    `npm run build` runs `scripts/check-frontend-build.mjs`, and
    `tests/frontend-shell.test.mjs` guards the shell.
  - Verified on the Windows host, in an LF worktree with npm 12.0.1: `npm ci`;
    `npm audit` (0 vulnerabilities); `npm run check` (68 pass, 2 skipped;
    svelte-check 295 files, 0 errors, 0 warnings); `npm run build`; and every
    local guard red-proof (see the plan's implementation log).
  - Re-verified after the 2026-10-24 merge of `main`, in an LF worktree with
    Node 26.12.1/npm 12.0.2: `node scripts/check-js-toolchain.mjs`; `npm ci`;
    `npm audit` (0 vulnerabilities); `npm run check` (68 pass, 0 fail, 6
    skipped, all POSIX-bash build-script cases; svelte-check 295 files, 0
    errors, 0 warnings); `npm run build` (B1-B6 passed); `node --test
    tests/`. The lock differs from `main`'s by exactly the 11 removed Kit
    packages.
  - What blocks the merge moved to `## Blockers` on 2026-11-12: the list that
    stood here had been overtaken by the branch's own record.
  - GitHub CI passed on PR #2 (https://github.com/roethlar/vela/pull/2), run
    40059537619 at `367bdf6`. Frontend: `npm ci`, `npm audit` (0
    vulnerabilities), `npm run check` (70 pass; svelte-check 0 errors, 0
    warnings), and `npm run build` (B1-B6 passed). Rust: the 1.89 MSRV check,
    and stable check, clippy `-D warnings` and test. cargo audit also passed.
    After the merge of `main`, run 40061360150 at `af96dc5` passed every job
    on Node 26.12.1/npm 12.0.2: `npm audit` (0 vulnerabilities), `npm run
    check` (77 pass, 0 skipped; svelte-check 0 errors, 0 warnings), `npm run
    build` (B1-B6 passed), the 1.89 MSRV check, stable check, clippy, 364 Rust
    tests, and cargo audit. (`main`'s npm audit gate is covered by decision
    `.agents/decisions.md` 2026-11-02; this branch does not clear the
    deepmerge advisory either, since it keeps vite-plugin-svelte.)
- 2026-08-20 macOS AirPlay diagnostics selected an AVFoundation-first production
  path, now drafted at `.agents/plans/macos-airplay-sync.md`; implementation is
  not approved. The affected server-backed MKV/E-AC-3 play was physically about
  two seconds audio-late under CoreAudio despite near-zero mpv `avsync`; changing
  that live process to AVFoundation restored physical sync, as it did again after
  a fresh CoreAudio launch. The same AVFoundation process followed AirPlay ->
  built-in speakers -> AirPlay and stayed synchronized, so Vela needs no route
  detector, listener, polling, guessed delay, or new setting. The production
  design adds macOS `--ao=avfoundation,coreaudio` before Advanced mpv arguments,
  with no non-macOS behavior change. Two manual volume-timing attempts were
  inconclusive (the second IPC OSD cue was not visible); upstream mpv 0.41 source
  directly applies AVFoundation mute/volume and pause controls, while packaged
  pause/resume and seek remain explicit acceptance checks. The owner settled
  own-config precedence on 2026-08-20: when `mpv_use_own_config=true`, Vela
  omits its AO preference and the expert's `mpv.conf` owns audio-output
  selection. The implementation is complete for Vela 1.0.61: the shipped play
  path uses a testable base-command helper, macOS appends
  `--ao=avfoundation,coreaudio` before Advanced arguments only when Vela owns
  defaults, and docs/release notes describe the contract. All four focused
  mutation proofs failed for the intended reason and restored green. Pinned
  Node/npm, npm audit/check/build, Rust MSRV/stable check, clippy, all 365 Rust
  tests, RustSec audit, `git diff --check`, and the native 1.0.61 DMG build pass.
  The freshly built DMG is running; packaged physical AirPlay/control
  acceptance remains pending and must not be inferred from automated evidence.
- 2026-08-20 owner review-process constraint: never start more than one Claude
  session without explicit owner approval; never impose a low timeout, end a
  Claude process, or discard any verdict for any reason without explicit owner
  approval. A failed capability/schema check is reported with its verdict
  retained and does not authorize a retry.
- 2026-08-20 macOS 27 local-network failure under investigation: Jellyfin at
  `altiera:8096` is reachable from the host and answers its public endpoint,
  but the installed `/Applications/Vela.app` is unsigned/ad-hoc and fails its
  authentication transport request. A fresh locally signed, notarized,
  Gatekeeper-accepted universal build independently confirmed the packaged
  `Info.plist` still lacks Apple's `NSLocalNetworkUsageDescription`; Apple says
  apps that connect directly to local hosts should declare it. `altiera` also
  advertises four IPv6 addresses that refuse 8096 before its working IPv4
  addresses; direct-IP Vela isolation test is still pending. Local signing
  setup details live in `.agents/machines.md`; no product fix is approved yet.
- Vela 1.0.60 is published as GitHub's Latest release (re-verified as of
  `aaa01c1`) from exact commit
  `10b968b583a44c4738ece324b5fcbb9d58276235` — the FIRST SIGNED release:
  the macOS universal DMG is Developer ID signed and notarized (spctl
  `Notarized Developer ID`), the Windows MSI + NSIS are Authenticode signed
  (Azure Trusted Signing); Linux and Arch artifacts remain unsigned by
  design. Its canonical workflow, checksum, local signature-inspection, and
  publication evidence lives in `.agents/plans/v1-release-readiness.md`.
- Product behavior remains as settled in `.agents/decisions.md`: Vela is a
  multi-server Plex/Jellyfin/experimental-Emby client, delegates HDR playback
  to external mpv, uses title-level watched state across duplicate copies, and
  offers Prefer Best, Prefer Compatible, Prefer Fastest Source, Ask Every Time,
  and per-title Play Version.

## Next

- Complete physical AirPlay sync, route-transition, pause/seek/mute/volume, and
  startup-preroll acceptance on the running Vela 1.0.61 DMG, then close the
  implementation record.
- Owner decision after direct-IP isolation: plan the macOS local-network fix
  (package the usage description, guard the built app metadata, and surface
  actionable transport causes instead of reqwest's shallow request error).

- `.agents/plans/mpv-window-geometry.md`: landed 2026-11-14 (`## Now`);
  what is left is the owner's acceptance on real desktops, listed in that
  file's log, and its review loop.
- Owner direction 2026-11-12: embedded playback through libmpv, with
  iOS/iPadOS as a porting goal (`.agents/decisions.md`). The HDR spike ran
  on 2026-11-12 (`.agents/plans/embedded-libmpv-hdr-spike.md`, Log): on
  `continuum` (KDE Wayland, AMD/Mesa) HDR survives embedding in a bare
  window, under GTK3 + WebKitGTK, and inside Tauri 2. Conditions: the
  library draws with mpv's older renderer (tone-mapping differs from
  today's above about 265 nits), and Vela must tag the surface itself.
  The owner confirmed by eye that it is HDR and that its greys do not
  match external mpv. Measured afterwards: a green cast in that comparison
  was the agent's missing `profile=gpu-hq`; with Vela's options the greys
  are neutral but brighter than external mpv across the range in every
  configuration measured but one; the two renderers tone-map differently
  and peak detection moves both. On real clips dark scenes match and
  bright scenes clip more highlight embedded; the owner: "the external
  looks slightly better on this display." Clicks
  on HTML drawn over the video reach the page. External mpv stays the
  shipped default player; the plan in the next bullet adds the embedded
  one beside it.
- Owner ruling 2026-11-12: keep both players; the embedded libmpv player
  is added next to external mpv (`.agents/decisions.md`). Plan APPROVED
  2026-11-12 and in flight (`## Now`): `.agents/plans/embedded-player.md`.
  It reuses the existing
  IPC-based trackers through one launch description and two launchers, and
  starts with probes. D1 is ruled: external is the default on every
  desktop OS and embedded is offered as experimental; iOS is embedded
  only. A codex openreview returned `acceptable_with_changes` with three
  admitted plan defects; the owner adopted the fixes and the plan is
  revised, not re-reviewed (`.agents/review/index.md`). D3, D5 and D6 are
  assumed under the owner's standing instruction, not ruled. D4 is
  closed: Vela stays MIT and mpv is only ever a separate dynamic library
  (`.agents/decisions.md`, "Licensing is settled"); do not raise licensing
  with the owner again. D7 is ruled: Vela ships its own pinned libmpv
  on every platform, Linux included; the plan gained a library-build
  slice and two probes for it.
- Launch marketing drafts are in-repo: GitHub social preview
  `docs/images/social-preview.png` (1280×640) and Reddit copy
  `docs/marketing/reddit-launch-post.md`. Still awaiting owner go to set the
  GitHub repo social image and to post on Reddit.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.

## Blockers

- No known product blocker.
- E2E `pagefail` fails on `continuum` (x86_64, WebKitGTK 2.54.2), first seen
  2026-11-12; it also fails on `2f283a2`, before the server-identity work.
  Investigated 2026-11-12 with a throwaway probe scenario (not committed);
  cause is a product race, not the harness or the host:
  - `onScroll` in `src/routes/+page.svelte` runs
    `if (el.scrollHeight - el.scrollTop - el.clientHeight < 600) loadMore()`
    with no check that the grid is still on screen.
  - Opening a detail removes the grid from the DOM. WebKit can deliver a
    queued, trusted `scroll` event to that element afterwards. Its
    `scrollHeight`, `scrollTop` and `clientHeight` are then all 0, so the
    test `0 < 600` passes and the next page is requested while the detail
    is open.
  - Evidence, 8 probe runs: in 6 a scroll event arrived with
    `isConnected: false`, all three sizes 0 and `.detail` present, and a
    second `StartIndex=60` request followed; in 2 the event arrived while
    the grid was still connected (`scrollTop` 20) and no extra request was
    made. The grid itself scrolls normally (`scrollHeight` 2823,
    `clientHeight` 694, window 1280x800).
  - In `pagefail` this refetches the page the scenario deliberately failed,
    so its precondition "back on the truncated grid" sees 130 cards, not
    60. The scroll comes from WebDriver scrolling the clicked card into
    view; a mouse click during inertial scrolling could do the same.
  - User-visible effect is small: a wasted background page request, and
    more cards than expected on Back. No data loss.
  - Unfixed as of `aaa01c1`. Proposed fix (needs a plan and a go): make
    `onScroll` ignore an element that is not connected or has no height,
    with a guard, then re-run `pagefail`. Whether the VM's older WebKit
    simply never delivers the late event is an assumption, not verified.
  - Seen again 2026-11-13 during the embedded-player run: `pagefail` passed
    once (the full run at `3e91c63`, 39 of 39) and then failed 15 runs of 15
    at `3e91c63`, `b719a65`, `599c539` and `bc6e8a1`, always with the same
    precondition (120 cards instead of 60). None of those commits touches
    the page; this is the race above, not a regression.
- CI does not exercise the package check (2026-11-14, the owner has not
  been asked yet). `tests/libmpv-packaging.test.mjs` needs `bsdtar` to
  build and unpack its stand-in packages; GitHub's runner image does not
  list it and the `frontend` job installs nothing, so those tests are
  skipped there. One step would fix it (`sudo apt-get install -y
  libarchive-tools` before `npm run check`); it is a change to `ci.yml`,
  which the agent does not make.
- The external player does not check the media server's TLS certificate
  (found 2026-11-13 by a run, unfixed, the owner has not been asked yet).
  The system's mpv up to 0.41 defaults to `tls-verify=no` and Vela does
  not set it, so mpv fetches the stream, and sends the stream's token,
  without verifying whom it is talking to; Vela's own client does verify.
  The mpv shipped for the embedded player (0.42) verifies by default, but
  against its own TLS library's trust store, which is not always the one
  Vela's client uses (`SSL_CERT_FILE` is honoured by one and not the
  other): a server on a private CA then browses and will not play, with no
  message. Details: `.agents/plans/embedded-player.slice6-notes.md`.
  DECIDED by the owner on 2026-11-14: the player checks, and a setting
  turns the check off. Plan `.agents/plans/tls-verify.md`, being built;
  this entry goes when it has landed.
- The `refresh` E2E flake remains recorded (1.0.59 full Linux run, 36/39,
  with `continueon` and `playverbs`). One cause of the continuation flakes
  is gone since 2026-11-13: scenarios sought in a file the moment mpv named
  it, and mpv can answer that with "property unavailable" (seen in
  `continuetv` and `continueon`). The harness now waits until mpv reports a
  position before any seek (`tests/e2e/mpv.mjs`). Whether that was the
  whole of the `continueon` and `playverbs` flakes is not established.
- The SvelteKit removal (`## Now`) cannot merge as it stands. Its merge
  blockers moved here on 2026-11-12, re-verified at `main` `aaa01c1` against
  branch head `769d5a8`:
  - GitHub reports PR #2 as `CONFLICTING` with `main`. `main` is 53 commits
    past the branch's last merge of it and has used versions 1.0.62 and
    1.0.63, so the branch must merge `main` again, carry the
    server-identity edits into `src/App.svelte`, and re-bump
    (`.agents/plans/remove-sveltekit.md`, "Obligation recorded
    2026-11-12"). Every verification recorded for the branch predates that
    merge.
  - The branch's own state records the Mac's automated gates as passed on
    2026-10-24 at `2767cbe`: the JS checks, `cargo +stable check --locked`,
    `tauri build --debug`, the automated half of the desktop smoke, and
    `tauri dev` with HMR. Still open there: every VM gate (Slice 0,
    Slice 1's E2E run with its F1/F2 red-proofs, the full `npm run e2e`,
    the webview-mount red-proof), the owner's visual checks on the Mac, and
    the Review.
  - Two things the branch recorded on 2026-10-24 have changed since: it
    calls codex unavailable, and codex ran reviews on 2026-11-12
    (`.agents/review/index.md`); its only E2E venue is the suspended VM, and
    `continuum` has been one since 2026-11-12 (`.agents/machines.md`).
    Whether the plan's VM gates may run on `continuum` is an owner decision.

## Verification

- Canonical commands live in `.agents/repo-guidance.md` (Verification). Do not
  duplicate here.
- Linux live-server venue details live in `.agents/machines.md`.
- Release verification immutable artifact hashes live in
  `.agents/plans/v1-release-readiness.md`.

## Active Sources

- `AGENTS.md` and `.agents/repo-guidance.md`
- `.agents/decisions.md`
- `.agents/machines.md`
- `.agents/push-policy.md`
- `.agents/plans/library-sorting.md` and
  `.agents/plans/show-last-episode-sort.md` (landed sorting baseline)
- `.agents/plans/library-sort-direction.md` (landed at `c0d1412`, 1.0.59)
- `.agents/plans/v1-release-readiness.md` (published-release evidence)
- `.agents/plans/remove-sveltekit.md` (approved; implemented on unmerged
  branch `remove-sveltekit`, which holds its current status and log; merge
  blockers in `## Blockers`)
- `.agents/plans/macos-airplay-sync.md` (implemented for 1.0.61; packaged
  physical acceptance pending)
- `README.md`, `RELEASE_NOTES.md`, `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
