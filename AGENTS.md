# Agent Guidance

## Prime Invariants

- Answer first; act only on an explicit go for its named scope. Plans, reports and specs aren't a go.
- Code changes require an approved plan. Non-shipped bookkeeping doesn't; if unsure, treat it as code.
- Commit slices with records. Branch work is incomplete until merged into main and its branch deleted. Push policy: `.agents/push-policy.md`.
- Only repo files are durable memory. Re-read this file under context pressure; hand off before judgment degrades.

## Repo-Specific Guidance

@.agents/repo-guidance.md

More specific rules win. Flag a contradiction you cannot reconcile.

## Invariants

- Record durable facts, decisions and open questions in `.agents/`, understandable without chat; label unverified inferences as assumptions.
- One canonical location per truth; pointers over copies. `.agents/state.md` is the sole current-state entry point, kept current by the working agent as work lands.
- Never bypass failed checks, guards, ignore rules or refusals unless proven not load-bearing. Otherwise stop or ask.
- Escalate for stalled progress, never duration: after 2–3 attempts without verifiable progress, report the obstacle and proposed next action.
- Keep this file portable; repo-specific rules live in `.agents/`. Refresh-governed artifacts are toolkit-owned. Edit toolkit sources, never installed copies. Exclude installed copies from repo lint/format; refresh restores divergence. Seeded policies (`.agents/push-policy.md`) are repo-owned and editable.

## Session Startup

Before changes, read this file, `.agents/repo-guidance.md` and `.agents/state.md`. Before trusting state, compare the local ref with `git ls-remote`; if unreachable, proceed with a caveat. Hook trust needs an explanation and explicit go.

## Source of Truth

Human request → this file and repo guidance → state, decisions and approved playbooks → code/tests/CI evidence → other docs. Fix the lower source on disagreement, or ask.

## Operator Requests

Read `.agents/playbooks/<name>.md` on invocation.

- `catchup` — re-ground and report; no other work until the owner responds.
- `handoff` — fast state snapshot.
- `decision` — record a settled decision in `.agents/decisions.md`; update affected guidance.
- `plan` — draft or update a durable plan.
- `playbook <name>` — run it; report if missing.
- `toolkit` — list owner verbs, one plain line each.

## Owner Gates

Ask one unsettled decision: context, action, consequence, recommendation. Approval stands within scope; silence authorizes nothing.

## Verification

Self-review is forbidden unless explicitly requested by the owner.

Before claiming completion, run repo guidance's verification entry point; docs-only changes need `git diff --check` unless they affect behavior. Prove new tests bite: revert the fix, watch it fail, restore. State checks not run.

## Git Safety

Merges, deletion, history rewrites (amend, rebase, squash, force-push) and outward-facing actions each need explicit authority. One finding per commit. Before branch deletion, verify content reached main with `git diff`; ancestry alone is insufficient. Setup/closeout: `.agents/playbooks/git.md`.

## Final Response

Lead with results. Name remaining work's pending step and one proposed action; never call incomplete work done.
