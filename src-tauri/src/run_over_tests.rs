//! Review finding pc-2: whether the page is told that a run of plays in
//! Vela's own window is over, when a launch fails after its session was
//! installed, when a play's clean end finds no play active, and when a source
//! choice for the run's next play ends without a play. Tests of the
//! command layer, kept apart from it; they run the real `install_and_launch`,
//! the tracker's verdict on a plain thread, the completion dispatcher's
//! `advance_playlist`, the view record the player view's events go through
//! and the fullscreen claim `player_fullscreen` keeps.

use super::*;
use std::sync::Mutex as StdMutex;

use crate::player::embedded::Session as TestPlay;
use crate::player::PlayerHandle;

fn item() -> ItemDto {
    ItemDto {
        rating_key: "test:k".to_string(),
        title: "k".to_string(),
        year: None,
        summary: None,
        duration_ms: None,
        media_type: Some("movie".to_string()),
        poster: None,
        series_poster: None,
        backdrop: None,
        view_offset_ms: None,
        played: Some(false),
        last_watched_at_ms: None,
        added_at_ms: None,
        index: Some(1),
        parent_index: None,
        grandparent_title: None,
        parent_title: None,
        parent_rating_key: None,
        grandparent_rating_key: None,
        source_id: "test".to_string(),
        provider_ids: vec![],
        backing: None,
        canonical_id: None,
        watch_key: None,
        detail_key: None,
    }
}

// ---- what the page is sent ----------------------------------------------

/// One event the page receives, as the page sees it, with what was true in
/// the app when it was sent: whether anything was playing or coming
/// (`busy`), and whether the page itself was still starting a successor
/// (`pending`, the page's `continuationPending`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Seen {
    Opened(String),
    Closed(String),
    /// A `playback-ended`: its session, whether it says the run is over, and,
    /// in the dispatcher's report only, whether it says a successor is coming
    /// (`None` when the report does not carry `successor` at all).
    Ended {
        session: String,
        run_over: bool,
        successor: Option<bool>,
    },
}

