// The judgements the end-to-end scenarios make of what they observed
// (tests/e2e/judge.mjs), held here without a run of the app: each rule, taken
// out, fails a test of its own. Where a scenario's own code asks the
// judgement, a test drives that code with a stand-in for mpv, so that the
// scenario is seen to act on the verdict, not only the verdict to be right.
import assert from 'node:assert/strict';
import test from 'node:test';
import { skipJumpVerdict } from './e2e/judge.mjs';
import { proveSkippedPast } from './e2e/scenarios/markers.mjs';

// markers.mjs's rule: the play resumed from 1 s ahead of a range at 2-12 s,
// a 5000 ms window, a 1 s margin past the range's start.
const MARKERS = { startS: 1, rangeStart: 2, rangeEnd: 12, windowMs: 5000, marginS: 1 };

// The verdicts as the scenario asks for them, after each sample: the first
// one given, and the index of the sample it was given at.
function decide(samples, rule = MARKERS) {
  for (let i = 0; i < samples.length; i++) {
    const verdict = skipJumpVerdict(samples.slice(0, i + 1), rule);
    if (verdict) return { at: i, verdict };
  }
  return { at: null, verdict: null };
}

// Ordinary playback: from `fromS`, 0.1 s every 100 ms, `count` samples.
const playing = (fromS, count, startMs = 0) =>
  Array.from({ length: count }, (_, k) => ({ t: Math.round((fromS + k / 10) * 100) / 100, ms: startMs + 100 * k }));

test('pa-1: ordinary playback into the range is seen inside it, and fails there', () => {
  const { at, verdict } = decide(playing(1, 60));
  assert.equal(at, 20, `ordinary playback must fail at the first sample at 3 s (rangeStart + margin): ${JSON.stringify(verdict)}`);
  assert.equal(verdict.ok, false);
  assert.match(verdict.why, /in one step; it was seen inside the range at 3 s \(the sample before was 2\.9 s\), 2000 ms after resuming from 1\.00 s$/);
});

test('pa-1: a play that does not cross the range fails once the window is over, and not before', () => {
  // A play held at 1 s (a skip that never comes, whatever playback does).
  const held = Array.from({ length: 60 }, (_, k) => ({ t: 1, ms: 100 * k }));
  const stalled = decide(held);
  assert.equal(stalled.at, 51, `the window (5000 ms) must end it at the first sample past 5000 ms, and not before: ${JSON.stringify(stalled)}`);
  assert.equal(
    stalled.verdict.why,
    "the position must be past the range's end (12 s) within 5000 ms of playing on from 1.00 s, which only a skip can do; it is 1",
  );
  // Ordinary playback, against a range it cannot reach inside the window:
  // the window is what ends it.
  const later = decide(playing(1, 60), { ...MARKERS, rangeStart: 7, rangeEnd: 17 });
  assert.equal(later.at, 51, `ordinary playback must fail once the window is over, and not before: ${JSON.stringify(later)}`);
  assert.match(later.verdict.why, /within 5000 ms of playing on from 1\.00 s, which only a skip can do; it is 6\.1$/);
});

test('pa-1: a skip that lands inside the range fails at that sample, even when a later one is past the end', () => {
  // The trace the first fix let through: the skip lands at 9 s, and playback
  // covers the rest, 11.96 s then 12.08 s.
  const landed = [9, 9.12, 9.24, 9.36, 9.48, 9.6, 9.72, 9.84, 9.96, 10.08, 10.2, 10.32, 10.44, 10.56, 10.68, 10.8, 10.92, 11.04, 11.16, 11.28, 11.4, 11.52, 11.64, 11.76, 11.88, 11.96, 12.08];
  const samples = [...playing(1, 10), ...landed.map((t, k) => ({ t, ms: 1000 + 100 * k }))];
  const { at, verdict } = decide(samples);
  assert.equal(at, 10, `a skip landing at 9 s must fail at that sample: ${JSON.stringify(verdict)}`);
  assert.equal(verdict.ok, false);
  assert.match(verdict.why, /it was seen inside the range at 9 s \(the sample before was 1\.9 s\), 1000 ms after resuming/);
  // Judged with every sample in hand, it is still that failure.
  assert.deepEqual(skipJumpVerdict(samples, MARKERS), verdict, 'a sample past the end after one inside the range must not pass');
});

test('pa-1: one jump across the range passes, and names the samples either side of it', () => {
  const samples = [...playing(1, 10), { t: 1.92, ms: 905 }, { t: 12.0, ms: 1005 }];
  assert.deepEqual(decide(samples), { at: 11, verdict: { ok: true, before: 1.92, after: 12, ms: 1005 } });
  // A sample just past the range's start, inside the margin, then the jump.
  const justPast = [...playing(1, 10), { t: 2.4, ms: 1000 }, { t: 12.3, ms: 1100 }];
  assert.deepEqual(decide(justPast), { at: 11, verdict: { ok: true, before: 2.4, after: 12.3, ms: 1100 } });
});

