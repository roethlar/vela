# Embedded player, Slice 6 (parity): what was found out before it is designed

Written 2026-11-13. Slice 6 of `.agents/plans/embedded-player.md` has to show
every row of the plan's 14-row behaviour inventory in both players. Before
designing it, the agent had five helpers look, on `main` as it stood before
Slice 5: four ran the scenarios that failed in the embedded venue's full run
of 2026-11-13 until each had a cause shown by a run, and one mapped the
inventory to what proves each row in each player. This file is their
findings, lightly edited. Nothing here has been changed in the repo yet.
Their proposed scenario changes existed as diffs in a session scratch
directory; a convenience copy is in `~/dev/vela-agent-keep/s6/` on
`continuum` (not a source of truth: they predate Slice 5 and will be redone
against the tree as it is then).

`autocrop` (Slice 5 gives the embedded player its own crop script and the
scenario `embeddedcrop`) and `pagefail` (the recorded product race in
`.agents/state.md`, Blockers) were not looked at.

## Three findings that are not about the tests

1. **The external player does not check the server's TLS certificate.**
   Shown by a run: system mpv 0.41 defaults to `tls-verify=no` and Vela
   does not set it. In the `multiplex` scenario mpv fetched the stream from
   a server whose certificate it could not have verified, and sent the Plex
   token to it. Vela's own client does verify. The embedded player's mpv
   (0.42) verifies by default. This is for the owner: setting
   `--tls-verify=yes` in the external launch would close it and would make
   the two players alike, and it could stop playback from servers whose
   certificate Vela's own client accepts and mpv's TLS library does not.
   Recorded in `.agents/state.md`, Blockers.
2. **The embedded player trusts a different set of certificate authorities
   than Vela's own client.** Vela's client honours `SSL_CERT_FILE`; mpv's
   TLS (FFmpeg with GnuTLS) does not. With a server on a private CA,
   browsing works and Play opens and closes the player at once, with no
   message. Also for the owner (same Blockers entry).
3. **A stream that will not open ends without a word, in both players.**
   Shown by a run (an HTTP 404): the play starts, checks in, checks out and
   ends; no message on any surface. In the embedded player the view flashes
   open and shut. Not a defect of the embedded player; recorded so that it
   is not rediscovered.

## The test venue itself

- **Headless weston goes idle after 300 seconds and then never draws a
  newly opened window.** WebDriver's input does not count as activity. This
  is what hung `watchstate`'s screenshot, and by the look of it other
  screenshot-taking scenarios late in a full run. Fix: start weston with
  `--idle-time=0` in `tests/e2e/run.mjs`. A full embedded run with it
  passed `watchstate`, `watchstate0` and `watchcurate`. Re-baseline the
  embedded run with this before reading anything else from it.
- **Headless weston has no seat, so the app's window is never active and
  the page never has focus** (`document.hasFocus()` is false, `:focus`
  never matches, GDK logs three `gdk_seat_get_keyboard` criticals per
  start). This is what failed `uifoundation`, and it means every scenario
  in that venue that relies on focus is measured in a state no user sees.
  Fix: `--fake-seat`. Run the whole embedded suite with it before keeping
  it; it may change pointer and hover state elsewhere.
- A scenario that picks up mpv's socket file sees a play that has already
  ended as ten seconds of `ECONNREFUSED` with no reason. Worth a clearer
  message in `MpvIpc.connect`.
- `tests/e2e/run.mjs` compares `onlyPlayer` only for inequality: a
  misspelt value makes a scenario skip in both runs.

## The failing scenarios, one by one

### `multiplex`

- Kind: scenario assumes the external player (shown by run).
- Stands for: Row 2 (Plex timeline and final stop): multiplex is the only Plex proof of that row. It also touches row 13 (user mpv arguments reach the player): the fix depends on the embedded player honouring two lines from mpv_extra_args, and it does. Row 6 (source choice) is only touched in passing: the Play Version backing choice and the override it persists.
- How it fails: Step `playAndQuit(driver, () => chooseBacking(driver, 'Mock Plex A'))` fails with `connect ECONNREFUSED /tmp/vela-<id>/mpv-<velapid>-<ts>.sock`. It failed in 2 of 2 unmodified embedded runs (emb-1, plus emb-2 with instrumentation only), each about 13 s end to end, and failed the same way in the 2026-11-13 full run. MpvIpc.connect retries for 10 s and then rethrows the last ECONNREFUSED. The page never shows an error: after the 10 s it is back on the Movies grid ("Shared Plex Movie · 2 sources").
- Cause: The stream fails to open because the shipped libmpv checks the mock server's certificate. The socket is not stale and two plays did not overlap: this run's private directory held exactly one socket, the one this play made. mpv's own log of that play (scratch mpv-emb-2.log, lines 455-482) shows `loadfile https://127.0.0.1:<port>/library/parts/machine-a/movie.mp4?download=1`, then `[e][ffmpeg] tls: Peer certificate failed verification`, `Failed to open`, `finished playback, loading failed (reason 4)`, and `quit` 13 ms later. With idle=once the core shuts down, so the socket file stays behind and refuses connections. The mock serves HTTPS under a private CA. Vela's own reqwest/native-tls client trusts that CA through SSL_CERT_FILE, which the scenario's environment() sets. mpv's TLS (FFmpeg with GnuTLS) does not read that variable. The players differ only in their defaults. I queried both libraries through libmpv (scratch tlsdefault.py): the shipped `.libmpv/out/libmpv/libmpv.so.2` is mpv v0.42.0 with tls-verify default=yes, and the system /usr/lib/libmpv.so.2 (v0.41.0, the external player) has tls-verify default=no. Vela itself never sets tls-verify or tls-ca-file: a grep for tls in src-tauri/src finds nothing. So the external player passes only because it does not check the certificate at all. The mpv-ext-1.log of the external pass has no TLS line and plays the stream.
- What a user would see: Not a defect in the embedding itself, but a real behaviour difference between the players. Take a server whose HTTPS certificate Vela's own client accepts but the machine's GnuTLS trust store rejects, for example a private CA trusted only through SSL_CERT_FILE/SSL_CERT_DIR, or a distribution where OpenSSL's and GnuTLS's stores differ (by reading). Browsing works, and Play opens and immediately closes the embedded player view with no error message. The external player with system mpv 0.41 plays the same stream (it does not verify). Seen in a run: when a stream fails to open, neither player shows any message; the page silently stays where it was. A self-signed server is not affected, because Vela's own client already refuses it (no accept-invalid option, by reading).
- Fix: Fix it in the scenario, with no branch on the player. In seed(), call seedConfig with mpv_extra_args `--vo=null\n--ao=null\n--tls-verify=yes\n--tls-ca-file=${tls.ca}`. mpv then trusts the same private CA that Vela's own client trusts through SSL_CERT_FILE, and both players fetch the stream with verification on. That is stronger for the external player, not weaker. The diff is in (scratch) multiplex/multiplex-fix.diff. One wording correction for that comment: 'a system mpv before that does not' is only established for 0.41.0. A cleaner version would export seedConfig's default `--vo=null\n--ao=null` from helpers.mjs instead of repeating it. Separately, the owner should decide one product question (see user_visible): should the embedded player trust what Vela's own client trusts (for example pass SSL_CERT_FILE on as tls-ca-file), or is the stricter default intended? Should the external launch also set --tls-verify=yes (see other_findings)?

### `sourcepolicy`