/// A report that says only whether the run is over (the tracker's, a failed
/// launch's, or a source choice's that ended without a play).
fn ended(session: &str, run_over: bool) -> Seen {
    Seen::Ended {
        session: session.to_string(),
        run_over,
        successor: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct PageEvent {
    seen: Seen,
    busy: bool,
    pending: bool,
}

/// What is true in the app, as the test drives it.
#[derive(Default)]
struct World {
    /// The old play's player has not ended.
    a_running: bool,
    /// The old play ended by itself at a clean end of file.
    a_clean: bool,
    /// The new play started and runs.
    b_running: bool,
    /// A launch is installed and has not finished.
    launching: bool,
    /// The dispatcher authorised Continue Playing: the page starts the next
    /// play, which is the launch that follows.
    coming: bool,
    /// The page has not yet had the answer of the play it started.
    pending: bool,
    /// Later plays of the run, in mpv's own window: the dispatcher's
    /// successor "C", and the play "D" a step-down put in B's place.
    c_running: bool,
    d_running: bool,
}

#[derive(Default)]
struct Page {
    events: StdMutex<Vec<PageEvent>>,
    world: StdMutex<World>,
}

impl Page {
    fn busy(&self) -> (bool, bool) {
        let w = self.world.lock().unwrap();
        (
            w.a_running || w.b_running || w.c_running || w.d_running || w.launching || w.coming,
            w.pending,
        )
    }

    fn send(&self, seen: Seen) {
        let (busy, pending) = self.busy();
        self.events.lock().unwrap().push(PageEvent {
            seen,
            busy,
            pending,
        });
    }

    fn ended(&self, end: &serde_json::Value) {
        self.send(Seen::Ended {
            session: end["sessionId"].as_str().expect("a session").to_string(),
            run_over: end["runOver"] == serde_json::json!(true),
            successor: end.get("successor").and_then(serde_json::Value::as_bool),
        });
    }

    fn events(&self) -> Vec<PageEvent> {
        self.events.lock().unwrap().clone()
    }
}

/// The view of `session` opened or closed, through the record, as the
/// embedded play's lifecycle sends it.
fn view(state: &AppState, page: &Page, session: &str, opened: bool) {
    state.player_view.announce(session, opened, || {
        page.send(if opened {
            Seen::Opened(session.to_string())
        } else {
            Seen::Closed(session.to_string())
        })
    });
}

/// The page's own handling of these events, as `+page.svelte` does it with
/// the helpers of `src/lib/playerView.ts` (the Node test
/// `tests/player-view.test.mjs` runs the very sequences these tests produce
/// through the real helpers): where it leaves the player's fullscreen.
fn page_leaves_at(events: &[PageEvent]) -> Option<usize> {
    let mut showing: Option<&str> = None;
    // Whether a report has named the session whose view shows.
    let mut showing_ended = false;
    for (index, event) in events.iter().enumerate() {
        match &event.seen {
            Seen::Opened(session) => {
                showing = Some(session);
                showing_ended = false;
            }
            Seen::Closed(session) => {
                if showing == Some(session.as_str()) {
                    showing = None;
                    showing_ended = false;
                }
            }
            Seen::Ended {
                session,
                run_over,
                successor,
            } => {
                if showing == Some(session.as_str()) {
                    showing_ended = true;
                }
                let no_live_view = showing.is_none() || showing_ended;
                let leaves = if *run_over {
                    no_live_view || showing == Some(session.as_str())
                } else if *successor == Some(false) {
                    no_live_view && !event.pending
                } else {
                    false
                };
                if leaves {
                    return Some(index);
                }
            }
        }
    }
    None
}

/// The finding's rule: the page leaves fullscreen at a report whenever
/// nothing is playing or coming, and never while something is.
fn assert_page_left_rightly(events: &[PageEvent], final_busy: bool, what: &str) {
    match page_leaves_at(events) {
        Some(at) => {
            assert!(
                !events[at].busy,
                "{what}: the page leaves fullscreen at {at} while something plays or is coming: {events:#?}"
            );
            assert!(
                !final_busy,
                "{what}: something still plays, and the page has left fullscreen: {events:#?}"
            );
        }
        None => assert!(
            final_busy,
            "{what}: nothing plays or is coming, and nothing told the page: {events:#?}"
        ),
    }
}

// ---- the real steps ------------------------------------------------------

/// A tracker's report of the end of `session`, taken as the tracker takes
/// it, on a plain thread outside any runtime.
fn tracker_reports(state: &AppState, page: &Page, session: &str, clean: bool) {
    let run_over = std::thread::scope(|scope| {
        scope
            .spawn(|| tracker_end_is_run_over(state, clean, session))
            .join()
            .expect("the tracker's thread")
    });
    page.send(ended(session, run_over));
}

/// The completion dispatcher's handling of a clean end, and its report, as
/// `lib.rs` sends it (`clean_end_report`, the payload `lib.rs` emits).
async fn dispatcher_reports(state: &AppState, page: &Page, session: &str) -> AfterCleanEnd {
    let after = advance_playlist(state, session).await;
    if after == AfterCleanEnd::ContinuePlaying {
        let mut w = page.world.lock().unwrap();
        w.coming = true;
        w.pending = true;
    }
    let completion = PlaybackCompletion {
        session_id: session.to_string(),
        item_key: "test:k".to_string(),
        watch_key: None,
        started_at_ms: 0,
        watch_backings: Vec::new(),
        media_type: None,
    };
    page.ended(&clean_end_report(&completion, after));
    after
}

fn wait_until_ended(player: &mut PlayerHandle) {
    for _ in 0..20_000 {
        if matches!(player.has_ended(), Ok(true)) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_micros(100));
    }
    panic!("a stopped test play did not end");
}

/// One tick of the reaper in `lib.rs`: an ended player leaves the slot.
fn reaper_tick(state: &AppState) {
    let mut guard = state.current_child.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(child) = guard.as_mut() {
        if matches!(child.has_ended(), Ok(true)) {
            *guard = None;
        }
    }
}

/// What the launch does first, as the real one does: stop the player in the
/// slot and hand it to the reap queue.
fn stop_old_player(state: &AppState, page: &Page) {
    let taken = state
        .current_child
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(mut player) = taken {
        let _ = player.kill();
        wait_until_ended(&mut player);
        state
            .reap_queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(player);
        page.world.lock().unwrap().a_running = false;
    }
}

/// The page holds the player's fullscreen for the running embedded play
/// `session`, as `player_fullscreen` records it.
fn page_asks_fullscreen(state: &AppState, session: &str) {
    assert_eq!(
        claim_player_fullscreen(state, &serde_json::json!(session), true),
        FullscreenClaim {
            ask_window: true,
            held: true
        },
        "the running embedded play may put the window in fullscreen"
    );
}

/// A play "A" running in Vela's window, its view showing, the page holding
/// the player's fullscreen. With `fell_back`, an earlier play "E" of the run
/// had the view and the fullscreen, and "A" fell back to mpv's own window:
/// it has no view, and the page's claim stays.
async fn run_in_fullscreen(state: &AppState, page: &Page, fell_back: bool) {
    let item = item();
    page.world.lock().unwrap().a_running = true;
    if fell_back {
        install_playback_session(state, "E", &item, None, None).await;
        *state.current_child.lock().unwrap() =
            Some(PlayerHandle::Embedded(TestPlay::for_test_live("E")));
        view(state, page, "E", true);
        page_asks_fullscreen(state, "E");
        stop_old_player(state, page);
        // A, in mpv's own window, has already started when E's view closes.
        page.world.lock().unwrap().a_running = true;
        view(state, page, "E", false);
    }
    install_playback_session(state, "A", &item, None, None).await;
    let kind = if fell_back {
        crate::player::PlayerKind::External
    } else {
        crate::player::PlayerKind::Embedded
    };
    *state.active_playback_player.lock().await = Some(("A".to_string(), kind));
    *state.current_child.lock().unwrap() = Some(PlayerHandle::Embedded(TestPlay::for_test_live("A")));
    if !fell_back {
        view(state, page, "A", true);
        page_asks_fullscreen(state, "A");
    }
}

/// Run the real `install_and_launch` for `session` under the play lock, with
/// `launch` as its launch; the reports it gives go to the page.
async fn launch<L>(state: &AppState, page: &Page, session: &str, launch: L) -> bool
where
    L: std::future::Future<Output = Result<std::sync::Arc<std::sync::atomic::AtomicBool>, String>>,
{
    launch_play(state, page, session, None, launch).await
}

/// A playlist's position for the play `session`.
fn playlist_cursor(session: &str) -> PlaylistCursor {
    PlaylistCursor {
        owner: PlaylistOwner::Vela,
        playlist_id: "p".to_string(),
        entry_id: "e".to_string(),
        index: 0,
        session_id: session.to_string(),
    }
}

/// `launch`, for a play that may carry a playlist position of its own.
async fn launch_play<L>(
    state: &AppState,
    page: &Page,
    session: &str,
    cursor: Option<PlaylistCursor>,
    launch: L,
) -> bool
where
    L: std::future::Future<Output = Result<std::sync::Arc<std::sync::atomic::AtomicBool>, String>>,
{
    let _play = state.play_lock.lock().await;
    launch_locked(state, page, session, cursor, launch).await.is_ok()
}

/// `launch_play`, for a caller that already holds the play lock (a source
/// choice being resolved).
async fn launch_locked<L>(
    state: &AppState,
    page: &Page,
    session: &str,
    cursor: Option<PlaylistCursor>,
    launch: L,
) -> Result<std::sync::Arc<std::sync::atomic::AtomicBool>, String>
where
    L: std::future::Future<Output = Result<std::sync::Arc<std::sync::atomic::AtomicBool>, String>>,
{
    let item = item();
    page.world.lock().unwrap().launching = true;
    let launched = install_and_launch(
        state,
        PlayInstall {
            session_id: session,
            item: &item,
            run_kind: None,
            affinity_source_id: None,
            cursor,
            transcode_session: None,
        },
        async {
            let result = launch.await;
            let mut w = page.world.lock().unwrap();
            w.launching = false;
            if result.is_err() {
                w.coming = false;
            }
            result
        },
        |end| page.ended(&end),
    )
    .await;
    page.world.lock().unwrap().pending = false;
    launched
}

fn started() -> Result<std::sync::Arc<std::sync::atomic::AtomicBool>, String> {
    Ok(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
}

fn failed() -> Result<std::sync::Arc<std::sync::atomic::AtomicBool>, String> {
    Err("the player did not start".to_string())
}

/// The launch's own play started in Vela's window, its tracker could not be
/// started, and `playback::play` stopped it and left it in the slot.
fn own_play_started_and_stopped(state: &AppState, session: &str) {
    let mut play = PlayerHandle::Embedded(TestPlay::for_test_live(session));
    let _ = play.kill();
    wait_until_ended(&mut play);
    *state.current_child.lock().unwrap() = Some(play);
}

// ---- the four orders the reviewer gave (review finding pc-2) -------------

// Reviewer, comment 1: the old play's clean end is reported after its
// replacement failed. Its tracker says "not over" (a clean end), and the
// dispatcher used to answer "a successor is coming" because the ended play
// was not the active one, though no play was active at all.
#[tokio::test]
async fn reviewer_1_a_clean_end_reported_after_a_failed_replacement() {
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, false).await;
    // A reaches its clean end; before its tracker reports, a launch that
    // replaces it is installed, stops the (ended) player and fails.
    page.world.lock().unwrap().a_clean = true;
    assert!(
        !launch(&state, &page, "B", async {
            stop_old_player(&state, &page);
            failed()
        })
        .await
    );
    tracker_reports(&state, &page, "A", true);
    view(&state, &page, "A", false);
    assert_eq!(
        dispatcher_reports(&state, &page, "A").await,
        AfterCleanEnd::RunOver,
        "no play is active: nothing is coming"
    );
    let events = page.events();
    assert_page_left_rightly(&events, false, "reviewer 1");
    // The launch's own report comes right after A's view opened; without
    // it, the dispatcher's alone tells the page.
    assert_eq!(events[1].seen, ended("A", true));
    let mut without = events.clone();
    without.remove(1);
    assert_eq!(page_leaves_at(&without), Some(without.len() - 1));
}

// Reviewer, comment 2: the old play's clean end was reported before the
// replacement was installed; the replacement's own view opened, its play was
// stopped when its tracker could not be started, and the reaper emptied the
// slot before the launch looked. The view the page shows is the new play's.
#[tokio::test]
async fn reviewer_2_a_failed_launch_whose_view_opened_and_was_reaped() {
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, false).await;
    // A ends cleanly by itself and its tracker reports while it is still the
    // active play; the dispatcher authorises Continue Playing.
    stop_old_player(&state, &page);
    tracker_reports(&state, &page, "A", true);
    assert_eq!(
        dispatcher_reports(&state, &page, "A").await,
        AfterCleanEnd::ContinuePlaying
    );
    view(&state, &page, "A", false);
    // The page starts B.
    assert!(
        !launch(&state, &page, "B", async {
            stop_old_player(&state, &page);
            own_play_started_and_stopped(&state, "B");
            view(&state, &page, "B", true);
            reaper_tick(&state);
            assert!(state.current_child.lock().unwrap().is_none(), "the reaper emptied the slot");
            failed()
        })
        .await
    );
    view(&state, &page, "B", false);
    let events = page.events();
    assert!(
        events.iter().any(|e| e.seen == ended("B", true)),
        "the failed launch names the view the page shows: {events:#?}"
    );
    assert_page_left_rightly(&events, false, "reviewer 2");
}

// Reviewer, comment 3: two launches fail in a row. B fails before A's
// tracker reports; C is installed; A's tracker reports under C ("not
// over"); C fails.
#[tokio::test]
async fn reviewer_3_two_failed_launches_in_a_row() {
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, false).await;
    assert!(
        !launch(&state, &page, "B", async {
            stop_old_player(&state, &page);
            failed()
        })
        .await
    );
    assert!(
        !launch(&state, &page, "C", async {
            stop_old_player(&state, &page);
            tracker_reports(&state, &page, "A", false);
            failed()
        })
        .await
    );
    view(&state, &page, "A", false);
    let events = page.events();
    // A's own report, under C, says the run goes on.
    assert!(events.contains(&PageEvent {
        seen: ended("A", false),
        busy: true,
        pending: false
    }));
    // Without B's report (right after A's view opened), C's alone tells
    // the page.
    assert!(matches!(&events[1].seen, Seen::Ended { run_over: true, .. }));
    let mut without_b = events.clone();
    without_b.remove(1);
    assert_page_left_rightly(&without_b, false, "reviewer 3, C's report");
    assert_page_left_rightly(&events, false, "reviewer 3");
}

