// The judgements the end-to-end scenarios make of what they observed
// (tests/e2e/judge.mjs), held here without a run of the app: each rule, taken
// out, fails a test of its own. Where a scenario's own code asks the
// judgement, a test drives that code with a stand-in for mpv, so that the
// scenario is seen to act on the verdict, not only the verdict to be right.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import ts from 'typescript';
import { acceptedConnections, plexStopProblems, skipJumpVerdict } from './e2e/judge.mjs';
import { velaConnected } from './e2e/helpers.mjs';
import { proveSkippedPast } from './e2e/scenarios/markers.mjs';
import { assertPlexStopped } from './e2e/scenarios/multiplex.mjs';
import { play as playInWindowsetting } from './e2e/scenarios/windowsetting.mjs';
import { positionsWhile } from './e2e/helpers.mjs';

// markers.mjs's rule: the play resumed from 1 s ahead of a range at 2-12 s,
// a 5000 ms window, a 1 s margin past the range's start.
const MARKERS = { startS: 1, rangeStart: 2, rangeEnd: 12, windowMs: 5000, marginS: 1, maxGapMs: 500 };

// The verdicts as the scenario asks for them, after each sample: the first
// one given, and the index of the sample it was given at.
// A sample written `{ t, ms }` was asked for and answered at `ms`; one whose
// answer waited says when it was asked for (`askedMs`).
const timed = (samples) => samples.map((sample) => ({ askedMs: sample.ms, ...sample }));
function decide(samples, rule = MARKERS) {
  const all = timed(samples);
  for (let i = 0; i < all.length; i++) {
    const verdict = skipJumpVerdict(all.slice(0, i + 1), rule);
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
  assert.deepEqual(skipJumpVerdict(timed(samples), MARKERS), verdict, 'a sample past the end after one inside the range must not pass');
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

test('pa-1: a crossing counts only inside the window, and the first sample may be the jump', () => {
  // A position first seen past the end after the window is no proof of a
  // skip: by then ordinary playback has had time to get there. (This test
  // once required the opposite, a pass at 5050 ms; the reviewer showed a
  // play with no skip certified that way, through one slow read.)
  const held = Array.from({ length: 50 }, (_, k) => ({ t: 1, ms: 100 * k }));
  assert.deepEqual(decide([...held, { t: 12.2, ms: 5050 }]), {
    at: 50,
    verdict: {
      ok: false,
      why:
        "the position was first seen past the range's end (12 s) 5050 ms after playing on from 1.00 s, " +
        'after the 5000 ms in which only a skip can get it there; it is 12.2 (the sample before was 1 s)',
    },
  });
  // At exactly the window's end it still counts.
  assert.deepEqual(decide([...held, { t: 12.2, ms: 5000 }]).verdict, { ok: true, before: 1, after: 12.2, ms: 5000 });
  // The reviewer's trace: ordinary playback, then one read that took
  // 9400 ms, and the next sample past the end with no skip at all.
  const slow = decide([...playing(1, 5), { t: 12.26, ms: 11256 }]);
  assert.equal(slow.verdict.ok, false, 'a play with no skip, seen past the end after one slow read, must fail');
  assert.match(slow.verdict.why, /^the position was first seen past the range's end \(12 s\) 11256 ms after playing on from 1\.00 s, after the 5000 ms/);
  // With no sample before the jump, the sample before it is where the play
  // was put.
  assert.deepEqual(
    decide([{ t: 12.1, ms: 100 }], { ...MARKERS, startS: 1.25 }),
    { at: 0, verdict: { ok: true, before: 1.25, after: 12.1, ms: 100 } },
    'the sample before a jump seen first is the start',
  );
});

test('pa-1: a crossing counts only if the play was watched across it', () => {
  const upTo = (ms) => Array.from({ length: ms / 100 }, (_, k) => ({ t: 1 + (k + 1) / 10, ms: 100 * (k + 1) }));
  // 500 ms between the position before and the one past the end still counts; 501 does not.
  assert.equal(decide([...upTo(900), { t: 12.0, ms: 1400 }]).verdict.ok, true);
  const gap = decide([...upTo(900), { t: 12.0, ms: 1401 }]).verdict;
  assert.equal(
    gap.why,
    "the play was not watched across the jump: 501 ms lie between the position seen before it (1.9 s) and the one past the range's end (12 s), " +
      'more than the 500 ms allowed; a skip that landed short of the end could have played the rest unseen',
  );
  // Reads that failed in between do not count as watching.
  const blind = decide([...upTo(900), { t: null, ms: 1100 }, { t: null, ms: 1300 }, { t: 12.0, ms: 1500 }]).verdict;
  assert.match(blind.why, /^the play was not watched across the jump: 600 ms lie between/);
  // A first sample that is the jump is measured from the resume.
  assert.equal(decide([{ t: 12.1, ms: 500 }]).verdict.ok, true);
  assert.match(decide([{ t: 12.1, ms: 501 }]).verdict.why, /^the play was not watched across the jump: 501 ms lie between the position seen before it \(1 s\)/);
});

test('pa-1: the gap across the jump runs from when the position before it was asked for', () => {
  // Verification round 3: a read asked for at 900 ms was answered with
  // 1.91 s and the answer then waited 3300 ms to be read; the skip had
  // landed at 9 s meanwhile and the play ran on. Timed by when they were
  // read, the two samples are 101 ms apart; the positions are 3.4 s apart.
  const upTo = (ms) => Array.from({ length: ms / 100 }, (_, k) => ({ t: 1 + (k + 1) / 10, ms: 100 * (k + 1) }));
  const waited = decide([...upTo(800), { t: 1.91, askedMs: 900, ms: 4205 }, { t: 12.31, askedMs: 4305, ms: 4306 }]).verdict;
  assert.equal(
    waited.why,
    "the play was not watched across the jump: 3406 ms lie between the position seen before it (1.91 s) and the one past the range's end (12.31 s), " +
      'more than the 500 ms allowed; a skip that landed short of the end could have played the rest unseen',
  );
  // The same two readings with the first one asked for late: it was read
  // when the play was there, and the jump was watched.
  assert.equal(decide([...upTo(800), { t: 1.91, askedMs: 4200, ms: 4205 }, { t: 12.31, askedMs: 4305, ms: 4306 }]).verdict.ok, true);
  // At the limit: asked for at 900, the one past the end answered at 1400 and at 1401.
  assert.equal(decide([...upTo(800), { t: 1.9, askedMs: 900, ms: 1200 }, { t: 12, askedMs: 1300, ms: 1400 }]).verdict.ok, true);
  assert.match(
    decide([...upTo(800), { t: 1.9, askedMs: 900, ms: 1200 }, { t: 12, askedMs: 1300, ms: 1401 }]).verdict.why,
    /^the play was not watched across the jump: 501 ms lie between/,
  );
  // A failed read's times are not the position's: the gap still runs from
  // the last position seen.
  assert.match(
    decide([...upTo(800), { t: 1.9, askedMs: 900, ms: 905 }, { t: null, askedMs: 1000, ms: 1300 }, { t: 12, askedMs: 1400, ms: 1405 }]).verdict.why,
    /^the play was not watched across the jump: 505 ms lie between the position seen before it \(1\.9 s\)/,
  );
});

test('pa-1: a sample without the time it was asked for, or asked for after it was answered, is refused', () => {
  assert.throws(() => skipJumpVerdict([{ t: 1.5, ms: 500 }], MARKERS), /^Error: skipJumpVerdict: a sample needs the time its read was asked for \(askedMs\) and the time it was answered \(ms\), in that order: \{"t":1\.5,"ms":500\}$/);
  assert.throws(() => skipJumpVerdict([{ t: 1.5, askedMs: 501, ms: 500 }], MARKERS), /in that order: \{"t":1\.5,"askedMs":501,"ms":500\}$/);
  assert.throws(() => skipJumpVerdict([{ t: null, askedMs: 100 }], MARKERS), /in that order/);
  assert.equal(skipJumpVerdict([{ t: 1.5, askedMs: 500, ms: 500 }], MARKERS), null);
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

// The same scenario code in made-up time, so that slow answers cost nothing
// to wait for and the play can be modelled honestly: it plays at one second
// per second from the moment mpv is told to resume, a skip is a jump of the
// position, and an answer can be slow. `timeline` gives the position at a
// time since the resume was asked for; `slow` gives how long the answer to a
// read begun at that time takes (0 if not given); `held` how long that
// answer, once given, then waits before the scenario gets to read it (the
// test's process busy, a reply held in a buffer: the position in it does not
// change while it waits); `resumeAnswerMs` how long the answer to the resume
// takes.
function madeUpTime(timeline, { slow = () => 0, held = () => 0, resumeAnswerMs = 0, failing = () => false } = {}) {
  let now = 1000;
  let resumeAskedAt = null;
  let paused = false;
  const clock = {
    now: () => now,
    sleep: async (ms) => {
      now += ms;
    },
  };
  const mpv = {
    async getProp(name) {
      if (name === 'pause') return paused;
      assert.equal(name, 'time-pos');
      if (resumeAskedAt === null) return 1;
      const begun = now - resumeAskedAt;
      now += slow(begun);
      if (failing(begun)) throw new Error('mpv: no reply to get_property');
      // What mpv answers is the position when it answers...
      const answer = timeline(now - resumeAskedAt);
      // ...and the scenario may get to read that answer only later.
      now += held(begun);
      return answer;
    },
    async setProp(name, value) {
      if (name === 'pause') {
        paused = value;
        if (!value) {
          resumeAskedAt = now;
          now += resumeAnswerMs;
        }
      } else assert.equal(name, 'time-pos');
    },
  };
  return { mpv, clock, elapsed: () => now - resumeAskedAt };
}
const playsOn = (ms) => 1 + ms / 1000; // no skip at all
const prove = ({ mpv, clock }) => proveSkippedPast(mpv, 'auto-skip: the intro range', 2, 12, { clock });

test('pa-1: ordinary playback seen past the range late, through one slow read, fails', async () => {
  // The reviewer's first trace: no skip, one read that takes 9400 ms, and the
  // next position is past the end, 11 seconds after the resume.
  const world = madeUpTime(playsOn, { slow: (begun) => (begun >= 1800 && begun < 1900 ? 9400 : 0) });
  await assert.rejects(prove(world), /^Error: auto-skip: the intro range: the position was first seen past the range's end \(12 s\) \d+ ms after playing on from 1\.00 s, after the 5000 ms/);
});

test('pa-1: the time a slow answer to the resume takes counts against the window', async () => {
  // Verification round 2: the clock used to start when the resume had been
  // answered. No skip; the answer to the resume takes 9400 ms, the next read
  // 1800 ms: the play is at 12.2 s, 11.2 real seconds on.
  const world = madeUpTime(playsOn, { resumeAnswerMs: 9400, slow: (begun) => (begun < 9500 ? 1800 : 0) });
  await assert.rejects(prove(world), /^Error: auto-skip: the intro range: the position was first seen past the range's end \(12 s\) 11\d\d\d ms after playing on from 1\.00 s, after the 5000 ms/);
});

test('pa-1: a skip that lands short and plays the rest behind a slow read, or behind reads that fail, fails', async () => {
  // Verification round 2: the skip lands at 9 s one second in and the play
  // goes on from there; a read begun at about 900 ms takes 3300 ms, so the
  // next position seen is past the end, inside the window, and nothing was
  // seen in between.
  const landsShort = (ms) => (ms < 1000 ? 1 + ms / 1000 : 9 + (ms - 1000) / 1000);
  const slowRead = madeUpTime(landsShort, { slow: (begun) => (begun >= 900 && begun < 1000 ? 3300 : 0) });
  await assert.rejects(
    prove(slowRead),
    /^Error: auto-skip: the intro range: the play was not watched across the jump: 3\d\d\d ms lie between the position seen before it \(1\.\d+ s\) and the one past the range's end \(12\.\d+ s\), more than the 500 ms allowed/,
  );
  // The same with reads that fail all through that time.
  const failedReads = madeUpTime(landsShort, { failing: (begun) => begun >= 900 && begun < 4100 });
  await assert.rejects(prove(failedReads), /^Error: auto-skip: the intro range: the play was not watched across the jump: 3\d\d\d ms lie between/);
});

test('pa-1: a skip that lands short behind an answer that waits to be read fails', async () => {
  // Verification round 3, through the scenario's own loop: the read asked
  // for at 900 ms is answered at once (1.9 s) and the answer waits 3300 ms;
  // the skip lands at 9 s at 1000 ms and the play goes on. The next read,
  // 100 ms after the first was at last read, is past the end.
  const landsShort = (ms) => (ms < 1000 ? 1 + ms / 1000 : 9 + (ms - 1000) / 1000);
  const waits = madeUpTime(landsShort, { held: (begun) => (begun >= 900 && begun < 1000 ? 3300 : 0) });
  await assert.rejects(
    prove(waits),
    /^Error: auto-skip: the intro range: the play was not watched across the jump: 3400 ms lie between the position seen before it \(1\.9 s\) and the one past the range's end \(12\.3 s\), more than the 500 ms allowed/,
  );
  // Every answer waiting a little is still watching: 150 ms each, with the
  // 100 ms between reads, puts 400 ms across the jump.
  const skips = (ms) => (ms < 1000 ? 1 + ms / 1000 : 12 + (ms - 1000) / 1000);
  await prove(madeUpTime(skips, { held: () => 150 }));
  // 250 ms each puts 600 ms across it: the proof does not hold, and says so.
  await assert.rejects(prove(madeUpTime(skips, { held: () => 250 })), /the play was not watched across the jump: 600 ms lie between/);
});

test('pa-1: a real skip passes in made-up time, and a watched one a little slow still does', async (t) => {
  const logged = [];
  t.mock.method(console, 'log', (line) => logged.push(line));
  const skips = (ms) => (ms < 1000 ? 1 + ms / 1000 : 12 + (ms - 1000) / 1000);
  await prove(madeUpTime(skips));
  // Reads that each take 150 ms: 400 ms lie between asking for the position
  // before the jump and the answer past it, inside the gap allowed.
  await prove(madeUpTime(skips, { slow: () => 150 }));
  assert.equal(logged.length, 2, JSON.stringify(logged));
  // Reads that each take 300 ms put 700 ms there: too slow to say that the
  // play was watched across the jump, though it was a real skip.
  await assert.rejects(prove(madeUpTime(skips, { slow: () => 300 })), /the play was not watched across the jump: 700 ms lie between/);
  assert.match(logged[0], /^auto-skip: the intro range: from 1\.00 s, last sample before the jump 1\.9\d s, first after 12\.\d\d s, 1\d\d\d ms after the play resumed$/);
});

test('pa-1: the scenario counts time on a clock that cannot go back', STAND_IN, async (t) => {
  // Verification round 2: with the wall clock, a clock set back during the
  // run hid seconds. The scenario's own clock is the process's monotonic one;
  // here the wall clock runs backwards all through a real one-jump run.
  const source = fs.readFileSync(new URL('./e2e/scenarios/markers.mjs', import.meta.url), 'utf8');
  const body = source.slice(source.indexOf('export async function proveSkippedPast('), source.indexOf('\nasync function endSession('));
  assert.doesNotMatch(body, /Date\.now|new Date/, 'proveSkippedPast reads no wall clock');
  assert.match(source, /const REAL_CLOCK = \{\n {2}now: \(\) => performance\.now\(\),/);
  const mpv = standInMpv((ms) => (ms < 900 ? 1 + ms / 1000 : 12.5));
  const logged = [];
  t.mock.method(console, 'log', (line) => logged.push(line));
  await proveSkippedPast(mpv, 'auto-skip: the intro range', 2, 12);
  const ms = Number(logged[0].match(/(\d+) ms after the play resumed$/)[1]);
  assert.ok(ms >= 800 && ms < 2500, `the jump is timed by the real elapsed time: ${logged[0]}`);
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
  `the stopped report's position (${time} ms) is where the play stopped: mpv was last seen at ${seen.length > 0 ? seen[0] : 7458} ms ` +
  'as it was stopped, and the item ends at 10000 ms';

test('pa-7: the reports of a play stopped where it was seen pass', () => {
  assert.deepEqual(judgeStop(GOOD), []);
  // Several "playing" reports, and a stop a moment after mpv was read.
  assert.deepEqual(judgeStop([report('playing', 1000), report('playing', 4916), report('stopped', 7900)], 7458), []);
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
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 7959)]), [WHERE(7959)], 'just over 500 ms late');
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 7958)]), [], '500 ms late is allowed');
  // The reviewer's case: a stop that acted at once, and a report a second
  // and a half on, which a two-second allowance let through.
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 8958)]), [WHERE(8958)], '1.5 s late');
  // At the item's end, though within 500 ms of where mpv was.
  assert.deepEqual(judgeStop([report('playing', 4916), report('stopped', 10000)], 9700), [WHERE(10000, 9700)], "at the item's end");
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
      `the play's position as it was stopped was read from mpv, not ${lastSeenMs}`,
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

