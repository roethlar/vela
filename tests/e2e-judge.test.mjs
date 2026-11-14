// The judgements the end-to-end scenarios make of what they observed
// (tests/e2e/judge.mjs), held here without a run of the app: each rule, taken
// out, fails a test of its own. Where a scenario's own code asks the
// judgement, a test drives that code with a stand-in for mpv, so that the
// scenario is seen to act on the verdict, not only the verdict to be right.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { acceptedConnections, plexStopProblems, skipJumpVerdict } from './e2e/judge.mjs';
import { proveSkippedPast } from './e2e/scenarios/markers.mjs';
import { assertPlexStopped } from './e2e/scenarios/multiplex.mjs';

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

test('pa-1: the end is judged before the window, and the first sample may be the jump', () => {
  // A jump seen at the first sample past the window still passes: a sample
  // past the end decides it before the window is looked at.
  const held = Array.from({ length: 50 }, (_, k) => ({ t: 1, ms: 100 * k }));
  assert.deepEqual(
    decide([...held, { t: 12.2, ms: 5050 }]),
    { at: 50, verdict: { ok: true, before: 1, after: 12.2, ms: 5050 } },
    'a jump at the first sample past the window must pass',
  );
  // With no sample before the jump, the sample before it is where the play
  // was put.
  assert.deepEqual(
    decide([{ t: 12.1, ms: 100 }], { ...MARKERS, startS: 1.25 }),
    { at: 0, verdict: { ok: true, before: 1.25, after: 12.1, ms: 100 } },
    'the sample before a jump seen first is the start',
  );
});

test('pa-1: a failed read past the window ends it, and says the position is unknown', () => {
  const held = Array.from({ length: 51 }, (_, k) => ({ t: 1, ms: 100 * k }));
  const { at, verdict } = decide([...held, { t: null, ms: 5100 }]);
  assert.equal(at, 51);
  assert.equal(
    verdict.why,
    "the position must be past the range's end (12 s) within 5000 ms of playing on from 1.00 s, which only a skip can do; it is null",
  );
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
// A seek to 1 s lands at `landsAt` (the scenario takes anything from 0.9 to
// 1.5 s as the start, as a real seek may land on a frame near it), so that a
// verdict judged from a start other than the one seen shows.
function standInMpv(positionAt, { landsAt = 1 } = {}) {
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
      } else if (name === 'time-pos') pos = value === 1 ? landsAt : value;
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
  const line = logged[0].match(/^auto-skip: the intro range: from 1\.00 s, last sample before the jump (\d+\.\d\d) s, first after 12\.50 s, \d+ ms after the play resumed$/);
  assert.ok(line, `the line logged: ${logged[0]}`);
  // The play ran about 0.9 s before the jump, so the sample before it is
  // well past the start (the start itself would mean it named the wrong one).
  assert.ok(Number(line[1]) >= 1.3 && Number(line[1]) < 2, `the sample before the jump is the last one taken before it: ${logged[0]}`);
});

test('pa-1: the markers scenario judges from where the seek landed, and names the sample before the jump', STAND_IN, async (t) => {
  // The seek lands at 1.25 s; the play runs 0.9 s, is seen just past the
  // range's start (inside the margin) for 0.6 s, then is past the end.
  const mpv = standInMpv((ms) => (ms < 900 ? 1.25 + ms / 1000 : ms < 1500 ? 2.6 : 12.5), { landsAt: 1.25 });
  const logged = [];
  t.mock.method(console, 'log', (line) => logged.push(line));
  await proveSkippedPast(mpv, 'auto-skip: the intro range', 2, 12);
  assert.equal(logged.length, 1, `one line is logged: ${JSON.stringify(logged)}`);
  const line = logged[0].match(/^auto-skip: the intro range: from 1\.25 s, last sample before the jump (\d+\.\d\d) s, first after 12\.50 s, \d+ ms after the play resumed$/);
  assert.ok(line, `the line logged: ${logged[0]}`);
  assert.ok(Number(line[1]) >= 1.55 && Number(line[1]) <= 2.6, `the sample before the jump is the last one taken before it: ${logged[0]}`);
});

test('pa-1: the markers scenario fails a play that never crosses the range, once the window is over', STAND_IN, async () => {
  // Held where the seek landed: no skip comes. About 5 s.
  const mpv = standInMpv(() => 1.25, { landsAt: 1.25 });
  await assert.rejects(
    proveSkippedPast(mpv, 'auto-skip: the intro range', 2, 12),
    /^Error: auto-skip: the intro range: the position must be past the range's end \(12 s\) within 5000 ms of playing on from 1\.25 s, which only a skip can do; it is 1\.25$/,
    'the scenario must fail, by the window, a play that is never past the range',
  );
});