// Reviewer, comment 4: a play that fell back to mpv's own window inherits
// the run's fullscreen; its replacement fails after its tracker reported
// "not over". A real mpv-like process stands in for mpv's own window.
#[cfg(unix)]
#[tokio::test]
async fn reviewer_4_a_play_that_fell_back_to_mpv_own_window() {
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, true).await;
    let child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep is available");
    *state.current_child.lock().unwrap() = Some(PlayerHandle::External(
        crate::player::external::ManagedChild::for_test(child),
    ));
    assert!(
        !launch(&state, &page, "B", async {
            stop_old_player(&state, &page);
            tracker_reports(&state, &page, "A", false);
            failed()
        })
        .await
    );
    let events = page.events();
    assert_eq!(
        events.last().map(|e| &e.seen),
        Some(&ended("B", true)),
        "no view shows: the failed launch names its own session"
    );
    assert_page_left_rightly(&events, false, "reviewer 4");
}

// ---- every order of a stated set of events --------------------------------

/// The events of one scenario. A play "A" runs in the player's fullscreen
/// (or, `fell_back`, plays in mpv's own window after an embedded play of the
/// same run had the view and the fullscreen); a launch "B" replaces it; maybe
/// a second launch "C" follows B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// A's player reaches a clean end of file by itself.
    AEndsClean,
    /// A's tracker reports its end (clean or not, by how A ended).
    TrackerA,
    /// The completion dispatcher handles A's clean end and reports.
    DispatchA,
    /// A's view closes.
    CloseA,
    /// The reaper ticks once.
    Reap,
    /// B is installed (`install_and_launch` begins).
    BInstall,
    /// B's launch stops the player in the slot.
    BStop,
    /// B's own play starts in Vela's window (and, when B fails, is stopped
    /// again and left in the slot).
    BStart,
    /// B's view opens.
    BOpen,
    /// B's view closes.
    BClose,
    /// B's launch ends: it fails (and is undone and reports), or it starts.
    BEnd,
    /// C is installed, stops the player in the slot, and its launch ends
    /// (always a failure), in three steps.
    CInstall,
    CStop,
    CEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// B's launch stops A and fails.
    Fails,
    /// B's launch stops A, starts B in Vela's window, and fails after that.
    FailsAfterOwnView,
    /// B's launch fails before it stops A (the real launch never does; the
    /// rule must still hold for it).
    FailsBeforeStopping,
    /// B starts in Vela's window; its view may be announced before or after
    /// its launch returns.
    Starts,
    /// B starts in mpv's own window (it fell back): it has no view.
    StartsInOwnWindow,
}

impl Outcome {
    fn starts(self) -> bool {
        matches!(self, Outcome::Starts | Outcome::StartsInOwnWindow)
    }
}

#[derive(Debug, Clone, Copy)]
struct Scenario {
    fell_back: bool,
    a_ends_clean: bool,
    outcome: Outcome,
    then_c_fails: bool,
    /// B is a playlist's play: it carries a playlist position of its own, so
    /// the dispatcher's arm for "the position is another play's" answers
    /// without the play lock, and A's dispatch may come while B's launch
    /// runs, before B's view has opened.
    b_from_playlist: bool,
}

impl Scenario {
    fn steps(&self) -> Vec<Step> {
        use Step::*;
        let mut steps = vec![TrackerA, Reap, BInstall, BEnd];
        if self.a_ends_clean {
            steps.extend([AEndsClean, DispatchA]);
        }
        if !self.fell_back {
            steps.push(CloseA);
        }
        match self.outcome {
            Outcome::Fails => steps.push(BStop),
            Outcome::FailsAfterOwnView | Outcome::Starts => {
                steps.extend([BStop, BStart, BOpen]);
                if self.outcome == Outcome::FailsAfterOwnView {
                    steps.push(BClose);
                }
            }
            Outcome::StartsInOwnWindow => steps.extend([BStop, BStart]),
            Outcome::FailsBeforeStopping => {}
        }
        if self.then_c_fails {
            steps.extend([CInstall, CStop, CEnd]);
        }
        // A that is never stopped and never ends plays on: none of its end's
        // events happen.
        if !self.a_ends_clean && self.outcome == Outcome::FailsBeforeStopping && !self.then_c_fails {
            steps.retain(|s| !matches!(s, TrackerA | CloseA));
        }
        steps
    }

    /// Whether `step` may come next, after `done`.
    fn ready(&self, step: Step, done: &[Step]) -> bool {
        use Step::*;
        let has = |s: Step| done.contains(&s);
        let in_b = has(BInstall) && !has(BEnd);
        let in_c = has(CInstall) && !has(CEnd);
        let a_stopped_by_launch = has(BStop) || has(CStop);
        let a_ended = has(AEndsClean)
            || (self.outcome != Outcome::FailsBeforeStopping && has(BStop))
            || has(CStop);
        match step {
            // A clean end happens only while A still plays.
            AEndsClean => !a_stopped_by_launch,
            TrackerA | CloseA => a_ended,
            // The dispatcher's arms that take the play lock wait for a
            // launch to finish; with a playlist position of B's installed,
            // A's dispatch takes the arm that does not (and C carries none).
            DispatchA => has(TrackerA) && (!in_b || self.b_from_playlist) && !in_c,
            Reap => true,
            BInstall => true,
            BStop => has(BInstall),
            BStart => has(BStop),
            BOpen => has(BStart),
            BClose => has(BOpen),
            BEnd => {
                has(BInstall)
                    && match self.outcome {
                        Outcome::Fails => has(BStop),
                        // The play's view is announced on its own thread,
                        // before or after its launch returns.
                        Outcome::FailsAfterOwnView
                        | Outcome::Starts
                        | Outcome::StartsInOwnWindow => has(BStart),
                        Outcome::FailsBeforeStopping => true,
                    }
            }
            CInstall => has(BEnd),
            CStop => has(CInstall),
            CEnd => has(CStop),
        }
    }

    /// Every order of `steps()` that `ready` allows.
    fn orders(&self) -> Vec<Vec<Step>> {
        fn walk(s: &Scenario, left: &mut Vec<Step>, done: &mut Vec<Step>, out: &mut Vec<Vec<Step>>) {
            if left.is_empty() {
                out.push(done.clone());
                return;
            }
            for i in 0..left.len() {
                let step = left[i];
                if !s.ready(step, done) {
                    continue;
                }
                left.remove(i);
                done.push(step);
                walk(s, left, done, out);
                done.pop();
                left.insert(i, step);
            }
        }
        let mut out = Vec::new();
        walk(self, &mut self.steps(), &mut Vec::new(), &mut out);
        out
    }
}

/// Apply one step that is not a launch's beginning or end, nor a dispatch.
fn apply(state: &AppState, page: &Page, scenario: &Scenario, step: Step) {
    use Step::*;
    match step {
        AEndsClean => {
            let mut slot = state.current_child.lock().unwrap();
            let player = slot.as_mut().expect("A is in the slot");
            let _ = player.kill();
            wait_until_ended(player);
            let mut w = page.world.lock().unwrap();
            w.a_running = false;
            w.a_clean = true;
        }
        TrackerA => {
            let clean = page.world.lock().unwrap().a_clean;
            tracker_reports(state, page, "A", clean);
        }
        CloseA => view(state, page, "A", false),
        Reap => reaper_tick(state),
        BStop | CStop => stop_old_player(state, page),
        BStart => {
            if scenario.outcome.starts() {
                // Which player B runs in makes no difference to what is
                // read of it here; in mpv's own window it has no view.
                *state.current_child.lock().unwrap() =
                    Some(PlayerHandle::Embedded(TestPlay::for_test_live("B")));
                page.world.lock().unwrap().b_running = true;
            } else {
                own_play_started_and_stopped(state, "B");
            }
        }
        BOpen => view(state, page, "B", true),
        BClose => view(state, page, "B", false),
        DispatchA | BInstall | BEnd | CInstall | CEnd => unreachable!("not a simple step"),
    }
}

