<!-- toolkit-owned; edits are drift — see AGENTS.md -->

# Playbook: defect review (`codereview`)

Review a pinned change for observable defects, then verify each admitted
fix against its finding. The caller is the coder/orchestrator; the named
reviewer gives a second judgment. `openreview` owns the unprimed approach
question. The owner chooses the playbook and reviewer.

Evidence decides correctness. A clean review and a declined finding are
valid outcomes; never manufacture issues or accept them merely because a
reviewer proposed them.

## Dispatch grammar

`/codereview <harness> <model> <effort> [<base>..<head>]`; `review` is a
pure alias. A range starts Change review (defect generation); without one,
continue the active per-finding loop. If none exists, ask which finding
or range to review.

Use the owner's literal model word verbatim. No model lists, mappings,
denylists, or guesses: let the harness accept or reject it. An explicit
model/effort overrides cached defaults for this dispatch only; record
`(inline, session-only)` and store it as a default only if asked.
`frontier` is the reserved model-slot word forcing that tier, recorded
`escalated: owner`. Never switch harnesses implicitly.

Omitted model: use the routed tier's cached pair, or ask once and record
the answer. Omitted effort: standard defaults to high, frontier to xhigh;
the owner-named pair governs unsupported levels.

A bare `codereview` asks which reviewer to run in one line, with prior
harness/model dispatches from the machine-local cache as recall. That list
is a reminder, never a menu. Probe nothing to build it; an empty cache asks
without a list. Do not store the answer as a bare-invocation default.
Reviewer selection never authorizes self-review; AGENTS.md governs that
exception.

## Deriving the reviewer incantation

Probe presence, `--version`, and `--help`; drill one subcommand level if
needed to find headless, non-interactive, one-shot invocation, prompt input,
model/effort flags, and output capture. Commands in this playbook are
illustrative; use the repo's recorded verification command.

**Capability proof:** in the real dispatch, require the reviewer to read a
repo file and run one allowed command, then report whether both succeeded. Use a separate smoke prompt only when qualifying a
transport without a review to run. Bound launch time; TUI/TTY input or an
incorrect flag means adjust the incantation. On a tool/permission denial,
retry once in a fresh process; another denial means record the transport
unsupported and stop. Never hunt for a workaround.

Cache verified transport facts in gitignored
`.agents/review/harnesses.local.json`. This is machine-local dispatch
convenience, not repo truth. Use a verified MCP transport when available,
otherwise CLI; do not commit server registrations or model defaults.

```json
{
  "transport": "mcp | cli",
  "probed": "<harness version>",
  "tiers": {
    "standard": {"model": "<id>", "effort": "<level>", "flags": ["..."]},
    "frontier": {"model": "<id>", "effort": "<level>", "flags": ["..."],
                 "grade": "competitive | fallback"}
  }
}
```

Transport flags are keyed by harness version/profile; re-probe on change.
Tier pairs are owner judgments, have no version key, and survive upgrades.
Record a pair when the owner names or accepts it; ask only for an absent
pair or a model rejected at dispatch. A single-model harness may differ by
effort; record identical pairs explicitly when no distinction exists.

On a cache hit, validate capabilities and pins in the first real dispatch,
not a separate ping. A model/connection failure invalidates the entry:
re-probe and retry once. Version changes and wrapper/proxy quirks are
dispatch notes, not confirmation gates.

### Self-permissioning launch

Grant tools at launch, never by widening persistent settings: file
reads/searches, named Git inspection and worktree commands, the exact
verification command, and proof edits in the reviewer's disposable worktree
at the pinned head. The reviewer creates that worktree for revert/restore
proofs; read-only inspection needs none. Never run proof edits in the coder's
working tree.

Do not grant wildcard Git or shell permission. Beyond that worktree
lifecycle, grant no commit, push, ref-changing, or network-mutation
authority. Worktrees share Git metadata; isolation does not widen authority.
If the transport cannot enforce the grant, report it unsupported.

### Dispatch provenance

Every outcome and finding records:

```text
Reviewer: <harness> / <resolved model id> / <effort> / <tier>
  [escalated: <ordered list of ALL matched triggers>]
```

