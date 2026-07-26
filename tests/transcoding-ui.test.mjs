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
