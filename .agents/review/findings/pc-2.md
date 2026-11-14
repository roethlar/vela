# pc-2: a play that ends in an error leaves Vela's window in fullscreen for 50 seconds

**Severity**: MEDIUM — after an embedded play in fullscreen ends in an error, the player view goes and the library stays fullscreen until a 50-second backstop, with no successor coming.
**Status**: In progress
**Branch**: —
**Commit**: `4118fea`, `c872dad`, `060a5fd`

## Evidence

Reviewer, verbatim: "src/routes/+page.svelte:112 requires two playback-ended events before leaving fullscreen. An error-ending play produces only one: playback.rs:681 excludes errors from the clean-completion dispatcher that emits the second event. Executing the page helpers for fullscreen → player-closed → playback-ended produced no fullscreen-off request, only the 50,000 ms timer from +page.svelte:94."

## Predicted observable failure

Reviewer, verbatim: "When an embedded play ends with a playback error, its view disappears but the library remains in player-owned fullscreen for 50 seconds despite there being no successor."

Reviewer's suggested direction, verbatim: "Report an explicit session-scoped terminal outcome, including errors and whether a successor is pending, and leave player-owned fullscreen on that outcome. Reserve the backstop for missing events."

## What

While a play runs in the embedded player the page can put Vela's window in fullscreen, and it keeps the window there from one play of a run to the next. It leaves fullscreen when the run is over. It learnt that a run was over from the SECOND `playback-ended` of a play: the first comes from the play's tracker, before the backend has decided on a successor; the second comes from the completion dispatcher, which runs only after a clean end of file. A play that ends any other way (an error, a quit over mpv's socket that the page did not ask for, mpv dying) is reported once. The page then had nothing to go on, and left fullscreen only when its 50-second backstop fired: the view gone, the library fullscreen, nothing coming.

Reproduced in the app before anything was changed, in the embedded venue: a playlist's second play whose stream the server refuses, and a play quit over mpv's socket, each left the window fullscreen for the ten seconds the test watched.

The attack on the first fix found the same hole one step further on: a playlist that ends cleanly and whose NEXT entry cannot be started is also a run with nothing coming, and it too waited out the backstop. That is fixed here as well.

## Approach

The backend is the side that knows how a play ended and whether anything replaced it, so it says so, per session, as a flag:

- **The tracker's `playback-ended`** now also carries `sessionId` and `runOver`. `runOver` is true when the end was not a clean end of file AND no other play has become the active one (`end_is_run_over`). A replaced play is never the end of its run: a step-down, and any play the backend or the user starts, installs the new session before the old player is stopped.
- **The dispatcher's `playback-ended`** (after a clean end) carries the same two fields; `runOver` is true there when the run had a successor to start and could not start it. `advance_playlist` now answers with one of three outcomes (`AfterCleanEnd`: Continue Playing may apply; a successor started, is being chosen, or the play was replaced; the run is over) where it answered yes or no. What it does is unchanged; only the "no" is now told apart.
- **Whether the end was clean has to be known when the tracker reports.** mpv's "end of file" is read by a separate watcher thread, which could be a moment behind the tracker. For an embedded play the tracker's report therefore waits until that watcher has stopped (it stops as mpv closes its socket; a watcher that never connected gives up once the play has ended; one second at most). A play in mpv's own window reports exactly when it did before.
- **The page** (`endLeavesPlayerFullscreen` in `playerView.ts`, called from `playbackEnded`): a report that says the run is over leaves fullscreen at once, unless another play's view is showing by then. Any other report follows the old rule (no view showing, the second report, nothing pending on the page). The 50-second backstop stays for a report that never comes.
- `playback-ended` keeps its job, the refresh of watch state, and its payload only grows, by two ids-and-flag fields, for both players. Nothing reads the new fields for a play in mpv's own window.

The eight ways a play can end, and what the page now does (observed in the embedded venue unless marked):

| How it ends | What the page is told | Fullscreen |
| --- | --- | --- |
| Clean, successor started by the backend (a playlist) | tracker: not over; the successor's view opens; dispatcher: not over | kept |
| Clean, successor started by the page (Continue Playing) | tracker: not over; `continue-playing`; dispatcher: not over, with the page still deciding | kept, then left by the page's own end-of-run path if it finds nothing (by reading and unit test) |
| Clean, nothing follows | tracker: not over; dispatcher; the page's end-of-run path | left (as before) |
| Clean, the playlist's next entry cannot be started | tracker: not over; dispatcher: over | left at once |
| The user stops from the view | the view leaves fullscreen itself | left (as before) |
| An error (the server refuses the stream) | tracker: over | left at once |
| mpv quit over its socket, or dying | tracker: over | left at once (dying: by reading) |
| The quality step-down, or a play replaced by another | tracker: not over (another session is the active one) | kept (by reading and unit tests) |

Written by one helper agent, attacked by another (three findings: the playlist case above, a missing test, and the wait for the watcher being unbounded in a corner), repaired by a third; then read and verified by the coordinating agent.

## Files changed

- `src-tauri/src/commands.rs` — `end_is_run_over`, `PlaybackAdvance::saw_eof`, the tracker's report, `AfterCleanEnd`, `advance_playlist` and its two helpers; tests.
- `src-tauri/src/lib.rs` — the dispatcher uses the three outcomes and reports `runOver`.
- `src-tauri/src/playback.rs` — `eof_watch`, `after_eof_watcher`, the end-of-file watcher told when the play has ended; tests.
- `src/lib/playerView.ts`, `src/routes/+page.svelte` — the rule and its call.
- `tests/player-view.test.mjs` — one test per case, and the wiring on both sides.
- `tests/e2e/scenarios/embeddedfullscreenend.mjs` (new, six legs), `tests/e2e/mockjf.mjs` (a switch that makes the mock refuse a stream), `tests/e2e/README.md`.

## Guard proof

Run 2026-11-14 by the coordinating agent in a disposable worktree at the fix's content; `cargo +stable test --locked --lib commands::tests` and `node --test tests/player-view.test.mjs tests/player-controls.test.mjs`.

| Injection | Rust | Node |
| --- | --- | --- |
| Unchanged | PASS | PASS |
| No end is ever the end of its run (the original defect, on the backend's side) | FAIL `an_end_is_the_end_of_the_run_only_when_it_is_not_clean_and_nothing_replaced_the_play` | FAIL `the tracker's report of an end names its session and says whether the run is over` |
| A play that was replaced (a step-down) still ends its run | FAIL, the same test | FAIL, the same test |
| The page ignores what the report says (the original defect, on the page's side) | | FAIL, five tests, among them `an end in an error leaves fullscreen at its one report (case 5)` |
| An old play's error end leaves fullscreen although another play's view shows | | FAIL `a step-down keeps fullscreen: the old play is replaced, not over` |

Also run by the coordinating agent at the fix's content: the full check (577 Rust tests; `npm audit` fails on `main` for a reason of its own, see `.agents/state.md`, Blockers); six scenarios in the embedded venue, among them the new `embeddedfullscreenend` with its six legs (6 of 6; the error leg left fullscreen 6 ms after the view went); and the WHOLE suite in the external player, because the fix changes code both players use to go on to the next play: 41 of 42, the one failure being `pagefail`, the recorded product race. No panic in any app log of those runs.

The helpers ran, each alone and each failing its named test: the whole fix taken out (the scenario's error and quit legs sit in fullscreen until the test's ten seconds are up); a clean end counted as the end of a run (the playlist leg loses its fullscreen in the gap); the page ignoring the flag; the flag left out of the tracker's report; the page not handing the report to the rule; a replaced play ending its run; `saw_eof` never true; an embedded play's report not waiting for the watcher (a clean end read 400 ms late is taken for an error); an external play's report made to wait; the dispatcher never saying the run is over (the failed-successor leg sits in fullscreen); the flag made to wait for the page's own pending successor; the watcher's give-up removed, and its wait set to 30 seconds.

## Coder dispute (if any)

## Known gaps

- **A stream that fails in the middle of a play** was not staged: the test clips are small enough that mpv has them whole. The error in the scenario is a stream refused when it is opened. mpv may also report a stream cut short as a clean end; then the play counts as finished, as it does in mpv's own window.
- **The step-down, a replaced play, and a successor the page starts** are held by unit tests on both sides and by reading (the new session is installed before the old player is stopped), not end to end in fullscreen.
- **A replacement that is installed and then fails to start** was listed here as a gap, by reading. The reviewer reopened the finding for it, and it is closed by the repair of round 2 (below).
- **An end that reports nothing at all** (the play's bookkeeping failed before the report) still relies on the backstop, as before.
- An embedded play's end report can be up to a second late when mpv is gone before its socket ever answered.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `0aeeff0..3d2fef6`, 2026-11-14 00:45 UTC
(`.agents/review/codereview-2026-11-14-embedded-controls.md`). Reviewer's
title: "Playback errors leave Vela fullscreen for 50 seconds". Reviewer's severity: MEDIUM.

### Verification round 1, recorded 2026-11-14 02:17 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d7ce-e93e-7842-b0bd-47dec7fd6aa6`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 02:07 UTC, finished 02:14 UTC.
- Reviewed head `4118fea55f8dddb27aac30cd2d18078c4aa41a3c`, base
  `a909c206eb53e21d5e8d6f1181fd0df69960b6be` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/commands.rs:4167 — A provisional replacement is treated as a guaranteed successor. During automatic step-down, the old tracker can report runOver=false before the replacement fails to launch. Cleanup at line 5391 clears the session without emitting a terminal outcome; apply_step_down only logs the failure. Replaying the actual page handlers confirms fullscreen remains until the 50-second backstop in either close/report order. Emit a terminal outcome when that replacement fails and nothing follows.

### Repair (round 2), `c872dad`

The reviewer's case, traced in the code before anything was changed. A play
that replaces a running one is installed as the active play BEFORE its
launch stops the old player. The old play's tracker then reports its end,
sees another play active, and says the run goes on. When the launch then
failed, the failure branch cleared the new session, put the playlist
position back and returned the error, and nothing told the page; the
step-down only logs the error. Replayed through the page's real helpers
before the change, in both orders of the old play's report and its view
closing: no event leaves fullscreen, only the 50-second backstop does.

What the backend does now (the page is unchanged):

- **A tracker notes an end it reports as "the run goes on"**
  (`tracker_end_is_run_over`, the tracker's verdict, now one function):
  when another play is the active one, the ended play's session is noted in
  `AppState.ends_told_run_goes_on`, under the lock of the active session.
- **Installing a play, starting it and undoing a failed start are one
  function**, `install_and_launch`, with the undo in `undo_failed_launch`
  (the same steps as before). It reads which play it replaces before it
  installs anything, and only when that play ran in Vela's own window; it
  empties the noted ends at its start, so one noted for an earlier launch is
  never taken for this one's.
- **A launch that failed after its session was installed reports the run
  over** (`failed_launch_ends`): for the play it replaced, when that play's
  end was noted during this launch; and for itself, when its own play had
  already started in Vela's own window and so had its view announced (found
  by the helpers' second attack: such a play has no tracker, and nothing
  else would ever report it). The reports are `playback-ended` with
  `sessionId` and `runOver: true`, and nothing else in them. None is sent
  while another play is active.
- **Why the note, and not "there was an active play"**: the active session
  is not cleared when a play ends. The first version of this repair reported
  for any active embedded play, and so for one that had ended long ago, and
  for one that was still running because the launch failed before stopping
  it; the helpers' attacks found both. With the note, only an end the old
  play's own tracker reported during this launch brings the report. The undo
  clears the new session under the same lock the tracker notes under, so by
  then every such end has been noted, and a tracker that reports later finds
  no play active and says the run is over itself.

What the page is told when a replacement fails, per caller:

| Who started the replacement | Now |
| --- | --- |
| The automatic quality step-down | the report naming the replaced play; the page leaves fullscreen at once |
| The user, while another play runs | the same report, and the command's error as before ("Couldn't play …") |
| The page's Continue Playing successor | nothing new: the old play had ended by itself, was never noted, and the page's own end-of-run path leaves, as before |
| The backend's playlist successor | nothing new from the launch when the old play had ended before it; the dispatcher's report says the run is over, as before |
| Any of them, replacing a play in mpv's own window | nothing new |

Orders of events, for the replaced play's own report (R), the launch's
report (F) and the old view closing (C): in each of the eight orders in
which the launch reports, the page leaves at F. When the replaced play's
tracker reports only after the failure was undone, nothing was noted and
there is no F: R then finds no play active and says the run is over itself.
A report that arrives while another play's view shows leaves nothing. The
page's count of reports starts again at every view opening or closing, so
the extra report cannot be taken for a later play's second report. Each is
replayed through the page's real helpers in `tests/player-view.test.mjs`.

Files: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` (the new field;
a test-only constructor of the whole state), `src-tauri/src/player/mod.rs`
(`PlayerHandle::is_embedded_play`), `tests/player-view.test.mjs`,
`tests/player-selection.test.mjs`.

Written by one helper agent, attacked twice by two others (ten findings in
all, eight fixed, two left as the gaps below), repaired after each attack,
and checked by a further one; then read, landed as one commit and injected
by the coordinating agent.

Guard proof for round 2, run 2026-11-14 by the coordinating agent in the
main clone at `c872dad`, one injection at a time, each restored from the
commit; `cargo +stable test --locked --lib commands::tests` (70 tests) and
`node --test tests/player-view.test.mjs tests/player-controls.test.mjs tests/player-selection.test.mjs`
(55 tests):

| Injection | Rust | Node |
| --- | --- | --- |
| Unchanged | PASS | PASS |
| The failed launch hands no report on (the reopened defect) | FAIL `a_replacement_that_fails_to_start_reports_the_replaced_run_over`, `a_failed_launch_whose_view_opened_reports_its_own_run_over` | FAIL, the source check of the launch |
| The real call site drops what the launch gives it to report | (cannot see it: no app in a unit test) | FAIL `a launch that fails after replacing a play reports that play run over, and no other path does` |
| A tracker does not note an end told "the run goes on" | FAIL, three tests, among them `a_tracker_notes_only_an_end_reported_while_another_play_is_active` | |
| The replaced play is reported whether or not its end was noted (the repair's first version) | FAIL `a_failed_play_after_an_ended_embedded_play_reports_nothing`, `a_launch_that_fails_before_stopping_the_old_play_reports_nothing`, `an_end_noted_for_an_earlier_launch_is_not_this_ones` | FAIL, the source check |
| Ends noted for an earlier launch are kept | FAIL `an_end_noted_for_an_earlier_launch_is_not_this_ones` | FAIL, the source check |
| A report is made while another play is active | FAIL `a_failed_replacement_reports_nothing_while_another_play_is_active`, `a_failed_launch_ends_the_run_only_when_no_play_is_active` | |
| A replaced play in mpv's own window is reported too | FAIL `a_failed_replacement_of_an_external_play_reports_nothing_more`, `the_replaced_view_is_the_active_embedded_play_and_never_the_new_one` | |
| The launch's own opened view is not reported | FAIL `a_failed_launch_whose_view_opened_reports_its_own_run_over` | FAIL, the source check |
| The page ignores a report that says the run is over | | FAIL, eight tests, among them `a play that replaced the one showing and fails to start leaves fullscreen at once, in every order, whoever started it` |

The Rust tests run the real `install_and_launch` and the tracker's own
function on a plain thread, against the whole app state; what they cannot
reach is the two places that need the running app (the tracker's call of
its verdict, and the emit at the launch's call site), and those are held by
checks of the source text that were each seen to fail with the call taken
out.

Also run at the landed content: the full check (589 Rust tests; `npm audit`
fails on `main` for a reason of its own, see `.agents/state.md`, Blockers);
by the coordinating agent, three scenarios in the embedded venue
(`embeddedfullscreenend` with its six legs, `embeddedcontrols`,
`embeddedskip`: 3 of 3, no panic in an app log); and by the checking helper,
at the same Rust and page sources, the WHOLE suite in the external player:
41 of 42, the one failure being `pagefail`, the recorded product race.

Not covered, and why:

- **No end-to-end leg.** A launch that fails after its session is installed
  cannot be produced in the real app from outside: a stream the server
  refuses fails before anything is installed (that is leg 6 of the
  scenario), and the step-down is decided from mpv's own figures, which the
  harness cannot set. It would take a hook in the product for tests, which
  was not added.
- **Two failed launches in a row within about a second.** If a replacement
  fails before the replaced play's tracker has reported, nothing is noted
  and the launch reports nothing; the tracker then reports "over" itself,
  which is right. But if a further play is installed before that tracker
  reports, the tracker says "goes on" under it, and if that play fails too,
  it replaced nothing and reports nothing: the backstop. By reading. The
  reviewer reopened the finding for this; closed in round 3 (below).
- **A play that fell back to mpv's own window during a run in Vela's
  window**, replaced by a launch that fails, gets no report: it is recorded
  as external. As before. The reviewer reopened the finding for this too;
  closed in round 3 (below).
- **The error line of a play the user started** can be cleared by the
  refresh the new report triggers, when that play really replaced a running
  one and failed. The window is then out of fullscreen with the library
  showing and no line saying why. Keeping an error across a refresh is the
  page's business and outside this finding.
- A clean end noted while a replacement was installed can bring two reports
  that say the same; the cost is one more refresh.

One slip by a helper while this was written, undone within minutes and
never committed: a file write with a relative path landed in the main clone
instead of the helper's own worktree (`src-tauri/src/lib.rs`), and was
restored from the commit. While it stood it made a check run by the
coordinating agent, for an unrelated commit, fail; that check was run again
on a clean tree before anything was committed.

### Verification round 2 (repair-delta redispatch), recorded 2026-11-14 04:02 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d828-f915-7ee1-bfee-2fe98d517e09`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 03:45 UTC, finished 03:57 UTC.
- Reviewed head `c872dadba2526ba3bc52d8998aefde65bab6357f`, base
  `712aac89147145d6fc0c7a3a6767dea87a6edfad` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/commands.rs:4190 — A clean EOF reported after replacement failure still returns runOver=false. Cleanup emits nothing because that end was not yet noted; advance_playlist then returns Successor because cleanup cleared the active session. I reproduced both verdicts and replayed tracker-report → view-close → dispatcher-report through the page: fullscreen remains until the 50-second backstop. The claimed late-tracker fallback must handle clean ends too.
  - src-tauri/src/commands.rs:4347 — current_child is not durable evidence that this launch announced a view: the reaper at lib.rs:238 can clear the ended handle before this read, including during transcode teardown. With the predecessor's clean report preceding installation, a failed replacement whose own view opened then produces no terminal report. A deterministic reaper interleaving and page replay reproduced the backstop. Preserve the opened-view identity independently of the reapable slot; the stand-in launch currently omits this interleaving.
  - src-tauri/src/commands.rs:4337 — The acknowledged consecutive-failure gap remains this finding's failure. If B fails before A's tracker reports, then C installs, A reports runOver=false under C, and C fails, replaced is None and A's recorded end is ignored. The real helpers produced no terminal report; page replay retained fullscreen for 50 seconds in either close/report order. Preserve the outstanding terminal obligation across failed launches.
  - src-tauri/src/commands.rs:4264 — The acknowledged fallback gap also remains a defect: an external fallback can inherit an embedded run's fullscreen. If its replacement fails after its tracker reports runOver=false, this Embedded-only filter suppresses the terminal outcome. Replaying the old embedded report, old view close, and external fallback's report leaves fullscreen until the backstop. Preserve embedded-run ownership through fallback while keeping ordinary external-only playback unaffected.

### Repair (round 3), `060a5fd`

Each of the reviewer's four orders was first written as a test against the
second repair's code, before anything was changed. All four failed, for
the reasons the reviewer gave.

**Why the second repair could not hold.** It decided what to tell the page
from which reports of a play's end had already been made: a note the old
play's tracker left, which play the new one replaced, what was in the
player slot. The four orders are four different arrival orders of the same
reports. A rule that hangs on the order has a hole for every order nobody
listed, so this repair does not add four cases; it changes the rule.

**The rule now: the state of the app decides.** A launch that failed after
its session was installed is undone as before (`undo_failed_launch`) and
then tells the page the run is over when three things hold at that moment
(`report_failed_launch_end`, `failed_launch_ends_run`):

1. **Nothing is playing.** The launch reads which player is in the slot
   before it installs anything. Only that same player, still in the slot
   and not ended, counts as playing afterwards (`nothing_playing`). An
   empty slot, an ended player, or a player the launch itself started and
   stopped all mean nothing plays. Players carry a serial number for this
   (`player::new_serial`, `PlayerHandle::serial`).
2. **No play is active.**
3. **The page holds the player's fullscreen.** The backend now keeps the
   page's claim (`AppState::player_fullscreen_held`). It is set where
   `player_fullscreen` turns fullscreen on for the running embedded play,
   under the slot's lock (`claim_player_fullscreen`); cleared where the
   command turns it off, and where the window is seen to leave fullscreen
   however that came about (`follow_fullscreen`). It outlasts the play
   that asked, as the page's own claim does, also across a play that fell
   back to mpv's own window.

It then sends ONE `playback-ended` with `sessionId` and `runOver: true`,
and nothing else in it. The session it names is the view the page is
showing, or the launch's own when no view shows. Which view shows is kept
in a record that changes in the same step as the `player-opened` and
`player-closed` events are sent (`player::ViewRecord::announce`, the only
sender of those events), and the report is sent under that record's lock
(`ViewRecord::report`), so no opening or closing can come between the name
it reads and the report.

No tracker's report is read for any of this. Taken out: the second
repair's note of ends told "the run goes on"
(`AppState::ends_told_run_goes_on`), its reading of the replaced play
(`replaced_embedded_session`, `replaced_view_session`,
`failed_launch_ends`), and `PlayerHandle::is_embedded_play`.

**The completion dispatcher** (`advance_playlist`) now says the run is
over whenever the ended play is not the active one and NO play is active
(`replaced_or_over`); it used to say a successor was coming. No play is
active only after a failed launch was undone, so this is the reviewer's
first order seen from the dispatcher's side. Its report
(`clean_end_report`, which `lib.rs` now sends as it stands) also carries
`successor`: true when another play is active or one is being chosen.

**The page** (`endLeavesPlayerFullscreen`) no longer counts reports. The
helpers' second attack found that counting left fullscreen at the FIRST
report of a successor that plays in mpv's own window, because such a play
opens no view to start the count again. The rule is now:

| The report | The page leaves fullscreen |
| --- | --- |
| says the run is over | unless the view of a play that still plays shows |
| is the dispatcher's, and says neither "over" nor "a successor is coming" | when no such view shows and the page is not itself starting a successor (Continue Playing) |
| anything else (a play's own report that is not the end of the run; "a successor is coming") | never |

A view "of a play that still plays" is told from a view that is only
waiting to close: once a report has named the session whose view shows,
that play has ended (`showingPlayEnded`). Every report of an end is sent
after its play's player has ended.

**The page's claim follows the backend's.** `player_fullscreen` answers
whether the claim is held once the request is dealt with (a refused "on"
changes nothing), and the page takes its own claim from that answer. The
page sends its requests one at a time, each after the last was answered
(`fullscreenRequests` in `playerControls.ts`), so the window is asked in
the order the page asked. `player_fullscreen` also refuses a play that
has been told to stop or has ended
(`PlayerHandle::is_running_embedded_play`); a launch whose tracking could
not be started leaves such a play in the slot.

**A source choice that ends without a play** is the same failure by
another door, and is closed here too. When a playlist's next entry needs
the user to choose a source, the dispatcher says a successor is coming and
the page keeps fullscreen while the dialog is up. Cancelling it, letting
it expire, or choosing a source whose play is refused or fails before its
session is installed, told the page nothing; only the backstop left
fullscreen. The two commands now run through `cancel_choice` and
`resolve_choice`, which report the end of the run from the same state:
nothing is playing, no choice is still waiting, and the page holds the
player's fullscreen (`report_choice_end`, `choice_end_ends_run`). A launch
that failed after its session was installed has already reported
(`PlayFailure::launched`), so the choice adds nothing then.

The reviewer's comments, and where each is held:

| Comment | Closed by | Test (`commands::run_over_tests`) |
| --- | --- | --- |
| 1, a clean end reported after the replacement failed | the launch reports from state; and the dispatcher, finding no play active, says the run is over by itself | `reviewer_1_a_clean_end_reported_after_a_failed_replacement` |
| 2, the reaper empties the slot before the read | the view comes from the record kept with the events, not from the slot; an emptied slot means nothing plays | `reviewer_2_a_failed_launch_whose_view_opened_and_was_reaped` |
| 3, two failed launches in a row | each failed launch decides from the state after its own undo | `reviewer_3_two_failed_launches_in_a_row` |
| 4, a play that fell back to mpv's own window | the claim on fullscreen is kept by the backend and outlasts the fallback; no check of which player ran remains | `reviewer_4_a_play_that_fell_back_to_mpv_own_window` |

**Every order, not a list of them.**
`in_every_order_the_page_leaves_fullscreen_exactly_when_the_run_is_over`
enumerates every order of a stated set of events and runs each on a fresh
app state through the real functions: `install_and_launch` under the play
lock, the tracker's verdict on a plain thread, `advance_playlist`, a
reaper tick, `ViewRecord::announce`, `claim_player_fullscreen`,
`cancel_choice` and `resolve_choice`. The events: the old play ending
cleanly or being stopped, its tracker's report, its dispatch, its view
closing, a reaper tick, the steps of a second launch (which fails, fails
after its own view opened, fails before stopping the old player, starts in
Vela's window, or starts in mpv's own window), that launch's view opening
and closing, a third launch that fails, a dispatch that lands inside a
launch, a successor being chosen (cancelled, expired, refused, its launch
failing, or starting), and successors in mpv's own window following one
another or stepped down. 42,838 orders. For each, the test keeps what was
true when each event was sent, and asserts that the page leaves fullscreen
at a report sent when nothing was playing or coming, never at one sent
while something was, and that it has left by the end exactly when nothing
plays. The 736 distinct sequences of events the page receives are kept in
`tests/fixtures/run-over-orders.json`, compared byte for byte, and
replayed by `tests/player-view.test.mjs` through the page's real helpers.

**For somebody who only uses mpv's own window**, nothing they see
changes. The claim is set only for a running embedded play, so it is never
set, and neither a failed launch nor a source choice sends anything new
(`a_failed_launch_sends_nothing_new_to_somebody_who_uses_mpv_own_window`,
`a_choice_sends_nothing_new_to_somebody_who_uses_mpv_own_window`). The
dispatcher's existing report, sent when it was before, can now say
`runOver: true` where it said false, and carries `successor`. The page
acts on both only by leaving a fullscreen it holds, which it never does
without the embedded view. `continue-playing` is sent exactly when it was.

Settled by the coordinating agent while landing this, as details of how it
is built:

- **`successor` is sent to both players' pages.** The brief for this
  repair allowed only `runOver` to change for mpv's own window. The field
  is what lets the page stop counting reports; it changes nothing that
  user sees. Kept.
- **The source choice is part of this finding.** It is the finding's
  failure (the library fullscreen, nothing coming, until the backstop),
  and the dispatcher's `successor` made it reachable from a playlist. It
  could not be left for a later finding without landing a regression.
- **A play in mpv's own window keeps the run's fullscreen only until the
  backstop.** When the last view has closed and a successor plays in mpv's
  own window for more than 50 seconds, the page's backstop takes Vela's
  window out of fullscreen while that play goes on. The window that
  changes is the one behind mpv's own; the play is not touched. Left as it
  is.
- `ViewRecord::announce` is built only on Linux, where its one caller is.

Files: `src-tauri/src/commands.rs`, `src-tauri/src/run_over_tests.rs`
(new: these tests, apart from the command layer), `src-tauri/src/lib.rs`,
`src-tauri/src/player/mod.rs`, `control.rs`, `external.rs`,
`embedded/host.rs`, `embedded/session.rs`; `src/lib/playerView.ts`,
`src/lib/playerControls.ts`, `src/routes/+page.svelte`;
`tests/fixtures/run-over-orders.json` (new), `tests/player-view.test.mjs`,
`tests/player-controls.test.mjs`, `tests/player-command.test.mjs`,
`tests/player-selection.test.mjs`, `tests/embedded-player.test.mjs`,
`tests/dependency-policy.test.mjs` (its check that a source choice is
taken once follows the code into `resolve_choice`).

Written by one helper agent, attacked twice by two others (seventeen
findings in all: two the same HIGH, a successor's source choice that never
ended the run; fourteen fixed, one kept as the first decision above, two
left as the gaps below), repaired after each attack, and checked by a
further one, which ran nine injections of its own and both end-to-end
venues. Then read in full, landed as one commit and injected again by the
coordinating agent.

Guard proof for round 3, run 2026-11-14 by the coordinating agent in the
main clone at `060a5fd`, one injection at a time, each restored from the
commit and the tree checked clean after it. Rust:
`cargo +stable test --locked --lib -- commands:: player::control::tests`;
Node: `node --test` on `tests/player-view.test.mjs`,
`player-controls`, `player-selection`, `player-command`,
`embedded-player` and `dependency-policy`.

| Injection | Rust | Node |
| --- | --- | --- |
| Unchanged | PASS (131 tests) | PASS (105 tests) |
| A failed launch hands no report on (the reopened defect) | FAIL all four `reviewer_*` tests, `in_every_order_…`, `a_choice_whose_launch_failed_is_reported_once` | FAIL `once a play has installed its session, only the launch-failure path ends it early` |
| A failed launch reports whether or not the page holds fullscreen | FAIL `a_failed_launch_sends_nothing_new_to_somebody_who_uses_mpv_own_window`, `a_failed_launch_ends_the_run_only_when_all_three_hold` | |
| Any player in the slot that has not ended counts as playing (the serial is not compared) | FAIL `a_play_the_launch_started_and_stopped_plays_nothing_before_it_has_ended`, `nothing_plays_unless_the_player_found_before_the_launch_still_runs` | |
| With no play active the dispatcher still says a successor is coming | FAIL `reviewer_1_…`, `the_dispatcher_says_the_run_is_over_when_no_play_is_active`, `in_every_order_…` | |
| The report names the launch's own session whatever view shows | FAIL `reviewer_1_…`, `the_view_record_follows_the_page_and_names_the_view_showing`, `in_every_order_…` | |
| The window leaving fullscreen does not clear the backend's claim | (needs the running app) | FAIL `the window's real fullscreen reaches the page from GDK, followed once from setup`, `the page's claim on fullscreen is kept where it is asked for and where the window leaves it` |
| The dispatcher's report never says a successor is coming | FAIL seven tests, among them `a_successor_in_mpv_own_window_keeps_fullscreen_until_the_run_is_over`, `a_successor_whose_view_has_not_opened_keeps_fullscreen`, `in_every_order_…` | FAIL `the dispatcher's report of a clean end names its session and says whether the run is over` |
| A cancelled source choice reports nothing | FAIL `a_successor_being_chosen_keeps_fullscreen_and_a_choice_ended_without_a_play_ends_the_run`, `in_every_order_…` | FAIL `a source choice's end is reported from the state of the app, through the real commands` |
| A chosen play that fails before it is installed reports nothing | FAIL the same two | FAIL the same |
| The dispatcher's "neither" report leaves although the view of a running play shows | | FAIL `a clean end whose successor the backend starts keeps fullscreen across the gap (case 1)`, and case 3 |
| A view whose play has ended still counts as a running play's | | FAIL `in every order the backend produces, the page leaves fullscreen at a report exactly when the run is over`, `a successor in mpv own window keeps fullscreen until a report says the run is over`, and case 3 |
| The page ignores what the backend answers it holds | | FAIL `fullscreen for the player is held by the page and left on events` |
| Fullscreen is granted to a play that is not the running embedded one | FAIL `the_page_claims_fullscreen_only_for_the_running_embedded_play`, `a_refused_request_for_fullscreen_is_answered_as_holding_nothing` | FAIL `fullscreen is asked of the window only for the running embedded play, after the lock` |
| The fullscreen command answers what was asked, not what is held | (needs the running app) | FAIL the same, and `the page's claim on fullscreen is kept where it is asked for and where the window leaves it` |
| A play told to stop may still take fullscreen | FAIL `player::control::tests::with_a_player::only_the_running_embedded_play_counts_as_running`, and the two claim tests | |
| Fullscreen requests are sent at once, not one after the other | | FAIL `fullscreen requests reach the backend one at a time, in the order asked` |
| `lib.rs` reports a clean end without the dispatcher's answer | (needs the running app) | FAIL `the dispatcher's report of a clean end names its session and says whether the run is over` |
| The view's events are sent without the record | (needs the running app) | FAIL `the view record and the view events are one step` |
| The play path does not mark a failed launch as already reported | PASS: the Rust tests hand `resolve_choice` a stand-in play, so only the source check sees the real call | FAIL `once a play has installed its session, only the launch-failure path ends it early`, `a launch that fails reports the run over only from the state of the app, and nothing else sends that report` |

The helpers ran 56 injections of their own while writing it and nine more
when checking it; each failed its named tests.

Also run at the landed content: the full check (602 Rust tests; `npm
audit` fails on `main` for a reason of its own, see `.agents/state.md`,
Blockers). End to end, by the coordinating agent at
`060a5fd`: three scenarios in the embedded venue (`embeddedfullscreenend`
with its six legs, `embeddedcontrols`, `embeddedskip`), 3 of 3, no panic in
the log. And by the checking helper, at the commit before the
coordinating agent's last two edits (comments in the tests, and
`announce` built only on Linux): the WHOLE suite in the external player,
41 of 42, the one failure being `pagefail`, the recorded product race.

Not covered, and why:

- **No end-to-end leg for a launch that fails after its session is
  installed.** It cannot be staged from outside the running app: a missing
  mpv falls back to the one on the path, a broken configuration is caught
  before installation, and the step-down is decided from mpv's own
  figures. It would take a hook in the product for tests, which was not
  added. Nor is there a leg for a source choice: it needs the same title
  on two servers with the Ask policy, which the harness does not stage.
- **A source choice ended in the instant another clean end is being
  dispatched** (or while the page's own Continue Playing is deciding) can
  report the run over just before a successor starts. That successor then
  plays outside fullscreen. Fullscreen is lost, never held.
- **A source choice that expires with its dialog still open** is reported
  when the user next cancels or chooses, not at the moment it expires; the
  dialog is on screen for all that time. One that expired before the page
  ever fetched it (the page unresponsive for two minutes) is never
  reported, and the backstop leaves.
- **A clean end whose completion cannot be written** (a write error in
  Vela's own state) sends no dispatcher report at all, as before this
  finding; with nothing else playing, the backstop leaves.
- **The page's claim and the backend's can differ in two ways, neither
  reachable from the page's own controls, by reading.** Both need the
  window's "left fullscreen" event to arrive after a newer "on" was
  granted. Every "on" the page sends is decided on having seen the window
  not fullscreen (`!fullscreen` in the key and the button), so the event
  of any earlier "off" has been handled by then, by the backend first.
  What is left is the user leaving fullscreen with the compositor's own
  shortcut within the millisecond or so between an "on" being granted and
  its answer reaching the page: the page then believes it holds a
  fullscreen the window does not have.
- **The enumeration's limits.** One reaper tick per order; an undo and
  its report are one step; no third launch after a second that had its own
  view or that started (the count of orders passes 100,000); the
  dispatcher's playlist arms that need a server or the playlist store on
  disk are composed from the real `install_and_launch` and
  `after_successor` or `successor_failed`, not run whole. What "playing or
  coming" means in it is the test's own model, and the Node replay checks
  the page's helpers against that model, not the model itself. Time is not
  in it, so neither is the backstop.
- **What needs the running app is held by checks of the source text**:
  `player_fullscreen` calling `claim_player_fullscreen` and answering its
  `held`; `follow_fullscreen` clearing the claim; the host sending view
  events only through the record; `lib.rs` sending `clean_end_report` as
  it stands; the two commands going through `cancel_choice` and
  `resolve_choice`. Each was seen to fail with its call taken out.
- **A panic inside the launch** after a new player is in the slot counts
  as nothing playing. Release builds abort on panic.
- **Not built for macOS or Windows here**; the branches for them are by
  reading.
- **The error line of a play the user started** can still be cleared by
  the refresh the report triggers, as in round 2.
