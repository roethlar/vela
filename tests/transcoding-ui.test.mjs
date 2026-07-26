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
test('Automatic is offered only once something implements it', () => {
  const offers = settings.match(/<option value="automatic">/g) ?? [];
  assert.equal(offers.length, 1, 'the value must appear in exactly one place');
  assert.match(
    settings,
    /\{#if playbackQuality === "automatic"\}[\s\S]{0,200}<option value="automatic">/,
    'Automatic may only be shown to a config that already stores it, never offered outright',
  );
  // The help text must not describe behaviour the build does not have.
  assert.doesNotMatch(
    settings,
    /Automatic starts at Original and steps down/,
    'the help text must not promise a step-down that nothing implements',
  );
  // The gate exists because the step-down is absent. If it ever arrives, this
  // test is what sends someone back here to remove the gate.
  //
  // Matched on the SUBSCRIPTION, not on the property name: `playback.rs` names
  // the drop count in tests asserting it is not a playback position, and a bare
  // string match failed on those. What means "Automatic is implemented" is mpv
  // being asked to report the property, not the property being mentioned.
  assert.doesNotMatch(
    playback,
    /observe_property[^\]]*decoder-frame-drop-count/,
    'the step-down landed: withdraw the tr-8 gate in Settings.svelte and this guard',
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
