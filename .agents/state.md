# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- HANDOFF, 2026-11-17 ~18:20 EST. Branch `main`, head `d645615` (pushed to
  origin and github). Task in progress: the Linux AppImage starts on Wayland
  (`.agents/plans/appimage-wayland.md`, its Log). Stage: fix landed
  (`de91332`, `25b4e8d`); release run 44195133042's AppImage passes the
  package check and plays in Vela's window on this AMD machine. NEXT:
  (1) read the codex hunt `ai1` (scratchpad `review-final`, slice `ai1`,
  range `03a4d98..25b4e8d`; record it in a new
  `.agents/review/codereview-2026-11-17-appimage.md`, repair findings one
  per commit); (2) the owner's NVIDIA test of that AppImage when the
  machine (10.1.10.77) is back on Linux; (3) then correct RELEASE_NOTES'
  AppImage lines. Live blockers: none of the agent's; the AppImage's NVIDIA
  proof waits on the owner's machine. Settled today and verified on all
  three platforms by the owner: N1 (NVIDIA offered), W1 (weston), the
  Settings restructure and its repairs, audit-what-ships
  (`.agents/decisions.md`, 2026-11-17 entries). Still owner's: the slow-runner
  plan (`.agents/plans/e2e-slow-runner.md`, draft), tagging.

- APPLE PORTS, PLANS DRAFTED 2026-11-18, NOT APPROVED. Owner rulings of
  that day (`.agents/decisions.md`): iPadOS first, then iOS, then tvOS;
  on all three, video plays through one engine built on Apple's own
  pipeline, never mpv, so Apple TV shows real Dolby Vision. Plans:
  `.agents/plans/ipados-port.md` (its Part A holds what all three share:
  the player seam, the engine, the media libraries, the probes),
  `.agents/plans/ios-port.md`, `.agents/plans/tvos-port.md`. Review
  (`codereview codex gpt-6.2-astra xhigh`, owner-named for these
  dispatches; record `.agents/review/codereview-2026-11-18-apple-plans.md`):
  - first hunt: 50 findings, all repaired;
  - repair hunt: 39 closed, 10 reopened, 30 new;
  - after the owner's ruling of 2026-11-18 (no tests designed or reviewed
    for unwritten code), 9 declined and the rest repaired. 33 rows wait
    for a verdict.

  NEXT, the owner's: whether a further codex round runs, scoped to
  design and excluding tests. O1 is answered (devices in
  `.agents/machines.md`, "Apple test devices"); the next owner input is
  O3, Xcode on `nagatha` (O2 is ruled:
  TestFlight and development signing now, the App Store later).