// The position the stopped report is held to is read all through the stop
// (helpers.mjs, positionsWhile; review finding pa-7, verification round 1).
// A stand-in for mpv whose play goes on at one second per second from
// `fromMs`, and which can stop answering.
function playingStandIn(fromMs, { goneAfterMs = Infinity } = {}) {
  const started = Date.now();
  return {
    getProp(name) {
      assert.equal(name, 'time-pos');
      const elapsed = Date.now() - started;
      if (elapsed > goneAfterMs) return new Promise(() => {});
      return Promise.resolve((fromMs + elapsed) / 1000);
    },
  };
}
const after = (ms, value) => new Promise((resolve) => setTimeout(() => resolve(value), ms));

test('pa-7: the position is read all through a stop that takes long, and the report is held to the last reading', async (t) => {
  t.mock.method(console, 'log', () => {});
  // The reviewer's case: the play is at 7458 ms when the stop begins, the
  // embedded view's controls take 2250 ms to come up, and the play is
  // stopped, correctly, about 9700 ms in.
  const readings = [];
  const result = await positionsWhile(playingStandIn(7458), () => after(2250, 'stopped'), (seconds) => readings.push(Math.round(seconds * 1000)));
  assert.equal(result, 'stopped', 'what the action returns is returned');
  const last = readings.at(-1);
  assert.ok(readings.length >= 20, `it read all through: ${readings.length} readings`);
  assert.ok(readings[0] < 7600, `from the start of the stop: the first reading is ${readings[0]}`);
  assert.ok(last >= 9558 && last <= 9758, `to its end: the last reading is ${last}, within 150 ms of where the stop acted`);
  // A correct report at the stop passes against the last reading, and would
  // not against a reading taken before the stop began.
  assertPlexStopped(requestsOf(4916, 9717), last);
  assert.throws(() => assertPlexStopped(requestsOf(4916, 9717), 7458), assert.AssertionError);
});

