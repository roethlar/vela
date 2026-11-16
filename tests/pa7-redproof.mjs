// Manual, serial mutation proof for the pa-7 port. Run from the repo root:
//   node tests/pa7-redproof.mjs [id-regexp]
// Changes tests only, restoring the exact pre-mutation bytes in finally.
// Do not run other tests or edit these files concurrently. No git writes.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';

const helper = 'tests/e2e/helpers.mjs';
const judge = 'tests/e2e/judge.mjs';
const scenario = 'tests/e2e/scenarios/multiplex.mjs';
const unit = 'tests/e2e-judge.test.mjs';
const good = 'a play held still is stopped where it was held';
const near = 'a play is not held where it has been near';
const frame = 'the measured hold has its own exclusion band';
const callback = 'the held-position callback finishes before the stop';
const finalCheck = 'the final recheck refuses each way';
const wire = 'multiplex has its play held still before the stop';
const count = 'no stopped report, duplicate stops';
const cases = [];
const add = (id, file, test, before, after, reason) => cases.push({ id, file, test, edits: [[before, after]], reason });

add('H01-pause', helper, good, "  await mpv.setProp('pause', true);\n  const pausedAt", '  const pausedAt', 'mpv: the play has ended (mpv last said {"pause":false');
add('H02-pause-state', helper, 'a play is held only while mpv says', 'last.pause === true && last.seeking', 'last.seeking', 'the play is still held');
add('H03-seek-state', helper, 'a play is taken as held only once its move has landed', "last.seeking === false && typeof", 'typeof', 'the play is still held');
add('H04-steady-count', helper, near, 'run.length >= steadyReads', 'run.length >= 1', 'Missing expected rejection');
add('H05-steady-position', helper, frame, 'run[0] === last.t', 'true', 'callbackReads');
add('H06-move', helper, good, "  await mpv.setProp('time-pos', atS);", '', 'timed out waiting for mpv to hold the play paused at 9 s');
add('H07-landing-wide', helper, frame, 'Math.round(t * 1000) - atMs) <= 50', 'Math.round(t * 1000) - atMs) <= 51', 'Missing expected rejection');
add('H08-landing-narrow', helper, frame, 'Math.round(t * 1000) - atMs) <= 50', 'Math.round(t * 1000) - atMs) <= 49', 'timed out waiting for mpv to hold the play paused at 9 s');
add('H09-pause-distance', helper, near, 'Math.max(furthestMs, Math.round(pausedAt * 1000))', 'furthestMs', 'Missing expected rejection');
add('H10-prior-distance', helper, near, 'Math.max(furthestMs, Math.round(pausedAt * 1000))', 'Math.round(pausedAt * 1000)', 'Missing expected rejection');
add('H11-distance-edge', helper, near, 'furthest < atMs - toleranceMs', 'furthest <= atMs - toleranceMs', 'Missing expected rejection');
cases.at(-1).edits.push(['furthest < Math.round(heldAt * 1000) - toleranceMs', 'furthest <= Math.round(heldAt * 1000) - toleranceMs']);
add('H12-measured-distance', helper, frame, 'furthest < Math.round(heldAt * 1000) - toleranceMs', 'true', 'Missing expected rejection');
add('H13-await-caller', helper, good, '  await positionAtStop(heldAt);', '  void positionAtStop(heldAt);', "the stop is asked only once the caller's wait for Vela is over");
add('H14-late-callback', helper, callback, '  await stop();\n  return heldAt;', '  await stop();\n  await positionAtStop(heldAt);\n  return heldAt;', 'no callback after the action ended');
for (const [id, property] of [['H15-final-pause', 'pause: true'], ['H16-final-seek', 'seeking: false'], ['H17-final-position', 't: heldAt']]) {
  add(id, helper, finalCheck, '    now,\n    { pause: true', `    { ...now, ${property} },\n    { pause: true`, 'Missing expected rejection');
}
add('H18-await-stop', helper, callback, '  await stop();\n  return heldAt;', '  void stop();\n  return heldAt;', 'the stop must finish before stopStill returns');
add('H19-stop-error', helper, callback, '  await stop();\n  return heldAt;', '  try { await stop(); } catch {}\n  return heldAt;', 'Missing expected rejection');
add('H20-caller-error', helper, callback, '  await positionAtStop(heldAt);', '  try { await positionAtStop(heldAt); } catch {}', 'Missing expected rejection');
add('H21-return-measured', helper, frame, '  return heldAt;', '  return atS;', '8.96');
add('H22-deliver-measured', helper, callback, '  await positionAtStop(heldAt);', '  await positionAtStop(heldAt - 1);', '8 !== 9');
add('E01-external-delay', helper, 'external user stop preserves the hold', '  await stop();\n  return heldAt;', "  await mpv.setProp('pause', false);\n  await stop();\n  return heldAt;", 'the engine still ends at the measured hold');
add('E02-embedded-delay', helper, 'embedded user stop preserves the hold', '  await stop();\n  return heldAt;', "  await mpv.setProp('pause', false);\n  await stop();\n  return heldAt;", 'the scenario loses its IPC before the play ends');

add('P01-timeline', scenario, near, "          request.path === '/:/timeline' &&\n", '', 'Missing expected rejection');
add('P02-playing', scenario, near, "          request.query.state === 'playing' &&\n", '', 'Missing expected rejection');
add('P03-held-lower', scenario, near, 'Math.abs(Number(request.query.time) - heldAtMs)', '(Number(request.query.time) - heldAtMs)', 'Missing expected rejection');
add('P04-held-upper', scenario, near, 'Math.abs(Number(request.query.time) - heldAtMs)', '(heldAtMs - Number(request.query.time))', 'Missing expected rejection');
add('P05-held-edge', scenario, near, '<= STILL_TOLERANCE_MS', '< STILL_TOLERANCE_MS', 'timed out waiting for a');

