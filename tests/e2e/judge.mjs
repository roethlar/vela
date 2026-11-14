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

// What is wrong with the timeline reports Plex received for one play that was
// stopped (review finding pa-7; multiplex.mjs): `reports` are the query
// objects of the play's /:/timeline requests, in the order they came;
// `lastSeenMs` is the play's position read from mpv just before the stop;
// `seekMs` the position the play was sent to before it; `durationMs`,
// `ratingKey` and `key` the item's. One sentence per broken rule, none when
// all is well:
// - exactly one report says state=stopped, and it is the last;
// - before the last there is at least one report, and all say "playing";
// - the stopped report names the item (its ratingKey, and its key);
// - its time is past the seek and inside the item;
// - its time is where the play stopped: no earlier than lastSeenMs - 500, no
//   later than lastSeenMs + 2000 (the stop takes a moment, never seconds),
//   and before the item's end (a report at the full duration tells Plex it
//   was watched to the end); lastSeenMs must have been read;
// - it carries the item's duration;
// - no report says the play got further than the stopped report's time.
export function plexStopProblems(reports, { lastSeenMs, durationMs, ratingKey, key, seekMs }) {
  const stopped = reports.filter((q) => q.state === 'stopped');
  const one = `the play sends exactly one stopped timeline report, not ${stopped.length}`;
  // With none there is nothing more to judge; with more, the first is judged.
  if (stopped.length === 0) return [one];
  const problems = stopped.length === 1 ? [] : [one];
  const report = stopped[0];
  if (reports.at(-1) !== report) problems.push('the stopped report comes last');
  const earlier = reports.slice(0, -1);
  if (!(earlier.length > 0 && earlier.every((q) => q.state === 'playing'))) {
    problems.push('"playing" reports, and only those, come before it');
  }
  if (report.ratingKey !== ratingKey) {
    problems.push(`the stopped report names the item played by its ratingKey ${ratingKey}, not ${report.ratingKey}`);
  }
  if (report.key !== key) problems.push(`the stopped report names the item played by its key ${key}, not ${report.key}`);
  const finalMs = Number(report.time);
  if (!(finalMs >= seekMs && finalMs <= durationMs)) {
    problems.push(
      `the stopped report carries the final position, past the ${seekMs} ms seek and inside the ${durationMs} ms item, not ${report.time}`,
    );
  }
  if (!Number.isFinite(lastSeenMs)) {
    problems.push(`the play's position just before the stop was read from mpv, not ${lastSeenMs}`);
  }
  if (!(finalMs >= lastSeenMs - 500 && finalMs <= lastSeenMs + 2000 && finalMs < durationMs)) {
    problems.push(
      `the stopped report's position (${report.time} ms) is where the play stopped: mpv was at ${lastSeenMs} ms ` +
        `just before the stop, and the item ends at ${durationMs} ms`,
    );
  }
  if (report.duration !== String(durationMs)) {
    problems.push(`the stopped report carries the item's duration (${durationMs} ms), not ${report.duration}`);
  }
  if (!reports.every((q) => Number(q.time) <= finalMs)) {
    problems.push('no report says the play got further than its final position');
  }
  return problems;
}