test('pa-7: reading the position ends with the stop, when mpv has gone, and when the stop fails', async () => {
  // mpv stops answering 300 ms in; the stop takes 800 ms: the readings end
  // where mpv was last heard, and the wait is the stop's, not a read's.
  const readings = [];
  const started = Date.now();
  await positionsWhile(playingStandIn(7000, { goneAfterMs: 300 }), () => after(800), (seconds) => readings.push(Math.round(seconds * 1000)));
  const took = Date.now() - started;
  assert.ok(readings.length >= 3 && readings.at(-1) <= 7300, `the readings end where mpv was last heard: ${readings.at(-1)}`);
  assert.ok(took < 1500, `the wait is the stop's (800 ms), not a read's: ${took} ms`);
  // Nothing is handed over once the action has ended.
  const late = [];
  await positionsWhile(playingStandIn(7000), () => after(120), (seconds) => late.push(seconds));
  const count = late.length;
  await after(200);
  assert.equal(late.length, count, 'no reading arrives after the action has ended');
  // A stop that ends the play at once, and the connection with it (the
  // external player's: mpv's own quit), still has the reading taken before
  // it began.
  let gone = false;
  const startedAt = Date.now();
  const quits = {
    getProp: () => (gone ? new Promise(() => {}) : Promise.resolve((7458 + (Date.now() - startedAt)) / 1000)),
  };
  const atOnce = [];
  await positionsWhile(
    quits,
    () => {
      gone = true;
      return after(100);
    },
    (seconds) => atOnce.push(Math.round(seconds * 1000)),
  );
  assert.deepEqual(atOnce.length, 1, `one reading, taken before the stop began: ${JSON.stringify(atOnce)}`);
  assert.ok(atOnce[0] >= 7458 && atOnce[0] < 7558, `where the play was then: ${atOnce[0]}`);
  // A stop that fails is not swallowed, and the reading stops with it.
  await assert.rejects(positionsWhile(playingStandIn(7000), () => after(50).then(() => Promise.reject(new Error('the stop failed'))), () => {}), /the stop failed/);
});