add('W01-hold-wired', helper, wire, 'if (positionAtStop) await stopStill(mpv, stop, { positionAtStop, furthestMs: beforeSeekMs });', 'if (positionAtStop) await stop();', 'playAndQuit stops the play once, held still first when asked');
add('W02-prior-position', helper, wire, "const beforeSeekMs = Math.round((await mpv.getProp('time-pos')) * 1000);", 'const beforeSeekMs = 6000;', 'tells stopStill how far the play got before its seek');
add('W03-caller-wait', scenario, wire, '        await untilPlexSawHeld(() => mockA.state.requests, lastSeenMs);', '', 'multiplex keeps where the play was held');
add('W04-caller-position', scenario, wire, '        lastSeenMs = Math.round(seconds * 1000);', '        lastSeenMs = 9000;', 'multiplex keeps where the play was held');
add('W05-judge-wired', scenario, wire, '    assertPlexStopped(mockA.state.requests, lastSeenMs);', '', 'judges the stopped report against it');
add('W06-ordinary-stop', helper, wire, '    else await stop();', '    else await stopStill(mpv, stop, { positionAtStop, furthestMs: beforeSeekMs });', 'playAndQuit stops the play once, held still first when asked');

add('J01-missing-stop', judge, count, 'if (stopped.length === 0) return [one];', 'if (stopped.length === 0) return [];', 'not 0');
add('J02-duplicate-stops', judge, count, 'stopped.length === 1', 'true', 'not 2');
add('J05-final-order', judge, count, 'if (reports.at(-1) !== report)', 'if (false)', 'the stopped report comes last');
add('J07-playing-before', judge, 'anything but "playing" reports before', 'if (!(earlier.length > 0 && earlier.every((q) => q.state === \'playing\')))', 'if (false)', '"playing" reports, and only those');
add('J08-early-position', judge, 'a stopped report away from where', 'finalMs >= lastSeenMs - 500', 'finalMs >= lastSeenMs - 2000', '6458');
add('J09-late-position', judge, 'a stopped report away from where', 'finalMs <= lastSeenMs + 500', 'finalMs <= lastSeenMs + 2000', '7959');
add('J10-item-end', judge, 'a stopped report away from where', '&& finalMs < durationMs', '', '10000');
add('J11-observation', judge, 'a position of mpv', 'if (!Number.isFinite(lastSeenMs))', 'if (false)', 'read from mpv');
add('J12-monotonic', judge, 'any report further on than the stop fails', 'if (!reports.every((q) => Number(q.time) <= finalMs))', 'if (false)', 'no report says the play got further');
add('J13-duration', judge, "the stopped report's time and duration swapped", 'if (report.duration !== String(durationMs))', 'if (false)', "the stopped report carries the item's duration");
add('J14-ratingKey', judge, 'a stopped report for another item fails', 'if (report.ratingKey !== ratingKey)', 'if (false)', 'by its ratingKey');
add('J15-key', judge, 'a stopped report for another item fails', 'if (report.key !== key)', 'if (false)', 'by its key');
add('J16-seek', judge, 'a stopped report short of the seek', 'finalMs >= seekMs && finalMs <= durationMs', 'finalMs <= durationMs', 'past the 6000 ms seek');

// fr-14: duplicate the tracker's unconditional successful final send. The
// scenario's judge must catch it through the held-play test, with no failure
// or retry injected into the transport.
add('T01-double-stop', unit, good, "      timeline('stopped', final(last));", "      timeline('stopped', final(last));\n      timeline('stopped', final(last));", 'the play sends exactly one stopped timeline report, not 2');

const selected = process.argv[2] ? cases.filter(({ id }) => new RegExp(process.argv[2]).test(id)) : cases;
assert.ok(selected.length > 0, 'at least one mutation selected');
let failures = 0;
for (const mutation of selected) {
  const original = fs.readFileSync(mutation.file);
  let broken = original.toString();
  for (const [before, after] of mutation.edits) {
    assert.equal(broken.split(before).length, 2, `${mutation.id}: exactly one mutation site`);
    broken = broken.replace(before, after);
  }
  let result;
  try {
    fs.writeFileSync(mutation.file, broken);
    result = spawnSync(process.execPath, ['--test', '--test-reporter=tap', `--test-name-pattern=${mutation.test}`, 'tests/e2e-judge.test.mjs'], {
      encoding: 'utf8', timeout: 30000, maxBuffer: 4 * 1024 * 1024,
    });
  } finally {
    fs.writeFileSync(mutation.file, original);
    assert.deepEqual(fs.readFileSync(mutation.file), original, `${mutation.id}: exact restoration`);
  }
  const output = `${result.stdout ?? ''}${result.stderr ?? ''}`;
  const failedTest = output.match(/^not ok \d+ - (.+)$/m)?.[1];
  const killed = result.status === 1 && failedTest && output.includes(mutation.reason) && !result.error;
  console.log(`${killed ? 'KILLED' : 'FAILED PROOF'} ${mutation.id}: exit ${result.status}; ${failedTest ?? 'no failing test'}; expected reason: ${mutation.reason}`);
  if (!killed) {
    failures++;
    console.log(output, result.error ?? '');
  }
}
console.log(`${selected.length - failures}/${selected.length} mutations failed their guards for the expected reason; all files restored byte for byte.`);
process.exitCode = failures ? 1 : 0;