Take resolved identity from the invocation transcript/envelope, never
reviewer prose. If unavailable because a wrapper or proxy obscures it,
record what was dispatched and that limitation. Failed calls stay failed.
Record harness version, base/head SHAs, UTC timestamp and capability/proof
results alongside the verdict. Provider switching requires an owner go.

## Reviewer tiers and routing

- **standard:** owner-named best-value model/effort pair; default high.
- **frontier:** owner-named stronger configured pair; default xhigh.
  The owner grades it `competitive` or `fallback`.

Strength is the owner's judgment, not something inferred from a model name
or CLI help. There are two tiers and no economy review role. Effort is
part of capability; the depth order is high < xhigh < max. Only the owner
chooses a different provider.

### Escalation triggers

Evaluate mechanical triggers from the diff and record before dispatch.
Record all matched IDs in order.

- **T1 — sensitive paths:** match changed paths against Git pathspecs:
  `**/auth*`, `**/secret*`, `**/*credential*`, `**/crypto*`,
  `**/migrations/**`, `**/schema*`, `**/*.proto`, `**/wire/**`,
  `**/serializ*`. If `.agents/review/sensitive-paths` exists, it
  replaces the entire default list. Matching is approximate; do not invent
  extra patterns per dispatch. Routes to frontier.
- **T2 — severity:** the finding's committed `**Severity**:` is HIGH or
  CRITICAL with an impact reason. Read the finding, not the verdict.
  Routes to frontier.
- **T3 — proof integrity:** missing proof, a failed verification command,
  or an orchestrator repeat disagreeing with the recorded result halts
  before dispatch. Return to the coder until deterministic evidence exists;
  if unresolved, ask the owner. This is a blocker, not an escalation.
- **T4 — declared-file drift:** compare repair paths with the declared
  file set snapshotted before repair. Expansion halts as contested; it
  does not authorize wider work or full replay.
- **T5 — reopen:** redispatch one tier higher within the named harness.
  At frontier, use a fresh frontier session and record `T5 (ceiling)`.

A `fallback` frontier cannot silently adjudicate an escalation: halt as
contested unless the owner has explicitly accepted that fallback dispatch.
Record that acceptance with the matched triggers. The owner may instead
name another harness.

Escalation, including frontier-ceiling reopen, always starts a fresh
conversation. A tier or effort change also starts fresh; do not alter effort
inside an existing thread. Otherwise a repair delta may reuse an MCP thread
with its pinned pair. A reopened valid proof is T5; an invalid proof is T3.

## Verdict handling

The orchestrator judges the evidence against the dispatched pins and
capability proof. Use every readable response: prose, JSON, Markdown,
mixed or partial output, including usable data from a failed call. Require
evidence of the actual review and proof; field names, echoed pins, literal
booleans, envelopes and schemas are not acceptance gates.

Never request re-emission or repeat a review because of response formatting.
Interpret usable data directly. Resolve substantive missing evidence or
contradictions through the existing proof and dispute rules; formatting
alone is neither missing evidence nor a contested outcome.

## Change review (defect generation)

Resolve both endpoints of `<base>..<head>` to SHAs. Supply those pins,
the repository path and the bounded defect-hunt mandate.
The reviewer reads the pinned diff from the shared repository; do not pipe
it a caller-curated diff. Standard is the default; T1 or owner force may
route frontier. T2–T5 apply only to per-finding rounds.

Report a clean change or candidate findings with title, file/line evidence,
predicted failure, severity and better approach. A clean result means no
findings. Apply Verdict handling. Record a valid clean outcome with
provenance and stop the review pass; it does not close unfinished work.
Every returned finding is a candidate for intake, not an instruction to fix.

## Finding intake and triage

Require concrete file/line evidence and the triggering input/condition,
a predicted observable failure, and justified severity tied to impact.
Style preference, unsupported suspicion, duplication and out-of-scope
work do not qualify.

- **ADMITTED:** assign an ID, write its finding record and an open index row.
- **DECLINED:** record a `[-]` row and one reason in
  `.agents/review/<id>.contested.md`. No fix is authorized by a decline.