- Kind: scenario assumes the external player (shown by run).
- Stands for: 6 (source choice / policy: Prefer Best, Prefer Compatible, Prefer Fastest, a persistent Play Version override, Ask Every Time with its choice dialog). The merged-hierarchy tail (Play Version on a merged episode) is not about playback.
- How it fails: The embedded run fails with WebDriver 'element not interactable' at setPolicy -> the click on the Settings button (sourcepolicy.mjs:79, called from :299), right after the first playDetail. It failed the same way in 2 of 2 embedded runs (emb-1.log, emb-2-probe.log). The ordinary venue passed (ext-1.log). The first play's source choice (Prefer Best -> the 4K HDR distant copy) had already passed in the embedded player.
- Cause: A race in the scenario. No play is left running that should have ended. playDetail and playVersion send mpv `quit` over IPC and return straight away. In the embedded player the view leaves only when that session's `player-closed` arrives, so the next click lands while the view still covers the page. Instrumentation (instrumentation.diff, emb-2-probe.log) found player-open=true, the view present and elementFromPoint = DIV.player at the moment of the click. When the scenario polled for the view instead of clicking (emb-3-sourcepolicy-wait.log), the view was gone within about 109 ms after each quit, three times out of three. The page was then on DIV.hero, and the Settings click and both later policy changes went through. With only that wait, the same race moved on to the next uncovered click: playDetail's Play button right after playVersion (sourcepolicy.mjs:126 from :330).
- What a user would see: None. A user of the embedded player does not have this problem: the view leaves about 0.1 s after the play stops, and the page is then usable.
- Fix: In the scenario: after each play, stop it the way its user would and wait until the page is the user's again before the next click. In my copy this is a new helper, `stopPlayAsUser(driver, mpv, socketPath, what)` in helpers.mjs. In the embedded player it clicks the view's Close button; in the external player it sends mpv `quit`, as before. It then waits until the socket refuses connections, and until neither `html.player-open` nor `.player` exists. playDetail and playVersion call it in place of `mpv.quit()`. The branch is the stop gesture itself, which legitimately differs: the embedded user's stop is the view's Close, the external user's is mpv's own window. The rest of the wait is the same for both players, and in the external player it only adds a check that the play ended. Nothing the scenario proves is weakened, and in the embedded player every policy, the override and both Ask choices are now proven end to end. Depends on Slice 5 in two ways. S3 replaces `player_close` with `player_command stop`, which keeps a Close control. The controls auto-hide, so the helper must move the pointer before clicking Close, as the design already says for embeddedplay. Esc or q would also work.

### `playlistplay`

- Kind: scenario assumes the external player (shown by run).
- Stands for: 5 (playlist advance: arbitrary start index, same-key replacement with the old Stopped parked, a live edit re-read by the dispatcher, mixed-source advance, unavailable-entry skip, silent resume, dispatcher-owned recents and Home repaint, read-only playlists.json). It also carries window-state inheritance, which the plan's inventory lists as external-only by nature.
- How it fails: The embedded run fails with 'element not interactable' at clickRowAction 'Beta Start' 'Play' for the SECOND time (playlistplay.mjs:278 via :63), in 2 of 2 embedded runs (emb-1.log, emb-2-probe.log). The ordinary venue passed (ext-1.log).
- Cause: The play that covers the page is the first Beta Start play, and it is SUPPOSED to be running. The scenario starts it, pauses it on purpose, and then clicks Play on the same row so that the new play replaces the running one while the old Stopped response is parked for 5 s. Probe (emb-2-probe.log): player-open=true, view present, elementFromPoint DIV.player; the first play's path was b0 and pause=true. Two more steps of the scenario also act on the page while a play runs: three 'Move Alpha Edited Next up' clicks, which prove the dispatcher re-reads the store, and goHome, which keeps Home on screen so the dispatcher's repaint is proven without a navigation refetch. In the embedded player both are covered by the view in the same way. Nothing failed to end, so this is not a defect.
- What a user would see: None that is a defect. The differences are by design: an embedded user replaces a play by Close then Play, and cannot edit a playlist or see Home while a film plays.
- Fix: In the scenario (s6-covered-scenarios.diff), branching on the player only where the behaviour legitimately differs. (1) Same-key replacement: an embedded user cannot press Play while the view covers the page. In the embedded player the scenario therefore closes the first play with the view's Close button and presses Play again, while the old Stopped is still parked (5 s delay). The replacement-session and stale-tracker assertions are unchanged. A probe found the replacement starting 50-230 ms after the click, with the old Stopped request sent but not yet answered, so the hazard is still live in the embedded player. (2) The live edit and keeping Home on screen are not reachable by an embedded user, because the view covers the page. They are kept anyway so that the BACKEND behaviour (dispatcher re-read, dispatcher repaint) is proven in the embedded player: a new helper, `clickUnderPlay`, first asserts the view really is up, then clicks with a DOM `element.click()`. In the external player it remains an ordinary WebDriver click. This stands in for no user action and must be recorded as such: what is unproven for the embedded player is a USER editing a playlist or watching Home during a play, which that player cannot offer. (3) Window-state assertions (fullscreen and window-maximized on mpv's window) are gated to the external player. The inventory says they are external-only, and in the embedded player they are properties of a library with no window (see other findings). (4) At the end of the playlist the scenario now waits until the page is uncovered before clicking Playlists. This adds an assertion: the view must not outlive the last play. (5) releaseToEof waits until time-pos is available before seeking (see tried).

### `serverplaylists`

- Kind: scenario assumes the external player (shown by run).
- Stands for: 5 (server-playlist advance: server order, read-only, the session-owned sequence advancing, the dispatcher repaint of Home, owner loss stopping the run without rerouting or falling into Continue Playing)
- How it fails: The embedded run fails with 'element not interactable' at goHome (helpers.mjs:152 via serverplaylists.mjs:190), in 2 of 2 embedded runs (emb-1.log, emb-2-probe.log). The ordinary venue passed (ext-1.log).
- Cause: The play that covers the page is the first server-playlist item, Server First. It is SUPPOSED to be running: the scenario paused it on purpose so that Home could be on screen before the end-of-file advance. Probe (emb-2-probe.log): player-open=true, view present, elementFromPoint DIV.player; the first play's path was server-one and pause=true. Nothing failed to end.
- What a user would see: None. By design, an embedded user cannot see Home during the run.
- Fix: In the scenario (s6-covered-scenarios.diff): go Home through `clickUnderPlay`. In the embedded player it asserts the view is up and clicks through the DOM; in the external player it is the ordinary click. The rest is unchanged. This keeps the proof of the dispatcher repaint (Server Second rendered, Server First suppressed, with no navigation refetch) in the embedded player. Recorded as unproven there: a user SEEING Home during the run, which the embedded player cannot offer. Added at the end: `waitForPageUncovered` after owner loss, so a run stopped by owner loss must leave the page usable. In the embedded player this would catch a view that outlives the run. In the external player it holds trivially.

### `surfaces`