/// Run one order through the real functions; what the page was sent, and
/// whether anything is playing or coming at the end.
async fn run_order(scenario: &Scenario, order: &[Step]) -> (Vec<PageEvent>, bool) {
    use Step::*;
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, scenario.fell_back).await;
    let mut i = 0;
    while i < order.len() {
        match order[i] {
            DispatchA => {
                dispatcher_reports(&state, &page, "A").await;
            }
            BInstall | CInstall => {
                let (session, end) = if order[i] == BInstall { ("B", BEnd) } else { ("C", CEnd) };
                let j = i + order[i..].iter().position(|s| *s == end).expect("the launch ends");
                let inside = &order[i + 1..j];
                let starts = session == "B" && scenario.outcome.starts();
                let cursor = (session == "B" && scenario.b_from_playlist).then(|| playlist_cursor("B"));
                launch_play(&state, &page, session, cursor, async {
                    for step in inside {
                        if *step == DispatchA {
                            dispatcher_reports(&state, &page, "A").await;
                        } else {
                            apply(&state, &page, scenario, *step);
                        }
                    }
                    if starts {
                        started()
                    } else {
                        failed()
                    }
                })
                .await;
                i = j;
            }
            step => apply(&state, &page, scenario, step),
        }
        i += 1;
    }
    let (busy, _) = page.busy();
    // Leave no test play running.
    if let Some(mut player) = state.current_child.lock().unwrap().take() {
        let _ = player.kill();
    }
    (page.events(), busy)
}

// ---- a successor being chosen, or playing in mpv's own window -----------

/// How a source choice the dispatcher asked the user to make for A's
/// successor B ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChoiceEnd {
    /// The user cancels it.
    Cancel,
    /// It expired before the user chose.
    Expired,
    /// The user chose; the play was refused or failed before its session
    /// was installed.
    Refused,
    /// The user chose; B was installed and its launch failed.
    LaunchFails,
    /// The user chose; B starts in Vela's window.
    Starts,
}

/// A run whose play A, in Vela's window and the player's fullscreen, ends
/// cleanly, and whose successor the dispatcher asks the user to choose, or
/// starts in mpv's own window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chain {
    /// A's dispatch asks the user to choose B's source; the choice ends.
    Choice(ChoiceEnd),
    /// A's dispatch starts B in mpv's own window; B ends cleanly, and its
    /// dispatch starts C there too, or (`last`) cannot.
    OwnWindowEnds { last: bool },
    /// A's dispatch starts B in mpv's own window; a step-down puts D there in
    /// B's place; D ends cleanly, and its dispatch authorises Continue
    /// Playing, or (`last`) its successor cannot be started.
    OwnWindowSteppedDown { last: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    CloseA,
    TrackerA,
    Reap,
    DispatchA,
    ChoiceEnds,
    OpenB,
    BEndsClean,
    TrackerB,
    DispatchB,
    StepDown,
    DEndsClean,
    TrackerD,
    DispatchD,
}

impl Chain {
    fn links(self) -> Vec<Link> {
        use Link::*;
        let mut links = vec![CloseA, TrackerA, Reap, DispatchA];
        match self {
            Chain::Choice(end) => {
                links.push(ChoiceEnds);
                if end == ChoiceEnd::Starts {
                    links.push(OpenB);
                }
            }
            Chain::OwnWindowEnds { .. } => links.extend([BEndsClean, TrackerB, DispatchB]),
            Chain::OwnWindowSteppedDown { .. } => {
                links.extend([StepDown, TrackerB, DEndsClean, TrackerD, DispatchD])
            }
        }
        links
    }

    /// Whether `link` may come next, after `done`.
    fn ready(self, link: Link, done: &[Link]) -> bool {
        use Link::*;
        let has = |l: Link| done.contains(&l);
        match link {
            CloseA | TrackerA | Reap => true,
            DispatchA => has(TrackerA),
            ChoiceEnds | BEndsClean | StepDown => has(DispatchA),
            OpenB => has(ChoiceEnds),
            TrackerB => has(BEndsClean) || has(StepDown),
            DispatchB => has(TrackerB),
            DEndsClean => has(StepDown),
            TrackerD => has(DEndsClean),
            DispatchD => has(TrackerD),
        }
    }

    /// Every order of `links()` that `ready` allows.
    fn orders(self) -> Vec<Vec<Link>> {
        fn walk(c: Chain, left: &mut Vec<Link>, done: &mut Vec<Link>, out: &mut Vec<Vec<Link>>) {
            if left.is_empty() {
                out.push(done.clone());
                return;
            }
            for i in 0..left.len() {
                let link = left[i];
                if !c.ready(link, done) {
                    continue;
                }
                left.remove(i);
                done.push(link);
                walk(c, left, done, out);
                done.pop();
                left.insert(i, link);
            }
        }
        let mut out = Vec::new();
        walk(self, &mut self.links(), &mut Vec::new(), &mut out);
        out
    }
}

fn chains() -> Vec<Chain> {
    let mut all: Vec<Chain> = [
        ChoiceEnd::Cancel,
        ChoiceEnd::Expired,
        ChoiceEnd::Refused,
        ChoiceEnd::LaunchFails,
        ChoiceEnd::Starts,
    ]
    .into_iter()
    .map(Chain::Choice)
    .collect();
    for last in [false, true] {
        all.push(Chain::OwnWindowEnds { last });
        all.push(Chain::OwnWindowSteppedDown { last });
    }
    all
}

/// Whether `session`'s player runs, in the test's world.
fn set_running(page: &Page, session: &str, running: bool) {
    let mut w = page.world.lock().unwrap();
    match session {
        "A" => w.a_running = running,
        "B" => w.b_running = running,
        "C" => w.c_running = running,
        "D" => w.d_running = running,
        other => panic!("no play {other}"),
    }
}

/// The player in the slot, `session`'s, reaches a clean end of file.
fn ends_clean(state: &AppState, page: &Page, session: &str) {
    let mut slot = state.current_child.lock().unwrap();
    let player = slot.as_mut().expect("the play is in the slot");
    let _ = player.kill();
    wait_until_ended(player);
    drop(slot);
    set_running(page, session, false);
}

/// A launch stops the player in the slot, `session`'s, and queues it for
/// the reaper; then `next` starts in its place.
fn replace_in_slot(state: &AppState, page: &Page, session: &str, next: &str) {
    stop_old_player(state, page);
    set_running(page, session, false);
    *state.current_child.lock().unwrap() = Some(PlayerHandle::Embedded(TestPlay::for_test_live(next)));
    set_running(page, next, true);
}

fn completion(session: &str) -> PlaybackCompletion {
    PlaybackCompletion {
        session_id: session.to_string(),
        item_key: "test:k".to_string(),
        watch_key: None,
        started_at_ms: 0,
        watch_backings: Vec::new(),
        media_type: None,
    }
}

/// The dispatcher's playlist arm for `ended`'s clean end starts `next` (in
/// mpv's own window: it has no view) through the real `install_and_launch`
/// and answers with the real `after_successor`; its report as `lib.rs` sends
/// it.
async fn dispatch_starts(state: &AppState, page: &Page, ended: &str, next: &str) {
    assert!(
        launch_play(state, page, next, Some(playlist_cursor(next)), async {
            replace_in_slot(state, page, ended, next);
            started()
        })
        .await
    );
    let after = after_successor(
        state,
        PlayCommandResult::Started {
            session_id: next.to_string(),
        },
    )
    .await;
    page.ended(&clean_end_report(&completion(ended), after));
}

/// The dispatcher's playlist arm for `ended`'s clean end could not start its
/// successor (the real `successor_failed`); its report.
async fn dispatch_fails(state: &AppState, page: &Page, ended: &str) {
    let after = successor_failed(state, ended).await;
    page.ended(&clean_end_report(&completion(ended), after));
}

const CHOICE: &str = "Q";

