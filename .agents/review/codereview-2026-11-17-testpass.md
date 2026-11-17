# Codereview 2026-11-17 — the release's test pass

Outcome: codereview codex hunts over the work of the release's test pass
(opened by the decision of 2026-11-16, "Get 1.0.64 release-ready", "Also
superseded"). The first, over `35d6757..f2dcbc3`: **findings**, three
candidates, all admitted (one MEDIUM, two LOW). tp-3 is repaired; tp-1 and
tp-2 are being repaired (codex coding task `cxhub2`).

## Dispatch record

- Why: the range lands product code (`2ab8ac5`, the order of Home's first
  refresh after a clean end, written by codex and amended by the working
  agent's review) and harness code the working agent wrote (`94662e3`, the
  Windows suite's DevTools port; `f2dcbc3`, the runner's report of a
  changed worktree), and the test of pl-9 (`50b8893`).
- Reviewer: codex / host default / host default / standard, called plainly
  as in `codereview-2026-11-16-platforms.md`, in a disposable worktree at
  the head SHA (`~/dev/vela-agent-keep/wt/rv-final-fix7`); schema-valid,
  `capability_ok` true, both pins echoed.

## Findings

| ID   | Severity | Finding (one line)                                                      | Status | Commit |
|------|----------|--------------------------------------------------------------------------|--------|--------|
| tp-1 | MEDIUM   | A clean end whose EOF is seen after the tracker's end gets three reports | in repair (`cxhub2`) | |
| tp-2 | LOW      | A deferred first report is lost if 64 later ends overflow the join queue | in repair (`cxhub2`) | |
| tp-3 | LOW      | The runner's report of a changed worktree can crash it before it saves results | `[x]` | `834a82a` |

### tp-1

Reviewer, verbatim, evidence: "src-tauri/src/commands.rs:6338 emits
immediately when saw_eof is false. The passing Rust test
an_external_play_reports_its_end_without_waiting_for_the_eof_watcher
demonstrates that an external clean EOF can be detected afterward. Once
joined, src-tauri/src/lib.rs:367 unconditionally emits another first
report, followed by the final report." Predicted failure: "A successfully
admitted clean end produces three Home refreshes instead of two, failing
completionhub's exact-refresh assertion under this ordering. The earliest
refresh still reads local state before admission, so the ordering repair
also misses this path."

Admitted. Repair in progress (codex, `cxhub2`): the completion carries
whether its tracker already reported, and the dispatcher sends its first
report only when it did not; in the late-EOF path the tracker's report
precedes the admission, as before `2ab8ac5`.

### tp-2

Reviewer, verbatim, evidence: "src-tauri/src/commands.rs:6339 suppresses the
tracker report before mark_ended enqueues the completion. While the
dispatcher handles an earlier completion, 64 subsequent tracker ends can
evict that entry at commands.rs:4328. A temporary Rust test using the
actual PlaybackAdvance methods confirmed that saw_eof remains true but
take_ready returns None." Predicted failure: "Under this backlog, the page
never receives playback-ended for the evicted session: its tracker report
was suppressed and neither dispatcher report is reachable."

Admitted (it needs 64 play ends while the dispatcher is still busy with an
earlier completion, but the report must not depend on that). Repair in
progress (codex, `cxhub2`): the bounded queues evict what can never be
joined first, and a completion that leaves them with its report still owed
gets that report then.

### tp-3

Reviewer, verbatim, evidence: "tests/e2e/run.mjs:459 samples readTree(),
then line 463 samples it again. If only the first final sample differs
from the starting tree and the change disappears before the second, find
returns undefined. Executing this source block with those samples
reproduced TypeError: Cannot read properties of undefined (reading 'head')
at line 464." Predicted failure: "A transient final worktree change
crashes the runner before fs.writeFileSync, losing the completed run's
results instead of recording them as dirty."

Admitted. Repair (`834a82a`, the working agent's): one final sample serves
both.
