# Playbook: synchronous cross-harness review (`review`)

A portable workflow for reviewing a multi-fix sweep (security pass, refactor,
bug-fix batch) on one git repo with strong per-fix verification. You — the agent in
the harness you launched from — play the **coder**. The **reviewer** is a second,
independent agent harness (`codex`, `agy`, `grok`, a subagent, …) that you dispatch
**headless and one-shot per finding** to get a different model's eyes on the fix.

Invoke it with `review <agent>` (in Claude Code: the tab-completable `/review
<agent>`). This file is durable guidance; it defers to this repo's `AGENTS.md` and
`.agents/` layout wherever they overlap. Where this playbook and the repo's
invariants disagree, the invariants win.

## What this loop is for

The loop exists to converge on **correct** code, not merely on **changed** code. Two
roles only add signal if each can return the unwelcome answer: the reviewer must be
able to find nothing, and the coder must be able to reject a finding. A loop where the
reviewer always finds something and the coder always agrees produces motion without
information — it will "fix" non-problems, accept wrong corrections, and oscillate, while
looking productive. Guard against both halves explicitly:

- **Reviewer inflation.** A reviewer who treats "find an issue" as the task will almost
  always return one. A pass that surfaces no material issue is a valid, complete,
  expected result — not a failure to do the job.
- **Author capitulation.** A coder who treats every finding as valid will "fix" things
  that were never broken and accept critiques that are wrong. Agreement is only signal
  when disagreement was available.

The cure is the same one the repo already trusts elsewhere: **route correctness through
verification, not through agreement.** Two roles agreeing is still opinion. A test that
fails before a fix and passes after is evidence. Every gate below is built so that a
finding has to predict an observable failure and a fix has to demonstrate it closed one.

## Atomic unit

The whole loop rests on one rule: **one finding ↔ one branch ↔ one verdict**. That is
what keeps each fix independently reviewable and bisectable. It is the same discipline
as the repo's one-item-per-commit rule, applied across two roles. Broad multi-finding
branches are forbidden unless the owner explicitly asks for a sweep.

## Governance alignment (read first)

This playbook is reconciled with the standard `.agents/` governance so it does not
create a parallel canon or bypass owner gates:

- **Status nests under `.agents/`, it does not compete with it.** `.agents/state.md`
  remains the single discoverable current-state entry point. The loop's status index
  lives at `.agents/review/index.md`; `state.md` *points* to it while a loop is
  active rather than duplicating the finding table (pointer doc points; it does not
  keep a second copy of an enumeration another doc owns). There is no root
  `REVIEW.md`.
