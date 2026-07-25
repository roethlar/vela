// A Settings control that promises server conversion while playback ignores it
// tells the user they have addressed a stall they have not (finding tr-1). The
// quality control must not reach the UI before the play path honours it.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const settings = fs.readFileSync(path.join(repoRoot, 'src/lib/Settings.svelte'), 'utf8');

test('the quality control is not offered before playback honours it', () => {
  const readyMatch = /const QUALITY_CONTROL_READY = (true|false);/.exec(settings);
  assert.ok(readyMatch, 'the readiness gate must exist while the wiring is pending');

  const field = settings.indexOf('id="playback-quality"');
  assert.ok(field > 0, 'the quality field should still be authored, only withheld');

  if (readyMatch[1] === 'false') {
    // The field must sit inside the gate, not merely near it.
    const gate = settings.indexOf('{#if QUALITY_CONTROL_READY}');
    const endGate = settings.indexOf('{/if}', gate);
    assert.ok(gate > 0 && gate < field && field < endGate,
      'the quality field must be inside the readiness gate');
  }
});

// The other half of the same defect: the help text names a behaviour, so it may
// only ship when that behaviour exists.
test('the quality help text ships only with the control', () => {
  const promises = settings.includes('asks\n            your server to convert it');
  const gated = settings.includes('{#if QUALITY_CONTROL_READY}');
  const ready = /const QUALITY_CONTROL_READY = true;/.test(settings);
  assert.ok(!promises || gated || ready,
    'copy promising server conversion must be gated until the feature works');
});