// And the wiring: playAndQuit reads through its stop when asked, and
// multiplex asks.
test('pa-7: multiplex has the position read through the stop of its play', () => {
  const helpers = fs.readFileSync(new URL('./e2e/helpers.mjs', import.meta.url), 'utf8');
  const quit = helpers.slice(helpers.indexOf('export async function playAndQuit('), helpers.indexOf('\n}\n', helpers.indexOf('export async function playAndQuit(')));
  assert.equal(
    quit.split(
      "    const stop = () => stopPlayAsUser(driver, mpv, socketPath, 'the play', { key });\n" +
        '    if (positionAtStop) await positionsWhile(mpv, stop, positionAtStop);\n' +
        '    else await stop();\n',
    ).length,
    2,
    'playAndQuit stops the play once, reading its position through the stop when asked',
  );
  assert.equal(quit.split('stopPlayAsUser(').length, 2, 'and stops it nowhere else');
  const multiplex = fs.readFileSync(new URL('./e2e/scenarios/multiplex.mjs', import.meta.url), 'utf8');
  assert.equal(
    multiplex.split('      positionAtStop: (seconds) => {\n        lastSeenMs = Math.round(seconds * 1000);\n      },\n').length,
    2,
    'multiplex keeps the last reading',
  );
  assert.equal(multiplex.split('lastSeenMs = ').length, 3, 'and sets it nowhere else (its declaration apart)');
  assert.equal(multiplex.split('assertPlexStopped(mockA.state.requests, lastSeenMs);').length, 2, 'and judges the stopped report against it');
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

const SOCKET = '/tmp/vela-test/mpv-1.sock';
const procRow = (state, socket = SOCKET) => `0000000000000000: 00000003 00000000 00000000 0001 ${state} 123 ${socket}`;
const procWithClients = (count) => [
  'Num RefCount Protocol Flags Type St Inode Path',
  procRow('01'), // listener
  procRow('02'), // connecting
  procRow('03', ''), // unnamed peer
  procRow('03', `${SOCKET}.other`), // another play
  ...Array.from({ length: count }, () => procRow('03')),
].join('\n');

for (const connections of [3, 4, 5]) {
  test(`pw-8: velaConnected waits for all ${connections} clients on this socket`, async () => {
    let reads = 0;
    await velaConnected(SOCKET, connections, {
      readProcNetUnix: () => {
        reads++;
        assert.ok(reads <= connections, 'the exact requested count must release the wait');
        return procWithClients(reads);
      },
      intervalMs: 0,
    });
    assert.equal(reads, connections, 'every missing client must keep the wait pending');
  });
}

test('pw-8: velaConnected accepts extra clients and reads /proc/net/unix by default', async (t) => {
  const reads = [];
  t.mock.method(fs, 'readFileSync', (...args) => {
    reads.push(args);
    return procWithClients(4);
  });
  await velaConnected(SOCKET, 3, { timeoutMs: 0, intervalMs: 0 });
  assert.deepEqual(reads, [['/proc/net/unix', 'utf8']], 'the live wait reads the kernel socket table');
});

test('pw-8: velaConnected times out naming the socket and required count', async () => {
  await assert.rejects(
    velaConnected(SOCKET, 4, { readProcNetUnix: () => procWithClients(3), timeoutMs: 0, intervalMs: 1 }),
    { message: `timed out waiting for mpv at ${SOCKET} to accept 4 connections (Vela and the scenario)` },
  );
});

// And the scenario's use of it: a play is handed back, to be changed or
// ended, only after mpv has accepted three connections: the scenario's own
// and Vela's two. The scenario's own play() is run, with stand-ins for the
// app's page and for mpv, and with the kernel's socket table made up: the
// shared velaConnected the scenario imports reads it (no socket, no display).
test('pw-8: windowsetting hands a play back only once Vela is connected to its mpv', async (t) => {
  let connections = 0;
  let tableReads = 0;
  t.mock.method(fs, 'readFileSync', (file, encoding) => {
    assert.deepEqual([file, encoding], ['/proc/net/unix', 'utf8'], 'play() reads the kernel socket table and nothing else');
    tableReads++;
    return procWithClients(connections);
  });
  let started = false;
  const mpv = {
    getProp: async (name) => (name === 'path' ? '/media/stream.mp4' : null),
    setProp: async () => {},
  };
  const appAndMpv = {
    openLibraryGrid: async () => {},
    mpvSocketSnapshot: () => new Set(),
    openDetailAndPlay: async () => {
      started = true;
    },
    waitForNewMpvSocket: async () => SOCKET,
    connectMpv: async (socket) => {
      assert.equal(socket, SOCKET);
      return mpv;
    },
  };
  // The start of this play is recorded once the play has been started.
  const readConfigSafely = () => ({
    recents: [started ? { session_id: 'this play', ended_at_ms: 0 } : { session_id: 'the last play', ended_at_ms: 1 }],
  });
  let handedBack = null;
  const playing = playInWindowsetting({ driver: {}, readConfigSafely }, appAndMpv).then((played) => {
    handedBack = played;
    return played;
  });
  for (const count of [0, 1, 2]) {
    connections = count;
    // The wait has looked at the table twice with this count, or the play
    // has been handed back.
    const before = tableReads;
    const deadline = Date.now() + 5000;
    while (tableReads < before + 2 && handedBack === null && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 1));
    }
    assert.equal(handedBack, null, `the play was handed back with ${count} of the three connections`);
    assert.ok(tableReads >= before + 2, `the wait kept looking with ${count} connections`);
  }
  connections = 3;
  const played = await playing;
  assert.equal(played.mpv, mpv, 'the play is handed back once the three are there');
  assert.equal(played.session, 'this play');
});

