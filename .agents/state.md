# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- VELA 1.0.66 IS READY TO BE RELEASED, AS FAR AS THE AGENT CAN TAKE IT
  (2026-11-16). It is NOT tagged and NOT published; the owner verifies the
  macOS and Windows builds first. The whole account, with what was
  verified and the owner's steps in order, is
  `.agents/plans/v1-release-readiness.md`, "Release candidate"; read that
  before doing anything about the release. What happened that day, in
  order:
  - The owner's word: "you have permission to update whatever you need to
    to get the release ready. then do a final round of reviews with
    codex, then come to me with actions you cannot do"
    (`.agents/decisions.md`: the workflows are the agent's to edit under
    it; tagging and publishing, `sudo`, signing identities and by-eye
    judgments stay the owner's; the rule that set tests aside is
    superseded).
  - Done as 1.0.65: the Linux packages carry the embedded player's
    library and each is checked in the release workflow; CI compiles the
    backend on Windows and for both macOS architectures; the release
    notes; the final review (`.agents/review/codereview-2026-11-16-release.md`:
    seven codex hunts by the end of the day, sixteen findings, all closed);
    a mutation check of three window-state repairs and the tests it
    showed missing; an embedded play closed by its user no longer reports
    a position it was never at (`3e0a485`).
  - The owner's answers to the first list of steps (`.agents/decisions.md`,
    the entries of 2026-11-16): the Apple agreement "should be done"; "no
    more azure signing. I'm not paying monthly for that", so the Windows
    installers ship unsigned (`fc991a8`), with alternatives given and none
    chosen; the builds are verified by the owner BEFORE any tag.
  - Then the release stopped at `npm audit` on advisories against
    SvelteKit, and the owner had the approved SvelteKit removal landed:
    `dee0f8b`, Vela 1.0.66 (the entry after the next; the blocker's entry
    rotated to `docs/history/state-archive.md`).
  - Verified for 1.0.66, on the removal's branch before the merge: the
    full check (727 Rust tests, 676 Node tests then, 732 at the merge),
    both end-to-end suites in full with the parity verdict PASS (external
    48 of 48, embedded 51 of 51, at `a13be71`), the plan's red-proofs, a
    codex hunt over the whole change. `cargo audit` is not installed on
    `continuum`; CI runs it.
  - HANDOFF, 2026-11-16 about 17:25 UTC (the session doing this work was
    at 98% of its weekly allowance and may have stopped here). In flight:
    1. The release workflow by hand, run 43914914442 at `dee0f8b`, for
       the owner's macOS and Windows builds. Result (17:30 UTC): both
       audits, the libmpv job, the Linux bundles, the Arch package and
       the WINDOWS build passed; the Windows installers (unsigned) are
       the run's artifact `vela-windows`, with `vela-linux` and
       `vela-arch`: `gh run download 43914914442 --repo roethlar/vela`,
       or the run's page under Artifacts. The MACOS build did not run:
       GitHub cancelled the job while it waited for a runner, as it did
       to several of CI's macOS jobs that day; nothing failed. So there
       is no macOS build yet, and whether Apple's notarization now
       passes is still NOT known (the owner said the agreement "should
       be done"). The agent asked GitHub to run the cancelled jobs of
       that run again (`gh run rerun 43914914442 --failed`) at about
       17:30 UTC; its outcome is not recorded. To read it: `gh run view
       43914914442 --repo roethlar/vela --json jobs`. When the macOS job
       passes, its artifact is `vela-macos`. The owner verifies the
       builds before any tag.
    2. The 1.0.66 package for `continuum` is built from `dee0f8b` and
       checked: `~/dev/vela-agent-keep/pkg/vela-1.0.66-1-x86_64.pkg.tar.zst`,
       sha256 `be81c99f348508ccdaed1c2e7e196f241826561cd92550215f6ce828ebf00e94`. Its package check passed and its own binary passed
       five scenarios in the external player and seven in the embedded
       one. The owner installs it with `sudo pacman -U`.
    3. The title bar (the owner asked on 2026-11-16 why it is so tall and
       cannot be hidden with KDE's rule, and was told it would be fixed
       after the SvelteKit removal). Cause: on Wayland the windowing
       library under Tauri (tao 0.35.3, `WlHeader::setup`) gives the
       window a GTK header bar as its titlebar, so the window is
       client-side decorated. Measured on `continuum`'s KWin 6.8.2 with a
       GTK 3 window built the same way: as tao leaves it, the bar is
       48 px inside the window and KWin draws none; with
       `set_titlebar(None)` before the window is first shown, or after
       `hide(); unrealize()`, KWin draws its own 28 px bar, cleanly;
       taking it off a realized window resizes the window and makes GTK
       warn. A coding task for the change is with codex: worktree
       `~/dev/vela-agent-keep/wt/cx-titlebar` (branch `cx-titlebar`, cut
       from `545467c`), started 17:16 UTC; its report will be
       `cx/titlebar-report.md` in that session's scratchpad, or read the
       worktree's `git diff`. Wanted: only on Wayland where the
       compositor prefers server-side decorations
       (`gdk_wayland_display_prefers_ssd`), nothing changed elsewhere, so
       the end-to-end suite under weston sees the window it sees today.
       Still to do: review the diff; build it and look at it on the real
       desktop (how KWin sees a window can be read with a KWin script
       through `qdbus6 org.kde.KWin /Scripting`: frame size, client size,
       and their difference in height is KWin's own bar) with the
       config and data directories pointed at a scratch place, never the
       owner's; check that the embedded player's video still lines up;
       both end-to-end suites; land it with a bump to 1.0.67 (packages
       and artifacts named 1.0.66 exist); a new package and a new run by
       hand for the owner. NOT landed, NOT verified.
    4. CI on `main` is green again: `dee0f8b` and `545467c` passed every
       job.
  - NOT verified by anyone: this version running on macOS or Windows (CI
    compiles it there, nothing more), and everything that takes eyes.
- THE FRONT END IS PLAIN SVELTE 5 AND VITE SINCE 2026-11-16 (`dee0f8b`;
  plan `.agents/plans/remove-sveltekit.md`, whose last log entry has the
  landing; decisions of 2026-10-23 and 2026-11-16). SvelteKit and
  adapter-static are gone; `index.html` loads `src/main.ts`, which mounts
  `src/App.svelte` (what records before that date call
  `src/routes/+page.svelte`). Left for the owner: the plan's visual checks
  on the Mac.
- THE EMBEDDED PLAYER IS OFFERED IN SETTINGS (2026-11-15). The owner
  ruled that day that what is built is to be usable from the app, and
  that the owner is Vela's only user (`.agents/decisions.md`; read it
  before describing anything as held back from users). Settings, Player,
  "Video player" landed at `3046f83`, its documents at `3505387`, and
  the certificate setting's documents at `efd6421`. The embedded player
  plan's log has what was checked, including the packaged app playing
  embedded from its own library, and where on `continuum` the package
  built with the library is. Left for the owner: installing that package
  and the by-eye checks (the plan's "Owner checks outstanding" and "For
  the owner").
- ALSO 2026-11-15: `pagefail` IS FIXED (`b066dc0`), and no scenario is
  allowed to fail any more (`tests/e2e/parity.mjs`, `KNOWN_FAILURES`, is
  empty). The fix has two halves, both needed and each shown by taking it
  out: the page no longer loads more for a browse grid that has left the
  document, and the scenario's scroll helper sends one scroll event, not
  two (the blocker's entry and what its diagnosis missed are in
  `docs/history/state-archive.md`, rotated that day). The full run of both suites at `b066dc0`, each building its app, passed everything: external 48 of 48, embedded 50 of 50, parity verdict PASS (the first run with no failure at all; results kept on `continuum` in `~/dev/vela-agent-keep/full-b066dc0-e2e/`).
  One known intermittent failure remains, `completionhub` in the embedded
  player (Blockers); wherever the account below calls `pagefail` the one
  recorded failure, that was true when it was written.
- ALSO 2026-11-15: SETTINGS SAVE EACH CHANGE AS IT IS MADE (`5fc040f`;
  plan `.agents/plans/settings-autosave.md`, decision of that day). The
  Player tab's seven Save buttons are gone; a setting added later gets
  none. What was checked, and one timeout that could not be repeated, are
  in the plan's log.
- ALSO 2026-11-15: THE OWNER IS USING THE EMBEDDED PLAYER and reports
  what the by-eye checks were for. First report: its Audio and Subtitles
  lists closed the moment they opened while a video played; fixed at
  `2a954d0` (they are drawn in the page now; the embedded player plan's
  log). Second report: a double-click on the picture did not toggle
  fullscreen; fixed at `e64efb3`. The package on `continuum` is rebuilt
  after each fix (the plan's "How to try it" names the commit). Expect
  more such reports; each is fixed as it comes (decision of 2026-11-14
  on what is obviously broken). 2026-11-16: the embedded player answers
  the mouse (`013cee3`; the owner asked for "all the mouse actions that
  make sense for standard video player apps", the set is in the plan's
  log), and testing it uncovered that a play's final report to the
  server could be lost to a closed connection, in both players; fixed
  at `e50aa83`.
- The embedded player (`.agents/plans/embedded-player.md`, approved
  2026-11-12) and the player window carried from play to play
  (`.agents/plans/mpv-window-geometry.md`, landed 2026-11-14 at `9815e10`
  as Vela 1.0.64, not released) are both on `main`, and every codex
  review of them is closed (`.agents/review/index.md` has no open
  finding). Each plan's "Implementation log" says where it stands. Read
  the embedded plan's "Owner checks outstanding" and "For the owner"
  before telling the owner anything, and the player-window plan's log
  for the owner's acceptance steps on real desktops, none of them done.
  Gaps left on purpose are in the findings' records (`pw-1`: nothing was
  built or run on macOS or Windows, and no real second screen; `pw-9`,
  "Not taken"). Going back to an older Vela after using this one can
  need Settings recovery (the release notes say when). The account of
  that run, written while it went on, rotated to
  `docs/history/state-archive.md` on 2026-11-16. Of the test repairs it
  left parked, `pa-7`'s is being landed (entry above); `pa-5`'s (codex,
  worktree `cx-pa5r5` on `continuum`) was never reviewed or landed.
- HOW TO WORK HERE: rules the owner set since 2026-11-12, most of them
  in `.agents/decisions.md` under their dates; read them there before
  doing anything. Every slice with repo code gets a `codereview codex`
  pass, and each commit is pushed (2026-11-12, "plans are approved").
  Settle implementation and failure-mode details yourself and record
  them; bring the owner only what changes Vela for its user, briefly,
  and never put an answered ask again (2026-11-12, 2026-11-15).
  Licensing is closed: do not raise it. What is obviously broken is
  fixed without asking (2026-11-14, which also says what is still
  brought to the owner). Coding tasks may go to codex, called plainly,
  and a repair codex wrote is closed by the agent's review, since codex
  cannot review code it wrote (2026-11-14). What is built for the owner
  is made usable from the app, and the owner is Vela's only user
  (2026-11-15). On 2026-11-13 the owner switched on multi-agent
  workflows for the session doing this work; the agent reads that as
  the approval the 2026-08-20 constraint below asks for before more than
  one Claude session is started, for that session only. The same day
  the owner asked that agents started from then on run on Opus 5.5.
  Reviews of record still go to codex, and no agent is cut short.
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
- E2E `completionhub` fails now and then in the embedded player. First seen
  2026-11-15: once in the full embedded run at `3505387` (which therefore
  failed the parity verdict: 48 of 50), then in 2 of 49 runs of the scenario
  alone on the same build. A product race, older than the Settings control
  landed that day (which touches none of this code), and with next to no
  effect on a real user:
  - When a play ends, the tracker thread emits `playback-ended` and only
    then hands the completion to the dispatcher (`src-tauri/src/commands.rs`,
    the play's end notify: `app.emit("playback-ended", ...)` before
    `advance.mark_ended(...)`). The dispatcher (`src-tauri/src/lib.rs`)
    then records the clean completion locally (`admit_clean_completion`:
    the session leaves recents and its identities are tombstoned), writes
    "played" to the servers, and emits a second `playback-ended`.
  - On the first event the page's `loadHome` asks for three things at
    once: `get_hubs` (the server), and `get_recents` and
    `get_continue_tombstones` (local, answered at once). When those two
    are answered before the dispatcher's admission is written, the page
    merges the server's Continue Watching, which still lists the finished
    episode, with local state that does not yet hide it.
  - Evidence, from the scenario made to print what the page showed (not
    committed): in the failing run, after the first Home response that
    follows the end, the page showed "Hub Series, S1 · E1 – Completed
    Episode" where the scenario expects nothing, with the server's
    "played" request arrived and unanswered; 3.9 s later, after that
    answer and the second refresh, it showed "S1 · E2 – Follow-up
    Episode". In every passing run the first look showed nothing.
  - User-visible effect: the finished item can stay in Continue Watching
    until the server has recorded it as played and the second refresh
    lands. With a real server that is a fraction of a second, and the
    second refresh normally supersedes the first before it is shown. The
    scenario delays the server's answer by five seconds to look at the
    state in between.
  - The external player runs the same code; it has not been seen to fail
    there.
  - Not fixed, on purpose: the effect is negligible and the ordering here
    has a long review history (`tests/clean-eof-refresh-order.test.mjs`).
    It waits for the release's test pass (decision of 2026-11-14 on
    tests). Candidate fix: the page reads the two local lists after
    `get_hubs` has answered, and the two local commands wait, bounded, for
    a completion that has been handed to the dispatcher and not yet
    admitted.
  - Until then: an embedded run that fails the verdict on `completionhub`
    alone is this race until shown otherwise; run the scenario again by
    itself before reading it as a regression.
- The `refresh` E2E flake remains recorded (1.0.59 full Linux run, 36/39,
  with `continueon` and `playverbs`). A candidate cause, seen 2026-11-15:
  `refresh.mjs` and `watchposition.mjs` scrolled the grid with the helper
  `pagefail` had until `b066dc0`, which moved the grid AND sent a synthetic
  scroll event. That is two events, and the second can ask again for a page
  whose request has just failed. Both scroll with one event since `ece850b`
  (2026-11-16; external 4 of 4 and embedded 3 of 3 that day); whether that
  was the flake is not established. One cause of the continuation flakes
  is gone since 2026-11-13: scenarios sought in a file the moment mpv named
  it, and mpv can answer that with "property unavailable" (seen in
  `continuetv` and `continueon`). The harness now waits until mpv reports a
  position before any seek (`tests/e2e/mpv.mjs`). Whether that was the
  whole of the `continueon` and `playverbs` flakes is not established.

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
- `.agents/plans/remove-sveltekit.md` (landed 2026-11-16 at `dee0f8b`, Vela
  1.0.66; the owner's visual checks on the Mac are open)
- `.agents/plans/macos-airplay-sync.md` (implemented for 1.0.61; packaged
  physical acceptance pending)
- `README.md`, `RELEASE_NOTES.md`, `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
