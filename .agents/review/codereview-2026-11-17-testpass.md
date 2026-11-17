# Codereview 2026-11-17 — the release's test pass

Outcome: codereview codex hunts over the work of the release's test pass
(opened by the decision of 2026-11-16, "Get 1.0.64 release-ready", "Also
superseded"). The first, over `35d6757..f2dcbc3`: **findings**, three
candidates, all admitted (one MEDIUM, two LOW), all repaired. The second,
over `f2dcbc3..019c08b`: **findings**, four candidates, all admitted (two
MEDIUM, two LOW), all repaired (codex coding task `cxfix8`, closed by the
working agent's review). The third, over `019c08b..dc3c9e8`: one LOW
finding (tp-8), repaired. The fourth, over tp-8's repair
(`dc3c9e8..6f92baa`): one LOW finding (tp-9), repaired. A fifth, over
tp-9's repair (`6f92baa..17893e6`), was clean: every finding of the pass so
far, tp-1 to tp-9, is closed.

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
| tp-1 | MEDIUM   | A clean end whose EOF is seen after the tracker's end gets three reports | `[x]` | `c3ea91d` |
| tp-2 | LOW      | A deferred first report is lost if 64 later ends overflow the join queue | `[x]` | `c3ea91d` |
| tp-3 | LOW      | The runner's report of a changed worktree can crash it before it saves results | `[x]` | `834a82a` |
| tp-4 | MEDIUM   | build.rs misses a Cargo build directory set apart from the target directory | `[x]` | `1342e6a` |
| tp-5 | MEDIUM   | An interrupted Windows run can leave its scenario's test CA trusted | `[x]` | `f29d77c` |
| tp-6 | LOW      | build.rs's guard passes with the copy taken out | `[x]` | `d1cc1b3` |
| tp-7 | LOW      | settingValue accepts an expected default before the setting has loaded | `[x]` | `d48bc08` |
| tp-8 | LOW      | The click retry's guard never runs the script that picks the element | `[x]` | `6f92baa` |
| tp-9 | LOW      | The click retry's guard never runs the script's not-found path | `[x]` | `17893e6` |

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

Admitted. Repair (`c3ea91d`, codex's, closed by the working agent's
review): the completion carries whether its tracker already reported, and
the dispatcher sends its first report only when it did not; in the
late-EOF path the tracker's report precedes the admission, as before
`2ab8ac5`.

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
earlier completion, but the report must not depend on that). Repair
(`c3ea91d`, with tp-1): the bounded queues evict what can never be joined
first, and a completion that leaves them with its report still owed gets
that report then.

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

## The second hunt: `f2dcbc3..019c08b`

Over `build.rs`'s copy (`e87ffe2`), the tp-1/tp-2 repair (`c3ea91d`) and
the Windows end-to-end venue fixes. **findings**: four, all admitted;
codex coding task `cxfix8` repairs them.

### tp-4 (MEDIUM)

Reviewer, verbatim, evidence: "src-tauri/build.rs:23-28 derives the
destination exclusively from OUT_DIR. Running the exact helper in a minimal
Cargo 1.100 crate with separate build.build-dir and target-dir produced
intermediates/debug/mpv-scripts while the executable was under
target/debug; no scripts existed beside it." Predicted failure: "Debug and
unbundled builds using a separate intermediate directory still lack the
resources needed for markers and autocrop."

### tp-5 (MEDIUM)

Reviewer, verbatim, evidence: "tests/e2e/run.mjs:217-219 removes the CA only
in runScenario's finally block. The signal handler at lines 49-57 invokes
activeCleanups, which contains scenario.cleanup but not CA removal, then
calls process.exit. Executing these source blocks with mocked Windows
operations reproduced trust-ca, scenario-cleanup and exit:130 without
untrust-ca." Predicted failure: "Ctrl+C or SIGTERM during a CA-backed
scenario can leave its test CA in LocalMachine\Root after the run, affecting
trust for every account on that machine."

### tp-6 (LOW)

Reviewer, verbatim, evidence: "tests/dependency-policy.test.mjs:1504-1508
checks the call site, destination calculation and directory traversal, but
never verifies copying. Replacing src-tauri/build.rs:33-34 with a
side-effect-free expression left all 365 dependency-policy tests passing,
including this guard."

### tp-7 (LOW)

Reviewer, verbatim, evidence: "tests/e2e/helpers.mjs:112-115 accepts value
equality without observing load completion. continuetv.mjs:242 expects
'only-tv' and windowsetting.mjs:263 expects 'remember', exactly their
initial values in src/lib/Settings.svelte:173-175. A probe returning the
initial default followed by an incorrect loaded value made settingValue
return after its first read."

Repairs (codex's, `cxfix8`, closed by the working agent's review, which
red-proofed tp-6's copy and tp-7's busy check again): tp-4 `1342e6a` (the
copy moves to `src-tauri/build_support.rs`; the end-to-end runner puts the
scripts beside the executable Cargo reported, whatever the layout; a build
script cannot see a build directory set only in a Cargo config file, and
says so), tp-5 `f29d77c` (the CA's removal lives in the cleanup that normal
teardown and the signal handler share), tp-6 `d1cc1b3` (a Rust unit test
copies into temporary trees in both layouts and checks every file), tp-7
`d48bc08` (each loaded Settings control carries `aria-busy` until its value
is known, and `settingValue` waits for that).

## The third hunt: `019c08b..dc3c9e8`

Over the Windows set's Schannel patch (`3c4a510`), the tp-4 to tp-7
repairs and the Windows harness since. **findings**: one, admitted.

### tp-8 (LOW)

Reviewer, verbatim, evidence: "tests/e2e-harness.test.mjs:827 supplies a
successful script response without executing it; assertions at :834 only
inspect fragments. Replacing Driver.click's tag-filtering .find(...) with
.at(0) left all 64 harness tests passing. Executing that mutation against
a fixed DIV overlay covering a BUTTON scrolls the overlay instead of the
button." Predicted failure: "The guard falsely accepts a broken retry that
leaves the button covered."

Admitted. Repair (the working agent's): the test runs the emitted script,
as WebDriver would, against a fake `elementsFromPoint` with an overlay over
the button and a scrolled window, and checks that only the button scrolls;
seen failing with the first element at the point taken and with the
window's scroll ignored.

## The fourth hunt: tp-8's repair

Over `dc3c9e8..6f92baa`. **findings**: one, admitted.

### tp-9 (LOW)

Reviewer, verbatim, evidence: "tests/e2e-harness.test.mjs:845 always
supplies a matching button, while :853 supplies a canned false response.
Changing tests/e2e/driver.mjs:161 to `if (!found) return true;`, or
deleting that check, leaves all 64 harness tests passing." Predicted
failure: "When no matching tag exists at the recorded point, the guard
accepts regressions that retry without locating or scrolling the target."

Admitted. Repair (the working agent's): the test also runs the script
against an empty list and an overlay-only list, expecting false and nothing
scrolled; seen failing with the not-found result turned to true and with
the check deleted.