test('pw-8: velaConnected rejects invalid counts before reading the kernel table', async () => {
  for (const count of [0, -1, 2.5, NaN, undefined]) {
    await assert.rejects(
      velaConnected(SOCKET, count, { readProcNetUnix: () => { throw new Error('must not read'); } }),
      { message: 'connections must be a positive integer' },
    );
  }
});

// Hold the wiring as executable statements, not a comment or a call hidden
// in an unused function/branch. Each await must be directly in its existing
// function's block (or try / the mergedview finish arm), immediately before
// the named step. The pairs below are the survey's observation boundaries.
// Using the parser also rejects `if (false) await ...`, a detached promise,
// and an early return that leaves the text of the call present but dead.
const BOTH = "runPlayer === 'embedded' ? 4 : 3";
const CONNECTION_SITES = [
  ['helpers.mjs', 'playAndQuit', 'socketPath', BOTH, 'if (during) await during(mpv);'],
  ['scenarios/windowsetting.mjs', 'play', 'socket', '3', 'return { mpv, session: recent.session_id };'],
  ['scenarios/continueoff.mjs', 'run', 'socket', BOTH, "await mpv.setProp('pause', true);"],
  ['scenarios/continueon.mjs', 'nextMpv', 'socket', BOTH, "await mpv.setProp('pause', true);"],
  ['scenarios/continuetv.mjs', 'nextMpv', 'socket', BOTH, "await mpv.setProp('pause', true);"],
  ['scenarios/completionhub.mjs', 'startCompletedEpisode', 'socket', BOTH, "await mpv.setProp('pause', true);"],
  ['scenarios/playlistplay.mjs', 'nextMpv', 'socket', BOTH, 'return { mpv, socket };'],
  ['scenarios/serverplaylists.mjs', 'nextPlayer', 'socket', BOTH, 'return mpv;'],
  ['scenarios/replaceplay.mjs', 'playing', 'socketPath', BOTH, 'return mpv;'],
  ['scenarios/playback.mjs', 'run', 'socketPath', BOTH, "await mpv.setProp('time-pos', 6);"],
  ['scenarios/watchstate.mjs', 'run', 'socketPath', BOTH, "await mpv.setProp('time-pos', 6);"],
  ['scenarios/mergedview.mjs', 'playFromMenu', 'socketPath', BOTH, "await mpv.setProp('pause', true);", 'finish'],
  ['scenarios/autocrop.mjs', 'playAndAssertCrop', 'socketPath', '3', 'if (park !== undefined)'],
  ['scenarios/embeddedcrop.mjs', 'play', 'socketPath', '4', 'return { mpv, socketPath, first };'],
  ['scenarios/embeddedfullscreenend.mjs', 'run', 'socketPath', '4', 'await viewReady(driver);', null, 4],
  ['live/plex.mjs', 'completeEpisodeAndContinue', 'firstSocket', BOTH, 'await first.setProp("time-pos", Math.max(0, duration - 0.8));'],
];