test('pa-1: the markers scenario fails a skip that lands just past the margin', STAND_IN, async () => {
  // The skip lands at 3.05 s, just past rangeStart + 1 s, and the play stays
  // there: a wider margin would let the window end it instead.
  const mpv = standInMpv((ms) => (ms < 900 ? 1.25 + ms / 1000 : 3.05), { landsAt: 1.25 });
  await assert.rejects(
    proveSkippedPast(mpv, 'auto-skip: the intro range', 2, 12),
    /^Error: auto-skip: the intro range: the play must jump from the range's start \(2 s\) past its end \(12 s\) in one step; it was seen inside the range at 3\.05 s \(the sample before was [\d.]+ s\), \d+ ms after resuming from 1\.25 s$/,
    'the scenario must fail a skip that lands just past the margin',
  );
});

// multiplex.mjs's rule for Plex A's timeline reports of one stopped play: the
// item is ratingKey 1, key /library/metadata/1, 10000 ms long; the play was
// sent to 6000 ms before the stop.
const ITEM = { durationMs: 10000, ratingKey: '1', key: '/library/metadata/1', seekMs: 6000 };
const report = (state, time, extra = {}) => ({
  state,
  time: String(time),
  duration: '10000',
  ratingKey: '1',
  key: '/library/metadata/1',
  ...extra,
});
// As seen in passing runs: one "playing" report while the play runs, then
// the stopped one where mpv was just before the stop.
const GOOD = [report('playing', 4916), report('stopped', 7458)];
const judgeStop = (reports, lastSeenMs = 7458) => plexStopProblems(reports, { ...ITEM, lastSeenMs });
// (lastSeenMs is 7458 unless given, undefined included.)
const WHERE = (time, ...seen) =>
  `the stopped report's position (${time} ms) is where the play stopped: mpv was at ${seen.length > 0 ? seen[0] : 7458} ms ` +
  'just before the stop, and the item ends at 10000 ms';

test('pa-7: the reports of a play stopped where it was seen pass', () => {
  assert.deepEqual(judgeStop(GOOD), []);
  // Several "playing" reports, and a stop a moment after mpv was read.
  assert.deepEqual(judgeStop([report('playing', 1000), report('playing', 4916), report('stopped', 9000)], 7458), []);
});

test("pa-7: the stopped report's time and duration swapped fails", () => {
  // The swap the first fix let through: the stop reported at the item's full
  // duration (10000), with the position as its duration.
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 10000, { duration: '7458' })]), [
    WHERE(10000),
    "the stopped report carries the item's duration (10000 ms), not 7458",
  ]);
});

test('pa-7: a stopped report away from where the play stopped fails', () => {
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 6458)]), [WHERE(6458)], 'one second early');
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 6957)]), [WHERE(6957)], 'just over 500 ms early');
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 6958)]), [], '500 ms early is allowed');
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 9459)]), [WHERE(9459)], 'more than 2 s late');
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 9458)]), [], '2 s late is allowed');
  // At the item's end, though within 2 s of where mpv was.
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 10000)], 8500), [WHERE(10000, 8500)], "at the item's end");
});

test('pa-7: no stopped report, two, or one that is not last fails', () => {
  assert.deepEqual(judgeStop([report('playing', 4916), report('playing', 7458)]), [
    'the play sends exactly one stopped timeline report, not 0',
  ]);
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 7458), report('stopped', 7458)]), [
    'the play sends exactly one stopped timeline report, not 2',
    'the stopped report comes last',
    '"playing" reports, and only those, come before it',
  ]);
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 7458), report('playing', 7000)]), [
    'the stopped report comes last',
    '"playing" reports, and only those, come before it',
  ]);
});

test('pa-7: anything but "playing" reports before the stopped one, or none, fails', () => {
  assert.deepEqual(judgeStop([report('paused', 4916), report('stopped', 7458)]), [
    '"playing" reports, and only those, come before it',
  ]);
  assert.deepEqual(judgeStop([report('stopped', 7458)]), ['"playing" reports, and only those, come before it']);
});

