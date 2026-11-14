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
    NOT yet done in Slice 3: packaging the library into the Linux packages.
    That is being built as an opt-in on the local branch
    `embedded-packaging` (a worktree beside the clone); nothing of it is on
    `main` yet, and the release workflow is not touched.
  - Slice 4 (HDR) landed at `e8db400`. Its codex review returned THREE
    findings (`hd-1` to `hd-3`): `hd-1` and `hd-3` are fixed and verified
    by codex; `hd-2` is fixed and with codex (`.agents/review/index.md`).
    Their design, with what was measured against the shipped library and
    what the fixes changed about it, is
    `.agents/review/hd-3-hd-2.design.md`. The slice's acceptance is a
    by-eye comparison only the owner can make.
  - WHERE THE UNLANDED WORK IS (2026-11-13, evening). Several pieces are
    finished on local branches of this clone (worktrees beside it, named
    `~/dev/vela-*-wt`) and are NOT on `main` and not pushed. Each was
    written by helper agents from a recorded design, attacked by other
    helpers and repaired; none has yet been verified by the agent that
    coordinates, reviewed by codex, or landed. Land them in this order:
    - (Packaging the mpv library into the Linux packages LANDED at
      `4b8293e`, opt-in: `scripts/build.sh --with-libmpv`. Released packages
      do not contain it; the release workflow is untouched. With codex
      for its review.)
    - (Slice 5, controls and cropping, LANDED at `3d2fef6`; see the plan's
      log. Its codex review returned TWO findings, `pc-1` and `pc-2`, both
      admitted and being fixed: `.agents/review/index.md`. Its cropping
      script is built under an assumption the owner has not ruled on: the
      plan's "For the owner", item 3.)
    - Then Slice 6 (parity), being written since the evening of
      2026-11-13 on the local branch `slice6` (tests and harness only; no
      product code). What was found out for it is
      `.agents/plans/embedded-player.slice6-notes.md`: why each scenario
      that fails in the embedded venue fails (none is a defect of the
      embedded player; two are the test compositor), what proves each
      inventory row in each player, and a design for the parity guard.
    The owner's by-eye checks and the switch-on (Settings control, release
    notes, version bump) are NOT the agent's to do.
  - Active review loop: see `.agents/review/index.md`. One finding
    (`ps-2`) is with the owner after four reopens
    (`.agents/review/ps-2.contested.md`).
  - On 2026-11-13 the owner switched on multi-agent workflows for the
    session that is doing this work. The agent reads that as the approval
    the 2026-08-20 constraint below asks for before more than one Claude
    session is started, for that session only. Reviews of record still go
    to codex, and no agent is cut short. Later the same day the owner
    asked that agents started from then on run on Opus 5.5.
  Next in the queue after this plan:
  `.agents/plans/mpv-window-geometry.md` Slices 1-3, also approved. All
  three are written on the local branch `geometry` (head `a711082`, a
  worktree beside the clone), attacked and repaired by helpers, not yet
  verified by the coordinating agent, not reviewed by codex. Nothing of it
  is on `main`, and it lands only after the embedded plan's slices, as the
  owner ordered ("Both, embedded first"); the version bump and release
  notes are made at that landing.
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

- `.agents/plans/mpv-window-geometry.md` (APPROVED 2026-11-12 for Slices
  1-3; queued behind the embedded player plan, not started): carry the mpv
  window's size from one play to the next. Owner ruled D1 "all in current
  session": manual plays inherit too, reversing the 2026-07-18 rule. A
  local probe shows `osd-width` / `osd-height` report the window size and
  `--geometry=WxH` reproduces it exactly. D2 ruled: a three-way setting
  (remember last / full screen / default), consulted for the first play of
  a run only, remember-last out of the box. D3 (position) withdrawn.
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
- The Arch package has no marker skipping (found 2026-11-13, unfixed, the
  owner has not been asked yet). `packaging/arch/PKGBUILD` installs the
  mpv scripts one by one (`autocrop.lua`, `vela-autocrop.lua`) and was not
  given a line for `vela-markers.lua` when that script was added on
  2026-07-25; `build:arch` runs `tauri build --no-bundle`, so nothing else
  stages it. Seen on `continuum`: the installed 1.0.63 package has no
  `/usr/lib/Vela/mpv-scripts/vela-markers.lua`. The launcher checks that
  the script exists and otherwise plays without it
  (`src-tauri/src/player/launch.rs`, "did not resolve; playing without
  skip controls"), so on an Arch install the skip button and auto-skip
  never appear, with one line on stderr and nothing in the window. The
  Tauri bundles (deb, rpm, AppImage) take the whole scripts directory
  (`src-tauri/tauri.conf.json`, `resources`) and should not be affected;
  that is by reading, no built bundle was opened. Proposed fix (needs a go:
  it changes what the Arch package does): one `install` line in the
  PKGBUILD and a test that every script the launcher resolves by name is
  installed there. Slice 5 of the embedded-player plan adds a line for its
  own new script and deliberately leaves this one alone until the owner
  answers.
- GitHub CI on `main` is RED since 2026-11-13 about 19:10 local, and not
  because of a commit: its `Frontend` job fails at `npm audit
  --audit-level=moderate`. Two moderate advisories against SvelteKit were
  published that day (`@sveltejs/kit` GHSA-pfhg-q3qv-9q6r, GHSA-27mw-82g8-phjj
  and GHSA-mw6w-2fmv-pvw3; `@sveltejs/adapter-static` GHSA-jf9x-2fg2-hx4w),
  and the gate fails closed on them by the 2026-11-02 decision. Because
  that step comes first, the job does not reach `npm run check` or the
  build; the Rust, libmpv and cargo-audit jobs pass. Every commit since
  was checked locally (`npm run check`, the build, the Rust set). The fix
  is prepared and NOT landed, because it changes the dependencies Vela is
  built with and nobody has said go: a plain `npm audit fix` (no
  `--force`), lockfile only, on the local branch `npm-audit-fix`: kit
  2.70.3 to 2.70.4, adapter-static 3.0.10 to 3.0.11 (both published
  2026-11-05), vite-plugin-svelte 7.2.0 to 7.3.1, and two small transitive
  packages. With it `npm audit --audit-level=moderate` passes (the four
  known low deepmerge findings remain), and `npm run check` and the build
  pass. Vela is not exposed meanwhile: the advisories concern the Node and
  Vercel adapters and server-side form handling, and Vela ships a static
  page.
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
  Proposed (needs a plan and a go: it changes playback for users whose
  server certificate mpv's library does not accept): assert
  `--tls-verify=yes` in the external launch after the user's options, and
  decide what the embedded player should trust.
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
