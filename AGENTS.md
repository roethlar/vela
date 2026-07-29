# Agent Guidance

## Prime Invariants
<!-- prime:begin — keep terse; re-grounded after compaction -->
These outrank everything below. After a context compaction, re-read this block from AGENTS.md.

- Words first. Answer questions and musings in words; act only on an explicit instruction or go. A handed-over report, plan, or spec is evidence to assess, not a decision to implement — but an owner's completion report inside an approved, already-scoped workflow is the go for the next step that workflow defines; new scope, changed risk, and separately gated actions still stop.
- No code change without an approved plan; docs and other edits that change nothing the repo ships don't need one (a README). When unsure, treat it as code. Do not expand scope without approval.
- Commit each slice as it lands; never leave finished work uncommitted. When work is landed and verified, its paperwork closes in the same motion — tracker issues closed, records synced — no fresh ask; anything beyond recording already-approved work still stops. History-rewrite and destructive or outward-facing actions always need an explicit go. Push policy: see `.agents/push-policy.md`.
- Repo is memory. Durable truth lives in the repo, not chat or working memory. Under context pressure, re-ground from AGENTS.md; prefer a fresh session when degraded.
<!-- prime:end -->

## Repo-Specific Guidance

@.agents/repo-guidance.md

Repo-specific rules live in `.agents/repo-guidance.md`, imported above (read it directly if your harness does not process `@` imports). It extends this file, never overriding AGENTS.md or any refresh-installed artifact — repo policy may set when an operator or tool is invoked, never rewrite an installed artifact's semantics; flag any genuine conflict.

## Universal Invariants

- The Prime Invariants are the hardest-to-reverse rules; this section adds the rest.
- Memory stores kept outside the repo are not durable memory, on any harness. Project-specific durable knowledge lives in the repo's `.agents/` files; out-of-repo stores keep only genuinely cross-project facts (owner identity).
- Record repo facts, decisions, invariants, verification rules, non-goals, and open questions in repo files, or explicitly report them as unrecorded. Write them generalized — tied to repo evidence or explicit human intent, meaningful without the conversation that produced them, never transient chat wording. Label inferred-but-unverified facts as assumptions until repo evidence supports them.
- One canonical location per durable truth: prefer pointers over copies; never keep a second copy of a count or enumeration another doc owns.
- One immediately discoverable current-state entry point (`.agents/state.md`), kept current by the working agent as work lands — never owner-gated; `handoff` keeps its snapshot role, the hygiene sweep rides `catchup`. Never reconstruct current state from chat, long journals, or tool-local memory.
- When repo documents disagree, flag the conflict instead of silently choosing the convenient source. Code and tests are evidence for behavior, owner-approved plans and guidance for intent — guidance authored during the current effort authorizes nothing in that effort.
- Specific over generic: an explicit authority or scope boundary, or a rule or decision whose wording removes discretion for the case it names ("unconditional", "no per-run choice", "deterministic"), outranks every generic default for that case — flag-conflicts, one-canonical-location, smallest-guidance-set included. Apply it as written; do not reopen the case it settles as a conflict or approval question against surrounding repo state such as git history.
- Prefer the smallest durable guidance set that fits the repo.
- Do not circumvent a roadblock whose provenance you have not established — a failing test, a guard or assertion, a lint or type error, a `.gitignore` rule, a refusal or permission denial, a config prohibition, a CI gate. Before removing or bypassing one, inspect its origin thoroughly enough to confirm it is not load-bearing; if you cannot, treat it as legitimate and stop or ask.
- Escalate an iterative process on stalled progress, never on duration or turn count. Each cycle must bank a verifiable delta — a test moving red→green, a finding closed with its guard proof, a build or type error resolved, a committed slice; a cycle that produces none is a stall. After a few consecutive stalled cycles (state the threshold you are using; default ~2-3), stop and surface to a human. A long run that banks a delta each cycle is healthy.
- `AGENTS.md` is governance only — it must be portable. The test: would this line still be true and useful if copied unchanged into an unrelated repo? Process, invariants, and operator definitions pass. Anything true only of *this* repo — a concrete source path, the repo's own name as a fact, its verification commands, a restatement of current state or the decisions queue — fails and lives in `.agents/`, with `AGENTS.md` pointing to it, never restating it. The toolkit's own standard layout — `.agents/state.md`, operator names — is portable and allowed.
- `AGENTS.md` and every other artifact installed by governance refresh — playbooks, skills, command wrappers, harness shims, hook settings — are toolkit-owned: no agent edit and no out-of-band edit is legitimate; route changes to the owner for the toolkit, and refresh restores any divergence to the shipped set. Keep linters and formatters off installed copies — nothing polices files no agent may fix. Durable repo-specific rules go to `.agents/repo-guidance.md` and facts to the other `.agents/` files.