test('pa-7: a stopped report for another item fails, by its ratingKey and by its key', () => {
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 7458, { ratingKey: '2' })]), [
    'the stopped report names the item played by its ratingKey 1, not 2',
  ]);
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 7458, { key: '/library/metadata/2' })]), [
    'the stopped report names the item played by its key /library/metadata/1, not /library/metadata/2',
  ]);
});

test('pa-7: a stopped report short of the seek, or past the item, fails', () => {
  // Short of the seek, though where mpv was said to be.
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 5900)], 5900), [
    'the stopped report carries the final position, past the 6000 ms seek and inside the 10000 ms item, not 5900',
  ]);
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 10001)], 9500), [
    'the stopped report carries the final position, past the 6000 ms seek and inside the 10000 ms item, not 10001',
    WHERE(10001, 9500),
  ]);
});

test('pa-7: a "playing" report further on than the stop fails', () => {
  assert.deepEqual(judgeStop([report('playing', 8000), report('stopped', 7458)]), [
    'no report says the play got further than its final position',
  ]);
});

test('pa-7: a stop at exactly the seek passes, and a duration only equal as a number fails', () => {
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 6000)], 6000), [], 'a stop at exactly the 6000 ms seek');
  for (const duration of ['10000.0', '1e4']) {
    assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 7458, { duration })]), [
      `the stopped report carries the item's duration (10000 ms), not ${duration}`,
    ]);
  }
});

test('pa-7: any report further on than the stop fails, by a millisecond, or after it', () => {
  assert.deepEqual(judgeStop([report('playing', 7459), report('stopped', 7458)]), [
    'no report says the play got further than its final position',
  ]);
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 7458), report('playing', 8000)]), [
    'the stopped report comes last',
    '"playing" reports, and only those, come before it',
    'no report says the play got further than its final position',
  ]);
});

// multiplex.mjs's assertPlexStopped, on requests as Plex A's mock records
// them: the scenario must hand the rules its timeline reports only, the
// item's facts and the position it read, and fail on any sentence.
const request = (path, query) => ({ path, query, tokenPresent: true, tokenMatches: true });
const requestsOf = (playing, stopped, extra = {}) => [
  request('/library/metadata/1', {}),
  request('/:/timeline', report('playing', playing)),
  // The final progress report, which is no timeline report: it says
  // "stopped" too, further on, and must not be judged with them.
  request('/:/progress', { state: 'stopped', time: '9999', key: '1' }),
  request('/:/timeline', report('stopped', stopped, extra)),
];

test('pa-7: the multiplex scenario passes the reports of a play stopped where it was seen, and prints them', (t) => {
  const logged = [];
  t.mock.method(console, 'log', (line) => logged.push(line));
  assertPlexStopped(requestsOf(4916, 7458), 7458);
  // The position it is handed is the one judged: a play seen at 9800 ms
  // and stopped there passes too.
  assertPlexStopped(requestsOf(4916, 9800), 9800);
  assert.deepEqual(logged, [
    `Plex A timeline reports: ${JSON.stringify([report('playing', 4916), report('stopped', 7458)])}`,
    `Plex A timeline reports: ${JSON.stringify([report('playing', 4916), report('stopped', 9800)])}`,
  ]);
});

test('pa-7: the multiplex scenario fails the swap, and a stop away from the position it read', (t) => {
  t.mock.method(console, 'log', () => {});
  assert.throws(
    () => assertPlexStopped(requestsOf(4916, 10000, { duration: '7458' }), 7458),
    (error) =>
      error instanceof assert.AssertionError &&
      error.message.startsWith("Plex A's timeline reports tell it the play was playing, and where it stopped: ") &&
      JSON.stringify(error.actual) === JSON.stringify([WHERE(10000), "the stopped report carries the item's duration (10000 ms), not 7458"]),
    'the swap the first fix let through',
  );
  assert.throws(
    () => assertPlexStopped(requestsOf(4916, 7458), 9800),
    (error) => error instanceof assert.AssertionError && JSON.stringify(error.actual) === JSON.stringify([WHERE(7458, 9800)]),
    'a stop 2342 ms short of where mpv was read',
  );
  // The seek it hands the rules is playAndQuit's 6000 ms: a stop short of
  // it fails even where mpv was read.
  assert.throws(
    () => assertPlexStopped(requestsOf(4916, 5900), 5900),
    (error) =>
      error instanceof assert.AssertionError &&
      JSON.stringify(error.actual) ===
        JSON.stringify(['the stopped report carries the final position, past the 6000 ms seek and inside the 10000 ms item, not 5900']),
    'a stop short of the seek',
  );
});