/// A source choice for the play `session`, to follow `replaces`, held as
/// `play_by_key_locked` holds it when the user is to choose.
async fn offer_choice(state: &AppState, replaces: &str, session: &str) -> PlaybackSourceChoiceRequestDto {
    let request = PlaybackSourceChoiceRequestDto {
        request_id: CHOICE.to_string(),
        title: "k".to_string(),
        choices: vec![PlaybackSourceChoiceDto {
            source_id: "s".to_string(),
            source_name: "S".to_string(),
            locality: "lan".to_string(),
            quality_label: "HD".to_string(),
        }],
    };
    state.playback_choices.lock().await.insert_at(
        PendingPlaybackChoice {
            request: request.clone(),
            created_at: Instant::now(),
            item: item(),
            start_from_beginning: false,
            session_id: session.to_string(),
            playlist: None,
            replace_session: Some(replaces.to_string()),
            run_kind: Some(PlaybackRunKind::VelaPlaylist),
        },
        Instant::now(),
    );
    request
}

/// The dispatcher's playlist arm for A's clean end found that the user must
/// choose B's source: the choice is held, and the real `after_successor`
/// answers; its report as `lib.rs` sends it.
async fn dispatch_asks_choice(state: &AppState, page: &Page) -> AfterCleanEnd {
    let request = offer_choice(state, "A", "B").await;
    page.world.lock().unwrap().coming = true;
    let after = after_successor(state, PlayCommandResult::SourceChoiceRequired { request }).await;
    page.ended(&clean_end_report(&completion("A"), after));
    after
}

fn never_played() -> PlayFailure {
    panic!("a choice that is no longer held starts no play")
}

/// The choice ends as `end` says, through the real `cancel_choice` and
/// `resolve_choice`; a play the user chose is started (or not) by a stand-in
/// for `play_by_key_locked` that runs the real `install_and_launch`.
async fn end_choice(state: &AppState, page: &Page, end: ChoiceEnd) {
    let report = |e: serde_json::Value| page.ended(&e);
    match end {
        ChoiceEnd::Cancel => {
            page.world.lock().unwrap().coming = false;
            assert!(cancel_choice(state, CHOICE, report).await);
        }
        ChoiceEnd::Expired => {
            // As the choices' time limit drops it.
            state.playback_choices.lock().await.clear();
            page.world.lock().unwrap().coming = false;
            let result = resolve_choice(state, CHOICE, "s", |_, _| async { Err(never_played()) }, report).await;
            assert!(result.is_err());
        }
        ChoiceEnd::Refused => {
            let result = resolve_choice(
                state,
                CHOICE,
                "s",
                |_, _| async {
                    page.world.lock().unwrap().coming = false;
                    Err(PlayFailure::unavailable("the source is gone".to_string()))
                },
                report,
            )
            .await;
            assert!(result.is_err());
        }
        ChoiceEnd::LaunchFails | ChoiceEnd::Starts => {
            let starts = end == ChoiceEnd::Starts;
            let result = resolve_choice(
                state,
                CHOICE,
                "s",
                |pending, _| async move {
                    launch_locked(state, page, &pending.session_id, None, async {
                        page.world.lock().unwrap().coming = false;
                        if starts {
                            replace_in_slot(state, page, "A", "B");
                            started()
                        } else {
                            stop_old_player(state, page);
                            failed()
                        }
                    })
                    .await
                    .map(|_| PlayCommandResult::Started {
                        session_id: pending.session_id.clone(),
                    })
                    .map_err(PlayFailure::launch_failed)
                },
                report,
            )
            .await;
            assert_eq!(result.is_ok(), starts);
        }
    }
}

/// Run one order of a chain through the real functions; what the page was
/// sent, and whether anything is playing or coming at the end.
async fn run_chain(chain: Chain, order: &[Link]) -> (Vec<PageEvent>, bool) {
    use Link::*;
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, false).await;
    ends_clean(&state, &page, "A");
    page.world.lock().unwrap().a_clean = true;
    for link in order {
        match (*link, chain) {
            (CloseA, _) => view(&state, &page, "A", false),
            (TrackerA, _) => tracker_reports(&state, &page, "A", true),
            (Reap, _) => reaper_tick(&state),
            (DispatchA, Chain::Choice(_)) => {
                dispatch_asks_choice(&state, &page).await;
            }
            (DispatchA, _) => dispatch_starts(&state, &page, "A", "B").await,
            (ChoiceEnds, Chain::Choice(end)) => end_choice(&state, &page, end).await,
            (OpenB, _) => view(&state, &page, "B", true),
            (BEndsClean, _) => ends_clean(&state, &page, "B"),
            (TrackerB, chain) => {
                let clean = matches!(chain, Chain::OwnWindowEnds { .. });
                tracker_reports(&state, &page, "B", clean)
            }
            (DispatchB, Chain::OwnWindowEnds { last: true }) => dispatch_fails(&state, &page, "B").await,
            (DispatchB, _) => dispatch_starts(&state, &page, "B", "C").await,
            (StepDown, _) => {
                assert!(
                    launch_play(&state, &page, "D", None, async {
                        replace_in_slot(&state, &page, "B", "D");
                        started()
                    })
                    .await
                );
            }
            (DEndsClean, _) => ends_clean(&state, &page, "D"),
            (TrackerD, _) => tracker_reports(&state, &page, "D", true),
            (DispatchD, Chain::OwnWindowSteppedDown { last: true }) => dispatch_fails(&state, &page, "D").await,
            (DispatchD, _) => {
                assert_eq!(
                    dispatcher_reports(&state, &page, "D").await,
                    AfterCleanEnd::ContinuePlaying
                );
            }
            (link, chain) => unreachable!("{link:?} is not a step of {chain:?}"),
        }
    }
    let (busy, _) = page.busy();
    if let Some(mut player) = state.current_child.lock().unwrap().take() {
        let _ = player.kill();
    }
    (page.events(), busy)
}

fn scenarios() -> Vec<Scenario> {
    let mut all = Vec::new();
    for fell_back in [false, true] {
        for a_ends_clean in [false, true] {
            for outcome in [
                Outcome::Fails,
                Outcome::FailsAfterOwnView,
                Outcome::FailsBeforeStopping,
                Outcome::Starts,
                Outcome::StartsInOwnWindow,
            ] {
                for then_c_fails in [false, true] {
                    // A second failed launch follows only a first that
                    // failed without a view of its own: with one, the
                    // orders grow past a hundred thousand, and the second
                    // launch adds nothing the first's view needs.
                    if then_c_fails
                        && (outcome.starts() || outcome == Outcome::FailsAfterOwnView)
                    {
                        continue;
                    }
                    for b_from_playlist in [false, true] {
                        // Only a clean end is dispatched. A dispatch inside
                        // B's launch is run without C after it: C carries no
                        // playlist position, so A's dispatch never comes
                        // inside C's launch, and after B's launch the
                        // orders are those of a B with no position.
                        if b_from_playlist && (!a_ends_clean || then_c_fails) {
                            continue;
                        }
                        all.push(Scenario {
                            fell_back,
                            a_ends_clean,
                            outcome,
                            then_c_fails,
                            b_from_playlist,
                        });
                    }
                }
            }
        }
    }
    all
}

/// One page-visible sequence, as a line of `tests/fixtures/run-over-orders.json`:
/// each event a token (`o` opened, `c` closed, `e` ended, then the session,
/// for an end without `successor` `1` when it says the run is over and `0`
/// when not, for the dispatcher's report, which carries `successor`, `R`
/// when it says the run is over, `s` when it says a successor is coming, and
/// `n` when it says neither; then `+`
/// when something was playing or coming as it was sent and `-` when not;
/// then `p` when the page was still starting a play of its own and `.` when
/// not), and after `|`, `+` or `-` for whether anything plays or is coming at
/// the end.
fn sequence_line(events: &[PageEvent], final_busy: bool) -> String {
    let flag = |on: bool| if on { '+' } else { '-' };
    let mut line: Vec<String> = events
        .iter()
        .map(|e| {
            let what = match &e.seen {
                Seen::Opened(s) => format!("o{s}"),
                Seen::Closed(s) => format!("c{s}"),
                Seen::Ended {
                    session,
                    run_over,
                    successor,
                } => {
                    let says = match (*run_over, *successor) {
                        (true, None) => '1',
                        (false, None) => '0',
                        (true, Some(_)) => 'R',
                        (false, Some(true)) => 's',
                        (false, Some(false)) => 'n',
                    };
                    format!("e{session}{says}")
                }
            };
            format!("{what}{}{}", flag(e.busy), if e.pending { 'p' } else { '.' })
        })
        .collect();
    line.push(format!("|{}", flag(final_busy)));
    serde_json::json!(line.join(" ")).to_string()
}

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/run-over-orders.json");

