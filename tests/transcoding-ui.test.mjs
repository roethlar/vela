// Finding tr-1: a Settings control that promises server conversion while the
// play path ignores it tells the user they have addressed a stall they have
// not. The gate that finding introduced is gone now that playback honours the
// setting, so what is guarded here is the invariant underneath it — the control
// may exist only while the play path actually reads the setting.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (rel) => fs.readFileSync(path.join(repoRoot, rel), 'utf8');

const settings = read('src/lib/Settings.svelte');
const commands = read('src-tauri/src/commands.rs');
const lib = read('src-tauri/src/lib.rs');
const playback = read('src-tauri/src/playback.rs');

test('the quality control ships only while playback reads the setting', () => {
  const offered = settings.includes('id="playback-quality"');
  if (!offered) return; // Withheld is always safe.

  assert.match(
    commands,
    /config::playback_quality\(cfg\.playback_quality\.as_deref\(\)\)/,
    'the play path must resolve the stored quality setting',
  );
  // Resolution has to receive it, or the value is read and then dropped.
  assert.match(
    commands,
    /resolve_stream\([\s\S]{0,200}&quality,/,
    'resolve_stream must be given the resolved quality',
  );
  assert.match(
    commands,
    /resolve_stream_version\([\s\S]{0,200}&quality,/,
    'resolve_stream_version must be given the resolved quality',
  );
});

// A transcode the user starts costs their server real work, and neither
// provider has a keep-alive that would expire it on its own.
test('a started transcode is always stopped', () => {
  assert.match(
    commands,
    /let transcode_session = resolved\.transcode_session\.clone\(\)/,
    'the teardown handle must be captured before the resolution is consumed',
  );
  assert.match(
    commands,
    /register_active_transcode\(&state\.active_transcode/,
    'the session must be owned by app state, not only by the end callback',
  );
});

// Slice 4 — the per-title one-off menu (`.agents/plans/server-transcoding.md`,
// owner ruling 2026-07-25). Quality nests under version when a title has several
// copies and is offered directly when it has one; the two labels never appear
// together; the choice governs one play and is stored nowhere.
const page = read('src/routes/+page.svelte');

test('quality nests under version, and the two labels are mutually exclusive', () => {
  // The nesting: the version submenu carries a per-copy quality row.
  assert.match(
    page,
    /aria-label="Play Version"[\s\S]{0,1200}toggleQualityMenu\(mi, b\)/,
    'each version in Play Version must expand to that copy\'s qualities',
  );
  // The collapsed form is the {:else if} of the same branch, so no title can
  // ever render both labels.
  assert.match(
    page,
    /\{#if \(mi\.backing\?\.length \?\? 0\) > 1 && mi\.canonicalId\}[\s\S]{0,2000}\{:else if mi\.sourceId[\s\S]{0,600}Play at Quality/,
    'Play at Quality must be the else-branch of Play Version, never a sibling',
  );
});

test('quality options are resolved only when the submenu opens', () => {
  // The Plex decision call is a round trip per version. Paying it on every
  // right-click would make the menu slow for everyone who never converts.
  assert.match(
    page,
    /async function toggleQualityMenu\([\s\S]{0,700}invoke<QualityOptions>\("quality_options"/,
    'the options request must live in the submenu toggle',
  );
  assert.doesNotMatch(
    page,
    /function openMenu\([\s\S]{0,800}quality_options/,
    'opening the context menu must not ask the server anything',
  );
});

// Finding or-6: with no item, the source answers for its DEFAULT version while
// the play path uses the policy's choice — so the menu could describe one
// version and play another, offering a tier that then degrades to Original.
test('the quality menu describes the version that will play', () => {
  assert.match(
    page,
    /invoke<QualityOptions>\("quality_options", \{[\s\S]{0,400}\n\s*item,/,
    'the menu must send the item so the answer can be pinned to the real version',
  );
  assert.match(
    commands,
    /\(None, Some\(item\)\) => selected_version_for\(&state, &item, &source\.id\(\), &raw\)/,
    'the backend must resolve the version the play path would choose',
  );
  // ...and only when the policy landed on the copy this row is about.
  // Pinned as the WHOLE condition: matching the comparison alone let a
  // regression wrap it in `true || (...)` and still pass.
  assert.match(
    commands,
    /\(selection\.source_id == source_id && selection\.raw_item_key == raw_item_key\)\s*\n\s*\.then_some\(selection\.version_id\)/,
    "another copy's version id must never describe this one",
  );
  // Ask mode must not turn a menu hover into a source-choice prompt.
  assert.match(
    commands,
    /Ok\(PlaybackSelectionOutcome::Choice\(_\)\) \| Err\(_\) => None/,
    'an ambiguous or failed selection must fall back, never prompt',
  );
});

test('a one-off quality is never persisted', () => {
  // It reaches the backend as a play argument...
  assert.match(
    page,
    /invoke<PlayCommandResult>\("play_item", \{[\s\S]{0,300}quality,/,
    'the chosen quality must travel with the play it starts',
  );
  // ...and the backend routes it to this launch only, never to config.
  assert.match(
    commands,
    /quality_override: quality\.as_deref\(\)/,
    'play_item must pass the one-off choice as an override',
  );
  assert.match(
    commands,
    /let quality = match quality_override[\s\S]{0,400}config::playback_quality/,
    'the override must win for this play and fall back to the stored setting',
  );
  // The setting has exactly one writer, and it is the Settings save. Anything
  // on the play path that assigns it is a one-off leaking into stored state.
  assert.equal(
    [...commands.matchAll(/cfg\.playback_quality = /g)].length,
    1,
    'the quality setting must have exactly one writer',
  );
  assert.match(
    commands,
    /pub fn set_mpv_advanced\([\s\S]{0,2000}cfg\.playback_quality = /,
    'that writer must be the Settings save, never the play path',
  );
  // Every other launch path keeps the setting: continuation, playlists, and the
  // source-choice reply must not inherit a previous play's one-off.
  assert.equal(
    [...commands.matchAll(/quality_override: None,/g)].length,
    3,
    'automatic continuation, playlist play, and source-choice replies use the setting',
  );
});

test('an invented quality cannot reach a source', () => {
  assert.match(
    commands,
    /quality_override\.filter\(\|value\| config::is_playback_quality\(value\)\)/,
    'the override must be validated against the same closed set as the setting',
  );
  const config = read('src-tauri/src/config.rs');
  assert.match(
    config,
    /pub fn is_playback_quality[\s\S]{0,200}playback_quality_values\(\)\.contains/,
    'that set must be the one the stored setting is validated against',
  );
});

// Slice 6 — Emby transcoding is best-effort and must be LABELLED limited rather
// than claimed (owner ruling 2026-07-25). It shares Jellyfin's implementation
// and has only ever been exercised against Jellyfin.
test('Emby transcoding is labelled unverified, not claimed', () => {
  assert.match(
    settings,
    /\{#if sources\.some\(\(s\) => s\.kind === "emby"\)\}[\s\S]{0,600}unverified on Emby/,
    'an Emby user must be told converting is unverified, and only an Emby user',
  );
  const readme = read('README.md');
  assert.match(
    readme,
    /Emby transcoding is best-effort and unverified/,
    'the README must say the same rather than implying Emby support',
  );
  // The claim the plan forbids: no document may assert Emby transcoding works.
  assert.doesNotMatch(
    readme,
    /transcoding (?:works|is supported) on Emby/i,
    'nothing may assert Emby transcoding works without a real server behind it',
  );
});

// Slice 6 — the README must state the cost of converting plainly, because it is
// the one thing a user cannot discover until their HDR is gone.
test('the README states what converting costs', () => {
  const readme = read('README.md');
  assert.match(
    readme,
    /\*\*Converting forfeits HDR and drops container chapters\.\*\*/,
    'the README must say converting costs HDR and chapters, in those terms',
  );
  assert.match(
    readme,
    /Playback quality[\s\S]{0,900}\*\*Automatic\*\* starts at Original[\s\S]{0,300}at most twice per play, never steps back up/,
    'the README must describe Automatic with both bounds the owner ruled',
  );
});

// Finding tr-6's wiring. The classifier, the retries and the credential-free
// failure text are all unit-guarded — but a guard-the-wiring sweep (2026-07-25)
// found that deleting the Plex call site left EVERY test green, because nothing
// proved a teardown actually goes through it. Same defect class that shipped two
// dead behaviours in slice 5.
test('both providers tear down through the classifier', () => {
  const plexLibrary = read('src-tauri/src/plex_library.rs');
  const jellyfin = read('src-tauri/src/source/jellyfin.rs');
  assert.match(
    plexLibrary,
    /pub async fn stop_transcode_session[\s\S]{0,400}crate::source::stop_transcode_request\("plex"/,
    'the Plex teardown must go through the shared classifier, not a bare send',
  );
  assert.match(
    jellyfin,
    /async fn stop_transcode[\s\S]{0,400}crate::source::stop_transcode_request\(self\.flavor\.kind\(\)/,
    'the Jellyfin/Emby teardown must go through the shared classifier too',
  );
  // A bare `.send()` on a teardown is what the classifier replaced; it must not
  // come back alongside it.
  assert.doesNotMatch(
    plexLibrary,
    /transcode_session_url[\s\S]{0,300}\.delete\([\s\S]{0,200}\.send\(\)/,
    'a teardown must never send its own request past the classifier',
  );
});

// Finding tr-9: the quality menu must not offer conversions for a version the
// server cannot deliver whole. `transcode_url` refuses to build one (guarded in
// Rust); this is the other half — the menu never advertising it in the first
// place, which no Rust test can reach without a live Plex server.
test('the Plex quality menu withholds conversion for a split-file version', () => {
  const plexSource = read('src-tauri/src/source/plex.rs');
  assert.match(
    plexSource,
    /let can_transcode = if !PlexLibrary::conversion_possible\(media\) \{\s*false/,
    'playback_options must report no transcoding for a version that cannot be converted whole',
  );
  assert.match(
    plexSource,
    /let split_file = !PlexLibrary::conversion_possible\(media\);/,
    'the play path must recognise the split-file case rather than truncating',
  );
});

// Finding tr-8: `Automatic` was selectable while nothing observed mpv's decoder
// drops or cache starvation and nothing stepped down — the same class as tr-1,
// a shipped option that does nothing. The rule it violated is the durable one:
// a control ships in the slice that makes it work, never before.
// Slice 5 made it work, so the gate is WITHDRAWN — and this now guards the same
// rule from the other side: the option and its implementation ship together, so
// neither can be removed or hollowed out without the other going with it.
test('Automatic is offered, and something implements it', () => {
  assert.match(
    settings,
    /<option value="automatic">Automatic<\/option>/,
    'Automatic is implemented, so it must be offered',
  );
  // A play must actually watch itself. Gated on the MODE, not on the resolved
  // quality: a step-down relaunches at a concrete tier, and reading the mode
  // off that quality left every replacement unwatched (finding or-1).
  assert.match(
    commands,
    /step_down: automatic_manages\(continues_automatic, &quality\)\s*\.then/,
    'the sampler must be gated on Automatic mode, not on the resolved quality',
  );
  assert.match(
    commands,
    /steps_taken: request\.steps_taken \+ 1,\s*\n\s*continues_automatic: true,/,
    'a step-down relaunch must declare that it continues an Automatic play',
  );
  // Finding or-5: the ladder is resolution-filtered, so its top rung can exceed
  // a modest source's own bitrate. A step-down that used it raw asked a
  // constrained link for MORE. The caller must use the bitrate-aware step, and
  // must feed it the source bitrate rather than a placeholder.
  assert.match(
    commands,
    /next_tier_below_bitrate\(\s*&request\.current_quality,\s*&options\.tiers,\s*options\.source_bitrate_kbps,\s*\)/,
    'a step-down must step below the quality the play is CARRYING, never a replayed one',
  );
  assert.match(
    commands,
    /current_quality: running_quality\.clone\(\)/,
    'the sampler must carry the quality its play is actually running at',
  );
  // Finding or-2: a step-down replaces the play IN PLACE, so it must inherit
  // the sequence context. Launching with `playlist: None` / `run_kind: None`
  // cleared the cursor and run state, and the next playlist entry or next
  // episode then never started when this one ended.
  // Asserted as a binding AND as the value handed to the launch: an earlier
  // form matched only the computation, so a regression that left the block in
  // place and passed `None` to the launch sailed through it.
  assert.match(
    commands,
    /let playlist = \{\s*\n\s*let cursor = state\.playlist_cursor\.lock\(\)\.await;[\s\S]{0,400}held\.session_id == request\.session_id/,
    'a step-down must inherit the playlist cursor of the session it replaces',
  );
  assert.match(
    commands,
    /session_id: &session_id,\s*\n\s*playlist,/,
    'and that inherited cursor must be the one the relaunch is given',
  );
  assert.match(
    commands,
    /state\.playback_run\.lock\(\)\.await[\s\S]{0,300}held\.session_id == request\.session_id/,
    'a step-down must inherit the run state of the session it replaces',
  );
  assert.doesNotMatch(
    commands,
    /replace_session: Some\(&request\.session_id\),\s*\n\s*run_kind: None,/,
    'the step-down relaunch must carry the run kind, not drop it',
  );
  // Finding or-3: without an explicit source, Ask Every Time re-entered source
  // selection for a duplicate title and returned an unobservable
  // SourceChoiceRequired — the play was never replaced.
  assert.match(
    commands,
    /let explicit_source = Some\(item\.source_id\.clone\(\)\)[\s\S]{0,120}\.or\(affinity\)/,
    'a step-down must pin the relaunch to the copy already playing',
  );
  assert.match(
    commands,
    /explicit_source_id: explicit_source\.as_deref\(\),\s*\n\s*persist_explicit_choice: false,/,
    "Automatic's own source pin must never be persisted as the user's choice",
  );
  // ...and the verdict must reach something that can start the replacement.
  assert.match(
    lib,
    /step_down_queue\.next\(\)\.await[\s\S]{0,220}apply_step_down/,
    'a verdict must reach a dispatcher that can start the replacement play',
  );
  // The help text may describe the step-down only because it now exists, and
  // must state both bounds the owner ruled on.
  assert.match(
    settings,
    /drops a step only if playback[\s\S]{0,140}at most twice, and never back up/,
    'the help text must state the cap and that stepping is one-way',
  );
});

// Finding tr-4: the teardown used to be a task detached from the playback-end
// callback, so app exit — which kills mpv and returns without joining any
// tracker — could end the process before the DELETE was sent. Each of the three
// paths that can be the last one to run must issue it itself.
test('every path that ends a play tears the transcode down', () => {
  // The tracker tail, on a thread outside any runtime: it must block on the
  // teardown rather than detach it.
  assert.match(
    commands,
    /take_active_transcode\(&transcode_slot, session\)[\s\S]{0,200}stop_transcode_record_blocking\(record\)/,
    'the play-end callback must claim its own session and stop it before returning',
  );
  assert.doesNotMatch(
    commands,
    /async_runtime::spawn\(async move \{[\s\S]{0,200}stop_transcode/,
    'teardown must never be detached into a task the process can outlive',
  );
  // Launch failure: no tracker will ever run, so this is the only chance.
  assert.match(
    commands,
    /take_active_transcode\(&state\.active_transcode, session\)[\s\S]{0,200}stop_transcode_record\(record\)\.await/,
    'a failed launch must stop the transcode its resolution already started',
  );
  // App exit: the last code that runs at all.
  assert.match(
    lib,
    /take_any_active_transcode\(&state\.active_transcode\)[\s\S]{0,200}stop_transcode_record_blocking\(record\)/,
    'the exit sweep must drain and stop any live transcode',
  );
});
