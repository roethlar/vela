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
    /stop_transcode\(&session\)/,
    'the play-end path must stop the transcode session it started',
  );
  assert.match(
    commands,
    /let transcode_session = resolved\.transcode_session\.clone\(\)/,
    'the teardown handle must be captured before the resolution is consumed',
  );
});