// Every order of each scenario's events (`Scenario::steps`, constrained only
// by what must come before what, `Scenario::ready`), and of each chain's
// (`Chain::links`, `Chain::ready`: a successor the dispatcher asks the user
// to choose, and successors in mpv's own window, one after another or
// stepped down), run through the real backend functions. In every order the page, applying its own rule, leaves
// the player's fullscreen at a report once nothing plays or is coming, and
// never while something plays or is coming. The distinct sequences the page
// receives are kept in `tests/fixtures/run-over-orders.json`, which the Node
// test replays through the page's real helpers; this test fails when what
// the backend sends no longer matches that file (to write it anew, run this
// test with VELA_WRITE_RUN_OVER_ORDERS=1, and read the difference).
//
// The ground truth (`World`: what plays or is coming) is this test's model,
// and the Node test takes it from the fixture: the two check the page's rule
// against the same model, not against each other. Time is not modelled: the
// page's backstop, which leaves fullscreen 50 s after the last view closed,
// never runs here.
//
// The orders are shared out over threads, each running its share on a
// runtime of its own, one order after another on a fresh `AppState`.
/// One order to run: of a launch scenario's steps, or of a chain's links.
enum Work {
    Launches(Scenario, Vec<Step>),
    Successors(Chain, Vec<Link>),
}

#[test]
fn in_every_order_the_page_leaves_fullscreen_exactly_when_the_run_is_over() {
    let mut work: Vec<Work> = scenarios()
        .into_iter()
        .flat_map(|scenario| {
            scenario
                .orders()
                .into_iter()
                .map(move |order| Work::Launches(scenario, order))
        })
        .collect();
    let launch_orders = work.len();
    work.extend(chains().into_iter().flat_map(|chain| {
        chain
            .orders()
            .into_iter()
            .map(move |order| Work::Successors(chain, order))
    }));
    let chain_orders = work.len() - launch_orders;
    let orders_run = work.len();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let share = orders_run.div_ceil(threads);
    let mut distinct = std::collections::BTreeSet::new();
    std::thread::scope(|scope| {
        let running: Vec<_> = work
            .chunks(share)
            .map(|chunk| {
                scope.spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("a runtime for the test");
                    let mut seen = std::collections::BTreeSet::new();
                    for item in chunk {
                        let (events, busy, what) = match item {
                            Work::Launches(scenario, order) => {
                                let (events, busy) = runtime.block_on(run_order(scenario, order));
                                (events, busy, format!("{scenario:?} {order:?}"))
                            }
                            Work::Successors(chain, order) => {
                                let (events, busy) = runtime.block_on(run_chain(*chain, order));
                                (events, busy, format!("{chain:?} {order:?}"))
                            }
                        };
                        assert_page_left_rightly(&events, busy, &what);
                        seen.insert(sequence_line(&events, busy));
                    }
                    seen
                })
            })
            .collect();
        for thread in running {
            distinct.extend(thread.join().expect("every order keeps the rule"));
        }
    });
    eprintln!(
        "{orders_run} orders ({launch_orders} of launches, {chain_orders} of successors), {} distinct sequences",
        distinct.len()
    );
    assert!(launch_orders > 10_000, "{launch_orders} orders");
    assert!(chain_orders > 100, "{chain_orders} orders");
    let mut text = String::from("[\n");
    let lines: Vec<String> = distinct.into_iter().collect();
    text.push_str(&lines.join(",\n"));
    text.push_str("\n]\n");
    if std::env::var_os("VELA_WRITE_RUN_OVER_ORDERS").is_some() {
        std::fs::write(FIXTURE, &text).expect("the fixture can be written");
    }
    let kept = std::fs::read_to_string(FIXTURE).expect("the fixture exists");
    assert!(
        kept == text,
        "what the backend sends the page no longer matches {FIXTURE} ({orders_run} orders, {} sequences)",
        lines.len()
    );
}

// ---- the pieces, one by one ----------------------------------------------

#[test]
fn a_failed_launch_ends_the_run_only_when_all_three_hold() {
    for nothing in [false, true] {
        for active in [false, true] {
            for held in [false, true] {
                assert_eq!(
                    failed_launch_ends_run(nothing, active, held),
                    nothing && !active && held,
                    "{nothing} {active} {held}"
                );
            }
        }
    }
}

#[test]
fn nothing_plays_unless_the_player_found_before_the_launch_still_runs() {
    let mut old = Some(PlayerHandle::Embedded(TestPlay::for_test_live("A")));
    let before = old.as_ref().map(PlayerHandle::serial);
    // Still in the slot and running: the launch failed before stopping it.
    assert!(!nothing_playing(&mut old, before));
    // A slot that was empty before, now holding a running player, is not
    // this case, but it is no player the launch found either.
    assert!(nothing_playing(&mut old, None));
    // Ended.
    if let Some(player) = old.as_mut() {
        let _ = player.kill();
        wait_until_ended(player);
    }
    assert!(old.is_some(), "an ended player stays until the reaper takes it");
    assert!(nothing_playing(&mut old, before));
    // Empty.
    assert!(nothing_playing(&mut None, before));
    // A player the launch started itself (another serial), whatever its state.
    let mut own = Some(PlayerHandle::Embedded(TestPlay::for_test_live("B")));
    assert!(nothing_playing(&mut own, before));
    if let Some(player) = own.as_mut() {
        let _ = player.kill();
    }
}

#[test]
fn every_player_has_a_serial_of_its_own() {
    let a = PlayerHandle::Embedded(TestPlay::for_test("A"));
    let b = PlayerHandle::Embedded(TestPlay::for_test("A"));
    assert_ne!(a.serial(), b.serial());
}

#[test]
fn the_view_record_follows_the_page_and_names_the_view_showing() {
    let record = crate::player::ViewRecord::default();
    let named = |record: &crate::player::ViewRecord| {
        let mut name = String::new();
        record.report("own", |session| name = session.to_string());
        name
    };
    assert_eq!(named(&record), "own");
    let mut sent = 0;
    record.announce("a", true, || sent += 1);
    assert_eq!(named(&record), "a");
    record.announce("b", true, || sent += 1);
    // A replaced play's late close does not hide the new play's view.
    record.announce("a", false, || sent += 1);
    assert_eq!(named(&record), "b");
    record.announce("b", false, || sent += 1);
    assert_eq!(named(&record), "own");
    assert_eq!(sent, 4, "every change is sent");
}

#[test]
fn the_page_claims_fullscreen_only_for_the_running_embedded_play() {
    let state = AppState::for_tests();
    let held = |state: &AppState| state.player_fullscreen_held.load(Ordering::SeqCst);
    let refused = |held: bool| FullscreenClaim {
        ask_window: false,
        held,
    };
    let granted = FullscreenClaim {
        ask_window: true,
        held: true,
    };
    let off = FullscreenClaim {
        ask_window: true,
        held: false,
    };
    // Nothing plays.
    assert_eq!(claim_player_fullscreen(&state, &serde_json::json!("A"), true), refused(false));
    assert!(!held(&state));
    *state.current_child.lock().unwrap() = Some(PlayerHandle::Embedded(TestPlay::for_test_live("A")));
    // Another play's view.
    assert_eq!(claim_player_fullscreen(&state, &serde_json::json!("B"), true), refused(false));
    assert!(!held(&state));
    assert_eq!(claim_player_fullscreen(&state, &serde_json::json!("A"), true), granted);
    assert!(held(&state));
    // A refused "on" leaves a claim already held as it is, and says so: the
    // page takes its own claim from that answer.
    assert_eq!(claim_player_fullscreen(&state, &serde_json::json!("B"), true), refused(true));
    assert!(held(&state));
    // Off always goes through, and gives the claim up.
    assert_eq!(claim_player_fullscreen(&state, &serde_json::json!("whatever"), false), off);
    assert!(!held(&state));
    // A play that was told to stop no longer may.
    if let Some(player) = state.current_child.lock().unwrap().as_mut() {
        let _ = player.kill();
    }
    assert_eq!(claim_player_fullscreen(&state, &serde_json::json!("A"), true), refused(false));
    assert!(!held(&state));
}

