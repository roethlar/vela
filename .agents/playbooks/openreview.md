<!-- toolkit-owned; edits are drift — see AGENTS.md -->

# Playbook: goal-first whole-change review (`openreview`)

A portable workflow for getting one independent, unprimed judgment of a whole
change from a second agent harness. You — the agent in the harness you launched
from — play the **coder/orchestrator**. The **reviewer** is a second, independent
agent harness (`codex`, `agy`, `grok`, a subagent, …) dispatched headless and
one-shot over a pinned commit range.

Invoke it with `openreview <agent>` (in Claude Code: the tab-completable
`/openreview <agent>`). This file is durable guidance; it defers to the repo's
`AGENTS.md` and `.agents/` layout wherever they overlap.

**Framing (deliberate):** where `codereview` verifies findings against their
records, this playbook withholds every rubric. The reviewer judges the change on
its own reading of the repository. Priming it — with the plan, a checklist,
suspected risks, or prior conclusions — turns independent review into
confirmation and is this playbook's cardinal defect. Selection between the two
is the owner's per-invocation call: conformance rubrics suit verification passes
and weaker reviewer models; the open question rewards stronger reviewers and
design-heavy changes. No auto-selection heuristic exists; the owner names the
playbook.

## The question (neutral by construction)

The substantive review prompt is exactly:

> Is the code as implemented the best way to achieve the goal?

That sentence is the whole substantive framing. Give the reviewer only the
mechanical coordinates needed to perform the review:

- the repository location (shared workspace — you do not pipe it the diff);
- the exact **base SHA** (merge-base with the main branch at dispatch time) and
  **head SHA**, so it evaluates `git diff <base-sha>..<head-sha>` against a
  fixed snapshot — a `main..branch` range is not stable if the main branch moves;
- permission to inspect the repository to discover the goal;
- disposable-worktree isolation: anything it runs or reverts happens in its own
  `git worktree` checked out at the head SHA — never in your working tree;
- side-effect boundaries (no commits, no pushes, no network mutations);
- the verdict schema below.

Those facts make the review reproducible; they do not tell the reviewer what
conclusion to reach. Do **not** summarize the plan or implementation, enumerate
areas to inspect, supply a risk checklist, suggest findings, repeat claimed
invariants, or disclose prior reviewer conclusions. Plans and finding records
remain repository evidence the reviewer may discover, not a rubric the caller
argues from. The reviewer chooses what to read, which alternatives to consider,
and what evidence matters. A clean "yes" is as valid as a well-supported "no".

## Dispatch

Derive the reviewer incantation live, per session, by probing — presence,
headless entry, JSON output mode, bounded smoke test — exactly as the
`codereview` playbook's "Deriving the reviewer incantation" section specifies
(that section is the canonical recipe; do not duplicate it here). Dispatch
headless, one-shot, in the harness's JSON output mode.

Tier routing is fixed: `openreview` always dispatches the harness's
owner-confirmed **frontier** pair at **max** effort (see the `codereview`
playbook's "Reviewer tiers and routing") — no escalation headroom exists
above it, so a contested round resolves by owner adjudication, never a
stronger redispatch. Eligibility is carried by the frontier entry's
`openreview_confirmed` field (amended 2026-07-18, owner adjudication of
OR3): the pair is dispatchable here only when that field matches the
current harness version. A `fallback`-grade pair so confirmed is a
legitimate openreview reviewer — dispatched when no competitive-grade
harness is available or when the owner names it — and its grade is
recorded in the outcome. A missing or `null` `openreview_confirmed`
blocks that harness fail-closed for this playbook and routes to the
owner; the orchestrator never infers openreview eligibility from a
codereview-only confirmation.

## Verdict contract (structured, fail-closed)

The reviewer returns its verdict in the JSON envelope. Its result payload must
match:

```json
{"verdict":"clean|findings","reviewed_sha":"<head-sha>","base_sha":"<base-sha>",
 "findings":[{"title":"…","evidence":"file:line — …","predicted_failure":"…",
  "severity":"CRITICAL|HIGH|MEDIUM|LOW","better_approach":"…"}]}
```

Parse the envelope's result field against this schema. **The orchestrator —
never the reviewer — computes acceptance.** Fail closed: any of {non-zero exit,
missing/invalid JSON envelope, payload not matching the schema, `verdict` not in
the enum, `reviewed_sha` ≠ the dispatched head SHA, `base_sha` ≠ the dispatched
base SHA, `findings` non-empty with verdict `clean` or empty with verdict
`findings`} → the outcome is **not** a clean pass. **Extraction before
rejection:** a prose-wrapped payload is not a parse miss — scan it for
candidate JSON objects, and when exactly one matches the schema, use it; the
review already happened, and surrounding prose is never an input to
acceptance. Zero or multiple schema matches → parse miss. On a parse miss,
re-prompt once for **re-emission only**: feed the reviewer its own output back
and ask for schema-only JSON — no re-review, no hint of the expected verdict.
If that still fails, route to the owner as contested. Never re-run a completed
review to fix formatting. A parse miss never silently becomes a clean verdict.

## Downstream: findings enter the codereview machinery

An openreview pass produces candidate findings, not fixes. Every returned
finding goes through the `codereview` playbook's **finding intake and triage**
gate (evidence, predicted observable failure, justified severity — ADMITTED or
DECLINED, recorded either way), and admitted findings are worked per that
playbook's per-finding flow: one finding ↔ one branch ↔ one verdict, guard
proof included. This playbook owns the dispatch and the verdict envelope;
`codereview` owns everything downstream.

Every outcome — clean or findings — records reviewer provenance
(amended 2026-07-18, owner adjudication of OR5): the harness, resolved
model id, effort, and grade, taken from the dispatch record of the
session that produced the verdict, never reconstructed after the fact.
A `clean` verdict is recorded as one plain sentence wherever the repo
tracks review outcomes:
"openreview <agent> (<model> @ <effort>, <grade>) over <base>..<head>:
no material issue". A clean line without provenance is an incomplete
record — a future reader must be able to tell **which** reviewer found
nothing, or the once-per-harness-version confirmation economy is
unauditable.

## Anti-patterns

- **Plan-conformance priming.** Telling the reviewer to validate against a plan,
  or preloading a checklist, suspected risks, preferred mutations, or expected
  findings. Ask only the neutral question; provide only the mechanical
  coordinates and the safety/output contract.
- **Treating "clean" as a failed pass.** An unprimed reviewer that finds nothing
  material has done the job. Do not re-dispatch shopping for findings.
- **Manufacturing findings.** The reviewer inventing issues so the pass has
  output; intake triage exists to decline these, and declining is the loop
  working.
- **Skipping intake.** Implementing a returned finding directly because the
  reviewer sounded confident. Every finding passes the evidence/predicted-failure
  gate first.
- **Reviewing against a moving base.** Pin base + head SHAs at dispatch.