test("pa-1: the range's end passes at exactly its value, and the margin ends at exactly its value", () => {
  assert.deepEqual(
    decide([{ t: 1.5, ms: 500 }, { t: 12, ms: 600 }]).verdict,
    { ok: true, before: 1.5, after: 12, ms: 600 },
    "a sample at exactly the range's end must pass",
  );
  const atMargin = decide([{ t: 1.5, ms: 500 }, { t: 3, ms: 600 }, { t: 12, ms: 700 }]);
  assert.equal(atMargin.at, 1, `a sample at exactly rangeStart + margin must fail: ${JSON.stringify(atMargin)}`);
  assert.match(atMargin.verdict.why, /seen inside the range at 3 s/);
});

test('pa-1: a read that failed changes nothing', () => {
  // A failed read just before each sample inside the window. (One past the
  // window ends it like any other sample, and says so: "it is null".)
  const withNulls = (samples) => samples.flatMap((s) => (s.ms < MARKERS.windowMs ? [{ t: null, ms: s.ms - 1 }, s] : [s]));
  const jump = [...playing(1, 10), { t: 1.92, ms: 905 }, { t: 12.0, ms: 1005 }];
  assert.deepEqual(decide(withNulls(jump)).verdict, decide(jump).verdict, 'null samples must not change a pass');
  const landed = [...playing(1, 10), { t: 9, ms: 1000 }, { t: 12.08, ms: 1100 }];
  assert.deepEqual(decide(withNulls(landed)).verdict, decide(landed).verdict, 'null samples must not change a failure inside the range');
  const held = Array.from({ length: 60 }, (_, k) => ({ t: 1, ms: 100 * k + 50 }));
  assert.deepEqual(decide(withNulls(held)).verdict, decide(held).verdict, 'null samples must not change a failure of the window');
  assert.equal(decide([{ t: null, ms: 100 }, { t: null, ms: 200 }]).verdict, null, 'null samples alone decide nothing');
});

// markers.mjs's proveSkippedPast itself, against a stand-in for mpv whose
// position, once the play is resumed, is `positionAt(ms since the resume)`.
// It must throw the verdict's failure and return on its pass: the scenario
// acts on the verdict.
function standInMpv(positionAt) {
  let paused = false;
  let pos = 0.5;
  let resumedAt = null;
  const made = Date.now();
  return {
    async getProp(name) {
      // Past any time the scenario can take, it answers no more, so that a
      // scenario sampling for good stops there and the test's timeout fails
      // it (a timer left running would keep the test process alive).
      if (Date.now() - made > 10000) return new Promise(() => {});
      if (name === 'pause') return paused;
      if (name === 'time-pos') return paused || resumedAt == null ? pos : positionAt(Date.now() - resumedAt);
      throw new Error(`the stand-in has no property ${name}`);
    },
    async setProp(name, value) {
      if (name === 'pause') {
        paused = value;
        if (!value) resumedAt = Date.now();
      } else if (name === 'time-pos') pos = value;
      else throw new Error(`the stand-in has no property ${name}`);
    },
  };
}

// A scenario that ignored a failing verdict would sample for good: the
// timeout makes that a failure, not a hang.
const STAND_IN = { timeout: 15000 };

test('pa-1: the markers scenario fails a skip that lands inside the range', STAND_IN, async () => {
  // The skip lands at 9 s after 0.9 s of playing, and playback would be past
  // the end 3 s later.
  const mpv = standInMpv((ms) => (ms < 900 ? 1 + ms / 1000 : 9 + (ms - 900) / 1000));
  await assert.rejects(
    proveSkippedPast(mpv, 'auto-skip: the intro range', 2, 12),
    /^Error: auto-skip: the intro range: the play must jump from the range's start \(2 s\) past its end \(12 s\) in one step; it was seen inside the range at 9\.\d+ s/,
    'the scenario must fail a skip that lands inside the range',
  );
});

test('pa-1: the markers scenario passes one jump across the range, and logs it', STAND_IN, async (t) => {
  const mpv = standInMpv((ms) => (ms < 900 ? 1 + ms / 1000 : 12.5));
  const logged = [];
  t.mock.method(console, 'log', (line) => logged.push(line));
  await proveSkippedPast(mpv, 'auto-skip: the intro range', 2, 12);
  assert.equal(logged.length, 1, `one line is logged: ${JSON.stringify(logged)}`);
  assert.match(logged[0], /^auto-skip: the intro range: from 1\.00 s, last sample before the jump 1\.\d\d s, first after 12\.50 s, \d+ ms after the play resumed$/);
});