// Finding pc-2: the page asks for fullscreen for a play that has just been
// told to stop. The backend
// refuses; its answer says the page holds nothing, so the page's claim is
// given up (the page side is `fullscreenRequests`, tested in Node). A launch
// that then fails has nothing to report, and nothing needs it: neither side
// holds fullscreen, and the window was never asked.
#[tokio::test]
async fn a_refused_request_for_fullscreen_is_answered_as_holding_nothing() {
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, false).await;
    assert!(claim_player_fullscreen(&state, &serde_json::json!(null), false).ask_window);
    if let Some(player) = state.current_child.lock().unwrap().as_mut() {
        let _ = player.kill();
    }
    let answer = claim_player_fullscreen(&state, &serde_json::json!("A"), true);
    assert!(!answer.ask_window && !answer.held, "{answer:?}");
}

// The view record's report and its events are each sent with the record's
// lock held, so that one never comes between another's read and its send.
// (Finding pc-2: the report names the view the page shows, and no opening or
// closing may come between the name it read and the report.) Checked inside
// each send by asking whether the lock is held, which does not depend on how
// fast another thread runs.
#[test]
fn the_view_record_sends_its_report_and_its_events_one_at_a_time() {
    let record = crate::player::ViewRecord::default();
    assert!(!record.is_locked());
    let mut sends = 0;
    record.report("own", |_| {
        assert!(record.is_locked(), "a report was sent without the record's lock");
        sends += 1;
    });
    record.announce("B", true, || {
        assert!(record.is_locked(), "an opening was sent without the record's lock");
        sends += 1;
    });
    record.announce("B", false, || {
        assert!(record.is_locked(), "a closing was sent without the record's lock");
        sends += 1;
    });
    assert_eq!(sends, 3);
    assert!(!record.is_locked());
}

// `successor_failed`: the run's successor could not be started. Another play
// active by then is the successor; with none, or with the ended play still
// the active one, the run is over. (The dispatcher's answers the run-over
// rule depends on, finding pc-2.)
#[tokio::test]
async fn a_successor_that_failed_ends_the_run_unless_another_play_is_active() {
    let state = AppState::for_tests();
    assert_eq!(successor_failed(&state, "A").await, AfterCleanEnd::RunOver);
    *state.active_playback_session.lock().await = Some("A".to_string());
    assert_eq!(successor_failed(&state, "A").await, AfterCleanEnd::RunOver);
    *state.active_playback_session.lock().await = Some("B".to_string());
    assert_eq!(successor_failed(&state, "A").await, AfterCleanEnd::Successor);
    // Its cursor is cleared only when it is the ended play's.
    *state.playlist_cursor.lock().await = Some(playlist_cursor("B"));
    assert_eq!(successor_failed(&state, "A").await, AfterCleanEnd::Successor);
    assert!(state.playlist_cursor.lock().await.is_some());
}

// The dispatcher's report says what `advance_playlist` answered, as flags:
// over, a successor coming, or neither (Continue Playing is sent apart).
#[test]
fn the_dispatchers_report_says_over_or_successor_coming() {
    let completion = PlaybackCompletion {
        session_id: "A".to_string(),
        item_key: "test:k".to_string(),
        watch_key: None,
        started_at_ms: 0,
        watch_backings: Vec::new(),
        media_type: None,
    };
    let report = |after| clean_end_report(&completion, after);
    assert_eq!(
        report(AfterCleanEnd::RunOver),
        serde_json::json!({ "sourceId": "test", "itemKey": "test:k", "sessionId": "A", "runOver": true, "successor": false })
    );
    assert_eq!(
        report(AfterCleanEnd::Successor),
        serde_json::json!({ "sourceId": "test", "itemKey": "test:k", "sessionId": "A", "runOver": false, "successor": true })
    );
    assert_eq!(
        report(AfterCleanEnd::ContinuePlaying),
        serde_json::json!({ "sourceId": "test", "itemKey": "test:k", "sessionId": "A", "runOver": false, "successor": false })
    );
}

// Finding pc-2: a run's next play started by the backend, while A's view has closed and before B's view has opened (B
// is still starting, or its view is announced only after its launch
// returned, or it plays in mpv's own window). The dispatcher's report says a
// successor is coming, and the page keeps fullscreen.
#[tokio::test]
async fn a_successor_whose_view_has_not_opened_keeps_fullscreen() {
    for own_window in [false, true] {
        let state = AppState::for_tests();
        let page = Page::default();
        run_in_fullscreen(&state, &page, false).await;
        stop_old_player(&state, &page);
        page.world.lock().unwrap().a_clean = true;
        view(&state, &page, "A", false);
        tracker_reports(&state, &page, "A", true);
        // B, a playlist's play, is installed; A's dispatch answers while B
        // still starts.
        assert!(
            launch_play(&state, &page, "B", Some(playlist_cursor("B")), async {
                assert_eq!(
                    dispatcher_reports(&state, &page, "A").await,
                    AfterCleanEnd::Successor
                );
                *state.current_child.lock().unwrap() =
                    Some(PlayerHandle::Embedded(TestPlay::for_test_live("B")));
                page.world.lock().unwrap().b_running = true;
                started()
            })
            .await
        );
        if !own_window {
            view(&state, &page, "B", true);
        }
        let events = page.events();
        assert_eq!(page_leaves_at(&events), None, "{events:#?}");
        assert_page_left_rightly(&events, true, "a successor still starting");
        let left = state.current_child.lock().unwrap().take();
        if let Some(mut player) = left {
            let _ = player.kill();
        }
    }
}

// Somebody who only ever uses mpv's own window: the page never holds the
// player's fullscreen, and a failed launch sends nothing new.
#[cfg(unix)]
#[tokio::test]
async fn a_failed_launch_sends_nothing_new_to_somebody_who_uses_mpv_own_window() {
    let state = AppState::for_tests();
    let page = Page::default();
    install_playback_session(&state, "A", &item(), None, None).await;
    *state.active_playback_player.lock().await =
        Some(("A".to_string(), crate::player::PlayerKind::External));
    let child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep is available");
    *state.current_child.lock().unwrap() = Some(PlayerHandle::External(
        crate::player::external::ManagedChild::for_test(child),
    ));
    assert!(
        !launch(&state, &page, "B", async {
            stop_old_player(&state, &page);
            failed()
        })
        .await
    );
    assert_eq!(page.events(), vec![]);
}

// The dispatcher's answers that changed: with no play active, nothing is
// coming. Where Continue Playing is authorised does not change.
#[tokio::test]
async fn the_dispatcher_says_the_run_is_over_when_no_play_is_active() {
    let state = AppState::for_tests();
    // No cursor, no play active.
    assert_eq!(advance_playlist(&state, "A").await, AfterCleanEnd::RunOver);
    // No cursor, the ended play active: Continue Playing.
    *state.active_playback_session.lock().await = Some("A".to_string());
    assert_eq!(advance_playlist(&state, "A").await, AfterCleanEnd::ContinuePlaying);
    // No cursor, another play active: it is the successor.
    *state.active_playback_session.lock().await = Some("B".to_string());
    assert_eq!(advance_playlist(&state, "A").await, AfterCleanEnd::Successor);
    // A cursor of another play.
    *state.playlist_cursor.lock().await = Some(PlaylistCursor {
        owner: PlaylistOwner::Vela,
        playlist_id: "p".to_string(),
        entry_id: "e".to_string(),
        index: 0,
        session_id: "B".to_string(),
    });
    assert_eq!(advance_playlist(&state, "A").await, AfterCleanEnd::Successor);
    *state.active_playback_session.lock().await = None;
    assert_eq!(advance_playlist(&state, "A").await, AfterCleanEnd::RunOver);
    // A successor that was refused because the ended play was replaced.
    assert_eq!(
        after_successor(&state, PlayCommandResult::Superseded).await,
        AfterCleanEnd::RunOver
    );
    *state.active_playback_session.lock().await = Some("C".to_string());
    assert_eq!(
        after_successor(&state, PlayCommandResult::Superseded).await,
        AfterCleanEnd::Successor
    );
    assert_eq!(
        after_successor(
            &state,
            PlayCommandResult::Started {
                session_id: "C".to_string()
            }
        )
        .await,
        AfterCleanEnd::Successor
    );
}