- **Merging into the main branch is owner-gated.** A reviewer "accepted" verdict
  records that a branch passed review; it does **not** authorize the agent to merge
  into the main branch. Default: leave the accepted branch (or hand off a
  `merge-<id>` branch) for an owner-approved merge. Never merge, push, or rewrite
  history without an explicit owner go (see the repo's Git Safety invariants).
- **Disagreement is a recorded verdict, never a silent veto.** Declining a finding,
  disputing one, or ruling a fix invalid are all logged outcomes that route to the
  owner when the two roles cannot agree. An agent never quietly drops a finding or
  overrides a critique without leaving the reason in the results trail. This keeps the
  loop inside the repo's "answer with words, act only on an explicit go" invariant.
- **Verification is the repo's observed command, not a hardcoded suite.** Run the
  automated verification command recorded in this repo's `AGENTS.md` / `.agents/`
  guidance before any commit. The example commands in this playbook are illustrative
  only.
- **Capabilities, not harness-specific tool or agent names.** Where this playbook
  names `codex`/`agy`/`grok`, those are *examples* of reviewer harnesses. The loop
  works with any agent CLI that can run headless; the per-harness specifics are
  derived live (see below), never baked into this file.

## Operator

`review <agent>` is the harness-neutral entry. In Claude Code it is the
tab-completable slash command `/review <agent>`; on another harness the owner speaks
"review \<agent\>". `<agent>` names the reviewer harness to dispatch.

The flow is **synchronous by construction**: the coder dispatches the reviewer and
blocks on its verdict before acting on that finding. There is therefore **no
quick/wait toggle and no Strict/Faster WIP mode** — the prior async loop's
parallelism knobs do not apply here. One finding is dispatched, reviewed, recorded,
and acted on before the next is dispatched.

## Deriving the reviewer incantation (probe-and-verify)

The only harness-specific fact the loop needs is **how to run `<agent>` headless,
non-interactive, one-shot**. This is **not** shipped as a human-maintained table and
**not** derived by parsing `--help` prose into a committed regex — both rot or break
silently. Instead derive it live, per harness, per session, by probing — the same
thing a capable agent already does when a human says "review this with grok":

1. **Presence + surface.** `command -v <agent>`; then `<agent> --help` and
   `<agent> --version`. The top-level help usually reveals whether the headless entry
   is a subcommand (`<agent> exec …`) or a flag (`<agent> -p …`).
2. **Drill if ambiguous.** If the headless entry is not obvious, drill one level
   (`<agent> exec --help`, `<agent> chat --help`, whichever the top level lists) to
   find the non-interactive flag and how to pass a prompt. Note the harness's JSON
   output flag here too (e.g. `--output-format json`) — the verdict contract uses it.
3. **Bounded smoke-test.** Run the candidate incantation with a trivial prompt (e.g.
   `<agent> exec "say OK"`) under bounds: a **timeout** (a hung process is a failed
   probe, not a wait); **non-interactive detection** (if it opens a TUI / alternate
   screen / waits on a TTY, the incantation is wrong — try the next candidate); and
   run it **from a real git repo** (a canned prompt in an arbitrary temp dir hides
   launch requirements — e.g. codex refuses a non-trusted dir and needs
   `--skip-git-repo-check`, agy must run from the real repo cwd). Treat a launch
   refusal as a flag to adjust, not a dead end.
4. **Use the verified incantation** to run the review. Probing is bounded to
   `--help`/`--version`/the trivial smoke prompt — never arbitrary commands.

**Optional session cache (convenience, not source of truth).** Once verified, you may
record an incantation in a gitignored machine-local file,
`.agents/review/harnesses.local.json`, to skip re-probing next session. Harness
availability and CLI syntax are machine-specific, so a `*.local.*` file is the correct
home (consistent with the repo's treatment of `settings.local.json` as untracked
machine state). The cache is advisory and **self-authored** — never hand-maintained;
the source of truth is "re-derive by probing," so a stale cache self-corrects on the
next smoke test.

## Per-finding flow

For each admitted finding (intake/triage and the coder's own guard proof are done —
see the gate below):

1. **Finish the fix** on a per-finding branch `fix/<id>-<slug>`, smallest coherent
   slice, touching only the files the finding doc declares.
2. **Dispatch the reviewer** headless and one-shot, in the harness's **JSON output
   mode** (the flag found while probing). Pass an **explicit base**: the reviewed
   branch **head SHA** and the **base SHA** (the merge-base with the main branch at
   dispatch time), so the reviewer evaluates `git diff <base-sha>..<head-sha>` against
   a fixed snapshot — a `main..branch` range is *not* stable if the main branch moves.
   The reviewer reads the code from the **shared workspace** (you do not pipe it the
   diff); it reads `.agents/review/findings/<id>.md`, and **independently performs the
   guard proof** (revert → confirm FAIL → restore → confirm PASS) **in its own `git
   worktree` checked out at the head SHA** — never by mutating your working tree. A
   reviewer that crashes mid-proof leaves only its disposable worktree dirty.
3. **Verdict contract (structured, fail-closed).** The reviewer returns its verdict in
   the JSON envelope. Its result payload must match:
   ```json
   {"verdict":"accepted|reopened|invalid","guard_confirmed":true,
    "reviewed_sha":"<head-sha>","base_sha":"<base-sha>","comments":["file:line — …"]}
   ```
   Parse the envelope's result field against this schema. **Fail closed:** any of
   {non-zero exit, missing/!valid JSON envelope, payload not matching the schema,
   `verdict` not in the enum, `reviewed_sha` ≠ the dispatched head SHA} → the outcome
   is **not accepted**. Re-prompt once with the schema restated; if it still fails,
   route the finding to the owner as contested. A parse miss never silently becomes an
   accept. (The harness's JSON mode guarantees a valid *envelope*, not that the model
   filled the *payload* to schema — hence the inner parse, not envelope-validity
   alone.)
4. **Record the verdict** into `.agents/review/findings/<id>.md` `## Reviewer
   comments` **before acting** — the durable trail. Capture: reviewer **harness name +
   version**, **reviewed head SHA + base SHA**, **`guard_confirmed`**, the
   **verdict**, a UTC **timestamp**, and the comments. Flip the finding **Status** and
   the index row. State whether this record is committed (it should be, as part of the
   verification history).
5. **Act on the recorded verdict:**
   - **accepted** → the branch is ready for an **owner-gated** merge. Do not merge,
     push, or rewrite history on agent authority; leave the branch (or hand off a
     `merge-<id>` branch).
   - **reopened** → apply fix-ups on the same branch, then re-run `review <agent>`.
   - **invalid** → write `.agents/review/<id>.contested.md` (which kind of
     disagreement, the reason, what the owner must decide) and route to the owner.
     Disagreement is a recorded verdict, never a silent veto.

## Finding intake and triage

This is the gate the false-positive problem dies at, before any branch is cut. It
applies whether findings come from a human, the coder, a separate review pass, or a
second model.

**A review pass that finds no material issue is a complete, valid result.** Record it
as one plain sentence ("Reviewed <scope>; no material issue found") and stop. Do not
manufacture findings to justify the pass. An empty findings table is a legitimate
outcome of this playbook.

Every candidate finding must carry three things before it can be admitted:

1. **Evidence** — concrete `file:line` citation(s) and the specific input, path, or
   condition that triggers the problem. A finding that cannot point at code is not a
   finding.
2. **Predicted observable failure** — what goes wrong that someone or something could
   detect: a wrong result, a crash, a security exposure, a failing or missing test, a
   measurable regression. "Could be cleaner," "not idiomatic," or "I would have written
   it differently" are not observable failures.
3. **Justified severity** — CRITICAL | HIGH | MEDIUM | LOW, with a one-line reason tied
   to the predicted failure, not to taste.

**Triage each candidate to a verdict:**

- **ADMITTED** → it has evidence, a predicted failure, and a justified severity. Give
  it an id, add a `[ ]` row, write the finding doc.
- **DECLINED** → it lacks evidence or a predicted observable failure, is style-only, is
  out of scope, or duplicates another finding. Record it as a `[-]` row and write one
  line in `.agents/review/<id>.contested.md` stating why. Declining is the expected
  fate of most stylistic or speculative findings; it is the loop working, not failing.

If a single agent is generating and triaging its own findings, it must still write the
DECLINED reasons down — the discipline is in making the rejection explicit and
reviewable, not in who performs it. Severity is not decoration: if you cannot write the
impact line, the finding is a DECLINE or a LOW, not a CRITICAL.

## Per-finding record: `.agents/review/findings/<id>.md`

Written when a finding is admitted; the coder completes the lower half when work starts.

```markdown
# <id>: <title>

**Severity**: CRITICAL | HIGH | MEDIUM | LOW — <one-line justification>
**Status**: Open | In progress | Verified | Contested
**Branch**: `fix/<id-lowercased>-<short-slug>`
**Commit**: `<git-sha>` (filled in after commit)

## Evidence
`file:line` citation(s) and the input/condition that triggers the problem.

## Predicted observable failure
What detectably goes wrong, and — where possible — the test or check that would catch
it. This is the claim the fix must prove it closed.

## What
Concise statement of the bug or risk. One paragraph.

## Approach
What was done and why it fixes the root cause rather than the surface symptom. Cite
the new/changed functions and files. 2–6 sentences.

## Files changed
- `path/to/file:lines` — what changed

## Guard proof
- `path/to/test::name` — the assertion. Reverting the fix makes this FAIL; restoring
  makes it PASS. If the change is genuinely untestable, state why and name the manual
  check that was run instead.

## Coder dispute (if any)
If the coder believes the finding is wrong or not worth fixing, state the reason here
instead of implementing, and route to a contested verdict. Empty otherwise.

## Known gaps
Anything uncertain, out of scope, or overlapping another finding the reviewer should
grade explicitly. Empty if nothing.

## Reviewer comments
Reviewer harness + version, reviewed/base SHA, guard_confirmed, verdict, UTC
timestamp, and the comments. On reopen the coder addresses these and re-runs the
review.
```

## Status index: `.agents/review/index.md`

Short, human-readable scoreboard. Per-finding detail lives in
`.agents/review/findings/<id>.md`; do not turn the index into a discussion log.

```markdown
# Review status

Workflow: see `.agents/playbooks/reviewloop.md`.
Per-finding detail: see `.agents/review/findings/<id>.md`.

## Legend
- `[ ]` Admitted, open (passed intake triage; not yet started)
- `[~]` In progress / pending review
- `[x]` Verified (awaiting owner-gated merge)
- `[!]` Contested — declined, disputed, or ruled invalid; awaiting owner adjudication
- `[-]` Declined at intake (kept for the record; no work)

## Findings

| ID    | Severity | Impact (one line)            | Status | Branch |
|-------|----------|------------------------------|--------|--------|
| sec-1 | HIGH     | <observable consequence>     | `[ ]`  |        |
| ...   | ...      | ...                          | ...    | ...    |
```

Add one line to `.agents/state.md` while a loop is active, e.g. "Active review loop:
see `.agents/review/index.md`." Remove it when the loop is done. `state.md` points;
it does not copy the table.

## Calibration anti-patterns

These are the failure modes that make a two-role loop produce motion without signal.
Name them when they appear; they are process defects, not code defects.

- **Reviewer inflation.** Returning a finding on every pass because "no issues" feels
  like not doing the job. Cure: an empty findings table is a valid result; every
  admitted finding needs a predicted observable failure.
- **Author capitulation.** Accepting every finding as valid and implementing a change
  for each. Cure: the coder must judge each finding and route wrong ones to a contested
  verdict instead of fixing them.
- **Severity decoration.** Tagging findings CRITICAL/HIGH without an impact line.
  Cure: no impact line, no high severity — downgrade or decline.
- **Churn without evidence.** A "fix" that no test can distinguish from the original.
  Cure: the guard proof; if reverting the fix breaks nothing, the change is churn and
  should be reopened or declined.
- **Convergence read as correctness.** Treating two roles agreeing as proof the code is
  right. Cure: agreement is not the gate; the guard proof is. The recorded verdict
  carries the proof, not the consensus.

## Anti-patterns

- **Broad sweeps.** "Fix sec-1..sec-9 in one commit" kills bisection. Owner-request
  only.
- **Manufacturing findings.** Inventing issues so a pass has output. A clean pass is a
  result.
- **Silent veto.** Dropping or overriding a finding without a contested record.
- **Accepting on a parse miss.** A missing, non-JSON, or off-schema verdict is **not**
  an accept. Fail closed: re-prompt once, then route to the owner as contested.
- **Reviewer mutating the coder's tree.** The reviewer's guard proof belongs in its
  own `git worktree`; it must never revert/restore in the coder's working tree.
- **Merging or pushing without an owner go.** Accepted is a verdict, not merge
  authority.
- **Rewriting history** (amend/rebase/squash/force-push) on reviewed work without an
  explicit owner go.
- **Editing the index prose freely.** It is a status board; discussion goes in the
  finding or contested doc.
- **Reviewing against a moving base.** Pin the base + head SHAs at dispatch; do not let
  `main..branch` drift mid-review.

## Knobs

- **Single-agent mode**: one agent alternates coder and reviewer hats (no foreign
  harness). Keep per-finding branches, the guard proof, and the recorded-verdict
  trail. The discipline that matters in this mode is writing the DECLINED and contested
  reasons down even though one mind holds both roles — that is what stops self-agreement
  from collapsing the loop.
- **Adjudicator (optional)**: when coder and reviewer disagree (a contested record), a
  third role — or the owner — reads the finding, the dispute, and the guard proof and
  issues a final ADMIT/DECLINE. Useful when the coder and reviewer are two models prone
  to deferring to each other.