- Kind: scenario assumes the external player (shown by run).
- Stands for: Not a playback row of the 14 directly. It guards where a failed Play is reported (per-surface status). In the embedded venue it touches row 13 (user mpv arguments) and decision D3 (an embedded play that cannot start falls back to the external player).
- How it fails: Embedded venue, 2 runs of the unchanged scenario (the 2026-11-13 full run, plus my instrumented run surfaces-emb-diag-1.log): both failed every time at surfaces.mjs:92 with 'timed out waiting for the Play failure, reported ON the detail'. Ordinary (Xvfb) venue: passes (ext-1.log).
- Cause: The scenario makes a play fail by putting a NUL ("\0") in mpv_extra_args. With the external player that makes Command::spawn fail. The embedded player starts no program. options.rs parse_user_line (lines 62-76) drops any Advanced line that does not start with '-'/'--', so the NUL line is quietly left out and the embedded play simply starts. Observed in the embedded venue (instrumented copy; DIAG in surfaces-emb-diag-1.log and full-emb-1.log for surfaces404): PlaybackInfo 200 twice, then GET /Videos/m1/stream 404 (this scenario's mock films have no mediaFile), then POST /Sessions/Playing and /Sessions/Playing/Stopped. The player view opened briefly (a MutationObserver saw 'player-open') and closed again. No error appeared on any surface (div.error, .detailerror, .scanerror, .mpverror, role=alert all empty), so the poll timed out. The play neither fell back nor failed: it started in the embedded player and ended when its stream would not load. A stream that will not load ends just as silently in the EXTERNAL player: surfaces404, the same scenario without the NUL, run in the Xvfb venue (ext-1.log), shows the same Start, stream 404, Stopped and no message. So this is not something only an embedded user would hit.
- Fix: In the scenario, branched by player only for how the failure is caused. The embedded player has no spawn to fail, and under D3 an embedded play fails only when the embedded start fails AND the external fallback fails too (start_player in playback.rs). In an --embedded run, environment() also sets VELA_EMBEDDED_LOAD_DELAY_MS=8000 and VELA_EMBEDDED_START_LIMIT_MS=500. These are the development-build hooks embeddedlatestart already uses; a released Vela ignores them. The embedded start then times out, the play falls back to mpv, and mpv cannot spawn because of the NUL. In that run one extra assertion checks that the detail line contains 'The external player could not start either', which proves the failure came through the embedded path and its fallback. Every other assertion is unchanged and applies in both players: the message is on the detail and not on the banner, PlaybackInfo was 200, no recent is created, the tombstone survives, the edit-line leg holds, and nothing displayed contains a URL query or a token. Diff: (scratch) surfaces/surfaces-scenario-fix.diff

### `watchstate`

- Kind: venue limitation (shown by run).
- Stands for: 2 (Jellyfin/Emby check-ins, final stop) and the playback-ended refresh of row 3. Everything those rows prove passes in the embedded player. Only the final screenshot fails.
- How it fails: 'GET /session/…/screenshot → no response within 30000ms', at the last step (screenshot('01-progress')), after every assertion has passed. It fails only late in a full --embedded run: it failed in the 2026-11-13 full run and in my full run full-emb-1.log, both for the instrumented copy and for an untouched copy (watchstate0). watchcurate failed the same way in that run. Run on its own in the embedded venue it passed 5 times out of 5 (3 instrumented, 2 untouched), and once after the 4 scenarios that precede it. It passes in the ordinary venue (ext-1.log).
- Cause: The cause is the test venue, not Vela and not a play. weston's default idle timeout is 300 s (weston 15.0.1, `--idle-time`). WebDriver's clicks happen inside WebKit, so weston never sees input. Once weston has gone idle, an app window opened after that point never renders a frame. Evidence: (a) in the failing full run, a requestAnimationFrame counter installed at the start of the watchstate session (before any play) was still 0 at the failure, while execute/sync kept answering in 1-2 ms. So the page is not hung; it is just never drawn. (b) aaidle, a scenario that holds one session open for 320 s, followed by the untouched watchstate0 reproduced the screenshot timeout (idle-default.log). The same pair with weston started with --idle-time=0 passed (idle-off.log). (c) aaidle followed by smoke, which never plays anything, hung on smoke's very first screenshot (idle-smoke.log). So it does not depend on a play being up or having been up. (d) A full --embedded run with --idle-time=0 passed watchstate, watchstate0 and watchcurate (full-emb-idle0.log). I have not established why, in a full run, the idle point lands where it does: plenty of screenshots after the first 5 minutes still worked, so something seems to reset weston's timer. What is shown is the mechanism and that turning the timeout off removes the failure.
- What a user would see: Nothing, as far as these runs show: the page stayed responsive and nothing hung during or after an embedded play. Not tested: how Vela's transparent embedded-mode window behaves when a real desktop compositor idles or locks the screen and then wakes.
- Fix: In the harness, not the scenario or the product: start the private weston with --idle-time=0 in tests/e2e/run.mjs. Proposed diff: (scratch) surfaces/run-mjs-weston-idle.diff. watchstate needs no change to run in both players.

### `uifoundation`

- Kind: venue limitation (shown by run).
- Stands for: None of rows 1-14. The scenario checks app-wide UI basics: theme switching and its persistence across a restart, OLED dimming, focus rings that follow the theme, and that shared primitives (progress track, no-art placeholder, primary and play buttons) compute the same styles everywhere. It plays nothing. It still belongs in the embedded run, for a reason the inventory does not cover: with the embedded player selected, Vela starts in a different window mode (transparent window, DMABUF/GPU renderer, per main.rs and lib.rs). This is the only scenario that checks computed theme and focus styles in that mode, so it is the automated guard for A2's 'no visible change to the rest of the app'. Slice 6 should list it as a window-mode check, not as a parity row and not as an exemption.
- How it fails: In the unmodified harness the --embedded venue fails 2 of 2 runs (emb1, emb2) at the first focusSnapshot (uifoundation.mjs:301, the dark-theme search input). The message is 'timed out after 15000ms waiting for input[aria-label="Search your libraries"] theme focus styles'. Everything before that passes: the OLED Black assertions and the switch to Vela Dark. The ordinary Xvfb venue passes 1 of 1 (ext1), and passes again with the proposed harness change (ext2-proposed).
- Cause: The headless weston that run.mjs starts (`--backend=headless --renderer=gl`) has no seat, so it never activates Vela's window and the page never gets focus. The window mode plays no part. Evidence:
(1) A diagnostic in my copy ran target.focus() before the wait. Under weston: activeElement === target, but document.hasFocus() is false, and :focus, :focus-visible and :focus-within do not match. The border stays rgb(35,40,51) rather than the accent. Under Xvfb all of these are true and the border is the accent rgb(229,160,13). The search input's ring is `.search:focus` (src/routes/+page.svelte:3947), so it never applies.
(2) I split the venue from the player with an instrumentation-only `--weston` flag in my copy: weston with the EXTERNAL player selected, so an opaque window and DMABUF off. The diagnostic was identical and the step failed the same way (westonext1). The cause is the venue, not the transparent/GPU mode.
(3) The driver log of every seatless weston run has three `Gdk-CRITICAL gdk_seat_get_keyboard: assertion 'GDK_IS_SEAT (seat)' failed`. With --fake-seat there are none.
(4) Adding weston's own `--fake-seat` option makes the failure go away. Runs with it: embedded 3 of 3 (emb3, emb4, emb8-proposed); weston with the external player 1 of 1 (westonext2). hasFocus, :focus and :focus-visible are all true. The focus colours are identical in transparent and opaque mode: dark rgb(229,160,13), One Light rgb(64,120,242), playlist input rgb(64,120,242). Without --fake-seat it fails every time (3 of 3 seatless weston runs), which is the red proof.
WebDriver Element Send Keys still types under seatless weston because WebKitWebDriver injects events in-process. That is why the 'trusted key events' measurement did not reveal the missing focus.
- What a user would see: Nothing. This is not a product defect. A real Wayland desktop always has a seat and activates Vela's window. With a seat, transparent-window mode computes the same focus ring colours and theme values as opaque mode, and the whole scenario passes in it: OLED, three themes, the restart-persisted theme, Settings icons, and the shared-primitive style comparisons. Did not establish: how the focus ring looks pixel by pixel in transparent mode on a real desktop. That is the owner's by-eye check.
- Fix: Fix it in the harness, not in the scenario or the product. In tests/e2e/run.mjs, start weston with `--fake-seat` (diff: (scratch) uifoundation/proposed-fake-seat.diff). The scenario needs no branch on the player: unchanged, it passes in both players and proves the same thing in each. Before landing, Slice 6 should run the whole --embedded suite with the seat, because a fake seat may also change pointer/hover state for other scenarios. I ran 8 embedded scenarios with it: uifoundation, embeddedplay, playback, search, playlistedit and imagepolish passed. markers and playlistplay failed, and neither failure is caused by the seat (see other findings). It may also be worth keeping a `--weston`-style switch (weston venue, external player) so venue failures can be told apart from player failures. Mine was instrumentation only and is in instrumentation.diff.

## The inventory, row by row

### Row 1: Play starts; resume at start_seconds

- External player, proven by: E2E `playback` (card -> detail -> Play; asserts mpv `path` is the mock stream, seek, quit, socket stops accepting, open recent then stamped recent, hero), `resume` (replay from the hero starts mpv at the stamped position, via helpers.playAndQuit's first time-pos), `playverbs` (Play vs Resume vs Play from Beginning drive distinct start positions). Incidental: `search`, `curation`, `watchcurate`. Rust: launch.rs golden lists carry `--start=` (`the_full_external_command_line_is_exactly_the_recorded_one` in player/external.rs).
- Embedded player, proven by: The same three scenarios run unchanged in the embedded run (no onlyPlayer) and PASSED on 2026-11-13: playback, resume, playverbs (also curation, search, watchcurate). `embeddedplay` additionally proves the play is in-process (`current-vo`=libmpv, /proc/<pid>/comm = vela) and survives a 2 s gap before loadfile. Rust: `the_full_embedded_launch_is_exactly_the_recorded_one` carries `--start=123.5`.
- Missing: none
- Proposal: Mark playback, resume, playverbs as standing for row 1 in both players (no change to the scenarios).

### Row 2: Plex timeline and Jellyfin/Emby check-ins, final stop

- External player, proven by: Jellyfin: `watchstate` (Start and Stopped check-ins, Stopped PositionTicks mid-clip, card refreshed without restart); `markers` and `embeddedplay`-style Stopped waits use it as a means. Plex: `multiplex` (/:/timeline and /:/progress reach Plex A with the source token header only).
- Embedded player, proven by: Jellyfin: `watchstate` reached and passed all its check-in assertions in the embedded run and then FAILED at its last line, `screenshot('01-progress')` (WebDriver screenshot no response within 30 s), so the scenario is red; `markers` (PASS) waits on Stopped check-ins for four plays; `embeddedplay` (PASS) waits on the Stopped report after Close and after EOF. Plex: `multiplex` FAILED in embedded with ECONNREFUSED on the play's socket, before any timeline assertion. By reading the Slice 5 design P5 (mpv leaves its socket file behind when it ends), the embedded Plex play had already ended when the scenario connected; why it ended (TLS mock trust via SSL_CERT_FILE in the shipped FFmpeg, the header include, or something else) is NOT established by me (another agent works on s6/multiplex).
- Missing: No passing embedded proof of Plex timeline/progress/final stop. Jellyfin is asserted in embedded but the scenario that holds it is red for an unrelated-looking screenshot hang whose cause is not established.
- Proposal: Run `multiplex` in both players once its embedded cause is found and fixed (it is the only hermetic Plex playback proof). Fix or bound the `watchstate` final screenshot (cause first); keep watchstate as row 2's Jellyfin proof in both players.

### Row 3: Final position on quit; recents stamp; playback-ended

- External player, proven by: `playverbs`, `curation`, `playback` (recent stamped with position after quit, hero shows Continue Watching), `watchstate` (playback-ended refetch), `watchcurate`. The 'quit' is an IPC `quit` (equivalent of the user's q / window close).
- Embedded player, proven by: playverbs, curation, playback, watchcurate PASS in embedded with the same IPC `quit`. `embeddedplay` proves the page's Close stops the play and a Stopped report follows, but asserts neither its position nor the recents stamp.
- Missing: The embedded user's real quit is the player view's Close (player_close -> Session::stop) or Escape, not an IPC quit; nothing asserts the final position, recents stamp or playback-ended after a Close. After Slice 5 the routes are Close, `q`, Escape and `stop` through `player_command`.
- Proposal: Give helpers.playAndQuit a per-player 'quit as a user does' step (external: IPC quit; embedded: click `.player button.close`, after Slice 5 also key `q`), so playback/playverbs/curation prove row 3 through the real route in each player. Alternatively extend `embeddedplay` leg 2 to assert the Stopped PositionTicks and the recent's ended_at_ms/position after Close.

### Row 4: Clean end -> watched, Continue Playing modes

- External player, proven by: `continueon`, `continueoff`, `continuetv`, `completionhub`; `mergedview` (clean EOF fans out once). Node: tests/clean-eof-refresh-order.test.mjs (source text of the dispatcher order).
- Embedded player, proven by: continueon, continueoff, completionhub, mergedview PASS in embedded. `continuetv` FAILED at line 334 'the frontend-selected next episode to inherit fullscreen state' - after its quit leg and the E1 clean-EOF/E2 launch had already succeeded; that assertion is window-state inheritance, which the plan lists as external-only.
- Missing: Row 4 itself is proven in embedded; continuetv is red only because it mixes an external-only assertion (fullscreen inheritance) into a row-4 scenario. Its remaining legs (season rollover, end of show, manual race) are therefore unproven in embedded today.
- Proposal: Make continuetv player-aware: the fullscreen set/inherit steps run only in the external player (or move them into an external-only window-inheritance scenario together with playlistplay's window legs); before Slice 5 record an exemption for the embedded counterpart; after Slice 5 assert the embedded counterpart (see continuetv field). Then continuetv stands for row 4 in both.

### Row 5: Playlist and server-playlist advance

- External player, proven by: `playlistplay` (arbitrary start, same-key replacement, live edit re-read, mixed-source advance, unavailable skip, silent resume, window-state inheritance, dispatcher recents, read-only store), `serverplaylists` (server order, read-only, session-owned advancement, owner loss stops), `continueon` playlist-boundary leg (Vela playlist A EOF -> B in the backend, then Continue Watching).
- Embedded player, proven by: `continueon` PASS in embedded, so Vela-playlist advance A->B->Continue Watching is proven. `playlistplay` FAILED at line 278 (second 'Play' row click: element not interactable) and `serverplaylists` FAILED at line 190 (`goHome` click), both BEFORE any advance. By reading: both click the library page while a play runs; in embedded the view is `position:fixed; inset:0; z-index:1000` and `.app` is `visibility:hidden` (src/lib/PlayerView.svelte, embeddedplay asserts app 'hidden'), so the page cannot be clicked.
- Missing: Server-playlist advancement has no embedded proof; playlistplay's legs (mixed-source advance, unavailable skip, silent resume, dispatcher repaint) have none either. Some legs are not reachable by an embedded user at all: editing a playlist and navigating Home while a play is on screen.
- Proposal: Restructure both scenarios so page interaction happens between plays (or before the first), keeping mid-play edits/navigation as external-only legs with a recorded reason ('the embedded view covers the page; a user cannot edit mid-play'). Window-state legs move to the external-only window-inheritance scenario. Then both stand for row 5 in both players.

### Row 6: Source choice / policy

- External player, proven by: `sourcepolicy` (Prefer Best/Compatible/Fastest, manual override persistence, Ask Every Time dialog before launch, hierarchy backings), `mergedview` (Play Version per backing, each plays its own server's stream). `multiplex` (explicit Plex backing override).
- Embedded player, proven by: `mergedview` PASS in embedded. `sourcepolicy` FAILED at line 299 (`setPolicy` -> click Settings: element not interactable) right after its first `playDetail`, which sends IPC `quit` fire-and-forget and returns without waiting; by reading, the embedded view was still mounted when Settings was clicked. `multiplex` failed earlier (row 2).
- Missing: Policy modes and Ask Every Time have no embedded proof.
- Proposal: In sourcepolicy's playDetail/playVersion, wait for the play to end (socket stops accepting, as helpers.playAndQuit does) and, for embedded, for `.player` to be gone, before the next UI step. Harmless for external. Then sourcepolicy stands for row 6 in both.

### Row 7: Stream auth header never in argv/URL/title

- External player, proven by: Rust (player/launch.rs): header_include_writes_quoted_fields_owner_only, header_include_guard_removes_only_its_exact_file, partial_header_include_write_removes_the_credential_file, header_include_fails_closed_on_conf_escaping_input, an_include_whose_removal_fails_stays_with_its_owner_until_it_is_removed; player/external.rs stream_auth_reaches_the_player_only_through_the_private_include, consumed_include_is_removed_before_a_replacement_launch, the_full_external_command_line_is_exactly_the_recorded_one (--title/--force-media-title, --include last). E2E hermetic: `multiplex` (mock Plex answers 401 and records a contract violation for any request, including /library/parts streams, without the right X-Plex-Token header, and flags any token query parameter; violations asserted empty at the end). Live: `live-transcode` (reads mpv's /proc/<pid>/cmdline and the include file mode).
- Embedded player, proven by: write_header_include is the same function for both players (build_launch_for), so its unit tests hold for both. Embedded-specific: the_full_embedded_launch_is_exactly_the_recorded_one (include after the protected tail, force-media-title, no --title), options.rs an_include_is_never_written_into_the_options_text, session.rs a_play_keeps_a_stream_auth_file_it_could_not_remove_and_removes_it_when_reaped and a_play_removes_a_consumed_stream_auth_file_at_once_when_it_can, tests/embedded-player.test.mjs 'both players give up a consumed stream-auth file the same way' and 'everything a launch wrote to disk has an owner'. No E2E: multiplex FAILED in embedded before the stream (row 2). live-transcode cannot run embedded as written: it finds argv by scanning /proc/*/cmdline for an mpv process (by reading).
- Missing: No end-to-end proof that an embedded play's stream request carries the header and nothing else does. 'argv' has no meaning in-process; the embedded analogue (option text, media-title, path) is held only by unit tests.
- Proposal: multiplex in both players (after its embedded cause is fixed) proves the header on the wire. Add to it, in both players, IPC assertions that `path` and `media-title` carry no token. Mark live-transcode external-only with an exemption naming the embedded unit tests above as the in-process counterpart (argv does not exist for an in-process player).

### Row 8: Skip markers (button / autoskip)

- External player, proven by: `markers` (ranges requested with all three filters, payload written, script injected, intro and commercial auto-skip seek past the range, endpoint failure costs only markers, all-Off injects nothing and asks nothing). `markerpayload` (real system mpv, no app: the script's payload contract, option over environment). Button, hitbox, click and Space binding: NOT automated (vo=null has no osd-dimensions); manual evidence in .agents/plans/skip-credits-intros-v2.md Slice 4. Rust: launch.rs marker_args_* and marker_payload_* tests; commands.rs only_enabled_marker_kinds_reach_the_player.
- Embedded player, proven by: `markers` PASS in embedded (auto-skip through script-opts payload). `markerpayload` PASS (it runs the system mpv directly in either run, so it is player-independent). Rust: the_full_embedded_launch_is_exactly_the_recorded_one (script-opts-append payload, no environment).
- Missing: Auto-skip: none. Button: no embedded button exists until Slice 5 (HTML skip button, `skip-active-marker`); the external button has only manual evidence.
- Proposal: Split row 8 in the data into 8a auto-skip (markers, both players) and 8b button: embedded `embeddedskip` (Slice 5 design) as the embedded proof; external recorded as a manual exemption pointing to skip-credits-intros-v2 Slice 4 evidence (venue cannot draw the OSD button).

### Row 9: Autocrop off / manual / auto

- External player, proven by: `autocrop` (stock autocrop.lua with trigger disabled + vela-autocrop shim; video-crop set on a fresh and a resumed play). Rust: launch.rs autocrop_* tests.
- Embedded player, proven by: None. `autocrop` FAILED in embedded (shim load marker never appeared); by reading launch.rs, the embedded launch deliberately injects no crop script (`autocrop_mode != "off" && !embedded`). Known and owned by Slice 5.
- Missing: Everything, until Slice 5's vela-crop.lua and `embeddedcrop`.
- Proposal: Per the Slice 5 design: mark `autocrop` onlyPlayer 'external' with counterpart `embeddedcrop` (onlyPlayer 'embedded'); the parity data pairs them for row 9.

### Row 10: Automatic quality step-down relaunch

- External player, proven by: Rust: automatic.rs (a_sustained_drop_storm_steps_down, a_starving_cache_steps_down, healthy_playback_never_steps_down, a_paused_player_is_not_starving, a_replacement_stream_gets_the_longer_cooldown, stepping_stops_at_the_cap, the_step_count_survives_a_relaunch); commands.rs a_step_down_replacement_is_still_managed_by_automatic, a_newer_verdict_replaces_an_unhandled_one; select.rs an_automatic_relaunch_neither_explains_nor_forgets; Node player-selection.test.mjs 'a relaunch stays in the player it replaces' (source-text regex on apply_step_down). The plan's 'live-transcode (live venue)' is NOT a proof: live/transcode.mjs lines 18-19 say it deliberately does not test Automatic ('remains a manual playtest').
- Embedded player, proven by: By reading: the embedded equivalent is the same code. playback::play spawns spawn_health_sampler on the IPC socket for whichever player started (playback.rs ~563), and apply_step_down relaunches with `player: replaced_player` from active_playback_player, so the relaunch is an embedded Session stop + create (A8 probe: leak-free, sub-second, Slice 0 throwaway code). Nothing exercises an embedded step-down: no unit test builds a step-down for an embedded session, and the 'stays in the player' check is a source regex. The page side (successor's player-opened arriving before the predecessor's player-closed keeps the view) is pinned by tests/player-view.test.mjs 'a replaced play's late close does not dismiss the view of the play that replaced it'.
- Missing: No proof in either player that a verdict reaches a relaunch end to end; for embedded, nothing proves the relaunch lands in the embedded player, keeps the view, and resumes at the position.
- Proposal: Two options for the owner (see open questions): (a) a development-build-only hook (like VELA_EMBEDDED_LOAD_DELAY_MS) that makes the health sampler emit one verdict, plus mock Jellyfin tiers, giving a `stepdown` scenario run in both players (asserts a new socket, same player kind via current-vo/pid, resume near the position, view stays mounted in embedded); or (b) a Rust test that drives apply_step_down's player choice through a pure function for both PlayerKinds, plus a named manual acceptance step on continuum in each player. Either way, fix the plan's row 10 text (live-transcode does not prove it).

### Row 11: Unrequested-play notice

- External player, proven by: Rust playback.rs: a_notice_that_reaches_the_player_is_reported_shown_once, a_notice_dropped_because_the_play_was_replaced_is_not_reported_shown, a_notice_the_player_did_not_take_is_not_reported_shown, a_notice_for_a_player_that_cannot_be_reached_is_not_reported_shown; commands.rs the_step_down_notice_stays_short; select.rs fallback-notice tests. E2E `playerfallback` (external-only) observes the fallback notice as a `show-text` in mpv's trace log (`--msg-level=cplayer=trace --log-file`), once per run. The only producer of PlaySpec.osd_notice is a step-down (playback.rs:262-267).
- Embedded player, proven by: By reading: spawn_osd_notice sends `show-text` over the IPC socket of whichever player started, and in-process mpv draws its OSD (A4; Slice 5 design S7 keeps show-text on mpv's OSD). No test sends a notice to an embedded play: the notice tests use a test socket, and the fallback notice by definition lands in the external player (embeddedlatestart).
- Missing: Nothing proves an unrequested-play notice is shown in an embedded play.
- Proposal: Bundle with row 10: the step-down scenario reads the notice the playerfallback way (user args `--msg-level=cplayer=trace --log-file=...`, which the embedded options path should also apply - not verified) in both players. Without a step-down hook: a named manual step, plus the existing notice unit tests recorded as player-agnostic.

### Row 12: A new play stops the previous one; app exit stops playback

- External player, proven by: Rust player/mod.rs: the_reaper_drops_a_player_only_on_a_confirmed_end (`retain_after_poll`; the plan's name `retain_child_after_try_wait` no longer exists, renamed in Slice 1), an_external_player_reports_its_end_only_after_the_process_exits, a_close_never_stops_an_external_player; external.rs consumed_include_is_removed_before_a_replacement_launch. Exit sweep: lib.rs RunEvent::Exit/ExitRequested handler (kill current_child and reap_queue under the play lock, remove consumed includes) has NO unit test; commands.rs exit_sweep_stops_a_transcode_whose_end_callback_never_ran covers only the transcode part. No E2E asserts the previous mpv died on a new play or that exit kills mpv (playlistplay replaces a same-key play but only checks the tracker stays a no-op).
- Embedded player, proven by: Rust: mod.rs a_close_stops_the_embedded_play_it_names_and_no_other; session.rs a_play_that_stops_when_asked_is_not_kept, a_play_that_will_not_stop_is_kept_not_let_go_of, a_play_has_ended_only_once_its_thread_has_finished_and_asking_does_not_wait, asking_whether_a_play_has_finished_never_waits_for_it; Node embedded-player.test.mjs 'a running embedded play is asked to stop, and waited for, before its window can go'; lib.rs calls embedded::wait_for_unfinished_plays(2 s) on exit (no test). E2E `embeddedplay` (PASS): Close stops the play (socket stops accepting) and the view goes. Replacement uses the same play_by_key_locked kill path for both players (by reading).
- Missing: Neither player has an end-to-end proof of replacement or exit. For embedded, a user cannot start another play while the view covers the page; replacement happens by auto-advance (predecessor already ended), step-down, or a backend-started play.
- Proposal: New scenario `replaceplay` in both players: start A, start B (external: page click; embedded: driver.exec(el.click()) on the hidden control, recorded as a test-only route, or a continuation race like continuetv's), assert A's socket stops accepting, A's Stopped report carries its position, B plays. New scenario `exitplay` in both: start a play, close the app (WebDriver window close / restart()), assert the Stopped check-in arrives and no mpv process (external: pgrep by socket) or socket listener remains. If app exit cannot be driven, a named manual step for both.

### Row 13: User mpv arguments and own-config mode

- External player, proven by: Rust launch.rs: base_mpv_command_preserves_render_defaults_without_managed_sync_overrides, macos_base_mpv_command_prefers_avfoundation_before_advanced_overrides, own_mpv_config_owns_audio_output_selection, non_macos_base_command_does_not_set_an_audio_output; external.rs golden full/minimal command lines; Node mpv-lifecycle.test.mjs 'Advanced arguments are still applied inside base_args' and the ordering tests. E2E: only indirect - every scenario seeds `--vo=null --ao=null` (not asserted), and playlistplay asserts configured `--fullscreen=yes --window-maximized=yes` take effect on a manual play. Own-config mode: no E2E.
- Embedded player, proven by: Rust: options.rs (ten tests incl. a_user_line_is_read_as_the_command_line_would_read_it, the_users_own_configuration_is_found_where_mpv_looks_for_it, the_user_cannot_make_mpv_load_configuration_files_by_itself, a_profile_is_never_in_one_file_with_the_options_after_it); launch.rs the_full/minimal_embedded_launch_is_exactly_the_recorded_one. E2E PASS: `embeddedownconfig` (own mpv.conf applied: options/volume=37; a redefined gpu-hq profile cannot take idle), `embeddedplay` (Advanced profile+include cannot take idle; user vo=null does not win). Node mpv-lifecycle.test.mjs 'the embedded player's required options follow everything the user controls'.
- Missing: Embedded is better proven than external here: external has no E2E that a user argument or own-config mode takes effect.
- Proposal: Make embeddedownconfig player-parameterised (current-vo assertion only in embedded; volume=37 from MPV_HOME mpv.conf and the end-at-EOF assertion in both) and add an Advanced-arguments leg (`--volume=37` -> options/volume) run in both, so row 13 has one scenario standing for both players.

### Row 14: Ends at end of file rather than idling

- External player, proven by: Node tests/mpv-lifecycle.test.mjs (lifecycle_args applied once, after base_args/window_state_args/screen_name_arg, before input-ipc-server); Rust launch.rs lifecycle_args_make_mpv_exit_when_its_file_ends (--idle=no). Implicitly every clean-EOF scenario (continueon, continueoff, completionhub, mergedview), since on_end fires only when the IPC connection drops.
- Embedded player, proven by: Rust launch.rs the_embedded_player_waits_for_its_first_file_and_exits_at_its_end (--idle=once); E2E PASS: embeddedplay leg 3 and embeddedownconfig (socket stops accepting at EOF, view goes) with user attempts to set idle=yes; clean-EOF scenarios continueon/continueoff/completionhub/mergedview PASS in embedded.
- Missing: none
- Proposal: Declare continueon (and completionhub) as standing for row 14 in both players; embeddedplay/embeddedownconfig remain embedded-only extras (the idle=once gap before loadfile is embedded-only by nature).

## Every scenario and what it stands for

Source: tests/e2e/scenarios/*.mjs at 92c854a (44 scenarios; `onlyPlayer` read from each default export; run.mjs lines 291-305 skip a scenario whose onlyPlayer differs from the run's player, and fail it if named). Result = the 2026-11-13 embedded run (e2e-embedded-full-1.log: 33/43 passed, 1 skipped). 'Both' = no onlyPlayer, so it runs in both players today.

| scenario | inventory rows | players today | embedded 2026-11-13 |
|---|---|---|---|
| autocrop | 9 | both | FAIL (shim load marker never seen; embedded launch injects no crop script by design; Slice 5) |
| combinedrecovery | not a playback behaviour | both | PASS |
| completionhub | 4 (clean EOF -> watched, Off stops), 14 implicit | both | PASS |
| configrecovery | not a playback behaviour | both | PASS |
| configrollback | not a playback behaviour | both | PASS |
| connectionrecovery | not a playback behaviour | both | PASS |
| connectionrollback | not a playback behaviour | both | PASS |
| continueoff | 4, 14 implicit | both | PASS |
| continueon | 4, 5 (Vela playlist A->B then Continue Watching), 14 implicit | both | PASS |
| continuetv | 4 (+ window-state inheritance, external-only by the plan) | both | FAIL line 334, fullscreen inheritance |
| curation | 3 (recents/hero via playAndQuit) | both | PASS |
| embeddedlatestart | none of the 14 (D3 fallback of a late embedded start; embedded-only by nature) | embedded only | PASS |
| embeddedownconfig | 13 (own config), 14 | embedded only | PASS |
| embeddedplay | 1 (in-process play), 12 (Close stops), 13 (Advanced profile), 14; player view (embedded-only) | embedded only | PASS |
| imagepolish | not a playback behaviour | both | PASS |
| markerpayload | 8 (script contract; runs the system mpv directly, not the app, so identical in both runs) | both | PASS |
| markers | 8 (auto-skip), 2 as a means (Stopped) | both | PASS |
| markwatched | not a playback behaviour | both | PASS |
| mergedrefresh | not a playback behaviour | both | PASS |
| mergedview | 6 (Play Version backings), 4 (EOF fans out once) | both | PASS |
| multiplex | 2 (Plex timeline/progress), 6 (backing override), 7 (stream header only, hermetic) | both | FAIL ECONNREFUSED on the play socket (play already ended; cause not established) |
| pagefail | not a playback behaviour | both | FAIL (known product race) |
| playback | 1, 3 | both | PASS |
| playerfallback | none of the 14 (D3 fallback + notice delivery; related to 11) | external only | SKIP |
| playlistedit | not a playback behaviour | both | PASS |
| playlistplay | 5 (+ 12 same-key replacement, + window-state inheritance external-only) | both | FAIL line 278, second row Play click 'element not interactable' (player view covers page) |
| playverbs | 1, 3 | both | PASS |
| plexdecision | not a player behaviour (capability diagnostic, no mpv) | both | PASS |
| recoveryresume | not a playback behaviour | both | PASS |
| refresh | not a playback behaviour | both | PASS |
| resume | 1 | both | PASS |
| scanlib | not a playback behaviour | both | PASS |
| search | 1 incidentally (plays the matching stream) | both | PASS |
| serverplaylists | 5 (server-playlist advance) | both | FAIL line 190, goHome click during play 'element not interactable' |
| smoke | not a playback behaviour | both | PASS |
| sortpersist | not a playback behaviour | both | PASS |
| sourcedeadend | not a playback behaviour | both | PASS |
| sourcepolicy | 6 | both | FAIL line 299, Settings click before the fire-and-forget quit's view closed |
| surfaces | not a playback behaviour (per-surface errors; uses a spawn failure as a means) | both | FAIL: no Play failure. By reading, the NUL in mpv_extra_args fails only Command::spawn; embedded options.rs:99 turns NUL into a space, so the embedded play succeeds |
| uifoundation | not a playback behaviour | both | FAIL (focus styles timeout; cause not established by me) |
| uimotion | not a playback behaviour | both | PASS |
| watchcurate | 3 as a means | both | PASS |
| watchposition | not a playback behaviour | both | PASS |
| watchstate | 2 (Jellyfin Start/Stopped with position), 3 (playback-ended refresh) | both | FAIL at the final screenshot (line 119) after every playback assertion passed; cause not established |

Live (tests/e2e/live, opt-in, not part of either run): live-jellyfin, live-plex (real servers; not player-specific in code), live-transcode (rows 7 for transcoded Plex; finds mpv argv via /proc/*/cmdline, so external-only by construction; explicitly does not test step-down).

## A design for the parity guard

Principle: the scenarios declare what they stand for as data on their default export; the inventory and the exemptions are a JavaScript module; the test imports both (the way run.mjs already imports every scenario, run.mjs:204-207) and never reads source text. Anything it does not understand it refuses.

1. Shared loader. Move run.mjs's loop into `tests/e2e/scenarios.mjs`: `export async function loadScenarios(dir)` (readdirSync *.mjs sorted, dynamic import, return default exports) and `export function validateScenario(s)` (name is a non-empty string; onlyPlayer is undefined, 'external' or 'embedded'; `rows` present). run.mjs uses both, so the runner and the guard cannot disagree. Today an onlyPlayer typo such as 'embeded' is SKIPPED in both runs (run.mjs:291 compares only for inequality) - by reading; validateScenario makes it a hard failure in the runner too. Importing scenarios has no side effects today (module scope holds only `let mock` and functions; by reading every scenario file's imports).

2. Data file `tests/e2e/parity.mjs`:
```js
export const PLAYERS = ['external', 'embedded'];
export const INVENTORY = [
  { row: 1, behaviour: 'Play starts; resume at start_seconds' },
  { row: 7, behaviour: 'Stream auth header never in argv/URL/title' },
  { row: 9, behaviour: 'Autocrop off / manual / auto' },
  // ... rows 1-14, row 8 possibly split into '8a' auto-skip and '8b' button
];
export const EXEMPTIONS = [
  { row: 9, player: 'embedded', kind: 'pending', until: 'Slice 5: embeddedcrop',
    reason: 'the stock crop filter is GPL-only and absent from the shipped FFmpeg' },
  { row: 10, player: 'external', kind: 'manual', step: 'step-down playtest on continuum (external player)',
    reason: 'no venue can force degraded playback (tests/e2e/live/transcode.mjs header)' },
  { row: 10, player: 'embedded', kind: 'manual', step: 'step-down playtest on continuum (embedded player)', reason: 'as above' },
];
```
3. Per scenario, on the existing export:
```js
export default { name: 'resume', rows: [1], async seed() {...}, async run() {...} };
export default { name: 'smoke', rows: [], ... };                        // explicitly not playback
export default { name: 'autocrop', onlyPlayer: 'external', rows: [9], counterpart: 'embeddedcrop', ... };
export default { name: 'embeddedplay', onlyPlayer: 'embedded', rows: [1, 12, 13, 14], embeddedOnly: 'player view, in-process proof', ... };
```
`rows: []` is how a scenario says 'not a playback behaviour'; a missing `rows` is refused. A one-player scenario must carry exactly one of `counterpart` (a scenario name) or `embeddedOnly`/`externalOnly` (a reason string, for behaviours outside the inventory by nature: window-state inheritance, --screen-name, D3 fallback, player view).

4. Test `tests/e2e-parity.test.mjs` (node:test; added to package.json `check`, which lists test files by name):
- every scenario passes validateScenario; names unique; no unknown keys among {name, onlyPlayer, rows, counterpart, externalOnly, embeddedOnly, seed, environment, cleanup, run} (refuse what it does not know);
- every entry of `rows` is a row id in INVENTORY; INVENTORY ids unique;
- a `counterpart` names an existing scenario whose onlyPlayer is the other player and whose `rows` equal this one's, and the relation is symmetric;
- coverage(row, player) = scenarios with row in rows and onlyPlayer undefined or equal to player; for every row and both players, coverage is non-empty XOR an EXEMPTIONS entry exists for (row, player) - so an exemption for a covered pair is refused as stale;
- every exemption names a known row and player, a kind in {manual, pending, unit-test}, and a non-empty reason; `pending` must name `until`.
Unit-test proofs (rows 7, 10-14) would be `kind: 'unit-test'` exemptions naming test and file; the guard can check the file exists but should NOT grep Rust for the name (that is the ps-2 pattern). If names must be checked, the honest structural route is a JSON list read by a Rust test via include_str! that asserts each listed fn is referenced by a `#[test]`... which Rust cannot introspect either, so record those names as data only and say so.

Mutations that must each make it fail (each to be red-proved): (a) delete `rows` from any scenario; (b) a row id 15 or '9b' not in INVENTORY in a scenario; (c) set `onlyPlayer: 'external'` on `markers` (row 8 embedded loses its only coverage) with no exemption; (d) set `onlyPlayer: 'embeded'`; (e) delete the row-9 embedded exemption before embeddedcrop exists; (f) keep that exemption after embeddedcrop lands (stale); (g) rename embeddedcrop so autocrop's counterpart dangles, or give both the same onlyPlayer; (h) change embeddedcrop's rows to [8]; (i) add a 15th INVENTORY row with nothing covering it; (j) duplicate a scenario name; (k) add an unknown key such as `rowz`. The static guard cannot see that a both-player scenario is red in one player (playlistplay today); that is the run's job, and Slice 6's exit criterion should be both full runs green with the guard green.

## `continuetv` and window state

What it asserts (continuetv.mjs 310-339): during E1 the scenario sets mpv's `fullscreen` property to true over IPC and reads it back, lets E1 reach a clean EOF, and then requires E2's freshly started mpv to report `fullscreen === true`. For mpv's own window that means: the backend observed the predecessor's window go fullscreen and launched the automatic successor with `--fullscreen=yes`, so the next episode opens fullscreen where the last one was. Mechanism (by reading): every launch gets a fresh `WindowStateObservation`; the position reader (playback.rs:701, started by the trackers for whichever player runs) sends `observe_property` for fullscreen, window-maximized, display-names, display-width, display-height and records owned boolean property-change events (`apply_ipc_event`). After a successful launch commands.rs:5269-5287 publishes `PlaybackWindowSession { session_id, observation }`. An automatic successor passes `replace_session`; `inherited_window_state` (commands.rs:4156) returns the snapshot only when the published session id equals it (manual plays get the default), and `inherited_screen_name` does the same for `--screen-name`. `build_launch_for` turns it into `window_state_args` (launch.rs:472, `--window-maximized=`, `--fullscreen=`) after user options so observed state beats `--fullscreen=yes` in mpv_extra_args.

Why it fails embedded: build_launch_for adds window_state_args and screen_name_arg only `if !embedded` (launch.rs:832-840), deliberately, and that omission is guarded by `the_full_embedded_launch_is_exactly_the_recorded_one` (a spec with fullscreen Some(true) yields no --fullscreen). E2's embedded mpv therefore keeps fullscreen=false. Setting `fullscreen` on a vo=libmpv handle only changes an option; nothing on screen changes.

What an embedded play publishes today (by reading, not run): the same observation and the same PlaybackWindowSession as an external play, populated from libmpv's `fullscreen`/`window-maximized` option values (mpv sends the current value when a property is first observed, so normally Some(false)/Some(false); Some(true) if something sets the option, as this scenario does) and whatever display-* the libmpv handle reports (not established). For an embedded successor it is inert. One cross-player leak follows by reading: a successor that was given the embedded player but falls back to the external one (D3, `if_embedded_cannot_start`, playback.rs:513-533) is re-described with `build_launch_for(spec, External, ...)` using the same `inherited_window_state`, so an embedded predecessor's Some(false) would become `--window-maximized=no --fullscreen=no` and override a user's configured `--fullscreen=yes`. Not run; worth one probe.

Embedded counterpart. Before Slice 5: none exists - the embedded view has no fullscreen (PlayerView.svelte has only Close), so the continuetv fullscreen legs need an exemption ('embedded player has no fullscreen until Slice 5') and the scenario must skip them in the embedded run. After Slice 5 (design S5): fullscreen is Vela's window, entered via `player_fullscreen`/`f`, real state from GDK `window-state-event` re-emitted as `window-fullscreen {on}`; the page holds 'fullscreen because of the player' outside the keyed view and leaves it only on f/Escape/button, the user's stop, the page's end-of-run with no successor, or a playback-ended with no view and no continuation pending. So the counterpart of 'the next episode inherits fullscreen state' is: the window stays fullscreen across the E1 -> E2 boundary and E2's view mounts in it. Assert in the embedded branch: enter fullscreen through the page during E1 (key `f` via WebDriver Actions, P1), then after E2's socket appears: `player-open` is set, window.innerHeight/innerWidth equal the output (weston 1280x800), and E2's `osd-dimensions` equal the page size x devicePixelRatio (the method the design names: page height and osd-dimensions, not Tauri's is_fullscreen). The negative counterpart: after E3 ends the show (no successor) the window has left fullscreen. Depends on Slice 5 landing as designed; the 'library page shows fullscreen for the gap' is recorded there for the owner's eye.

## Open questions the helpers left

- multiplex embedded: the play's socket refused connections (by Slice 5 design P5, mpv leaves its socket file behind when it ends), so the embedded Plex play ended almost at once. The cause (the shipped FFmpeg's TLS not trusting SSL_CERT_FILE for the HTTPS mock, the header include, or something else) is not established by me; s6/multiplex appears to be on it. Rows 2 (Plex) and 7 (E2E) depend on it.
- watchstate embedded: every playback assertion passed, then the final WebDriver screenshot got no response for 30 s. Cause not established; other scenarios take screenshots after plays and pass (markers, continueon). It may be a weston frame-callback stall after the subsurface goes away.
- Row 10 and row 11: should Slice 6 add a development-build-only hook that makes the health sampler emit one step-down verdict (in the style of VELA_EMBEDDED_LOAD_DELAY_MS), with mock tiers, so a `stepdown` scenario can run in both players and also carry the row-11 notice? Without it, both rows rest on unit tests plus manual playtests in each player. The plan's claim that live-transcode proves row 10 is wrong (live/transcode.mjs:18-19).
- Legs that an embedded user cannot reach (editing a playlist or navigating Home while a play is on screen, playlistplay/serverplaylists; window maximize/fullscreen inheritance and --screen-name): should Slice 6 record them as external-only legs inside parameterised scenarios, or split them into external-only scenarios with exemptions? I recommend splitting the window-state legs out (continuetv, playlistplay) into one external-only `windowinherit` scenario, and making the mid-play page legs external-only branches.
- Row 12: should replacement in embedded be driven through a test-only route (driver.exec(el.click()) on a control hidden behind the view), given that a user cannot reach one? Is app exit drivable in the harness (WebDriver window close or restart()) without leaving an orphaned process in the external venue?
- Possible cross-player leak (by reading, not run): an embedded predecessor publishes its libmpv `fullscreen`/`window-maximized` option values (normally false) as window state; a successor that was given the embedded player but falls back to the external one is described with that inherited state and would get --fullscreen=no --window-maximized=no over the user's configured values. Worth one probe or a unit test before Slice 6 records window inheritance as external-only.
- run.mjs honours `onlyPlayer` only by inequality: a misspelled value (e.g. 'embeded') makes the scenario SKIP in both runs, and so silently never run. The guard design validates it in a loader that both the runner and the test share.
- The plan's inventory names `retain_child_after_try_wait`; since Slice 1 the function is `player::retain_after_poll` with test `the_reaper_drops_a_player_only_on_a_confirmed_end` (src-tauri/src/player/mod.rs). The exit sweep (lib.rs RunEvent::Exit handler) has no unit test of its own apart from the transcode part. Row 12's text should be updated.
- surfaces is not about a playback row, but it fails embedded because its way of failing a play (a NUL in mpv_extra_args) only breaks Command::spawn; embedded options.rs:99 turns NUL into a space (by reading). It needs a failure trick that works in both players, or an external-only mark with a reason; s6/surfaces appears to be on it.
- Row 8 button: the external OSD skip button has only manual evidence (vo=null venue), while Slice 5 gives the embedded HTML button an E2E (`embeddedskip`). Does the parity rule accept an asymmetric pair (manual external, automated embedded), recorded as an exemption?