// Another play active once the failed launch is undone: something plays or
// is coming, and nothing is reported. (Under the play lock no other launch
// installs one; the rule must still hold.)
#[tokio::test]
async fn a_failed_launch_reports_nothing_while_another_play_is_active() {
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, false).await;
    let before = page.events().len();
    assert!(
        !launch(&state, &page, "B", async {
            stop_old_player(&state, &page);
            install_playback_session(&state, "C", &item(), None, None).await;
            failed()
        })
        .await
    );
    assert_eq!(page.events().len(), before, "{:#?}", page.events());
}

// The launch's own play, told to stop when its tracking could not be
// started, may not have ended yet when the launch looks: it plays nothing.
#[test]
fn a_play_the_launch_started_and_stopped_plays_nothing_before_it_has_ended() {
    let mut own = Some(PlayerHandle::Embedded(TestPlay::for_test_slow_to_stop("B")));
    let before = Some(crate::player::new_serial());
    if let Some(player) = own.as_mut() {
        let _ = player.kill();
        assert!(!matches!(player.has_ended(), Ok(true)), "still ending");
    }
    assert!(nothing_playing(&mut own, before));
}

// ---- a successor being chosen (review finding pc-2) -----------------------

#[test]
fn a_choice_ends_the_run_only_when_all_three_hold() {
    for nothing in [false, true] {
        for pending in [false, true] {
            for held in [false, true] {
                assert_eq!(
                    choice_end_ends_run(nothing, pending, held),
                    nothing && !pending && held,
                    "{nothing} {pending} {held}"
                );
            }
        }
    }
}

/// A in the player's fullscreen ends cleanly, its view closes, its tracker
/// reports, and the dispatcher asks the user to choose its successor's
/// source.
async fn successor_being_chosen(state: &AppState, page: &Page) {
    run_in_fullscreen(state, page, false).await;
    ends_clean(state, page, "A");
    page.world.lock().unwrap().a_clean = true;
    view(state, page, "A", false);
    tracker_reports(state, page, "A", true);
    assert_eq!(
        dispatch_asks_choice(state, page).await,
        AfterCleanEnd::Successor,
        "a successor being chosen is coming: the page keeps fullscreen while the user chooses"
    );
    let events = page.events();
    assert_eq!(page_leaves_at(&events), None, "{events:#?}");
}

// The dispatcher asks the user to choose the source of the run's next play;
// the page keeps fullscreen while the dialog is up. The user cancels it, or
// the choice has expired, or the play chosen is refused before it is
// installed: nothing plays or is coming, and the backend says the run is
// over, naming the play the choice was to follow (or, for a choice no longer
// held, its request: no view shows, and the page leaves whatever the name).
#[tokio::test]
async fn a_successor_being_chosen_keeps_fullscreen_and_a_choice_ended_without_a_play_ends_the_run() {
    for end in [ChoiceEnd::Cancel, ChoiceEnd::Expired, ChoiceEnd::Refused] {
        let state = AppState::for_tests();
        let page = Page::default();
        successor_being_chosen(&state, &page).await;
        end_choice(&state, &page, end).await;
        let events = page.events();
        let named = if end == ChoiceEnd::Expired { CHOICE } else { "A" };
        assert_eq!(events.last().map(|e| &e.seen), Some(&ended(named, true)), "{end:?}: {events:#?}");
        assert_page_left_rightly(&events, false, &format!("{end:?}"));
        // The playlist's run is given up with the choice.
        if end == ChoiceEnd::Cancel {
            assert!(state.playback_run.lock().await.is_none());
        }
    }
}

// The play chosen is installed and its launch fails: that launch reports the
// end of the run (naming its own play, as no view shows), and the choice's
// end adds nothing.
#[tokio::test]
async fn a_choice_whose_launch_failed_is_reported_once() {
    let state = AppState::for_tests();
    let page = Page::default();
    successor_being_chosen(&state, &page).await;
    let before = page.events().len();
    end_choice(&state, &page, ChoiceEnd::LaunchFails).await;
    let events = page.events();
    assert_eq!(events[before..].len(), 1, "{events:#?}");
    assert_eq!(events[before].seen, ended("B", true));
    assert_page_left_rightly(&events, false, "a chosen play that failed");
}

// A choice that ends while another still waits for the user, or while a play
// runs, leaves something coming or playing: nothing is reported.
#[tokio::test]
async fn a_choice_ending_while_another_waits_or_a_play_runs_reports_nothing() {
    // Another choice waits.
    let state = AppState::for_tests();
    let page = Page::default();
    successor_being_chosen(&state, &page).await;
    state.playback_choices.lock().await.insert_at(
        PendingPlaybackChoice {
            request: PlaybackSourceChoiceRequestDto {
                request_id: "second".to_string(),
                title: "k".to_string(),
                choices: Vec::new(),
            },
            created_at: Instant::now(),
            item: item(),
            start_from_beginning: false,
            session_id: "C".to_string(),
            playlist: None,
            replace_session: Some("A".to_string()),
            run_kind: None,
        },
        Instant::now(),
    );
    let before = page.events().len();
    assert!(cancel_choice(&state, CHOICE, |e| page.ended(&e)).await);
    assert_eq!(page.events().len(), before, "another choice waits");
    // A play runs.
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, false).await;
    offer_choice(&state, "A", "B").await;
    let before = page.events().len();
    assert!(cancel_choice(&state, CHOICE, |e| page.ended(&e)).await);
    let result = resolve_choice(&state, "gone", "s", |_, _| async { Err(never_played()) }, |e| page.ended(&e)).await;
    assert!(result.is_err());
    assert_eq!(page.events().len(), before, "A plays");
}

// Somebody who only ever uses mpv's own window: the page never holds the
// player's fullscreen, and a choice's end sends nothing new.
#[tokio::test]
async fn a_choice_sends_nothing_new_to_somebody_who_uses_mpv_own_window() {
    for end in [ChoiceEnd::Cancel, ChoiceEnd::Expired, ChoiceEnd::Refused] {
        let state = AppState::for_tests();
        let page = Page::default();
        offer_choice(&state, "A", "B").await;
        end_choice(&state, &page, end).await;
        assert_eq!(page.events(), vec![], "{end:?}");
    }
}

// ---- a successor in mpv's own window (review finding pc-2) ----------------

// The run's next play B, started by the dispatcher, plays in mpv's own
// window: no view opens for it. Its own first report, after A's two, does not
// end the run: B ends cleanly and its dispatcher starts C there too, or a
// step-down replaces B by D there. Only a report that says the run is over
// leaves.
#[tokio::test]
async fn a_successor_in_mpv_own_window_keeps_fullscreen_until_the_run_is_over() {
    // B ends cleanly; C follows.
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, false).await;
    ends_clean(&state, &page, "A");
    view(&state, &page, "A", false);
    tracker_reports(&state, &page, "A", true);
    dispatch_starts(&state, &page, "A", "B").await;
    ends_clean(&state, &page, "B");
    tracker_reports(&state, &page, "B", true);
    dispatch_starts(&state, &page, "B", "C").await;
    let events = page.events();
    assert_eq!(page_leaves_at(&events), None, "{events:#?}");
    assert_page_left_rightly(&events, true, "C plays");
    // C's successor cannot be started: the run is over.
    ends_clean(&state, &page, "C");
    tracker_reports(&state, &page, "C", true);
    dispatch_fails(&state, &page, "C").await;
    let events = page.events();
    assert_eq!(page_leaves_at(&events), Some(events.len() - 1), "{events:#?}");
    assert_page_left_rightly(&events, false, "the run is over");

    // B is stepped down to D.
    let state = AppState::for_tests();
    let page = Page::default();
    run_in_fullscreen(&state, &page, false).await;
    ends_clean(&state, &page, "A");
    view(&state, &page, "A", false);
    tracker_reports(&state, &page, "A", true);
    dispatch_starts(&state, &page, "A", "B").await;
    assert!(
        launch_play(&state, &page, "D", None, async {
            replace_in_slot(&state, &page, "B", "D");
            started()
        })
        .await
    );
    tracker_reports(&state, &page, "B", false);
    let events = page.events();
    assert_eq!(page_leaves_at(&events), None, "{events:#?}");
    assert_page_left_rightly(&events, true, "D plays");
    let left = state.current_child.lock().unwrap().take();
    if let Some(mut player) = left {
        let _ = player.kill();
    }
}