## Session Startup

1. Read `AGENTS.md`, `.agents/repo-guidance.md`, and `.agents/state.md` if present, plus relevant `.agents/` files, before making changes; note any untracked or ignored agent-control files that affect the task.
2. Clone freshness: before trusting `.agents/state.md`, compare this clone against its canonical remote with a read-only check (`git ls-remote <remote> HEAD` against the local ref). Behind or diverged — say so and treat recorded state as possibly stale; unreachable — proceed with a one-line caveat, never block.
3. This repo ships governance hooks (Claude Code only); if your harness gates hooks until the workspace is trusted, say what the hooks do and run the trust step only on an explicit go — never bypass the gate.

## Source Of Truth

1. Human request.
2. `AGENTS.md`, extended by `.agents/repo-guidance.md`.
3. `.agents/state.md` for current work; `.agents/decisions.md` for settled decisions; approved `.agents/playbooks/*`.
4. Current code, tests, and CI as evidence for behavior.
5. Existing docs, only when consistent with current repo evidence.

When sources disagree, apply the flag-conflicts invariant: surface the conflict and fix the lower-authority source, or ask which should win.

## Operator Requests

Treat these owner words as process requests. Where an entry says playbook, read `.agents/playbooks/<name>.md` at invoke time — the playbook is the authoritative procedure.

- `catchup` (playbook): re-ground and report — current state, next action, blockers, and one proposed first action; the state-hygiene sweep rides this pass. Make no other changes until the human responds.
- `handoff` (playbook): a fast save-my-place snapshot — seconds, not minutes; the next session resumes without chat context.
- `decision`: record a settled durable decision in `.agents/decisions.md` and update affected guidance.
- `plan` (playbook): draft or update a durable plan before broad implementation work — agent-facing and cold-implementable; owner decisions come to chat one at a time.
- `playbook <name>`: run the named approved playbook. If it does not exist, say so rather than guessing.
- `toolkit`: list the owner verbs in this repo, one plain line per verb — what each does and when to say it (the `toolkit` command/skill carries the list).

## Owner Gates

Any question put to the owner — a plan decision, an approval, a contested finding — is written for an owner arriving cold with no session memory. The ask carries everything needed to rule, in one short message: a line or two of context, the question, what concretely changes under each option, and the recommended option with its reason. State what stays blocked until the ruling lands; silence never authorizes proceeding. An ask answerable only by scrolling back, opening a plan document, or re-reading a transcript is malformed — rewrite it.

## Verification

Use the repo's current automated verification entry point recorded in `.agents/repo-guidance.md` (Verification).

- For code changes, run the current automated verification before claiming completion.
- When a change ships with a new test, prove the test guards it: temporarily revert the change, confirm the test fails, restore it, confirm everything passes. A test that passes with its fix reverted is vacuous — replace it.
- For docs-only changes, code verification is not required unless the docs affect setup, commands, runtime behavior, generated files, or user-visible behavior.
- For behavior that automation does not cover, run the relevant manual check, smoke test, or playtest, or state clearly that it was not run.
- If no verification entry point is recorded yet, identify the likely command from repo evidence, record it, and label uncertainty. Ask the human only when evidence conflicts, no plausible command exists, or the command appears destructive, expensive, credentialed, or otherwise unsafe to run automatically.

## Git Safety

- Never conclude a branch is merged from ancestry alone: `git branch --merged` can lie after an `-s ours` or octopus merge records ancestry without content. Verify the content actually arrived (`git diff <branch> <main>`) before deleting anything or treating work as landed.
- When working through findings or fixes, one item per commit, committed before starting the next; batch sweeps spanning many findings only on the owner's explicit request. Whether work happens on a branch is repo policy, not this rule's.
- Do not rewrite history or restructure existing commits without explicit owner approval: no `git commit --amend`, `rebase`, `squash`, force-push, reordering, or collapsing commits already made. Approval authorizes the scoped commit as announced, never a later rewrite of it. Default to a new commit per fix; if history genuinely needs reshaping, stop and ask.

## Final Response

Open with a bottom-line-first executive summary — what changed, what was validated, any remaining risk, anything still awaiting the owner; supporting detail follows, never precedes. While queued work remains, never end a response without naming the next work item and a concrete proposed action; a bare "x is blocked on y" is not an acceptable ending.