test("pa-7: a position of mpv's that was not read, or a time that is no number, fails", () => {
  for (const lastSeenMs of [null, undefined, NaN]) {
    assert.deepEqual(plexStopProblems(GOOD, { ...ITEM, lastSeenMs }), [
      `the play's position just before the stop was read from mpv, not ${lastSeenMs}`,
      WHERE(7458, lastSeenMs),
    ]);
  }
  const { time: _gone, ...noTime } = report('stopped', 7458);
  assert.deepEqual(judgeStop([report('playing', 4916), noTime]), [
    'the stopped report carries the final position, past the 6000 ms seek and inside the 10000 ms item, not undefined',
    WHERE(undefined),
    'no report says the play got further than its final position',
  ]);
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 'soon')]), [
    'the stopped report carries the final position, past the 6000 ms seek and inside the 10000 ms item, not soon',
    WHERE('soon'),
    'no report says the play got further than its final position',
  ]);
});

// windowsetting.mjs waits until Vela itself is connected to a play's mpv
// before it changes or ends the play (review finding pw-8: it used to reach
// mpv, change its window and quit it within milliseconds of the socket
// appearing, now and then before Vela had connected, and mpv then ended
// without telling Vela anything). The count it waits on, from the text of
// /proc/net/unix:
test('pw-8: the connections a socket has accepted are counted from /proc/net/unix', () => {
  const one = '/run/user/1000/vela/mpv-1.sock';
  const text = [
    'Num       RefCount Protocol Flags    Type St Inode Path',
    `0000000000000000: 00000002 00000000 00010000 0001 01 101 ${one}`, // listening
    `0000000000000000: 00000003 00000000 00000000 0001 03 102 ${one}`, // accepted
    '0000000000000000: 00000003 00000000 00000000 0001 03 103', // a connecting end
    `0000000000000000: 00000003 00000000 00000000 0001 03 104 ${one}`,
    '0000000000000000: 00000003 00000000 00000000 0001 03 105 /run/user/1000/vela/mpv-2.sock',
    `0000000000000000: 00000003 00000000 00000000 0001 03 106 ${one}`,
    `0000000000000000: 00000003 00000000 00000000 0001 02 107 ${one}`, // connecting, not yet connected
    '0000000000000000: 00000002 00000000 00000000 0002 01 108 /run/systemd/notify',
    '',
  ].join('\n');
  assert.equal(acceptedConnections(text, one), 3, 'the three accepted ends, not the listening socket or one still connecting');
  assert.equal(acceptedConnections(text, '/run/user/1000/vela/mpv-2.sock'), 1);
  assert.equal(acceptedConnections(text, '/run/user/1000/vela/mpv-3.sock'), 0);
  assert.equal(acceptedConnections(text, '/run/user/1000/vela/mpv-1.soc'), 0, 'a path that only begins the same is another socket');
  assert.equal(acceptedConnections('', one), 0);
});

// And the scenario's use of it: a play is handed back, to be changed or
// ended, only after mpv has accepted three connections: the scenario's own
// and Vela's two.
test('pw-8: windowsetting hands a play back only once Vela is connected to its mpv', () => {
  const source = fs.readFileSync(new URL('./e2e/scenarios/windowsetting.mjs', import.meta.url), 'utf8');
  const start = source.indexOf('    async function play() {');
  const end = source.indexOf('    async function setFullscreen(');
  assert.ok(start > 0 && end > start, 'the scenario has its play() and what follows it');
  const play = source.slice(start, end);
  // The whole statement, as the step straight before the play is handed
  // back: a wait that is there but skipped, or that asks for fewer, is not
  // this text.
  const lastStep =
    '\n      await pollUntil(\n' +
    "        () => acceptedConnections(fs.readFileSync('/proc/net/unix', 'utf8'), socket) >= 3,\n" +
    '        "Vela\'s own connections to this mpv",\n' +
    '        { intervalMs: 20 },\n' +
    '      );\n' +
    '      return { mpv, session: recent.session_id };\n';
  assert.equal(play.split(lastStep).length, 2, "play() waits for mpv's three accepted connections as its last step before it hands the play back");
  assert.equal(play.split('return ').filter((part) => part.startsWith('{ mpv')).length, 1, 'by its one way out');
  assert.equal(source.split('acceptedConnections(').length, 2, 'the count is asked there and nowhere else');
});