for (const [file, owner, socket, count, next, branch = null, occurrences = 1] of CONNECTION_SITES) {
  test(`pw-8: ${file} ${owner} awaits Vela before ${next}`, () => {
    const source = fs.readFileSync(new URL(`./e2e/${file}`, import.meta.url), 'utf8');
    const tree = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
    const calls = [];
    const visit = (node) => {
      if (ts.isCallExpression(node) && node.expression.getText(tree) === 'velaConnected') calls.push(node);
      ts.forEachChild(node, visit);
    };
    visit(tree);
    assert.equal(calls.length, occurrences, `${file}: every surveyed play must call velaConnected`);
    if (file !== 'helpers.mjs') {
      assert.ok(tree.statements.some((node) => ts.isImportDeclaration(node)
        && node.moduleSpecifier.text === '../helpers.mjs'
        && node.importClause?.namedBindings?.elements?.some((entry) => entry.name.text === 'velaConnected' && !entry.propertyName)),
      `${file}: the wait comes from the shared helper`);
    }
    for (const call of calls) {
      assert.deepEqual(call.arguments.map((arg) => arg.getText(tree)), [socket, count], `${file}: the derived connection count and this play's socket`);
      assert.ok(ts.isAwaitExpression(call.parent) && ts.isExpressionStatement(call.parent.parent), `${file}: the connection wait is awaited`);
      const statement = call.parent.parent;
      assert.ok(ts.isBlock(statement.parent), `${file}: the wait is an unconditional statement in its block`);
      const statements = statement.parent.statements;
      const index = statements.indexOf(statement);
      assert.ok(statements[index + 1]?.getText(tree).startsWith(next), `${file}: the connection wait must immediately precede ${next}`);
      let conditional = null;
      let node = statement;
      while (node && !ts.isFunctionDeclaration(node) && !ts.isMethodDeclaration(node)) {
        const parent = node.parent;
        if (ts.isBlock(parent)) {
          const preceding = parent.statements.slice(0, parent.statements.indexOf(node));
          assert.ok(!preceding.some((s) => ts.isReturnStatement(s) || ts.isThrowStatement(s)), `${file}: the wait is reachable`);
        } else if (ts.isIfStatement(parent)) {
          assert.equal(conditional, null, `${file}: no extra branch may skip the wait`);
          assert.equal(node, parent.thenStatement, `${file}: the wait is in the expected arm`);
          conditional = parent.expression.getText(tree);
        } else {
          assert.ok(ts.isTryStatement(parent) || ts.isFunctionDeclaration(parent) || ts.isMethodDeclaration(parent), `${file}: no wrapper may defer or skip the wait`);
        }
        node = parent;
      }
      assert.equal(node?.name?.getText(tree), owner, `${file}: the wait is in the play-driving function`);
      assert.equal(conditional, branch, `${file}: only the surveyed branch encloses the wait`);
    }
  });
}

// mergedview opens a context menu on its card by a script, which fails on a
// card that is not on the page (seen once, right after a play was stopped).
test('mergedview waits for its card to be on the page before it opens the menu on it', () => {
  const source = fs.readFileSync(new URL('./e2e/scenarios/mergedview.mjs', import.meta.url), 'utf8');
  const start = source.indexOf('\nasync function playFromMenu(');
  assert.notEqual(start, -1, 'playFromMenu exists');
  // Comments out, so that a wait that is only described does not count.
  const body = source.slice(start, source.indexOf('\n}\n', start)).replace(/^\s*\/\/[^\n]*\n/gm, '');
  assert.match(
    body,
    /const before = mpvSocketSnapshot\(\);\n {2}await driver\.waitFor\(\n {4}`return !!document\.querySelector\('button\.poster\[aria-label\^="Mock Movie"\]'\)`,\n {4}'the merged card on the page',\n {2}\);\n {2}await driver\.exec\(\n {4}`const el = document\.querySelector\('button\.poster\[aria-label\^="Mock Movie"\]'\);/,
    'the wait for the card comes directly before the script that uses it, unconditionally, and for the same card',
  );
});