The coder may dispute a finding instead of implementing it. Record the
reason and ask for adjudication; never silently veto or capitulate.
A coder triaging its own candidates still records declined reasons.

## Per-finding flow

Repo policy selects **one finding ↔ one commit ↔ one verdict** on main,
or **one finding ↔ one branch ↔ one verdict** on `fix/<id>-<slug>`.
Repairs use follow-up commits, never an amend. Broader multi-finding sweeps
need explicit owner scope. Completion follows `.agents/playbooks/git.md`.

The loop is synchronous: dispatch, await, record and act on one finding's
verdict before dispatching the next. No overlapping finding reviews.

1. Within implementation authority, declare the files, implement the fix
   and prove its test fails without the fix and passes with it. Commit the
   fix and its evidence. A genuinely untestable change must explain why
   and give the manual reproduction/check instead; do not claim an
   automated guard ran.
2. Dispatch the named reviewer at the routed tier with the finding path.
   On main, pin head
   to the fix commit and base to its parent; on a work branch, use its head
   and merge-base with main at dispatch. The reviewer reads the pinned diff
   and repeats the guard proof in its own worktree. Manual proof also
   needs independent confirmation.
3. Apply Verdict handling. Determine whether the fix is accepted, reopened
   or invalid, using the reviewer's conclusion, proof results and comments.
   Acceptance requires independently confirmed guard proof. A field or
   literal boolean is unnecessary; missing or failed proof cannot close it.
4. Record the verdict and provenance in the finding before acting. Commit
   the record and update its index row:
   - **accepted:** on main, close the finding with its records and required
     push. On a work branch, mark Awaiting merge and follow git closeout
     through integration and local/remote branch deletion. Acceptance grants
     no merge, push, or deletion authority.
   - **reopened:** repair in follow-up commits, then redispatch the repair
     delta with T5 routing.
   - **invalid:** record the disagreement in
     `.agents/review/<id>.contested.md` and ask the owner to adjudicate.
     A third harness already in the local cache may be offered, never
     auto-dispatched; no harness adjudicates a dispute it authored.

### Repair-delta redispatch

Pin base to the pre-repair head and head to the repaired head. The
orchestrator computes and supplies that repair diff, the original finding,
verification command and guard evidence. This is the explicit exception
to reviewers discovering their own diff.

Review only closure of the predicted failure and adjacent regressions in
the touched surface. Execute proof against the full current head in the
disposable worktree. Full replay requires an owner ask, including after
T4. Preserve the prior review and append the repair verdict.

## Per-finding record: `.agents/review/findings/<id>.md`

```markdown
# <id>: <title>

**Severity**: CRITICAL | HIGH | MEDIUM | LOW — <impact reason>
**Status**: Open | In progress | Verified | Awaiting merge | Awaiting deletion | Complete | Contested
**Branch**: fix/<id>-<slug>, or — for direct-to-main
**Commit**: <fix SHA>

## Evidence
<file:line, triggering input/condition, predicted observable failure>

## Approach
<what changed and why it closes that failure>

## Files changed
<declared paths; snapshot before repair>

## Guard proof
<test/command, fail without fix, pass with fix; or justified manual proof>

## Coder dispute
<reason, or none>

## Known gaps
<uncertainty/out-of-scope overlap, or none>

## Reviewer comments
<verdict, comments and Dispatch provenance fields for each round>

## Closeout
<main commit and completion receipt; for branch work, integration content
proof and local/remote branch absence; otherwise the pending step>
```

## Status index: `.agents/review/index.md`

Keep a table of ID, severity/impact, status, branch and compact reviewer
provenance. Details live in the finding, not the index.

- `[ ]` Open, admitted.
- `[~]` In progress / pending review.
- `[v]` Verified / awaiting merge.
- `[d]` Merged / awaiting deletion.
- `[x]` Complete: verified on main; any work branch merged and deleted locally and remotely.
- `[!]` Contested; owner adjudication pending.
- `[-]` Declined; no implementation work.

While any finding has pending work, keep one pointer to this index in
`.agents/state.md`. Remove it only after completion or explicit cancellation.