- PLATFORM PARITY, OWNER RULING 2026-11-16 (`.agents/decisions.md`, last
  entry; read it first). Vela works the same on Linux, macOS and Windows;
  a feature missing on one platform is a defect, and "by design" is never
  the answer. Before asking the owner to check anything on a platform, say
  what that platform has and lacks. The agent recommended holding the tag
  until the embedded player works on macOS and Windows; the tag is the
  owner's. Where it stands (2026-11-16, evening):
  - WHAT THE PLAN'S "Amendment, 2026-11-17" LISTED AS GAPS:
    - X11: NOT A GAP. Listing it was the working agent's error, against
      the plan's own non-goals, the README and the reason Vela exists (see
      `.agents/repo-guidance.md`, Mission Detail). The overlay mode built
      for it (`671cba9`, `4b91540`, `9303cee`, `d41a926`) is removed
      (`74df55a`).
    - macOS, VideoToolbox straight to the picture: done (`1e9905e`; probe
      M2's run 44061797549 decoded that way on a hosted Mac, the plan's
      Probe log).
    - NVIDIA's proprietary driver: ANSWERED 2026-11-17, offer Vela's
      window there anyway; the owner tests it on an NVIDIA machine of
      their own (`.agents/decisions.md`, that date).
  - Linux's end-to-end venue: ANSWERED 2026-11-17, the external suite
    moves from Xvfb (X11) to weston, where the embedded suite already
    runs (the same entry).
  - THE PLAN FOR BOTH: `.agents/plans/embedded-player.md`, "Amendment,
    2026-11-17 (2)", APPROVED by the owner 2026-11-17 ("Approve both").
    Slice N1 LANDED at `daa182c`: the NVIDIA refusal is gone
    (`startup.rs`, `select.rs`), with the docs and release notes; release
    run by hand 44098988278 at `daa182c` builds the owner's Linux packages
    for the NVIDIA test (steps in the slice). Slice W1 LANDED at
    `9d44c5e`: both suites run in a private headless weston, no X11
    display; `markerbutton` draws with `--vo=wlshm`; `playerfallback`
    proves the start-up marker's recovery instead of X11's refusal,
    red-proved twice. At `9d44c5e`, in a clean worktree: external 48 of
    48, embedded 51 of 51, parity verdict PASS. Codex hunts
    (`.agents/review/codereview-2026-11-17-nvidia-venue.md`): N1 clean;
    W1 found w1-1 (LOW, two comments still naming Xvfb), repaired at
    `998d14c`. CI passed at `daa182c`, `9d44c5e` and `998d14c`.
    WAITING ON THE OWNER: the NVIDIA test (the plan's Slice N1).
  - Landed: the macOS app declares why it reaches the local network
    (`d6a8033`; the notarized app of a release run carries the sentence);
    the player window finds its remembered screen on a Mac, Retina
    included (`b3635db`), on a Mac with two screens of one model
    (`e54b302`), on Windows, where mpv ignored `--screen-name` and takes
    only `--screen=<number>` (`3cc0667`), and on Linux for a play that
    starts fullscreen (`c49be2a`: mpv picks a fullscreen screen from
    `--fs-screen-name` only); version 1.0.68 (`be1a98e`).
  - The embedded player on macOS and Windows (the plan's amendment of
    2026-11-16, "Probe log", "Design that follows", "Landed"):
    - Windows: done but for the owner's checks. The set from the pins
      (`3dad7a8`, `8c40ff8`; plays on a Windows runner), the host
      (`6b206ec`, review fixes `ca07bfa`), the installers carrying the set
      (`4fe60b8`; a release run by hand proved both installers and the
      set unpacked from each).
    - macOS: the host (`623ec17`: the render API into a CAOpenGLLayer
      under the page) with HDR through the layer's EDR (`49f2c3c`); the
      set from the pins (`ba677b7`, fixes `c60a6d3`, `485628c`,
      `ad65c4d`) built and played on a hosted Mac, natively and under
      Rosetta (probe run 43969988737); its CI job runs on every push.
      Vela.app carries the set (`2181dcb`), proven by a release run by
      hand (43974146432: signed, notarized, the set played from inside the
      DMG's app on both architectures). Probe M2 (Vela's own embedded
      player on a hosted Mac, `probes/m2-vela`) found the picture black;
      three repairs (`cf23d90`, `e3b4005`, `a45999c`: mpv's updates drive
      `display()` on the main thread, which never waits inside a render)
      and mpv's fast profile for the VM's software renderer, and the test
      pattern plays in Vela's window (the plan's Probe log).
    - Windows: probe W2 (`probes/w2-vela`) found Vela refusing its own
      settings under the built-in Administrator account (the privacy check
      matched SDDL text, which names that account `LA`); repaired at
      `96c0f4d`, after which W2 played twice in Vela's window on a hosted
      Windows runner with the test pattern on screen (run 43987838828).
    - The platforms review (`.agents/review/codereview-2026-11-16-platforms.md`):
      twelve codex hunts found pl-1 to pl-26, and all are closed (pl-9,
      about tests alone, in the release's test pass at `50b8893`); the
      twelfth hunt, over the last repairs, found nothing.
  - THE OWNER'S CHECKS: `.agents/plans/embedded-player.md`, "For the
    owner: each platform, what it has, what to check (2026-11-17)": what
    each platform has and lacks, what machines proved, what to look at, in
    order. Builds: release run by hand 44098988278 at `daa182c` (every
    job passed, the macOS bundle on its third attempt: Apple's timestamp
    service failed once, GitHub had no Mac runner once; it adds Slice N1,
    Vela's window offered on NVIDIA, to run 44087348857 at `74df55a`,
    which carries pl-1 to pl-26, every product repair of the
    release's test pass: the clean-end report order, `build.rs`, the
    Windows set's revocation soft-fail, the marker script's paused
    button, the boot keeping Playlists; and the macOS set's VideoToolbox
    build, `1e9905e`, the only product difference from the earlier run
    44042221966 at `23d4745`, whose app code is the same); later commits
    are tests and records only, unless a later entry here says
    otherwise. The repaired
    marker script was checked inside its NSIS installer (next to
    `vela.exe`) and DMG (`Vela.app/Contents/Resources`). Probes M2
    (44018857651) and W2 (44018869179) played twice in Vela's window at
    `dcb0d2d`; the product changes since touch neither (the set's TLS on
    Windows, a mpv script, the page's boot) but the macOS set's
    VideoToolbox build, which M2 itself proved (44061797549 at
    `1e9905e`), and the Windows suite in full (below) plays in Vela's
    window at `aa8c0b0` and `74df55a`.
  - Linux unchanged through all of it: both end-to-end suites in full at
    `6b206ec`, `623ec17`, `c49be2a`, `49f2c3c`, `cf23d90`, `959f437`,
    `3061b52`, `0b2099a`, `2bc6e3c`, `c3ea91d`, `dc3c9e8`, `23d4745`,
    `74df55a` and `9d44c5e` (48 of 48, 51 of 51, parity PASS; `9d44c5e`,
    the first with both suites in weston, run in a clean worktree,
    contains the builds' commit `daa182c`, whose one product change acts
    on NVIDIA's driver alone); at `7f1db1f` the embedded suite lost
    `completionhub` once, at `f2dcbc3` and `019c08b` `markers` once each,
    and at `0357eec` `watchposition` twice on a loaded machine (all in
    Blockers).
  - THE RELEASE'S TEST PASS (2026-11-17; the decision of 2026-11-16
    reached the 2026-11-14 rule's condition). Done so far:
    `completionhub`'s race repaired (`2ab8ac5`, `c3ea91d`; the scenario
    alone 40 of 40 in the embedded player at `f2dcbc3`); pl-9's guard
    (`50b8893`); a codex hunt over it (tp-1 to tp-3, all repaired:
    `.agents/review/codereview-2026-11-17-testpass.md`). The Windows
    end-to-end suite runs at last: probe W3 (run 44003737295) found that
    Edge WebDriver's DevTools arguments never reach Vela's browser on the
    elevated runner, and the suite's Windows build now opens the port
    itself (`94662e3`); its runs are no longer dirty (`2302240`, line
    endings), each scenario keeps one WebView2 data folder (`f089797`),
    and the runner shows animations (`ae11f7f`). Its first full run
    (44007571185 at `2ab8ac5`: external 31 passed, 16 failed; embedded
    37 passed, 14 failed) found one real defect: under Cargo 1.100's
    build-dir layout a debug or --no-bundle build had no mpv scripts next
    to the binary, so markers and autocrop were missing (on every
    platform with that Cargo; installed bundles unaffected), repaired in
    `build.rs` (`e87ffe2`). The second full run (44013741142 at `e87ffe2`,
    clean: external 39 passed, embedded 42) failed only scenarios whose
    repairs landed after it (`73666ac` the Plex test CA, `f177a85`,
    `1b83109`, `7687583`, `2546470` timing on the slower runner,
    `019c08b` sourcepolicy skipped on Windows; the plan's Probe log). The
    third (44018219446 at `019c08b`: external 43 passed, 3 failed;
    embedded 48 passed, 2 failed) and fourth (44023883002 at `8d1a8b5`:
    embedded 49 passed, 1 failed) found a second real defect: the
    Windows set's FFmpeg verifies TLS through Schannel with a hard
    revocation check, so Vela's window on Windows refused HTTPS from a
    server whose certificate a Windows-trusted authority issued but
    without reachable revocation information; repaired in the set's
    build (`3c4a510`: revocation soft-fails, as browsers do; chain and
    host name still checked), and CI rebuilds the set. The other
    failures were the suite's own (`658a924`, `8d1a8b5`, `931542b`). A
    second codex hunt found tp-4 to tp-7, repaired (`1342e6a`,
    `f29d77c`, `d1cc1b3`, `d48bc08`: Settings controls now carry
    aria-busy until loaded); a third and a fourth found tp-8 and tp-9,
    repaired (`6f92baa`, `17893e6`), and a fifth was clean. The fifth
    Windows run (44028451739 at `a465771`, with the repaired set) passed
    the embedded suite in full (50) and the external but for
    `markerbutton` (45 passed), which found a third real defect: the
    marker script drew no skip button when a play was paused inside a
    marker before mpv's OSD had a size, the case the scenario makes.
    Reproduced with real mpv and repaired in the script (`0357eec`; the
    plan's Probe log), with tests under LuaJIT and Lua 5.2, which CI's
    `frontend` job installs, and in a CI run a missing one fails the
    check (`311347e`, tp-10 to tp-12). The sixth Windows run
    (44034984612 at `0357eec`) passed the embedded suite in full again
    (50) and `markerbutton`; its `playlistplay` found a fourth real
    defect: Playlists chosen while Vela starts was put back to Home by
    the boot's first Home load (repaired in the page, `2f6f37d`), and
    `playlistedit` looked for a playlist before the view had listed it
    (`aa8c0b0`). THE SEVENTH WINDOWS RUN PASSED BOTH SUITES IN FULL:
    44040455337 at `aa8c0b0`, not dirty, external 46 of 46, embedded 50
    of 50, parity verdict PASS. Codex hunts six and eight were clean;
    seven found tp-10 to tp-12, repaired. RELEASE_NOTES.md describes the
    embedded player on all three platforms and the fixes. Release run
    44042221966 at `23d4745` passed. At `74df55a` (overlay mode removed,
    the macOS set built with VideoToolbox) the Windows suite passed both
    players in full again (44087343362, not dirty: external 46, embedded
    50, parity PASS), and release run 44087348857 passed every job: the
    builds above.
  - Type-checking Windows and macOS Rust from Linux:
    `scripts/check-other-platforms.sh` (repo-guidance, Verification).
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
  - HANDOFF, 2026-11-16 about 18:10 UTC (written in case the session
    stops; the session moved from Fable to Opus at 98% of Fable's weekly
    allowance). Where things are:
    1. APPLE IS FIXED. The release workflow's run by hand 43914914442 at
       `dee0f8b` (1.0.66) passed every job once GitHub had a macOS runner
       for it: the macOS app was notarized ("Accepted"), stapled, and
       Gatekeeper reports "Notarized Developer ID"; the inventory job
       found all seven packages and wrote `SHA256SUMS`.
    2. THE TITLE BAR IS FIXED and the version is 1.0.67: `6b62e21` (on
       Wayland, when the compositor prefers to decorate windows itself,
       Vela takes tao's GTK header bar off its window during setup, so
       KWin draws its own), `f6f22cd` (the window's app id is "Vela", the
       name of the desktop file every package installs, so KWin shows
       Vela's icon in that bar), `b163241` (1.0.67). Seen on the owner's
       desktop with a scratch configuration: KWin's 28 px bar, content
       still 1280x753, the page rendering in both window modes, Vela's
       icon. Not seen: a video playing in the embedded player under the
       new bar (its position is measured at each play, `host.rs`).
    3. The final proofs of `b163241` PASSED (18:05 UTC): both end-to-end
       suites in full on a clean checkout, external 48 of 48 and embedded
       51 of 51, parity verdict PASS; the full check (733 Rust tests, 737
       Node tests; `cargo audit` not installed on `continuum`, CI runs
       it); the 1.0.67 package for `continuum`, built with the library and
       checked, its own binary passing in both players:
       `~/dev/vela-agent-keep/pkg/vela-1.0.67-1-x86_64.pkg.tar.zst`,
       sha256 `2c8e9580b5c6826dccfbf348552ad0ed2d18775d13fd1f724398b7cb44c1fa2f`. Logs: `~/dev/vela-agent-keep/final-b163241/`.
       The release workflow by hand at `b163241`, run 43921694491: the
       Windows, Linux and Arch builds passed. Its macOS job was cancelled
       by GitHub, as the first attempt of 43914914442's had been: "The job
       was not acquired by Runner of type hosted even after multiple
       attempts", with GitHub's notice "Due to capacity constraints, jobs
       targeting macOS arm64 runners may experience longer queue times".
       Not Vela's fault; re-running the job is the remedy: re-run at
       18:08 UTC (`gh run rerun 43921694491 --repo roethlar/vela
       --failed`), it got a runner and PASSED: the app notarized
       ("Accepted"), stapled, and accepted by Gatekeeper as "Notarized
       Developer ID"; the inventory job then passed, so the run is green
       with every artifact (`vela-macos`, `vela-windows`, `vela-linux`,
       `vela-arch`, `vela-sha256sums`).
    4. NOTHING IS LEFT FOR THE AGENT ON THE RELEASE. Told to the owner
       at 18:16 UTC. Then the owner: download the run's `vela-macos` and
       `vela-windows` artifacts (`gh run download 43921694491 --repo
       roethlar/vela`), check them, install the 1.0.67 package on
       `continuum`, and tag `v1.0.67` when satisfied.
  - QUEUED FOR 1.0.68, held off `main` on purpose: the owner, verifying
    the 1.0.67 builds (2026-11-16): "for the next rev, the "EXPERIMENTAL"
    tag in settings next to Video player is incorrect. the experimental
    feature was the embedded player, not the setting." Fixed on the LOCAL
    branch `next` (not pushed; its worktree was removed 2026-11-17, the
    branch kept; its change is on `main` by `git cherry`), commit `793021e`: the heading's badge is gone, the
    "Vela's window (experimental)" choice keeps the word, a guard holds
    both (red-proved); `npm run check` (738) and the build pass. It is kept
    off `main` so that `main` stays exactly the 1.0.67 the owner is
    verifying, and tagging `main` stays safe. To land it once `v1.0.67` is
    tagged (or at once, if the owner decides the fix goes into this
    release instead): merge `next` into `main`, bump to 1.0.68, write the
    notes (changes since 1.0.67 if 1.0.67 was published), run the usual
    checks, push, and a run by hand of the release workflow for the
    owner's builds.
  - NOT verified by anyone: this version running on macOS or Windows (CI
    compiles it there, nothing more), and everything that takes eyes.
- THE FRONT END IS PLAIN SVELTE 5 AND VITE SINCE 2026-11-16 (`dee0f8b`;
  plan `.agents/plans/remove-sveltekit.md`, whose last log entry has the
  landing; decisions of 2026-10-23 and 2026-11-16). SvelteKit and
  adapter-static are gone; `index.html` loads `src/main.ts`, which mounts
  `src/App.svelte` (what records before that date call
  `src/App.svelte`). Left for the owner: the plan's visual checks
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
  One known intermittent failure remained then, `completionhub` in the
  embedded player, repaired 2026-11-17 (its entry is in
  `docs/history/state-archive.md`); `markers` is the one now (Blockers).
  Wherever the account below calls `pagefail` the one recorded failure,
  that was true when it was written.
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

- RELEASES BLOCKED BY npm audit, 2026-11-17, OWNER DECISION ASKED: advisory
  GHSA-fc5q-39xc-vjpf (high, ReDoS) covers every `picomatch` up to 4.0.7,
  the newest published, so no fix exists yet; release run 44158190579
  failed its `npm audit` job and skipped every build. picomatch reaches
  Vela only through build tools (vite, svelte-check, tinyglobby, fdir):
  `npm audit --omit=dev` finds 0 vulnerabilities. The decision of
  2026-11-02 allows no overrides or per-advisory suppression, so the gate
  stands until the owner rules (wait for picomatch, or audit production
  dependencies only, or otherwise).
- SETTINGS RESTRUCTURE (`.agents/plans/settings-restructure.md`) LANDED and
  verified: S1 `0e4aef7`, S2 `47b2fc0`, S3 `ce6e96d`; hunt findings st-1 to
  st-8 repaired (`.agents/review/codereview-2026-11-17-settings.md`); both
  Linux suites in full at `0e4aef7`, `ce6e96d` and `d61b194` (48 of 48,
  51 of 51, parity PASS). For the owner's look, an Arch package of
  `a65f521` built locally with the libmpv set and passing
  `packaging/libmpv/check-package.sh`, on the NVIDIA machine as
  `~/vela-nv-test/vela-1.0.68-1-settings-x86_64.pkg.tar.zst` (sha256
  `455a9f3d…b142`). The owner installed it there: "verified on the other
  linux box" (2026-11-17). WINDOWS, 2026-11-17, at `84497a8` (release run
  44166626358, every job passed, after the owner's decision to audit what
  ships): the Windows suite passed both players in full (run 44166655927,
  parity PASS), and the owner, on the NVIDIA machine booted into Windows,
  with the plan's Windows checks: "works, but there's no working second
  screen to test". NOT TESTED on Windows: the remembered second screen and
  moving a playing window between screens (no second screen there).
  MACOS, 2026-11-17, the same release run (its macOS bundle passed on a
  second attempt after GitHub found no Mac runner): the owner, with the
  plan's macOS checks: "looks fine on mac". Second-screen checks not done.

- FOUND 2026-11-17, NOT FIXED: Vela's Linux AppImage always runs under
  X11. Tauri's bundler puts linuxdeploy's GTK plugin hook in it
  (`apprun-hooks/linuxdeploy-plugin-gtk.sh`), which does
  `export GDK_BACKEND=x11` unconditionally ("Crash with Wayland backend on
  Wayland", tauri-apps/tauri#8541). So the AppImage gets no HDR path and
  Vela's window is always refused there (`NotWayland`), against the rule
  that X11 is not supported in any way (`.agents/repo-guidance.md`). Seen
  on the owner's NVIDIA machine (`.agents/machines.md`): with `DISPLAY`
  unset the AppImage cannot start at all ("Trying x11 backend", then GTK
  fails). The deb, rpm and Arch packages carry no such hook. A fix
  changes packaging: it needs a plan and the owner's go.
- NVIDIA, Slice N1, first machine evidence (2026-11-17, the owner's
  machine: RTX 5090, NVIDIA open kernel module 620.70.06, KDE Plasma
  Wayland, Arch): run 44098988278's AppImage, extracted with the hook's
  `GDK_BACKEND=x11` line removed so it starts on Wayland, with a
  throwaway config asking for Vela's window. It started in embedded window
  mode (no refusal line, which a refused start prints), stayed up 20 s with
  no "Error 71" and no WebKit renderer error, and its page loaded (the
  start-up marker was gone within a second). The window DRAWS: the owner,
  looking at that machine's screen, "looks correct with the add source
  prompt" (2026-11-17). PLAYS on NVIDIA with the system's WebKitGTK: the
  deb build's binary (system WebKitGTK 2.54.2), driven through
  tauri-driver with the owner's Plex added in the test session, played a
  1 h 41 min film in Vela's window, its clock advancing 0:01 to 0:19 over
  20 s, with `get_player` answering offered, "Videos play in Vela's
  window". The owner's own test, on the AppImage, failed ("it doesn't
  play. there's no mpv window"): SECOND APPIMAGE DEFECT, the AppImage's
  bundled WebKitGTK (built on Ubuntu 22.04) loses its page about 2 s
  into a play in Vela's window on NVIDIA (the page's text goes empty; no
  core dump; Vela's process stays up). The Arch package of run
  44098988278 is on that machine (`~/vela-nv-test/`) for the owner to
  install (`sudo`). What to do with the AppImage is the owner's decision.
  The owner installed it; after a real restart (the first plays went to
  mpv's window with "restart Vela to use it": that Vela never restarted),
  the owner, 2026-11-17: "playback works in both the embedded and external
  players without issue". N1 IS PROVEN ON NVIDIA. Follow-up asked by the
  owner: `.agents/plans/settings-restructure.md` (draft).

- No known product blocker.
- A timing failure in a local end-to-end run proves nothing until the
  machine is known to be quiet: check `uptime`, and look for stray
  `mpv` and `weston` processes (`Xvfb` before 2026-11-17's Slice W1),
  first. On 2026-11-17, from about 06:50 to
  08:09 UTC, twelve `mpv` processes and their `Xvfb` servers, left by the
  agent's own reproduction of the marker defect (its script stopped
  `xvfb-run`, not what ran under it), kept this machine at a load of
  about 20; `watchposition` failed twice and `pagefail` once in that
  window (next entry).
- The Windows end-to-end suite PASSED IN FULL at `b871fcc` (run
  44115812289: external 46 of 46, embedded 50 of 50, parity verdict PASS),
  which carries N1, W1 and `smoke`'s repair. The intermittents below stay
  recorded; the draft plan `.agents/plans/e2e-slow-runner.md` awaits the
  owner. Before that, it did not pass at `9d44c5e` (run 44101184219),
  failing different scenarios on each of two attempts. Attempt 1: external
  in full, embedded 48 of 50 (`curation`, `resume`: "mpv: no reply to
  get_property"; both failure screenshots show the first play run to its
  end: `curation`'s card marked watched, `resume`'s Home empty, with no
  title to resume, where a stop at 6 s leaves one; ASSUMPTION, supported by
  those, not by timings (the run records none): on the slow runner `playAndQuit`'s wait for Vela's
  connections outlasted the 10-second clip, and the embedded player closed
  its pipe before the harness's `time-pos` question, helpers.mjs:607).
  Attempt 2: embedded in full, external 44 of 46 (`continuetv`: a WebDriver
  `execute/sync` got no response within 30 s; `smoke`, next: the page's
  version line read null; that one was a real race in the test, which
  read the footer before the page's `get_app_info` answer had drawn it:
  repaired in the commit after `0b485de`, red-proved with the version
  removed from the footer). Nothing that runs on Windows changed since
  `74df55a`, where both suites passed in full (44087343362): the product
  change of `daa182c` is Linux-only, and the harness change of `9d44c5e`
  touches Windows only in a log line. So these are the hosted runner's
  intermittents, not a defect found; they make the Windows suite a weak
  gate until its waits are made robust (proposed to the owner, not done).
- E2E `watchposition` and `pagefail` fail now and then in the embedded
  player, more often on a loaded machine. `watchposition`: the grid's
  `scrollTop` moves 1 to 4 px during one of its holds (777 to 776, 775
  or 773; seen in the successful, failed and stale legs). `pagefail`:
  back on the grid it truncated, page 2 has arrived after all (120 cards,
  not 60). At `23d4745`, each alone in the embedded player: just after
  the stray load above, while the machine was still paging back in,
  `watchposition` failed 2 of 10 and `pagefail` 1 of 10; quiet,
  `watchposition` 1 of 20 and `pagefail` 0 of 20. Neither failed in the
  26 full runs before 2026-11-17. Neither is visible to a user (a pixel
  or a few of scroll; a page that loads by itself). Known: WebKitGTK
  anchors scrolling (content inserted above the view moves `scrollTop`
  by its height), but a transform, a banner of any height above the
  shell, and a badge on a card below the view move nothing (scratch
  probes on a real grid); with the grid's anchoring off
  (`overflow-anchor: none`) both passed 20 of 20, too few runs to tell
  from 1 in 20 with it on. Candidate for `pagefail`, unconfirmed: WebKit
  sends the grid's scroll event a moment after it moves
  (`scrollGridToEnd`'s note), and if the doomed page-2 request has
  already failed, that event, within 600 px of the end (`onScroll`),
  loads page 2 again. The cause was not found and the investigation
  stopped there (stalled progress, AGENTS.md). An embedded run that
  fails the verdict on one of these alone: run it again by itself before
  reading it as a regression.
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
