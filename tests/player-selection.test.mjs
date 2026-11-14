// Which player a play uses (.agents/plans/embedded-player.md, "Selection",
// decisions D1 and D3). The Rust unit tests prove the choice itself
// (src-tauri/src/player/select.rs). These prove the play path actually makes
// that choice, carries it to the launch, and keeps it across a relaunch; and
// that the setting stays out of sight until the owner has done the checks the
// plan lists.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file) => fs.readFileSync(path.join(repoRoot, file), 'utf8').replace(/\r\n/g, '\n');
const commands = read('src-tauri/src/commands.rs');
const playback = read('src-tauri/src/playback.rs');
const lib = read('src-tauri/src/lib.rs');
const config = read('src-tauri/src/config.rs');
// Where a play that started publishes what it leaves behind.
const successCall =
  'let played = publish_started_play(played, window_slot, launched_window_session, || {';

function functionBody(source, signature, what) {
  const start = source.indexOf(`\n${signature}`);
  assert.notEqual(start, -1, `${what} must exist`);
  const end = source.indexOf('\n}\n', start);
  assert.notEqual(end, -1, `${what} must have a closing brace`);
  return source.slice(start, end);
}

test('every play chooses its player from the setting and what can run', () => {
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  // What is wanted: the launch's own player when it replaces a running play,
  // otherwise the saved setting. Read once, from the settings the copy is
  // chosen with.
  const select = functionBody(commands, 'async fn select_playback_version(', 'select_playback_version');
  assert.match(
    select,
    /let wanted_player =\s+player\.unwrap_or_else\(\|\| crate::player::PlayerKind::from_setting\(cfg\.player\.as_deref\(\)\)\);/,
  );
  // Whether the embedded player can run is asked only for a play that wants
  // it (the answer comes from the window), and asked of the real host.
  assert.match(
    select,
    /let embedded_available = match wanted_player \{\s+crate::player::PlayerKind::External => Ok\(\(\)\),\s+crate::player::PlayerKind::Embedded => \{\s+crate::player::embedded_availability\(&state\.embedded, state\.app_handle\.get\(\)\)\.await\s+\}\s+\};/,
  );
  // The choice: one call, with that availability, made once, before the
  // copy is ranked, so that a Compatible choice is made for the screen of
  // the player that will run the play, not the one asked for (review
  // finding pw-2), and carried to the launch.
  const choose = 'let upcoming_player = crate::player::choose(wanted_player, &embedded_available);';
  const at = select.indexOf(choose);
  assert.notEqual(at, -1, 'the choice of copy must choose the player with player::choose');
  assert.equal(select.indexOf('player::choose(', at + choose.length), -1, 'chosen exactly once');
  const ranks = select.indexOf('playback_compatibility_target(state, replace_session, upcoming_player.kind, &cfg)');
  assert.notEqual(ranks, -1, 'the copy is ranked for the player that will run the play');
  assert.ok(at < ranks, 'chosen before the copy is ranked');
  assert.equal(select.split('player: upcoming_player,').length - 1, 2, 'both ready selections carry it');
  assert.match(
    body,
    /let selection = select_playback_version\(\s+state,\s+item,\s+replace_session,\s+explicit_source_id,\s+prior_affinity\.as_deref\(\),\s+server_owned,\s+player,\s+\)\s+\.await\?;/,
  );
  assert.match(body, /let player_choice = selection\.player\.clone\(\);/);
  assert.doesNotMatch(body, /from_setting\(/, 'the play path reads the player setting once, with the copy');
  assert.doesNotMatch(body, /player::choose\(/, 'and does not choose the player a second time');
  assert.doesNotMatch(body, /embedded_availability\(/, 'or ask the window a second time');
  // ...and it reaches the launch, together with the one-line explanation.
  const spec = body.slice(body.indexOf('let spec = playback::PlaySpec {'));
  assert.match(spec, /^let spec = playback::PlaySpec \{\n\s+player: player_choice\.kind,/);
  assert.match(spec, /osd_notice: osd_notice\.or\(fallback_notice\),/);
});

test('once a play has installed its session, only the launch-failure path ends it early', () => {
  // Moving the playlist position, installing the session, stopping the
  // previous player and registering a transcode are undone in one place: the
  // `Err` arm after the launch. A failure that left the function between the
  // installation and that arm would skip all of it (review finding sel-1).
  //
  // Earlier versions of this test listed the ways a line of Rust can leave a
  // function early and looked for them in that stretch. No such list is
  // complete, and a pattern that reads Rust can be fooled. So the code was
  // reshaped instead: everything that starts the play is written as one
  // block, `launch`, BEFORE the installation, and run by a single line after
  // it. Inside the block a `?` or a `return` leaves the block, with a message
  // the arm receives; it cannot leave the function. What this test holds is
  // that the stretch from the installation to the arm is exactly these lines,
  // none of which can fail. (A panic is not an early return: release builds
  // abort on one, `panic = "abort"`, so no half-started Vela is left running.)
  //
  // The installation, the run of `launch` and the arm live in one function,
  // `install_and_launch`, which the play path calls once with the block
  // (review finding pc-2: so that a Rust test runs that very sequence). The
  // play path itself installs nothing.
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  const statements = (text) =>
    text
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line && !line.startsWith('//'));

  const calling = '    let stop = install_and_launch(\n';
  const from = body.indexOf(calling);
  assert.notEqual(from, -1, 'the play must still install and start itself through install_and_launch');
  assert.equal(body.indexOf('install_and_launch(', from + calling.length), -1, 'once');
  const callEnd = body.indexOf(';\n', from);
  assert.deepEqual(statements(body.slice(from, callEnd + 1)), [
    'let stop = install_and_launch(',
    'state,',
    'PlayInstall {',
    'session_id,',
    'item,',
    'run_kind,',
    'affinity_source_id: next_affinity,',
    'cursor,',
    'transcode_session: transcode_session.as_deref(),',
    '},',
    'launch,',
    '|end| report_playback_ended(state, end),',
    ')',
    '.await',
    '.map_err(PlayFailure::launch_failed)?;',
  ]);
  for (const installs of ['install_playback_session', 'playlist_cursor', 'undo_failed_launch', 'launch.await']) {
    assert.equal(body.indexOf(installs), -1, `the play path itself has no ${installs}`);
  }

  // `launch` is the block, defined once, before anything is installed, and
  // handed over once, unrun.
  const defined = body.indexOf('    let launch = async {\n');
  assert.notEqual(defined, -1, 'the steps that start the play are one block');
  assert.ok(defined < from, 'written before the installation');
  assert.equal(body.split('let launch =').length - 1, 1, 'written once');
  assert.equal(
    [...statements(body).join('\n').matchAll(/\blaunch\b/g)].length,
    2,
    'and named only where it is written and handed over',
  );

  // The installation, the launch and the arm, whole: note which player is in
  // the slot, install, run the launch, and on failure undo it and then give
  // the page the report the run is over when the state of the app says so
  // (review finding pc-2), then report the failure.
  const installAndLaunch = functionBody(commands, 'async fn install_and_launch<', 'install_and_launch');
  const opens = installAndLaunch.indexOf('\n{\n');
  assert.notEqual(opens, -1, 'install_and_launch opens its body on a line of its own');
  const sequence = installAndLaunch.slice(opens);
  assert.deepEqual(statements(sequence), [
    '{',
    'let before = state',
    '.current_child',
    '.lock()',
    '.unwrap_or_else(|e| e.into_inner())',
    '.as_ref()',
    '.map(crate::player::PlayerHandle::serial);',
    'let previous_cursor = {',
    'let mut cursor = state.playlist_cursor.lock().await;',
    'std::mem::replace(&mut *cursor, install.cursor)',
    '};',
    'install_playback_session(',
    'state,',
    'install.session_id,',
    'install.item,',
    'install.run_kind,',
    'install.affinity_source_id,',
    ')',
    '.await;',
    'match launch.await {',
    'Ok(stop) => Ok(stop),',
    'Err(message) => {',
    'undo_failed_launch(',
    'state,',
    'install.session_id,',
    'install.transcode_session,',
    'previous_cursor,',
    ')',
    '.await;',
    'report_failed_launch_end(state, install.session_id, before, &mut report_end).await;',
    'Err(message)',
    '}',
    '}',
  ]);
  // The undo, whole: stop the transcode, clear the session, put the playlist
  // position back.
  const undo = functionBody(commands, 'async fn undo_failed_launch(', 'undo_failed_launch');
  const undoBody = undo.slice(undo.indexOf(') {\n'));
  assert.deepEqual(statements(undoBody), [
    ') {',
    'if let Some(session) = transcode_session {',
    'if let Some(record) = take_active_transcode(&state.active_transcode, session) {',
    'stop_transcode_record(record).await;',
    '}',
    '}',
    'clear_playback_session_if(state, session_id).await;',
    'let mut cursor = state.playlist_cursor.lock().await;',
    'if cursor',
    '.as_ref()',
    '.is_none_or(|current| current.session_id == session_id)',
    '{',
    '*cursor = previous_cursor;',
    '}',
  ]);

  // The player setting is read before anything is resolved or installed: by
  // the choice of copy (review finding pw-2), whose answer carries it.
  const read = body.indexOf('let selection = select_playback_version(');
  const resolved = body.indexOf('let resolved = match selection.version_id.as_deref() {');
  assert.notEqual(read, -1, 'the play must read which player is wanted');
  assert.notEqual(resolved, -1, 'the play must still resolve its stream');
  assert.ok(read < resolved, 'the player setting is read before the stream is resolved');
  assert.ok(read < from, 'and before anything is installed');
  assert.equal(body.indexOf('let selection = select_playback_version(', read + 1), -1, 'and only once');
  assert.equal(body.split('let player_choice').length - 1, 1, 'and bound once');
});

test('a fallback is explained once, and only by a play that started', () => {
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  // One decision per play. It is told whether the play chooses its player
  // (the launch names none) or is an automatic relaunch of a running play,
  // which neither explains nor forgets (review finding sel-2; the rule itself
  // is unit-tested in src-tauri/src/player/select.rs).
  assert.match(
    body,
    /let notice_plan = state\s+\.embedded_fallback_notices\s+\.lock\(\)\s+\.unwrap_or_else\(\|e\| e\.into_inner\(\)\)\s+\.plan\(player\.is_none\(\), player_choice\.fallback\.as_ref\(\)\);/,
  );
  // A notice the launch already carries wins, and then there is nothing to
  // record as explained.
  assert.match(
    body,
    /let explained = notice_plan\.explain\.filter\(\|_\| osd_notice\.is_none\(\)\);\s+let fallback_notice = explained\.as_ref\(\)\.map\(\|reason\| reason\.notice\(\)\);/,
  );
  // The user has been told when the line reached the player, not when the
  // player started: a play replaced during the wait never shows it (review
  // finding sel-3). The record is made by a callback the launch carries next
  // to the notice itself.
  assert.match(
    body,
    /let told_when_shown = \|reason: crate::player::EmbeddedUnavailable\| \{\s+let notices = state\.embedded_fallback_notices\.clone\(\);\s+std::sync::Arc::new\(move \|\| \{\s+notices\s+\.lock\(\)\s+\.unwrap_or_else\(\|e\| e\.into_inner\(\)\)\s+\.explained\(&reason\);\s+\}\) as playback::NoticeShown\s+\};\s+let on_notice_shown = explained\.map\(told_when_shown\);/,
  );
  const spec = body.slice(body.indexOf('let spec = playback::PlaySpec {'));
  assert.match(spec, /osd_notice: osd_notice\.or\(fallback_notice\),\n\s+on_notice_shown,/);
  // ...which the play hands to the thread that sends the notice, and which
  // that thread calls only after the wait, the replaced-play check and a
  // successful write (the behaviour is unit-tested in playback.rs).
  assert.match(
    functionBody(playback, 'pub fn play(', 'playback::play'),
    /spawn_osd_notice\(\s+ipc_path\.clone\(\),\s+stop_flag\.clone\(\),\s+notice,\s+OSD_NOTICE_DELAY,\s+on_notice_shown,\s+\)/,
  );
  const notice = functionBody(playback, 'fn spawn_osd_notice(', 'spawn_osd_notice');
  assert.match(
    notice,
    /std::thread::sleep\(delay\);\s+if stop_flag\.load\(Ordering::Relaxed\) \{\s+return;\s+\}[\s\S]{0,400}if sent\.is_ok\(\) \{\s+if let Some\(on_shown\) = on_shown \{\s+on_shown\(\);/,
  );
  assert.equal(notice.split('on_shown()').length - 1, 1, 'reported in one place only');
  // A started play with nothing to fall back from forgets, inside the success
  // continuation and never before the launch.
  const success = body.slice(body.indexOf(successCall));
  assert.match(
    success,
    /^let played = publish_started_play\(played, window_slot, launched_window_session, \|\| \{[\s\S]{0,400}if notice_plan\.forget_when_started && !fell_back \{\s+state\s+\.embedded_fallback_notices\s+\.lock\(\)\s+\.unwrap_or_else\(\|e\| e\.into_inner\(\)\)\s+\.forget\(\);\s+\}/,
  );
  // The notice state is touched in exactly four places in the whole command
  // layer: the plan and the shown-callback before the launch, a second plan
  // for the case that the embedded player cannot start the play after all
  // (review finding ec-6; the test below), and the forgetting after the
  // launch succeeded. Anything else would tell the user at the wrong time.
  const uses = [...commands.matchAll(/embedded_fallback_notices/g)].map((use) => use.index);
  assert.equal(uses.length, 4, 'no other code reads or writes the notice state');
  const successAt = commands.indexOf(successCall);
  const launchAt = commands.indexOf('playback::play(\n');
  assert.ok(uses.slice(0, 3).every((use) => use < launchAt), 'planned before the launch');
  assert.ok(uses[3] > launchAt && uses[3] > successAt, 'forgotten only after it succeeded');
  assert.deepEqual(
    [...commands.matchAll(/\.(plan|explained|forget)\(/g)].map((call) => call[1]),
    ['plan', 'explained', 'plan', 'forget'],
    'through these calls and no others',
  );
});

test('a relaunch stays in the player it replaces', () => {
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  assert.match(
    body,
    /\*state\.active_playback_player\.lock\(\)\.await = Some\(\(session_id\.to_string\(\), player_choice\.kind\)\);/,
    'the session records the player it runs in',
  );
  const stepDown = functionBody(commands, 'pub(crate) async fn apply_step_down(', 'apply_step_down');
  assert.match(
    stepDown,
    /let replaced_player = state\s+\.active_playback_player\s+\.lock\(\)\s+\.await\s+\.as_ref\(\)\s+\.filter\(\|\(session, _\)\| session == &request\.session_id\)\s+\.map\(\|\(_, kind\)\| \*kind\);/,
    'a step-down reads the player of the exact session it replaces',
  );
  assert.match(stepDown, /continues_automatic: true,\n\s+player: replaced_player,/);
  // Every other launch is the user's own and chooses from the setting.
  const others = commands.split('player: replaced_player,').join('');
  const launches = others.split('continues_automatic: false,').length - 1;
  assert.ok(launches >= 4, 'the user-started launches must still be found');
  assert.equal(
    [...others.matchAll(/continues_automatic: false,\n\s+player: None,/g)].length,
    launches,
    'a launch the user starts never names a player',
  );
  // The record does not outlive its session.
  const clear = functionBody(commands, 'async fn clear_playback_session_if(', 'clear_playback_session_if');
  assert.match(
    clear,
    /let mut player = state\.active_playback_player\.lock\(\)\.await;\s+if player\s+\.as_ref\(\)\s+\.is_some_and\(\|\(session, _\)\| session == expected_session\)\s+\{\s+\*player = None;/,
  );
});

test('play refuses a player it cannot start before it starts anything', () => {
  // That the refusal refuses is tested by calling play (review finding
  // sel-4): src-tauri/src/playback.rs, the test named below. This test adds
  // what that one cannot see: that nothing is done before it.
  assert.match(playback, /fn play_refuses_a_player_it_cannot_start_and_starts_nothing\(\) \{/);
  const body = functionBody(playback, 'pub fn play(', 'playback::play');
  // The refusal, whole, as the first thing play does once it has taken its
  // arguments apart: an embedded play with nothing to start it is turned
  // away inside this one statement, by the `?` on the missing launcher. An
  // earlier shape of this test pinned only where the check stood; with the
  // check emptied and the refusal moved below the search for the mpv program,
  // `mpv --version` ran first and both tests passed (review finding sel-4).
  assert.match(
    body,
    /^\npub fn play\([^{]*\) -> Result<PlayStarted, String> \{\n[ \t]*let Players \{\n[ \t]*slot: player_slot,\n[ \t]*shutting_down,\n[ \t]*embedded,\n[ \t]*\} = players;\n(?:[ \t]*\/\/[^\n]*\n)*[ \t]*let embedded_start = match spec\.player \{\n[ \t]*crate::player::PlayerKind::External => None,\n[ \t]*crate::player::PlayerKind::Embedded => Some\(\n[ \t]*embedded\n[ \t]*\.ok_or_else\(\|\| NO_EMBEDDED_PLAYER\.to_string\(\)\)\?\n[ \t]*\.prepare\(&session_id\),\n[ \t]*\),\n[ \t]*\};\n/,
    'the first statement of play decides the player, and refuses there',
  );
  const guard = body.indexOf('let embedded_start = match spec.player {');
  assert.notEqual(guard, -1, 'play must check which player it was asked for');
  for (const later of ['new_ipc_path()', 'resolve_mpv()', 'build_launch(', 'external::launch(']) {
    const at = body.indexOf(later);
    assert.notEqual(at, -1, `play must still call ${later}`);
    assert.ok(guard < at, `the player check must come before ${later}`);
  }
});

test('a play the embedded player cannot start goes on in the external player', () => {
  // Decision D3: a play is never failed over the embedded player. The
  // decision is `start_player`, proven in playback.rs with stand-ins for both
  // players. This proves that every embedded play goes through it and no
  // external play does, that the external launch after a failure is
  // described afresh and on a socket of its own, that everything after it
  // follows the player that started, and that the command layer records
  // that player and not the one it chose (review finding ec-6).
  const play = functionBody(playback, 'pub fn play(', 'playback::play');
  assert.equal(play.split('start_player(').length - 1, 1);
  // The embedded player's turn: the lock on the player slot is taken, Vela
  // is checked not to be exiting (if it is, the play ends there and the
  // external player is NOT tried), and the player is started and published.
  // The lock is let go of when that closure returns.
  const embeddedTurn =
    /let \(\(\), kind\) = start_player\(\s+Some\(\|\| \{\s+let mut slot = player_slot\.lock\(\)\.unwrap_or_else\(\|e\| e\.into_inner\(\)\);\s+if shutting_down\.load\(Ordering::SeqCst\) \{\s+plan\.discard_marker_payload\(\);\s+return Err\(NotStarted::MustNot\(SHUTTING_DOWN\.to_string\(\)\)\);\s+\}\s+let player = match prepared \{\s+Ok\(start\) => start\(plan, own_config, &ipc_path\),\s+Err\(reason\) => \{\s+(?:\/\/[^\n]*\n\s+)*plan\.discard_marker_payload\(\);\s+Err\(reason\)\s+\}\s+\}\s+\.map_err\(NotStarted::CouldNot\)\?;\s+\*slot = Some\(player\);\s+Ok\(\(\)\)\s+\}\),/;
  assert.match(play, embeddedTurn);
  // The external player's turn after a failure. Looking for the mpv program
  // (which runs programs and waits for them) and describing the launch
  // happen with the lock NOT held: the exit handler waits for that lock on
  // the window's thread (reopened for looking inside it). The lock is taken
  // only then, and Vela is checked again not to be exiting before anything is
  // started.
  const externalTurn =
    /\|\| \{\s+(?:\/\/[^\n]*\n\s+)*let mpv_bin = find_mpv\(\)\?;\s+let path = another_ipc_path\(&ipc_path\)\?;\s+let plan = crate::player::launch::build_launch_for\(\s+spec,\s+crate::player::PlayerKind::External,\s+&cfg,\s+&path,\s+\)\?;\s+let mut slot = player_slot\.lock\(\)\.unwrap_or_else\(\|e\| e\.into_inner\(\)\);\s+(?:\/\/[^\n]*\n\s+)*if shutting_down\.load\(Ordering::SeqCst\) \{\s+plan\.discard_marker_payload\(\);\s+return Err\(SHUTTING_DOWN\.to_string\(\)\);\s+\}\s+\*slot = Some\(crate::player::external::launch\(&mpv_bin, plan\)\?\);\s+external_ipc_path = Some\(path\);\s+Ok\(\(\)\)\s+\},\s+\)\?;\s+kind/;
  assert.match(play, externalTurn);
  // Three places take that lock to start a player, each checks for the exit
  // first, and nowhere is mpv looked for between a lock and its start.
  assert.equal(play.split('let mut slot = player_slot.lock()').length - 1, 3, 'the three starts');
  assert.equal(play.split('shutting_down.load(Ordering::SeqCst)').length - 1, 3);
  for (const [, held] of play.matchAll(/let mut slot = player_slot\.lock\(\)[\s\S]*?\*slot = Some\(/g).map((m) => [m.index, m[0]])) {
    assert.doesNotMatch(held, /find_mpv\(|resolve_mpv\(|another_ipc_path\(|build_launch/, 'nothing slow between taking the lock and starting');
  }
  // A launcher that cannot even prepare the play is the embedded player
  // failing too: its answer is kept for `start_player`, not returned.
  assert.doesNotMatch(play, /\.prepare\(&session_id\)\?/);
  // The trackers, the watchers and the notice all take the socket of the
  // player that started.
  assert.match(
    play,
    /let ipc_path = external_ipc_path\.unwrap_or\(ipc_path\);\s+let fell_back = started_player != spec\.player;/,
  );
  const settled = 'let ipc_path = external_ipc_path.unwrap_or(ipc_path);';
  const after = play.slice(play.indexOf(settled) + settled.length);
  assert.doesNotMatch(after, /external_ipc_path/, 'nothing later can pick the other socket');
  // A play that fell back explains it, unless it already carries a notice.
  assert.match(
    play,
    /let \(notice, on_notice_shown\) = match &spec\.osd_notice \{\s+None if fell_back => \(\s+spec\.if_embedded_cannot_start\.notice\.clone\(\),\s+spec\.if_embedded_cannot_start\.on_notice_shown\.clone\(\),\s+\),\s+carried => \(carried\.clone\(\), spec\.on_notice_shown\.clone\(\)\),\s+\};/,
  );
  assert.match(play, /Ok\(PlayStarted \{\s+stop: stop_flag,\s+player: started_player,\s+\}\)$/);

  // The command layer: what such a play says is planned under the same
  // once-per-run rule as any other fallback...
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  assert.match(
    body,
    /let if_embedded_cannot_start = match player_choice\.kind \{\s+crate::player::PlayerKind::External => playback::EmbeddedFallback::default\(\),\s+crate::player::PlayerKind::Embedded => \{\s+let explain = state\s+\.embedded_fallback_notices\s+\.lock\(\)\s+\.unwrap_or_else\(\|e\| e\.into_inner\(\)\)\s+\.plan\(\s+player\.is_none\(\),\s+Some\(&crate::player::EmbeddedUnavailable::StartFailed\),\s+\)\s+\.explain;\s+playback::EmbeddedFallback \{\s+notice: explain\.as_ref\(\)\.map\(\|reason\| reason\.notice\(\)\),\s+on_notice_shown: explain\.map\(told_when_shown\),\s+\}\s+\}\s+\};/,
  );
  const spec = body.slice(body.indexOf('let spec = playback::PlaySpec {'));
  assert.match(spec, /on_notice_shown,\n\s+if_embedded_cannot_start,\n/);
  // ...and once the play has started, the player it runs in is the one
  // recorded for it (a relaunch stays in that player), and a play that fell
  // back does not count as proof that nothing needs explaining.
  assert.match(
    body,
    /let fell_back = played\s+\.as_ref\(\)\s+\.is_ok_and\(\|started\| started\.player != player_choice\.kind\);\s+if fell_back \{\s+\*state\.active_playback_player\.lock\(\)\.await =\s+Some\(\(session_id\.to_string\(\), crate::player::PlayerKind::External\)\);\s+\}\s+let played = played\.map\(\|started\| started\.stop\);/,
  );
  assert.ok(body.indexOf('let fell_back = played') < body.indexOf(successCall));
});

test('the window record names the player that ran the play', () => {
  // The next play opens its mpv window as the last one left it, but only an
  // external play has such a window (the rule is unit-tested in
  // commands.rs). The record names the player the play started in, which is
  // the external one when an embedded play fell back.
  const body = functionBody(commands, 'async fn play_by_key_locked(', 'play_by_key_locked');
  assert.match(
    body,
    /let played = played\.map\(\|started\| started\.stop\);\s+let started_player = if fell_back \{\s+crate::player::PlayerKind::External\s+\} else \{\s+player_choice\.kind\s+\};/,
  );
  assert.match(
    body,
    /let launched_window_session = PlaybackWindowSession \{\s+session_id: session_id\.to_string\(\),\s+player: started_player,\s+observation: window_observation,\s+\};/,
  );
  assert.ok(body.indexOf('let launched_window_session') < body.indexOf(successCall));
});

test('only a play that started writes the window record, in one place', () => {
  // The next play opens its window from this record, so a launch that fails
  // must leave the last started play's record in place: the record is
  // written in publish_started_play's success continuation and nowhere else
  // (the helper itself is unit-tested in commands.rs). Every other use is
  // listed here exactly, as a read, so a clear or an early publish on the
  // launch path cannot be added beside them.
  const production = commands.slice(0, commands.indexOf('\n#[cfg(test)]\nmod tests {'));
  const uses = (source, name) => [...source.matchAll(new RegExp(`\\b${name}\\b`, 'g'))].length;
  const reads = [
    // Settings shows the display the latest play observed.
    /let observed = state\s+\.playback_window_session\s+\.lock\(\)\s+\.unwrap_or_else\(\|error\| error\.into_inner\(\)\)\s+\.latest\(\)\s+\.map\(\|session\| session\.observation\.display_snapshot\(\)\);/g,
    // A Compatible choice is made for that display.
    /let current = state\s+\.playback_window_session\s+\.lock\(\)\s+\.unwrap_or_else\(\|error\| error\.into_inner\(\)\);\s+compatibility_display\(&current, replace_session, upcoming\)\s+\};/g,
    // The launch opens its window from it.
    /let current = state\s+\.playback_window_session\s+\.lock\(\)\s+\.unwrap_or_else\(\|e\| e\.into_inner\(\)\);\s+opening_window\(&current, first_of_run\)\s+\};/g,
    // The end callback hands the play's window to the settings.
    /let latest_window = state\.playback_window_session\.clone\(\);/g,
    // The one writer, given the slot.
    /let window_slot = &state\.playback_window_session;\s+let played = publish_started_play\(played, window_slot, launched_window_session, \|\| \{/g,
  ];
  for (const read of reads) {
    assert.equal([...production.matchAll(read)].length, 1, `expected exactly once: ${read}`);
  }
  assert.equal(
    uses(production, 'playback_window_session'),
    reads.length,
    'no other command-layer code may touch the window record',
  );
  assert.equal(uses(production, 'window_slot'), 2, 'the slot goes only to publish_started_play');
  assert.equal(uses(production, 'latest_window'), 2, 'the end callback only reads the record');
  // ...and hands it to write_play_end, which looks at it only inside the
  // settings change (review finding pw-5), through remember_window_in.
  assert.match(
    production,
    /let _ = write_play_end\(\s+\|change\| config::update\(change\),\s+&latest_window,\s+&session_id,\s+move \|cfg\| \{/,
  );
  const writeEnd = functionBody(commands, "fn write_play_end<'a>(", 'write_play_end');
  assert.match(
    writeEnd,
    /\) -> Result<\(\), String> \{\s+update\(Box::new\(move \|cfg\| \{\s+finish\(cfg\);\s+remember_window_in\(cfg, record, Some\(session_id\)\);\s+Ok\(\(\)\)\s+\}\)\)\s*$/,
  );
  const rememberIn = functionBody(commands, 'fn remember_window_in(', 'remember_window_in');
  assert.equal(rememberIn.split('.lock()').length - 1, 1, 'the record is looked at once');
  assert.match(
    rememberIn,
    /let ended = \{\s+let record = record\.lock\(\)\.unwrap_or_else\(\|e\| e\.into_inner\(\)\);\s+match session_id \{[\s\S]*?\}\s+\};\s+ended\.is_some_and\(/,
    'and let go of before the settings are changed',
  );
  assert.doesNotMatch(rememberIn, /\.take\(\)|\.publish\(|=\s*(?:None|Some\()/, 'and only read');
  const publish = functionBody(commands, 'fn publish_started_play<T>(', 'publish_started_play');
  assert.match(
    publish,
    /after_successful_play\(played, \|\| \{\s+window_session\s+\.lock\(\)\s+\.unwrap_or_else\(\|e\| e\.into_inner\(\)\)\s+\.publish\(launched\);\s+on_success\(\);\s+\}\)/,
  );
  assert.equal(uses(production, 'window_session'), 2, 'publish_started_play writes it once');
  // Quitting Vela reads it, through remember_window_in only, to hand the
  // latest window over: once to see whether there is anything to write, and
  // again inside the write (review finding pw-5).
  const onExit = functionBody(commands, "fn remember_window_on_exit_with<'a>(", 'remember_window_on_exit_with');
  assert.equal(onExit.split('.lock()').length - 1, 0);
  assert.match(onExit, /Ok\(mut cfg\) => remember_window_in\(&mut cfg, latest, None\),/);
  assert.match(
    onExit,
    /if changes \{\s+let _ = update\(Box::new\(move \|cfg\| \{\s+remember_window_in\(cfg, latest, None\);\s+Ok\(\(\)\)\s+\}\)\);\s+\}/,
  );
  assert.doesNotMatch(onExit, /\.take\(\)|\.publish\(|=\s*(?:None|Some\()/);
  // In lib.rs: the field, its empty start (in the app, and in the state a
  // unit test builds), and that exit read.
  assert.equal(uses(lib, 'playback_window_session'), 4);
  assert.equal([...lib.matchAll(/playback_window_session: Arc::new\(Mutex::new\(Default::default\(\)\)\),/g)].length, 2);
  assert.equal(lib.split('pub(crate) fn for_tests() -> Self {').length - 1, 1);
  assert.match(lib, /commands::remember_window_on_exit\(&state\.playback_window_session\);/);
});

test('the player setting is validated, and not yet shown to users', () => {
  assert.match(
    config,
    /validate_optional_closed\("player", self\.player\.as_deref\(\), &\["external", "embedded"\]\)\?;/,
  );
  // Owner, 2026-11-12: until the by-eye checks are done there is no Settings
  // control for the embedded player. It can be tried only by editing the
  // config file. Slice 6 of the plan removes this assertion on purpose.
  assert.doesNotMatch(lib, /commands::(get|set)_player\b/, 'no command exposes the player setting');
  assert.doesNotMatch(commands, /fn (get|set)_player\b/);
  for (const file of ['src/lib/Settings.svelte', 'src/routes/+page.svelte', 'src/lib/types.ts']) {
    assert.doesNotMatch(read(file), /embedded/i, `${file} must not mention the embedded player yet`);
  }
  for (const file of ['README.md', 'docs/usage.md', 'RELEASE_NOTES.md']) {
    assert.doesNotMatch(
      read(file),
      /embedded player|"player"\s*:/i,
      `${file} must not announce the embedded player yet`,
    );
  }
});
