// Judgements of what a scenario observed, as pure functions: they take what
// was seen and say whether it is right, and nothing here watches, waits or
// acts. The scenarios do the observing and ask these functions; the unit tests
// (tests/e2e-judge.test.mjs, part of `npm run check`) hold the rules, so that a
// rule taken out is seen without a run of the app.
//
// Importing this module does nothing.

// Whether the play SKIPPED a range rather than playing through it (review
// finding pa-1; markers.mjs, proveSkippedPast). The play was put at `startS`,
// just ahead of the range [rangeStart, rangeEnd] (seconds), and resumed; each
// sample is `{ t, ms }`: its position in seconds (null for a read that
// failed) and the milliseconds since the play resumed. The samples are judged
// in order:
// - one at or past the range's end decides it: the skip happened, and the
//   last position seen before it (`before`) and the sample past the end
//   (`after`, at `ms`) are returned;
// - one at or past rangeStart + marginS, short of the end, fails: the play
//   was seen inside the range, so whatever crossed it was not one jump (the
//   margin allows for the sample that sees the play just past the start,
//   before the skip lands);
// - once `windowMs` is over with neither, it fails: from startS ordinary
//   playback cannot cross the range in that time, only a skip can.
// Returns null while undecided, `{ ok: true, before, after, ms }` or
// `{ ok: false, why }`.
export function skipJumpVerdict(samples, { startS, rangeStart, rangeEnd, windowMs, marginS }) {
  let before = startS;
  let last = null;
  for (const { t, ms } of samples) {
    if (t != null && t >= rangeEnd) return { ok: true, before, after: t, ms };
    if (t != null && t >= rangeStart + marginS) {
      return {
        ok: false,
        why:
          `the play must jump from the range's start (${rangeStart} s) past its end ` +
          `(${rangeEnd} s) in one step; it was seen inside the range at ${t} s ` +
          `(the sample before was ${before} s), ${ms} ms after resuming from ${startS.toFixed(2)} s`,
      };
    }
    if (t != null) before = t;
    last = t;
    if (ms > windowMs) {
      return {
        ok: false,
        why:
          `the position must be past the range's end (${rangeEnd} s) within ${windowMs} ms ` +
          `of playing on from ${startS.toFixed(2)} s, which only a skip can do; it is ${last}`,
      };
    }
  }
  return null;
}
