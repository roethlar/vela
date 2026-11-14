// Runs the import watch (import-watch.mjs) and judges what it reports
// (review finding pa-4). Used by tests/e2e-parity.test.mjs, which must not
// import a scenario that the watched import did not clear.
import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const watchFile = path.join(path.dirname(fileURLToPath(import.meta.url)), 'import-watch.mjs');

// Runs `node <args>` with `input` on its standard input and a fourth pipe on
// descriptor 3, in a process group of its own. When `deadline` ms have passed
// the whole group is killed with SIGKILL, which a process can neither catch
// nor ignore, and which ends one that has stopped itself as well; the result
// says so (`killed: true`). A process that had already ended by then, with its
// pipes still open, has started something outside its group that holds them:
// there is nothing left to kill, and the result says that instead (`held:
// true`). Either way the pipes are not waited for beyond a moment more.
const AFTER_DEADLINE_MS = 1000;
export function runBounded(args, { input = '', deadline }) {
  return new Promise((resolve) => {
    const started = Date.now();
    const child = spawn(process.execPath, args, { stdio: ['pipe', 'pipe', 'pipe', 'pipe'], detached: true });
    const out = ['', '', '', ''];
    for (const fd of [1, 2, 3]) {
      child.stdio[fd].setEncoding('utf8');
      child.stdio[fd].on('data', (chunk) => {
        out[fd] += chunk;
      });
    }
    child.stdin.on('error', () => {});
    child.stdin.end(input);
    let killed = false;
    let held = false;
    let settled = false;
    let afterDeadline;
    const finish = (status, signal) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      clearTimeout(afterDeadline);
      // Whether the process itself has ended. After a kill, that it has is
      // not taken on trust (`ended`).
      const ended = child.exitCode !== null || child.signalCode !== null;
      // Nothing the import started in the watch's process group outlives the
      // check: the group is ended on every way out, a clean one too. `left`
      // says there was still something in it to end.
      let left = false;
      try {
        process.kill(-child.pid, 'SIGKILL');
        left = true;
      } catch {
        // No such process: the group is empty.
      }
      if (killed || held || left) for (const fd of [0, 1, 2, 3]) child.stdio[fd].destroy();
      resolve({
        pid: child.pid,
        status,
        signal,
        killed,
        held,
        ended,
        left,
        ms: Date.now() - started,
        stdout: out[1],
        stderr: out[2],
        channel: out[3],
      });
    };
    const timer = setTimeout(() => {
      held = child.exitCode !== null || child.signalCode !== null;
      killed = !held;
      try {
        process.kill(-child.pid, 'SIGKILL');
      } catch {
        child.kill('SIGKILL');
      }
      afterDeadline = setTimeout(() => finish(child.exitCode, child.signalCode), AFTER_DEADLINE_MS);
    }, deadline);
    // 'close' waits for the pipes as well as the end, so that everything
    // written is read; after a kill, the end is enough.
    child.on('close', finish);
    child.on('exit', (status, signal) => {
      if (killed) finish(status, signal);
    });
    // Node could not start at all.
    child.on('error', (err) => {
      if (settled || child.pid !== undefined) return;
      settled = true;
      clearTimeout(timer);
      resolve({
        pid: undefined,
        status: null,
        signal: null,
        killed: false,
        held: false,
        ended: true,
        left: false,
        ms: Date.now() - started,
        stdout: '',
        stderr: String(err),
        channel: '',
      });
    });
  });
}

// Imports the scenarios of `pairs` (directories and the prefix their
// scenarios are loaded with) under the watch, and returns its report, or
// `{ error }` saying how the run went wrong. The report is taken only from
// the watch's own channel, only if it is the one line there and carries this
// run's secret, and only if the process then ended by itself with status 0.
// `ran`, if given, is handed what runBounded returned (the process's id among
// it), for a test that goes on to check what is left of the process.
export async function importWatched(pairs, { deadline = 60000, ran = null } = {}) {
  const secret = crypto.randomBytes(32).toString('hex');
  const run = await runBounded(['--permission-audit', '--allow-fs-read=*', watchFile, ...pairs], {
    input: `${secret}\n`,
    deadline,
  });
  if (ran) ran(run);
  const tail = run.stderr.trim() ? `; its standard error: ${run.stderr.trim().slice(-2000)}` : '';
  if (run.held) {
    return {
      error: `the watch's process ended, but something the import started still held its pipes ${deadline} ms after the start${tail}`,
    };
  }
  if (run.killed && !run.ended) {
    return {
      error: `the import never let its process end: it was still running after ${deadline} ms, and had not ended a moment after it was sent SIGKILL${tail}`,
    };
  }
  if (run.killed) {
    return {
      error: `the import never let its process end: it was still running after ${deadline} ms and was killed (SIGKILL)${tail}`,
    };
  }
  if (run.signal) return { error: `the watch's process was ended by the signal ${run.signal}${tail}` };
  if (run.status !== 0) return { error: `the watch's process ended with status ${run.status}, not 0${tail}` };
  const lines = run.channel.split('\n');
  if (run.channel === '') {
    return {
      error:
        "the watch's process ended (status 0) without making its report: its exit listener was taken away, or its " +
        `report's descriptor closed${tail}`,
    };
  }
  if (lines.length !== 2 || lines[1] !== '' || !lines[0].startsWith(`${secret} `)) {
    return {
      error: `the watch's report channel held something other than one report carrying this run's secret: ${JSON.stringify(run.channel.slice(0, 500))}`,
    };
  }
  try {
    const report = JSON.parse(lines[0].slice(secret.length + 1));
    if (report === null || typeof report !== 'object' || Array.isArray(report)) throw new Error('not an object');
    // A process the import started was still running in the watch's group
    // when the watch's own had ended and reported (it has been ended).
    if (run.left && report.error === undefined) {
      return {
        ...report,
        error: "the import left a process running in the watch's process group after the watch's own had ended; it was ended (SIGKILL)",
      };
    }
    return report;
  } catch (err) {
    return { error: `the watch's report could not be read: ${err.message}` };
  }
}

// Whether a report of the watched import clears the scenarios for import
// into the test's own process: null if it does, otherwise what it saw.
export function importRefusal(report) {
  if (report === null || typeof report !== 'object') return `the watch made no report (${JSON.stringify(report)})`;
  if (report.error !== undefined) return `the watched import failed: ${report.error}`;
  const problems = [];
  if (!Array.isArray(report.loaded) || report.loaded.length === 0) problems.push('it loaded no scenario');
  for (const [key, what] of [
    ['calls', 'acted on the machine'],
    ['permission', 'did what the permission system checks'],
    ['resources', 'made async resources other than promises'],
    ['sharedChanged', 'changed the global object or a built-in'],
  ]) {
    if (!Array.isArray(report[key])) problems.push(`its report has no list of ${key}`);
    else if (report[key].length > 0) problems.push(`it ${what}: ${report[key].join('; ')}`);
  }
  if (report.newHandles !== 0) problems.push(`it left ${report.newHandles} handles open`);
  if (report.envChanged !== false) problems.push('it changed the environment');
  return problems.length === 0 ? null : `importing the scenarios under the watch: ${problems.join('; ')}`;
}

// Throws, with what the watch saw, unless the report clears the scenarios.
export function stopUnlessCleared(report) {
  const refusal = importRefusal(report);
  if (refusal !== null) {
    throw new Error(`no scenario was imported into this process, because ${refusal}`);
  }
  return report;
}
